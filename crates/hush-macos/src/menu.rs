//! Menu bar items, pressed through Accessibility without opening any menu.
//!
//! This is also how Hush sends Cmd shortcuts to a background app: AppKit
//! only routes key equivalents to the active app, so `cmd+s` as a key event
//! does nothing in the background, but pressing the menu item whose
//! shortcut is Cmd+S does exactly what the shortcut would.

use hush_core::{Error, Key, KeyCombo, MenuEntry, Result};
use objc2_core_foundation::{CFArray, CFRetained, CFString};

use crate::ax::{Element, elements_of};
use crate::cf;

/// Presses the item at `path`, like `["File", "Export", "PDF…"]`.
pub(crate) fn press_path(app: &Element, path: &[String]) -> Result<()> {
    let bar = app
        .element("AXMenuBar")
        .ok_or_else(|| Error::NotFound("the app has no menu bar".into()))?;
    let mut items = bar.elements("AXChildren");
    let mut found = None;
    for (depth, wanted) in path.iter().enumerate() {
        let titles: Vec<(Element, String)> = items
            .iter()
            .filter_map(|item| item.string("AXTitle").map(|title| (item.clone(), title)))
            .collect();
        let Some((item, _)) = titles
            .iter()
            .find(|(_, title)| same(title, wanted))
            .cloned()
        else {
            let names: Vec<String> = titles
                .iter()
                .map(|(_, t)| t)
                .filter(|t| !t.is_empty())
                .map(|t| format!("\"{t}\""))
                .collect();
            let place = if depth == 0 {
                "the menu bar".to_owned()
            } else {
                path[..depth].join(" > ")
            };
            return Err(Error::NotFound(format!(
                "no menu item \"{wanted}\" in {place}; it has {}",
                names.join(", ")
            )));
        };
        // A menu bar item or submenu item holds its menu as the only child.
        items = item
            .elements("AXChildren")
            .first()
            .map(|menu| menu.elements("AXChildren"))
            .unwrap_or_default();
        found = Some(item);
    }
    let item = found.ok_or_else(|| Error::Failed("menu needs a path".into()))?;
    press(&item, &path.join(" > "))
}

/// Presses the menu item whose keyboard shortcut is `combo`. Returns false
/// when no item has that shortcut.
pub(crate) fn press_shortcut(app: &Element, combo: KeyCombo) -> Result<bool> {
    let Key::Char(wanted) = combo.key else {
        return Ok(false);
    };
    let Some(bar) = app.element("AXMenuBar") else {
        return Ok(false);
    };
    let names: Vec<CFRetained<CFString>> = [
        "AXTitle",
        "AXMenuItemCmdChar",
        "AXMenuItemCmdModifiers",
        "AXChildren",
    ]
    .iter()
    .map(|a| cf::cfstr(a))
    .collect();
    let attributes = CFArray::from_retained_objects(&names);

    // Breadth-first over every menu, one batched read per item.
    let mut queue: Vec<Element> = bar.elements("AXChildren");
    while let Some(item) = queue.pop() {
        let Ok(values) = item.values(&attributes) else {
            break;
        };
        let character = values[1].as_deref().and_then(cf::string);
        if let Some(character) = character
            && character.chars().count() == 1
            && character
                .to_lowercase()
                .starts_with(wanted.to_ascii_lowercase())
            && values[2].as_deref().and_then(cf::number).map(|n| n as u32)
                == Some(modifier_bits(combo))
        {
            let title = values[0]
                .as_deref()
                .and_then(cf::string)
                .unwrap_or_default();
            press(&item, &title)?;
            return Ok(true);
        }
        if let Some(children) = values[3].as_deref() {
            queue.extend(elements_of(children));
        }
    }
    Ok(false)
}

