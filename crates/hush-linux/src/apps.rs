//! Running apps (from the accessibility registry), their windows, and
//! installed apps (from desktop entries).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use hush_core::{AppInfo, Role, WindowInfo};

use crate::bus::{Bus, Coords, Element, Reply};
use crate::roles::{self, Mapped, StateSet, state};

/// A running app: its accessibility root and what Hush reports about it.
#[derive(Debug)]
pub(crate) struct Running {
    pub root: Element,
    pub info: AppInfo,
}

/// Every app on the accessibility bus that has at least one window.
pub(crate) fn running(bus: &Bus) -> Reply<Vec<Running>> {
    let mut apps = Vec::new();
    for root in bus.children(&Element::desktop())? {
        // An app that quits mid-listing is simply left out.
        let Ok(windows) = windows(bus, &root) else {
            continue;
        };
        if windows.is_empty() {
            continue;
        }
        let pid = bus.pid(&root.bus).ok();
        let name = bus.name(&root).unwrap_or_default();
        let info = AppInfo {
            name: if name.is_empty() {
                pid.and_then(command).unwrap_or_default()
            } else {
                name
            },
            pid,
            id: None,
            path: pid.and_then(executable),
            frontmost: windows.iter().any(|w| w.states.has(state::ACTIVE)),
        };
        apps.push(Running { root, info });
    }
    apps.sort_by_key(|app| (!app.info.frontmost, app.info.name.to_lowercase()));
    Ok(apps)
}

/// One top-level window of an app, as listed.
#[derive(Debug)]
pub(crate) struct Window {
    pub element: Element,
    pub info: WindowInfo,
    pub states: StateSet,
}

/// The app's windows that are on screen or minimized, active one first.
pub(crate) fn windows(bus: &Bus, root: &Element) -> Reply<Vec<Window>> {
    let mut found = Vec::new();
    for element in bus.children(root)? {
        let states = bus.states(&element)?;
        let role = bus.role(&element)?;
        if !matches!(
            roles::map(&role, states),
            Mapped::Role(Role::Window | Role::Dialog)
        ) {
            continue;
        }
        let minimized = states.has(state::ICONIFIED);
        if !states.has(state::SHOWING) && !minimized {
            continue;
        }
        let info = WindowInfo {
            id: window_id(&element),
            title: bus.name(&element).unwrap_or_default(),
            frame: bus
                .frame(&element, Coords::Screen)
                .ok()
                .flatten()
                .unwrap_or_default(),
            main: states.has(state::ACTIVE),
            minimized,
        };
        found.push(Window {
            element,
            info,
            states,
        });
    }
    found.sort_by_key(|w| !w.info.main);
    if let Some(first) = found.first_mut() {
        first.info.main = true;
    }
    Ok(found)
}

/// A stable id for a window. AT-SPI has no window-server id that works on
/// both X11 and Wayland, so the id is the object's address, hashed.
pub(crate) fn window_id(element: &Element) -> u64 {
    let mut hasher = DefaultHasher::new();
    element.hash(&mut hasher);
    hasher.finish()
}

fn command(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim().to_owned())
}

fn executable(pid: u32) -> Option<String> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    Some(path.to_string_lossy().into_owned())
}

/// Installed apps whose name or desktop id contains `query`.
pub(crate) fn installed(query: &str) -> Vec<AppInfo> {
    let wanted = query.to_lowercase();
    let mut found: Vec<AppInfo> = Vec::new();
    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "desktop") {
                continue;
            }
            let Some(app) = desktop_entry(&path) else {
                continue;
            };
            let matches = app.name.to_lowercase().contains(&wanted)
                || app.id.as_deref().is_some_and(|id| id.contains(&wanted));
            // The first directory wins, as in the XDG spec.
            if matches && !found.iter().any(|a| a.id == app.id) {
                found.push(app);
            }
        }
    }
    found.sort_by_key(|app| app.name.to_lowercase());
    found
}

fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")));
    let shared = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(shared.split(':').map(PathBuf::from))
        .map(|dir| dir.join("applications"))
        .collect()
}

fn desktop_entry(path: &Path) -> Option<AppInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = parse_desktop_entry(&text)?;
    Some(AppInfo {
        name: fields.name,
        pid: None,
        id: path.file_stem().map(|s| s.to_string_lossy().into_owned()),
        path: fields.exec,
        frontmost: false,
    })
}

#[derive(Debug, PartialEq)]
struct DesktopFields {
    name: String,
    exec: Option<String>,
}

/// The `[Desktop Entry]` group of a desktop file, if it is an app a person
/// would launch.
fn parse_desktop_entry(text: &str) -> Option<DesktopFields> {
    let mut in_entry = false;
    let (mut name, mut exec, mut app, mut hidden) = (None, None, false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = Some(value.trim().to_owned()),
            "Exec" => exec = value.split_whitespace().next().map(str::to_owned),
            "Type" => app = value.trim() == "Application",
            "NoDisplay" | "Hidden" => hidden |= value.trim() == "true",
            _ => {}
        }
    }
    (app && !hidden).then_some(DesktopFields { name: name?, exec })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_desktop_entry_group_only() {
        let text = "[Desktop Entry]\nType=Application\nName=Calculator\nExec=gnome-calculator %U\n\n[Desktop Action new]\nName=New Window\n";
        assert_eq!(
            parse_desktop_entry(text),
            Some(DesktopFields {
                name: "Calculator".into(),
                exec: Some("gnome-calculator".into()),
            })
        );
    }

    #[test]
    fn skips_hidden_and_non_app_entries() {
        assert_eq!(
            parse_desktop_entry("[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n"),
            None
        );
        assert_eq!(
            parse_desktop_entry("[Desktop Entry]\nType=Link\nName=X\n"),
            None
        );
    }
}
