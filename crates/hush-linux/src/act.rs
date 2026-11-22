//! Carrying out steps through AT-SPI: actions, text and value writes,
//! selection and menus. None of these touch real input, so they work on
//! any app, including one on the user's own display.
//!
//! Keys and pointer events need the lane (Hush's own display), where they
//! can't reach the user; until an app runs there they fail closed.

use futures_util::future::join_all;
use hush_core::{Error, Op, Result};

use crate::bus::{Bus, Element};
use crate::roles::{role, state};

/// Action names, best first, for a press. Toolkits disagree: GTK says
/// "click" or "activate", Qt says "Press" or "Toggle", menus say "click".
const PRESS: [&str; 6] = ["click", "press", "activate", "toggle", "jump", "open"];
const SHOW_MENU: [&str; 5] = ["showmenu", "show menu", "menu", "popup", "press"];

/// GTK 3 raises an app's window when an assistive tool focuses one of its
/// elements, which would take focus from the user.
const FOCUS_ON_USER_DISPLAY: &str = "focusing an element raises the app's window on Linux, which would take your focus; use set or type with its ref instead";

const NEEDS_LANE: &str = "keys and pointer input on Linux only go to apps Hush runs on its own display, so they can't reach yours; for this app use click, set and type on refs";

pub(crate) async fn perform(
    bus: &Bus,
    window: &Element,
    in_lane: bool,
    op: Op<'_, Element>,
) -> Result<()> {
    match op {
        Op::Press(element) => act(bus, element, &PRESS).await,
        Op::ShowMenu(element) => act(bus, element, &SHOW_MENU).await,
        Op::SetValue(element, value) => set_value(bus, element, value).await,
        Op::Focus(element) if in_lane => {
            refused(bus.grab_focus(element).await.map_err(refuse)?, "focus")
        }
        Op::Focus(_) => Err(Error::Unsupported(FOCUS_ON_USER_DISPLAY.into())),
        Op::Select(element) => select(bus, element).await,
        Op::ScrollIntoView(element) => {
            refused(bus.scroll_to(element).await.map_err(refuse)?, "scroll")
        }
        Op::Type { text, into } => type_text(bus, window, into, text).await,
        Op::Menu(path) => menu(bus, window, path).await,
        Op::Lane | Op::Key { .. } | Op::Click { .. } | Op::Scroll { .. } => {
            Err(Error::Unsupported(NEEDS_LANE.into()))
        }
    }
}

/// A missing interface or method means the element can't do this, which
/// the engine treats as "try the fallback", not as a failed step.
fn refuse(error: zbus::Error) -> Error {
    if let zbus::Error::MethodError(name, _, _) = &error {
        let name = name.as_str();
        if name.ends_with("UnknownMethod")
            || name.ends_with("UnknownInterface")
            || name.ends_with("NotSupported")
        {
            return Error::Unsupported(format!("this element doesn't support that ({name})"));
        }
    }
    crate::platform::failed(error)
}

fn refused(done: bool, what: &str) -> Result<()> {
    if done {
        Ok(())
    } else {
        Err(Error::Failed(format!("the app refused to {what}")))
    }
}

async fn act(bus: &Bus, element: &Element, wanted: &[&str]) -> Result<()> {
    let names = bus.actions(element).await.map_err(refuse)?;
    let index = wanted
        .iter()
        .find_map(|w| {
            names
                .iter()
                .position(|n| n.to_lowercase().replace('_', " ") == *w)
        })
        .or_else(|| (!names.is_empty()).then_some(0))
        .ok_or_else(|| Error::Unsupported("this element has no actions".into()))?;
    let index = i32::try_from(index).unwrap_or(0);
    refused(bus.do_action(element, index).await.map_err(refuse)?, "act")
}

async fn set_value(bus: &Bus, element: &Element, value: &str) -> Result<()> {
    match bus.set_text(element, value).await.map_err(refuse) {
        Ok(done) => return refused(done, "change the text"),
        Err(Error::Unsupported(_)) => {}
        Err(e) => return Err(e),
    }
    let number: f64 = value
        .trim()
        .parse()
        .map_err(|_| Error::Unsupported("this element takes neither text nor a number".into()))?;
    bus.set_number(element, number).await.map_err(refuse)
}