/// Every item of the menu bar except the Apple menu, depth first in menu
/// order: one batched read per item (about 300 items in a pro app).
pub(crate) fn catalog(app: &Element) -> Result<Vec<MenuEntry>> {
    let bar = app
        .element("AXMenuBar")
        .ok_or_else(|| Error::NotFound("the app has no menu bar".into()))?;
    let names: Vec<CFRetained<CFString>> = [
        "AXTitle",
        "AXMenuItemCmdChar",
        "AXMenuItemCmdModifiers",
        "AXEnabled",
        "AXChildren",
    ]
    .iter()
    .map(|a| cf::cfstr(a))
    .collect();
    let attributes = CFArray::from_retained_objects(&names);
    let mut entries = Vec::new();
    // (item, its parent's path); reversed so pops come out in menu order.
    let mut stack: Vec<(Element, Vec<String>)> = bar
        .elements("AXChildren")
        .into_iter()
        .skip(1)
        .rev()
        .map(|item| (item, Vec::new()))
        .collect();
    while let Some((item, parent)) = stack.pop() {
        let Ok(values) = item.values(&attributes) else {
            break;
        };
        let title = values[0]
            .as_deref()
            .and_then(cf::string)
            .unwrap_or_default();
        if title.trim().is_empty() {
            continue; // a separator
        }
        let mut path = parent;
        path.push(title);
        // A menu bar item or submenu item holds its menu as the only child.
        let submenu: Vec<Element> = values[4]
            .as_deref()
            .map(elements_of)
            .and_then(|menu| menu.first().map(|m| m.elements("AXChildren")))
            .unwrap_or_default();
        if submenu.is_empty() {
            let shortcut = values[1]
                .as_deref()
                .and_then(cf::string)
                .filter(|c| !c.trim().is_empty())
                .map(|c| {
                    let bits = values[2].as_deref().and_then(cf::number).unwrap_or(0.0) as u32;
                    shortcut_text(&c, bits)
                });
            let enabled = values[3].as_deref().and_then(cf::boolean) != Some(false);
            entries.push(MenuEntry {
                path,
                shortcut,
                enabled,
            });
        } else {
            for child in submenu.into_iter().rev() {
                stack.push((child, path.clone()));
            }
        }
    }
    Ok(entries)
}

/// A shortcut in `key` step syntax from its character and modifier bits
/// (AXMenuItemCmdModifiers: 1 shift, 2 option, 4 control, 8 no command,
/// 16 Fn).
fn shortcut_text(character: &str, bits: u32) -> String {
    let mut parts = Vec::new();
    // Bit 4 is Fn (Emoji & Symbols is Fn+E, Enter Full Screen Fn+F).
    if bits & 16 != 0 {
        parts.push("fn");
    }
    if bits & 8 == 0 {
        parts.push("cmd");
    }
    if bits & 4 != 0 {
        parts.push("ctrl");
    }
    if bits & 2 != 0 {
        parts.push("alt");
    }
    if bits & 1 != 0 {
        parts.push("shift");
    }
    let key = match character {
        " " => "space".to_owned(),
        "\u{7f}" | "\u{8}" => "backspace".to_owned(),
        "\r" | "\u{3}" => "enter".to_owned(),
        "\u{1b}" => "escape".to_owned(),
        other => other.to_lowercase(),
    };
    let mut text = parts.join("+");
    if !text.is_empty() {
        text.push('+');
    }
    text.push_str(&key);
    text
}

fn press(item: &Element, label: &str) -> Result<()> {
    if item.value("AXEnabled").ok().and_then(|v| cf::boolean(&v)) == Some(false) {
        return Err(Error::Failed(format!(
            "menu item \"{label}\" is disabled right now"
        )));
    }
    crate::act::perform_action(item, "AXPress")
}

/// AXMenuItemCmdModifiers: bit 0 shift, 1 option, 2 control, 3 "no command".
fn modifier_bits(combo: KeyCombo) -> u32 {
    let m = combo.modifiers;
    u32::from(m.shift) | u32::from(m.alt) << 1 | u32::from(m.ctrl) << 2 | u32::from(!m.cmd) << 3
}

/// Titles match ignoring case and "..." versus "…".
fn same(title: &str, wanted: &str) -> bool {
    let normal = |s: &str| s.trim().replace("...", "…").to_lowercase();
    normal(title) == normal(wanted)
}

#[cfg(test)]
mod tests {
    use super::shortcut_text;

    #[test]
    fn shortcuts_read_like_key_steps() {
        assert_eq!(shortcut_text("S", 0), "cmd+s");
        assert_eq!(shortcut_text("Z", 1), "cmd+shift+z");
        assert_eq!(shortcut_text("K", 8), "k");
        assert_eq!(shortcut_text("F", 4 | 2), "cmd+ctrl+alt+f");
        assert_eq!(shortcut_text(" ", 8), "space");
        assert_eq!(shortcut_text("f", 24), "fn+f");
    }
}
