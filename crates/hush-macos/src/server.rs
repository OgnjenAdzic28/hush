//! The window server's list of windows: always current, in front-to-back
//! order, and covering every Space, unlike Accessibility's per-app list.

use objc2_core_foundation::{CFArray, CFDictionary, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
    kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerPID,
};

use crate::cf;

/// One normal (layer 0) window.
pub(crate) struct ServerWindow {
    pub(crate) id: u32,
    pub(crate) pid: i32,
    pub(crate) width: f64,
    pub(crate) height: f64,
    /// Has a title. Untitled ones are mostly an app's helper surfaces.
    pub(crate) titled: bool,
}

impl ServerWindow {
    /// Status items, hidden panels and other helpers are tiny.
    pub(crate) fn is_document_sized(&self) -> bool {
        self.width >= 100.0 && self.height >= 60.0
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Which {
    /// On screen on the current Space, front to back.
    OnScreen,
    /// On every Space, including minimized and off-screen windows.
    All,
}

pub(crate) fn windows(which: Which) -> Vec<ServerWindow> {
    let options = match which {
        Which::OnScreen => {
            CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements
        }
        Which::All => CGWindowListOption::OptionAll,
    };
    let Some(list) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else {
        return Vec::new();
    };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of dictionaries
    // mapping CFString keys to CF values.
    let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(list) };
    // SAFETY: framework constants that are always set.
    let (pid_key, layer_key, number_key, bounds_key, name_key) = unsafe {
        (
            kCGWindowOwnerPID,
            kCGWindowLayer,
            kCGWindowNumber,
            kCGWindowBounds,
            kCGWindowName,
        )
    };
    list.iter()
        .filter_map(|window| {
            let number = |key: &CFString| window.get(key).and_then(|v| cf::number(&v));
            if number(layer_key)? != 0.0 {
                return None;
            }
            let bounds = window.get(bounds_key)?;
            let bounds = bounds.downcast_ref::<CFDictionary>()?;
            // SAFETY: the bounds dictionary maps CFString keys to numbers.
            let bounds: &CFDictionary<CFString, CFType> = unsafe { bounds.cast_unchecked() };
            let side = |name: &str| {
                bounds
                    .get(&cf::cfstr(name))
                    .and_then(|v| cf::number(&v))
                    .unwrap_or(0.0)
            };
            Some(ServerWindow {
                id: number(number_key)? as u32,
                pid: number(pid_key)? as i32,
                width: side("Width"),
                height: side("Height"),
                titled: window
                    .get(name_key)
                    .and_then(|v| cf::string(&v))
                    .is_some_and(|name| !name.is_empty()),
            })
        })
        .collect()
}
