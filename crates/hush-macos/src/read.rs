//! Reading one window's accessibility tree into a snapshot.
//!
//! Speed comes from doing less, not from going faster:
//! - one batched round trip per element (role, name, value, state, frame and
//!   children together), measured at about half the time of separate reads;
//! - nothing outside the visible part of the window is descended into, so a
//!   long web page or document costs what is on screen;
//! - tables and outlines read only their visible rows and count the rest;
//! - a hard element budget so one huge window can't stall a call.

use hush_core::{Actions, AppInfo, Node, Rect, Role, Snapshot, States, WindowInfo};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType};

use crate::ax::{Element, elements_of};
use crate::cf;
use crate::roles::{self, Mapped};

/// Elements read per window before the rest is only counted.
const BUDGET: usize = 2500;

const ATTRIBUTES: [&str; 13] = [
    "AXRole",
    "AXSubrole",
    "AXTitle",
    "AXDescription",
    "AXValue",
    "AXEnabled",
    "AXFocused",
    "AXPosition",
    "AXSize",
    "AXChildren",
    "AXSelected",
    "AXExpanded",
    "AXPlaceholderValue",
];

enum A {
    Role,
    Subrole,
    Title,
    Description,
    Value,
    Enabled,
    Focused,
    Position,
    Size,
    Children,
    Selected,
    Expanded,
    Placeholder,
}

pub(crate) fn window(
    app: &AppInfo,
    info: &WindowInfo,
    window: &Element,
    menus: Vec<String>,
) -> Snapshot<Element> {
    let names: Vec<CFRetained<CFString>> = ATTRIBUTES.iter().map(|a| cf::cfstr(a)).collect();
    let attributes = CFArray::from_retained_objects(&names);
    let origin = (info.frame.x, info.frame.y);
    let screen_clip = info.frame;

    let mut nodes: Vec<Node<Element>> = Vec::new();
    // Depth-first, children pushed in reverse so nodes come out in pre-order.
    let mut stack: Vec<(Element, Option<usize>, Rect)> = vec![(window.clone(), None, screen_clip)];
    while let Some((element, parent, clip)) = stack.pop() {
        let values = element.values(&attributes);
        let get = |a: A| values[a as usize].as_deref();

        let role_name = get(A::Role).and_then(cf::string).unwrap_or_default();
        let subrole = get(A::Subrole).and_then(cf::string);
        let role = match roles::map(&role_name, subrole.as_deref()) {
            Mapped::Role(role) => role,
            Mapped::Skip if parent.is_some() => continue,
            Mapped::Skip => Role::Window,
        };

        let screen = frame(get(A::Position), get(A::Size));
        let mut node = Node {
            handle: element.clone(),
            role,
            name: name(role, &get, subrole.as_deref()),
            value: value(role, &get),
            frame: screen.map(|f| Rect {
                x: f.x - origin.0,
                y: f.y - origin.1,
                ..f
            }),
            states: states(role, &get),
            actions: actions(role),
            parent,
            unread: 0,
        };
        if role.is_text_input() && node.name.is_empty() {
            node.name = label(&element);
        }

        let index = nodes.len();
        let visible = screen.is_none_or(|f| f.is_empty() || f.intersects(&clip));
        let mut children = match get(A::Children) {
            Some(children) if visible => elements_of(children),
            _ => Vec::new(),
        };
        if !visible {
            node.states.set(States::OFFSCREEN, true);
        }
        if matches!(role, Role::Table | Role::Outline) && !children.is_empty() {
            let rows = element.elements("AXVisibleRows");
            if !rows.is_empty() {
                let total = element.count("AXRows").unwrap_or(rows.len());
                node.unread = total.saturating_sub(rows.len());
                // Headers and other non-row children still count.
                children.retain(|child| child.string("AXRole").as_deref() != Some("AXRow"));
                children.extend(rows);
            }
        }
        if nodes.len() + stack.len() + children.len() > BUDGET {
            node.unread += children.len();
            children.clear();
        }

        let child_clip = match (role, screen) {
            (Role::ScrollArea, Some(f)) => intersect(clip, f),
            _ => clip,
        };
        nodes.push(node);
        for child in children.into_iter().rev() {
            stack.push((child, Some(index), child_clip));
        }
    }
    Snapshot::new(app.clone(), info.clone(), nodes, menus)
}

