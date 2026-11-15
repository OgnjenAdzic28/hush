//! An app's windows, as Accessibility reports them.

use hush_core::{Rect, WindowInfo};

use crate::ax::Element;
use crate::cf;
use crate::private;

/// The app's windows with their elements, main window first.
pub(crate) fn list(app: &Element) -> Vec<(WindowInfo, Element)> {
    let mut windows: Vec<(WindowInfo, Element)> = app
        .elements("AXWindows")
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
