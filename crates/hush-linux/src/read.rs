//! Reading one window's AT-SPI tree into a snapshot.
//!
//! As on macOS, speed comes from reading less: nothing that isn't showing is
//! descended into, chrome like scroll bars is skipped, and a hard budget
//! keeps one huge window from stalling a call. Each element costs a few
//! D-Bus round trips (role, states, name, frame, children).

use hush_core::{AppInfo, Node, Role, Snapshot, States, WindowInfo};

use crate::bus::{Bus, Coords, Element, Reply};
use crate::roles::{self, Mapped, state};

/// Elements read per window before the rest is only counted.
const BUDGET: usize = 2500;

pub(crate) fn window(
    bus: &Bus,
    app: &AppInfo,
    info: &WindowInfo,
    root: &Element,
) -> Reply<Snapshot<Element>> {
    let mut info = info.clone();
    if let Ok(Some(frame)) = bus.frame(root, Coords::Screen) {
        info.frame = frame;
    }
    let mut nodes: Vec<Node<Element>> = Vec::new();
    // Depth-first, children pushed in reverse so nodes come out in pre-order.
    let mut stack: Vec<(Element, Option<usize>)> = vec![(root.clone(), None)];
    while let Some((element, parent)) = stack.pop() {
        let read = match read_one(bus, &element, parent.is_none()) {
            Ok(read) => read,
            // A child that went away between listing and reading.
            Err(_) if parent.is_some() => continue,
            Err(e) => return Err(e),
        };
        let Some((mut node, showing)) = read else {
            continue;
        };
        node.parent = parent;
        let mut children = if showing {
            bus.children(&element).unwrap_or_default()
        } else {
            node.states.set(States::OFFSCREEN, true);
            Vec::new()
        };
        if nodes.len() + stack.len() + children.len() > BUDGET {
            node.unread += children.len();
            children.clear();
        }
        let index = nodes.len();
        nodes.push(node);
        for child in children.into_iter().rev() {
            stack.push((child, Some(index)));
        }
    }
    Ok(Snapshot::new(app.clone(), info, nodes, Vec::new()))
}

/// One element, and whether it is showing. `None` for chrome that is never
/// worth reading.
fn read_one(bus: &Bus, element: &Element, root: bool) -> Reply<Option<(Node<Element>, bool)>> {
    let set = bus.states(element)?;
    let role = match roles::map(&bus.role(element)?, set) {
        Mapped::Role(role) => role,
        Mapped::Skip if !root => return Ok(None),
        Mapped::Skip => Role::Window,
    };
    let frame = bus.frame(element, Coords::Window).ok().flatten();
    let mut name = bus.name(element).unwrap_or_default();
    let mut value = None;
    if roles::has_text_value(role) {
        value = bus.text(element).ok();
    } else if roles::has_number_value(role) {
        value = bus.number(element).ok().map(number);
    }
    if name.trim().is_empty() {
        name = match role {
            // A label's text is its name; some toolkits leave Name empty.
            Role::Text => bus.text(element).unwrap_or_default(),
            _ => bus.description(element).unwrap_or_default(),
        };
    }
    let node = Node {
        handle: element.clone(),
        role,
        name: name.trim().to_owned(),
        value,
        frame,
        states: roles::states(role, set),
        actions: roles::actions(role),
        parent: None,
        unread: 0,
    };
    Ok(Some((node, root || set.has(state::SHOWING))))
}

/// Whole numbers without a decimal point, so a slider at 50 reads "50".
fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
