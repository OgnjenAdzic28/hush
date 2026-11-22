//! Reading one window's AT-SPI tree into a snapshot.
//!
//! As on macOS, speed comes from reading less: nothing that isn't showing is
//! descended into, chrome like scroll bars is skipped, and a hard budget
//! keeps one huge window from stalling a call. On top of that the tree is
//! read a level at a time, with every call for the level sent at once, so a
//! window costs about two round trips per level of depth rather than five
//! per element.

use futures_util::future::{OptionFuture, join_all};
use hush_core::{AppInfo, Node, Role, Snapshot, States, WindowInfo};

use crate::bus::{Bus, Coords, Element, Reply};
use crate::roles::{self, Mapped, state};

/// Elements read per window before the rest is only counted.
const BUDGET: usize = 2500;

/// A node read so far, with its children's places in the read order.
struct Read {
    node: Node<Element>,
    children: Vec<usize>,
}

pub(crate) async fn window(
    bus: &Bus,
    app: &AppInfo,
    info: &WindowInfo,
    root: &Element,
) -> Reply<Snapshot<Element>> {
    let snapshot = read(bus, app, info, root, Shown::Showing).await?;
    // GTK 4 before 4.10 marks only the window itself showing, and the rest
    // merely visible: the whole window would read as hidden.
    let nothing_shown = snapshot.nodes.len() > 1
        && snapshot.nodes[1..]
            .iter()
            .all(|node| node.states.has(States::OFFSCREEN));
    if nothing_shown {
        return read(bus, app, info, root, Shown::Visible).await;
    }
    Ok(snapshot)
}

/// Which state says an element is on screen.
#[derive(Clone, Copy)]
enum Shown {
    Showing,
    /// For toolkits that never report showing.
    Visible,
}

async fn read(
    bus: &Bus,
    app: &AppInfo,
    info: &WindowInfo,
    root: &Element,
    shown: Shown,
) -> Reply<Snapshot<Element>> {
    let mut info = info.clone();
    if let Ok(Some(frame)) = bus.frame(root, Coords::Screen).await {
        info.frame = frame;
    }
    let mut read: Vec<Read> = Vec::new();
    let mut level: Vec<(Element, Option<usize>)> = vec![(root.clone(), None)];
    while !level.is_empty() {
        let results = join_all(
            level
                .iter()
                .map(|(element, parent)| read_one(bus, element, parent.is_none(), shown)),
        )
        .await;
        let mut next = Vec::new();
        for ((_, parent), result) in level.into_iter().zip(results) {
            let (node, children) = match result {
                Ok(Some(found)) => found,
                Ok(None) => continue,
                // A child that went away between listing and reading.
                Err(_) if parent.is_some() => continue,
                Err(e) => return Err(e),
            };
            let place = read.len();
            read.push(Read {
                node: Node { parent, ..node },
                children: Vec::new(),
            });
            if let Some(parent) = parent {
                read[parent].children.push(place);
            }
            next.extend(children.into_iter().map(|child| (child, Some(place))));
        }
        let room = BUDGET.saturating_sub(read.len());
        for (_, parent) in next.iter().skip(room) {
            if let Some(parent) = *parent {
                read[parent].node.unread += 1;
            }
        }
        next.truncate(room);
        level = next;
    }
    Ok(Snapshot::new(
        app.clone(),
        info,
        pre_order(read),
        Vec::new(),
    ))
}

/// The nodes read level by level, put parents first as `Snapshot` needs,
/// with each `parent` pointing at its new place.
fn pre_order(mut read: Vec<Read>) -> Vec<Node<Element>> {
    let mut nodes = Vec::with_capacity(read.len());
    if read.is_empty() {
        return nodes;
    }
    let mut stack: Vec<(usize, Option<usize>)> = vec![(0, None)];
    while let Some((place, parent)) = stack.pop() {
        let index = nodes.len();
        let children = std::mem::take(&mut read[place].children);
        let mut node = read[place].node.clone();
        node.parent = parent;
        nodes.push(node);
        stack.extend(children.into_iter().rev().map(|child| (child, Some(index))));
    }
    nodes
}

/// One element and its children (none when it isn't showing). `None` for
/// chrome that is never worth reading.
async fn read_one(
    bus: &Bus,
    element: &Element,
    root: bool,
    shown: Shown,
) -> Reply<Option<(Node<Element>, Vec<Element>)>> {
    let (set, role_name, name, frame) = futures_util::join!(
        bus.states(element),
        bus.role(element),
        bus.name(element),
        bus.frame(element, Coords::Window),
    );
    let set = set?;
    let atspi_role = role_name?;
    let role = match roles::map(atspi_role, set) {
        Mapped::Role(role) => role,
        Mapped::Skip if !root => return Ok(None),
        Mapped::Skip => Role::Window,
    };
    let name = name.unwrap_or_default();
    let showing = root
        || set.has(match shown {
            Shown::Showing => state::SHOWING,
            Shown::Visible => state::VISIBLE,
        });
    // The second round: whatever the first round says is worth asking for.
    let (text, number, description, children) = futures_util::join!(
        OptionFuture::from(roles::has_text_value(role, set).then(|| bus.text(element))),
        OptionFuture::from(roles::has_number_value(role).then(|| bus.number(element))),
        OptionFuture::from(name.trim().is_empty().then(|| bus.description(element))),
        OptionFuture::from(showing.then(|| bus.children(element))),
    );
    let value = match (text, number) {
        (Some(text), _) => text.ok(),
        (_, Some(n)) => n.ok().map(format_number),
        _ => None,
    };
    let name = match description {
        Some(description) => description.unwrap_or_default(),
        None => name,
    };
    let mut states = roles::states(role, atspi_role, set);
    states.set(States::OFFSCREEN, !showing);
    let node = Node {
        handle: element.clone(),
        role,
        name: name.trim().to_owned(),
        value,
        frame: frame.ok().flatten(),
        states,
        actions: roles::actions(role),
        parent: None,
        unread: 0,
    };
    Ok(Some((
        node,
        children.and_then(Result::ok).unwrap_or_default(),
    )))
}

/// Whole numbers without a decimal point, so a slider at 50 reads "50".
fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
