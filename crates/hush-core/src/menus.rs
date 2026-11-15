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
}