/// Titles of the app's menu bar menus, without the Apple menu.
pub(crate) fn menu_titles(app: &Element) -> Vec<String> {
    let Some(bar) = app.element("AXMenuBar") else {
        return Vec::new();
    };
    bar.elements("AXChildren")
        .iter()
        .skip(1)
        .filter_map(|item| item.string("AXTitle"))
        .filter(|title| !title.is_empty())
        .collect()
}

fn name<'a>(role: Role, get: &impl Fn(A) -> Option<&'a CFType>, subrole: Option<&str>) -> String {
    let text = |a: A| get(a).and_then(cf::string).filter(|s| !s.trim().is_empty());
    if role == Role::Text {
        return text(A::Value).unwrap_or_default();
    }
    text(A::Title)
        .or_else(|| text(A::Description))
        .or_else(|| role.is_text_input().then(|| text(A::Placeholder)).flatten())
        .or_else(|| subrole.and_then(roles::subrole_name).map(str::to_owned))
        .unwrap_or_default()
}

/// A text field's visible label, when it only exists as a separate element.
fn label(element: &Element) -> String {
    element
        .element("AXTitleUIElement")
        .and_then(|label| label.string("AXValue").or_else(|| label.string("AXTitle")))
        .unwrap_or_default()
}

fn value<'a>(role: Role, get: &impl Fn(A) -> Option<&'a CFType>) -> Option<String> {
    match role {
        Role::Text | Role::CheckBox | Role::Radio | Role::Switch => None,
        _ => get(A::Value).and_then(cf::string),
    }
}

fn states<'a>(role: Role, get: &impl Fn(A) -> Option<&'a CFType>) -> States {
    let flag = |a: A| get(a).and_then(cf::boolean).unwrap_or(false);
    let mut states = States::default();
    states.set(
        States::DISABLED,
        get(A::Enabled).and_then(cf::boolean) == Some(false),
    );
    states.set(States::FOCUSED, flag(A::Focused));
    states.set(States::SELECTED, flag(A::Selected));
    states.set(States::EXPANDED, flag(A::Expanded));
    if matches!(
        role,
        Role::CheckBox | Role::Radio | Role::Switch | Role::Tab
    ) {
        match get(A::Value).and_then(cf::number) {
            Some(n) if n >= 2.0 => states.set(States::MIXED, true),
            Some(n) if n >= 1.0 => states.set(States::CHECKED, true),
            _ => {}
        }
    }
    if role == Role::Tab && states.has(States::CHECKED) {
        states.set(States::SELECTED, true);
    }
    states
}

/// What the role normally supports. Hush tries the action and falls back
/// when an app says no, which is cheaper than asking every element first.
fn actions(role: Role) -> Actions {
    use Role::*;
    match role {
        Button | MenuButton | Disclosure | Link | CheckBox | Radio | Switch | Tab | MenuItem => {
            Actions::PRESS
        }
        PopUp | ComboBox => Actions::PRESS | Actions::SHOW_MENU,
        Slider | Stepper => Actions::INCREMENT | Actions::DECREMENT | Actions::SET_VALUE,
        TextField | TextArea | SearchField | SecureField => Actions::SET_VALUE | Actions::FOCUS,
        Row => Actions::SELECT,
        _ => Actions::default(),
    }
}

fn frame(position: Option<&CFType>, size: Option<&CFType>) -> Option<Rect> {
    let p = cf::point(position?)?;
    let s = cf::size(size?)?;
    Some(Rect {
        x: p.x,
        y: p.y,
        w: s.width,
        h: s.height,
    })
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let w = ((a.x + a.w).min(b.x + b.w) - x).max(0.0);
    let h = ((a.y + a.h).min(b.y + b.h) - y).max(0.0);
    Rect { x, y, w, h }
}