async fn type_text(bus: &Bus, window: &Element, into: Option<&Element>, text: &str) -> Result<()> {
    let target = match into {
        Some(element) => element.clone(),
        None => focused_editable(bus, window).await?.ok_or_else(|| {
            Error::Unsupported("nothing editable has focus in this window; give type a ref".into())
        })?,
    };
    // At the caret only where the user would type: in the focused field.
    // An unfocused field's caret sits at 0, and text typed into a field by
    // ref is meant to go after what is there.
    let focused = bus
        .states(&target)
        .await
        .is_ok_and(|s| s.has(state::FOCUSED));
    let caret = match focused {
        true => bus.caret(&target).await.ok().flatten(),
        false => None,
    };
    let at = match caret {
        Some(caret) => caret,
        None => bus.character_count(&target).await.map_err(refuse)?,
    };
    refused(
        bus.insert_text(&target, at, text).await.map_err(refuse)?,
        "insert the text",
    )
}

async fn select(bus: &Bus, element: &Element) -> Result<()> {
    let (parent, index) = futures_util::join!(bus.parent(element), bus.index_in_parent(element));
    let (parent, index) = (parent.map_err(refuse)?, index.map_err(refuse)?);
    match bus.select_child(&parent, index).await.map_err(refuse) {
        Ok(done) => refused(done, "select it"),
        // A row whose list has no Selection interface may still have its own
        // "activate" or "select" action.
        Err(Error::Unsupported(_)) => act(bus, element, &["select", "activate", "click"]).await,
        Err(e) => Err(e),
    }
}

/// Elements read while searching a window, at most.
const SEARCH_LIMIT: usize = 2000;

/// The first element, breadth first, for which `wanted(role, states)` holds.
async fn find(
    bus: &Bus,
    root: &Element,
    wanted: impl Fn(u32, crate::roles::StateSet) -> bool,
) -> Result<Option<Element>> {
    let mut level = vec![root.clone()];
    let mut read = 0;
    while !level.is_empty() && read < SEARCH_LIMIT {
        read += level.len();
        let found = join_all(level.iter().map(|element| async move {
            let (r, s) = futures_util::join!(bus.role(element), bus.states(element));
            (r.ok(), s.ok())
        }))
        .await;
        for (element, (r, s)) in level.iter().zip(&found) {
            if let (Some(r), Some(s)) = (r, s)
                && wanted(*r, *s)
            {
                return Ok(Some(element.clone()));
            }
        }
        let showing: Vec<&Element> = level
            .iter()
            .zip(&found)
            .filter(|(_, (_, s))| s.is_some_and(|s| s.has(state::SHOWING)))
            .map(|(element, _)| element)
            .collect();
        let children = join_all(showing.into_iter().map(|e| bus.children(e))).await;
        level = children.into_iter().flatten().flatten().collect();
    }
    Ok(None)
}

async fn focused_editable(bus: &Bus, window: &Element) -> Result<Option<Element>> {
    find(bus, window, |_, s| {
        s.has(state::FOCUSED) && s.has(state::EDITABLE)
    })
    .await
}

/// Presses a menu bar command by its titles, like `["File", "Save As…"]`.
async fn menu(bus: &Bus, window: &Element, path: &[String]) -> Result<()> {
    let bar = find(bus, window, |r, _| r == role::MENU_BAR)
        .await?
        .ok_or_else(|| Error::Unsupported("this window has no menu bar".into()))?;
    let mut current = bar;
    for (depth, title) in path.iter().enumerate() {
        let children = bus
            .children(&current)
            .await
            .map_err(crate::platform::failed)?;
        let names = join_all(children.iter().map(|c| bus.name(c))).await;
        let wanted = simple(title);
        let position = names
            .iter()
            .position(|n| n.as_ref().is_ok_and(|n| simple(n) == wanted))
            .ok_or_else(|| {
                let shown: Vec<String> = names.into_iter().flatten().collect();
                Error::NotFound(format!("no menu item \"{title}\" (there is {shown:?})"))
            })?;
        current = children[position].clone();
        if depth + 1 == path.len() {
            return act(bus, &current, &PRESS).await;
        }
    }
    Err(Error::Failed("an empty menu path".into()))
}

/// Menu titles compared without case, mnemonic underscores or a trailing
/// ellipsis, so "save as" finds "_Save As…".
fn simple(title: &str) -> String {
    title
        .replace('_', "")
        .trim_end_matches('…')
        .trim_end_matches("...")
        .trim()
        .to_lowercase()
}
