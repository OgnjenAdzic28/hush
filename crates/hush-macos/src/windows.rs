//! An app's windows: the ones Accessibility lists, plus the ones the window
//! server knows about on other Spaces.
//!
//! AppKit answers `AXWindows` with the app's windows on the current Spaces
//! only (`+[NSCGSSpace currentManagedSpaces]`, traced on macOS 27.2), plus
//! minimized ones; a hidden app lists all of them. For every other window
//! the server reports, the cheapest route that works is used: an element
//! found earlier, one from a window-created notification, the app's main
//! and focused window (AppKit answers those whatever their Space), and a
//! remote-token lookup.
//!
//! What stays out of reach: a window opened on another Space before Hush
//! watched the app, that isn't its main or focused window and that no
//! assistive app has asked about. AppKit gives a window an element id only
//! when it first hands it out, and a token lookup needs that id. Hiding the
//! app would list it, but showing an app again from outside activates it
//! (`AXHidden` calls `-[NSApplication unhide:]`), so Hush doesn't.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use hush_core::{Rect, WindowInfo};

use crate::ax::Element;
use crate::cf;
use crate::private;
use crate::server::{self, Which};

/// How long a window no route could reach is left alone before trying
/// again. The token scan costs 34 to 88 ms, so it isn't repeated on every
/// call; the window can become reachable later (the user visits its Space).
const RETRY_AFTER: Duration = Duration::from_secs(10);

/// Off-Space window elements found by any route but `AXWindows`, by (pid,
/// window id), kept while their window exists so later calls don't search
/// again.
pub(crate) type Known = HashMap<(i32, u32), Element>;

/// The app's windows with their elements, main window first. `created` are
/// the windows the app announced since the last call.
pub(crate) fn list(
    app: &Element,
    pid: i32,
    created: Vec<Element>,
    known: &mut Known,
) -> Vec<(WindowInfo, Element)> {
    let mut elements = app.elements("AXWindows");
    let listed: Vec<u32> = elements.iter().filter_map(private::window_id).collect();
    let server_windows: Vec<_> = server::windows(Which::All)
        .into_iter()
        .filter(|w| w.pid == pid && w.is_document_sized())
        .collect();
    let on_server: Vec<u32> = server_windows.iter().map(|w| w.id).collect();
    // Only titled windows are searched for: the server also lists untitled
    // helper surfaces (Electron's 500x500 placeholders, panels) that have no
    // element, and searching for those cost every call the whole scan.
    let titled: Vec<u32> = server_windows
        .iter()
        .filter(|w| w.titled)
        .map(|w| w.id)
        .collect();
    known.retain(|(owner, id), _| *owner != pid || on_server.contains(id));
    for element in created {
        if let Some(id) = private::window_id(&element) {
            known.insert((pid, id), element);
        }
    }
    let mut missing: Vec<u32> = Vec::new();
    for id in on_server.iter().filter(|id| !listed.contains(id)) {
        match known.get(&(pid, *id)) {
            Some(element) => elements.push(element.clone()),
            None if titled.contains(id) && !recently_unreachable(pid, *id) => missing.push(*id),
            None => {}
        }
    }
    if !missing.is_empty() {
        let mut found: Vec<(u32, Element)> = ["AXMainWindow", "AXFocusedWindow"]
            .into_iter()
            .filter_map(|attribute| app.element(attribute))
            .filter_map(|element| Some((private::window_id(&element)?, element)))
            .filter(|(id, _)| missing.contains(id))
            .collect();
        found.dedup_by_key(|(id, _)| *id);
        let left: Vec<u32> = missing
            .iter()
            .copied()
            .filter(|id| !found.iter().any(|(known, _)| known == id))
            .collect();
        if !left.is_empty() {
            found.extend(private::windows_by_token(pid, &left));
        }
        for id in &missing {
            if !found.iter().any(|(known, _)| known == id) {
                mark_unreachable(pid, *id);
            }
        }
        for (id, element) in found {
            known.insert((pid, id), element.clone());
            elements.push(element);
        }
    }
    let mut windows: Vec<(WindowInfo, Element)> = elements
        .into_iter()
        .filter_map(|window| {
            let position = window
                .value("AXPosition")
                .ok()
                .and_then(|v| cf::point(&v))?;
            let size = window.value("AXSize").ok().and_then(|v| cf::size(&v))?;
            let flag = |name: &str| {
                window
                    .value(name)
                    .ok()
                    .and_then(|v| cf::boolean(&v))
                    .unwrap_or(false)
            };
            let info = WindowInfo {
                id: private::window_id(&window).map_or_else(|| fallback_id(&window), u64::from),
                title: window.string("AXTitle").unwrap_or_default(),
                frame: Rect {
                    x: position.x,
                    y: position.y,
                    w: size.width,
                    h: size.height,
                },
                main: flag("AXMain"),
                minimized: flag("AXMinimized"),
            };
            Some((info, window))
        })
        .collect();
    // Stable: AXWindows is already front-to-back within the app.
    windows.sort_by_key(|(info, _)| (!info.main, info.minimized));
    windows
}

/// An id for a window whose window-server id couldn't be read. The
/// element's hash is stable while the window exists; the top bit keeps it
/// apart from real window ids, which are 32-bit.
fn fallback_id(window: &Element) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    window.hash(&mut hasher);
    hasher.finish() | (1 << 63)
}

/// When each window no route could reach was last tried.
fn unreachable() -> &'static Mutex<HashMap<(i32, u32), Instant>> {
    static UNREACHABLE: OnceLock<Mutex<HashMap<(i32, u32), Instant>>> = OnceLock::new();
    UNREACHABLE.get_or_init(Mutex::default)
}

fn recently_unreachable(pid: i32, id: u32) -> bool {
    unreachable().lock().is_ok_and(|map| {
        map.get(&(pid, id))
            .is_some_and(|at| at.elapsed() < RETRY_AFTER)
    })
}

fn mark_unreachable(pid: i32, id: u32) {
    if let Ok(mut map) = unreachable().lock() {
        map.insert((pid, id), Instant::now());
    }
}
