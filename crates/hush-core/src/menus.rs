//! An app's menu commands as one compact list: how an agent learns an
//! app's whole vocabulary (and its shortcuts) in one call, then drives it
//! with `menu` and `key` steps.

use std::fmt::Write;

/// One menu item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    /// Titles from the menu bar down, like `["File", "Export", "Media…"]`.
    pub path: Vec<String>,
    /// In `key` step syntax, like `cmd+shift+e`.
    pub shortcut: Option<String>,
    pub enabled: bool,
}

/// One line per top-level menu:
/// `File: New [cmd+n] · Export > Media… [cmd+m] · Revert (off)`.
pub fn render(app: &str, entries: &[MenuEntry]) -> String {
    let mut out = format!(
        "# {app} menus · run one with menu A > B, or its shortcut with key · (off) = disabled now\n"
    );
    let mut current: Option<&str> = None;
    for entry in entries {
        let Some((top, rest)) = entry.path.split_first() else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        if current != Some(top.as_str()) {
            if current.is_some() {
                out.push('\n');
            }
            let _ = write!(out, "{top}: ");
            current = Some(top);
        } else {
            out.push_str(" · ");
        }
        out.push_str(&rest.join(" > "));
        if let Some(shortcut) = &entry.shortcut {
            let _ = write!(out, " [{shortcut}]");
        }
        if !entry.enabled {
            out.push_str(" (off)");
        }
    }
    if current.is_some() {
        out.push('\n');
    }
    out
}

/// The item that opens another ordinary window of the app, the way the
/// user would get a second Finder or browser window: an enabled "New Window"
/// or "New <Something> Window" item, or an item inside such a submenu
/// (Terminal's profiles). Private and incognito windows don't share the
/// user's state, and "New" alone makes a new document, so neither counts.
/// The one on Cmd+N wins, then the shortest path.
pub fn new_window(entries: &[MenuEntry]) -> Option<&MenuEntry> {
    let opens_window = |title: &str| {
        let lower = title.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        matches!(
            words.as_slice(),
            ["new", "window", ..] | ["new", _, "window", ..]
        ) && !words
            .iter()
            .any(|word| matches!(*word, "private" | "incognito" | "tab"))
    };
    entries
        .iter()
        .filter(|entry| entry.enabled && entry.path.iter().skip(1).any(|t| opens_window(t)))
        .min_by_key(|entry| (entry.shortcut.as_deref() != Some("cmd+n"), entry.path.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &[&str], shortcut: Option<&str>, enabled: bool) -> MenuEntry {
        MenuEntry {
            path: path.iter().map(|s| s.to_string()).collect(),
            shortcut: shortcut.map(str::to_owned),
            enabled,
        }
    }

    #[test]
    fn one_line_per_menu_with_shortcuts_and_disabled_items() {
        let text = render(
            "Editor",
            &[
                entry(&["File", "New"], Some("cmd+n"), true),
                entry(&["File", "Export", "Media…"], Some("cmd+m"), true),
                entry(&["File", "Revert"], None, false),
                entry(&["Clip", "Split"], Some("cmd+k"), true),
            ],
        );
        assert_eq!(
            text.lines().skip(1).collect::<Vec<_>>(),
            [
                "File: New [cmd+n] · Export > Media… [cmd+m] · Revert (off)",
                "Clip: Split [cmd+k]"
            ]
        );
    }

    #[test]
    fn the_new_window_item_is_found_in_the_usual_places() {
        let safari = [
            entry(&["File", "New Window"], Some("cmd+n"), true),
            entry(&["File", "New Private Window"], Some("cmd+shift+n"), true),
            entry(&["File", "New Tab"], Some("cmd+t"), true),
        ];
        assert_eq!(new_window(&safari).unwrap().path, ["File", "New Window"]);
        let finder = [
            entry(&["File", "New Folder"], Some("cmd+shift+n"), true),
            entry(&["File", "New Finder Window"], Some("cmd+n"), true),
        ];
        assert_eq!(
            new_window(&finder).unwrap().path,
            ["File", "New Finder Window"]
        );
        let terminal = [
            entry(&["Shell", "New Window", "Basic"], None, true),
            entry(
                &["Shell", "New Window", "New Window with Settings – Stable"],
                None,
                true,
            ),
            entry(
                &["Shell", "New Window", "New Window with Profile – Basic"],
                Some("cmd+n"),
                true,
            ),
        ];
        assert_eq!(
            new_window(&terminal).unwrap().path,
            ["Shell", "New Window", "New Window with Profile – Basic"]
        );
    }

    #[test]
    fn a_new_document_private_window_or_disabled_item_is_not_one() {
        let textedit = [entry(&["File", "New"], Some("cmd+n"), true)];
        assert!(new_window(&textedit).is_none());
        let private = [
            entry(&["File", "New Incognito Window"], Some("cmd+shift+n"), true),
            entry(&["File", "New Window"], Some("cmd+n"), false),
            entry(&["Window", "New Window"], None, true),
        ];
        assert_eq!(new_window(&private).unwrap().path, ["Window", "New Window"]);
        // A top-level menu named that way isn't an item.
        assert!(new_window(&[entry(&["New Window"], None, true)]).is_none());
    }
}
