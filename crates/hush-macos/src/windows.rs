//! An app's windows: the ones Accessibility lists, plus the ones the window
//! server knows about on other Spaces.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use hush_core::{Rect, WindowInfo};

use crate::ax::Element;
use crate::cf;
use crate::private;
use crate::server::{self, Which};

/// The app's windows with their elements, main window first.
pub(crate) fn list(app: &Element, pid: i32) -> Vec<(WindowInfo, Element)> {
    let mut elements = app.elements("AXWindows");
    let listed: Vec<u32> = elements.iter().filter_map(private::window_id).collect();
    let missing: Vec<u32> = server::windows(Which::All)
        .into_iter()
        .filter(|w| w.pid == pid && w.is_document_sized())
        .map(|w| w.id)
        .filter(|id| !listed.contains(id) && !unresolvable(pid, *id))
        .collect();
    if !missing.is_empty() {
        let found = private::windows_by_token(pid, &missing);
        for id in &missing {
            if !found.iter().any(|(known, _)| known == id) {
                mark_unresolvable(pid, *id);
            }
        }
        elements.extend(found.into_iter().map(|(_, element)| element));
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

/// Window ids the token scan didn't find, so later calls don't scan again.
fn unresolved() -> &'static Mutex<HashSet<(i32, u32)>> {
    static UNRESOLVED: OnceLock<Mutex<HashSet<(i32, u32)>>> = OnceLock::new();
    UNRESOLVED.get_or_init(Mutex::default)
}

fn unresolvable(pid: i32, id: u32) -> bool {
    unresolved()
        .lock()
        .is_ok_and(|set| set.contains(&(pid, id)))
}

fn mark_unresolvable(pid: i32, id: u32) {
    if let Ok(mut set) = unresolved().lock() {
        set.insert((pid, id));
    }
}
