//! Running apps (from the accessibility registry) and their windows.
//! Installed apps come from desktop entries (`installed.rs`).

use std::hash::{DefaultHasher, Hash, Hasher};

use futures_util::future::join_all;
use hush_core::{AppInfo, Role, WindowInfo};

use crate::bus::{Bus, Coords, Element, Reply};
use crate::installed;
use crate::roles::{self, Mapped, StateSet, state};

/// A running app: its accessibility root and what Hush reports about it.
#[derive(Debug)]
pub(crate) struct Running {
    pub root: Element,
    pub info: AppInfo,
}

/// Every app on the accessibility bus that has at least one window, all
/// asked at once.
pub(crate) async fn running(bus: &Bus) -> Reply<Vec<Running>> {
    let roots = bus.children(&Element::desktop()).await?;
    let found = join_all(roots.into_iter().map(|root| describe(bus, root))).await;
    let mut apps: Vec<Running> = found.into_iter().flatten().collect();
    apps.sort_by_key(|app| (!app.info.frontmost, app.info.name.to_lowercase()));
    Ok(apps)
}

/// `None` for an app without windows, or one that quit mid-listing.
async fn describe(bus: &Bus, root: Element) -> Option<Running> {
    let (windows, pid, name) =
        futures_util::join!(windows(bus, &root), bus.pid(&root.bus), bus.name(&root));
    let windows = windows.ok()?;
    if windows.is_empty() {
        return None;
    }
    let pid = pid.ok();
    let path = pid.and_then(executable);
    // The name a person knows the app by is its menu entry's ("Calculator"),
    // which toolkits often don't report ("gnome-calculator").
    let args = pid.map(arguments).unwrap_or_default();
    let entry = path
        .as_deref()
        .and_then(|p| installed::by_process(p, &args));
    let reported = name.unwrap_or_default();
    let name = match (&entry, reported.is_empty()) {
        (Some(entry), _) => entry.name.clone(),
        (None, false) => reported,
        (None, true) => pid.and_then(command).unwrap_or_default(),
    };
    let info = AppInfo {
        name,
        pid,
        id: entry.and_then(|e| e.id),
        path,
        frontmost: windows.iter().any(|w| w.states.has(state::ACTIVE)),
    };
    Some(Running { root, info })
}

/// One top-level window of an app, as listed.
#[derive(Debug)]
pub(crate) struct Window {
    pub element: Element,
    pub info: WindowInfo,
    pub states: StateSet,
}

/// The app's windows that are on screen or minimized, active one first.
pub(crate) async fn windows(bus: &Bus, root: &Element) -> Reply<Vec<Window>> {
    let children = bus.children(root).await?;
    let read = join_all(children.into_iter().map(|element| window(bus, element))).await;
    let mut found: Vec<Window> = read.into_iter().flatten().collect();
    found.sort_by_key(|w| !w.info.main);
    if let Some(first) = found.first_mut() {
        first.info.main = true;
    }
    Ok(found)
}

/// `None` unless the element is a window that is showing or minimized.
async fn window(bus: &Bus, element: Element) -> Option<Window> {
    let (states, role, title, frame) = futures_util::join!(
        bus.states(&element),
        bus.role(&element),
        bus.name(&element),
        bus.frame(&element, Coords::Screen),
    );
    let states = states.ok()?;
    if !matches!(
        roles::map(role.ok()?, states),
        Mapped::Role(Role::Window | Role::Dialog)
    ) {
        return None;
    }
    let minimized = states.has(state::ICONIFIED);
    if !states.has(state::SHOWING) && !minimized {
        return None;
    }
    let info = WindowInfo {
        id: window_id(&element),
        title: title.unwrap_or_default(),
        frame: frame.ok().flatten().unwrap_or_default(),
        main: states.has(state::ACTIVE),
        minimized,
    };
    Some(Window {
        element,
        info,
        states,
    })
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

/// The process's command line after the program.
fn arguments(pid: u32) -> Vec<String> {
    let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return Vec::new();
    };
    raw.split(|b| *b == 0)
        .skip(1)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect()
}

fn executable(pid: u32) -> Option<String> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    Some(path.to_string_lossy().into_owned())
}
