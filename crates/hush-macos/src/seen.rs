//! Elements Hush knows from two sources: Accessibility, and the window's
//! pixels (see `vision`). The engine treats both the same; operations that
//! need an accessibility element fail as unsupported on a seen one, so the
//! engine's pointer fallback at the element's frame takes over.

use hush_core::{Actions, Error, Node, Op, Result, Role, Snapshot, States, WindowInfo};

use crate::ax::Element;
use crate::vision;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    Ax(Element),
    /// A label read from the pixels, by `vision::Seen::id`.
    Seen(u64),
}

/// Below this many accessibility elements that name something, a window large enough to hold an
/// interface is read from its pixels too. Real AppKit windows list dozens;
/// FilmCraft's full editor lists 8.
const THIN: usize = 20;
const MIN_AREA: f64 = 500.0 * 350.0;

/// `snapshot` with handles wrapped, plus the labels read from the pixels
/// when its accessibility tree is too thin to describe the window.
pub(crate) fn with_pixels(snapshot: Snapshot<Element>, window: &WindowInfo) -> Snapshot<Handle> {
    let Snapshot {
        app,
        window: info,
        nodes,
        menus,
        ..
    } = snapshot;
    let mut nodes: Vec<Node<Handle>> = nodes
        .into_iter()
        .map(|n| Node {
            handle: Handle::Ax(n.handle),
            role: n.role,
            name: n.name,
            value: n.value,
            frame: n.frame,
            states: n.states,
            actions: n.actions,
            parent: n.parent,
            unread: n.unread,
        })
        .collect();
    // Unnamed containers say nothing: FilmCraft's 141 elements are 135
    // empty groups and 6 that name something.
    let meaningful = nodes
        .iter()
        .filter(|n| !n.name.is_empty() || n.value.is_some() || n.actions != Actions::default())
        .count();
    let thin = meaningful < THIN && info.frame.w * info.frame.h >= MIN_AREA;
    if thin && !nodes.is_empty() {
        let result = u32::try_from(window.id).map(|id| vision::read(id, info.frame));
        if let Ok(Err(hush_core::Error::Permission(_))) = &result {
            // Say why the window looks empty; `see` gives the whole message.
            nodes.push(Node {
                handle: Handle::Seen(0),
                role: Role::Text,
                name:
                    "(this window draws its own interface, and Screen Recording is off for Hush, \
                       so its pixels can't be read; see explains)"
                        .into(),
                value: None,
                frame: None,
                states: States::default(),
                actions: Actions::default(),
                parent: Some(0),
                unread: 0,
            });
        }
        if let Ok(Ok(labels)) = result {
            for label in labels {
                let duplicate = nodes.iter().any(|n| {
                    n.name.eq_ignore_ascii_case(&label.text)
                        && n.frame.is_some_and(|f| f.intersects(&label.frame))
                });
                if duplicate {
                    continue;
                }
                nodes.push(Node {
                    handle: Handle::Seen(label.id()),
                    role: Role::Seen,
                    name: label.text,
                    value: None,
                    frame: Some(label.frame),
                    states: States::default(),
                    actions: Actions::default(),
                    parent: Some(0),
                    unread: 0,
                });
            }
        }
    }
    Snapshot::new(app, info, nodes, menus)
}

/// The accessibility element behind a handle.
fn ax(handle: &Handle) -> Result<&Element> {
    match handle {
        Handle::Ax(element) => Ok(element),
        Handle::Seen(_) => Err(Error::Unsupported(
            "this element was read from the window's pixels; only pointer steps work on it".into(),
        )),
    }
}

/// `op` on accessibility elements, or unsupported when it names a seen one.
pub(crate) fn to_ax<'a>(op: Op<'a, Handle>) -> Result<Op<'a, Element>> {
    Ok(match op {
        Op::Lane => Op::Lane,
        Op::Press(h) => Op::Press(ax(h)?),
        Op::ShowMenu(h) => Op::ShowMenu(ax(h)?),
        Op::SetValue(h, value) => Op::SetValue(ax(h)?, value),
        Op::Focus(h) => Op::Focus(ax(h)?),
        Op::Select(h) => Op::Select(ax(h)?),
        Op::ScrollIntoView(h) => Op::ScrollIntoView(ax(h)?),
        Op::Type { text, into } => Op::Type {
            text,
            into: into.map(ax).transpose()?,
        },
        Op::Key { combo, repeat } => Op::Key { combo, repeat },
        Op::Menu(path) => Op::Menu(path),
        Op::Click {
            x,
            y,
            button,
            count,
        } => Op::Click {
            x,
            y,
            button,
            count,
        },
        Op::Scroll { x, y, dx, dy } => Op::Scroll { x, y, dx, dy },
    })
}
