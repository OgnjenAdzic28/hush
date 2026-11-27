//! Keeps the user's pointer off the lane.
//!
//! macOS has no display the pointer can't reach: every display touches
//! another, and the lane touches the user's at one corner (see `display`).
//! The pointer slips through that corner, and a click there makes the lane
//! the active display, which dims the menu bar on the user's own screen.
//!
//! A listen-only event tap sees the user's pointer moves. When one lands
//! in the lane, the pointer goes to the nearest point on the user's
//! displays, so it stops at the edge as it would at the edge of a lone
//! monitor. This is the one place Hush moves the user's pointer, and only
//! ever back onto their own screens. Hush's own clicks go to their app
//! directly and never pass through the tap.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::thread;

use hush_core::Rect;
use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, CGPoint, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGAssociateMouseAndMouseCursorPosition, CGEvent, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType, CGWarpMouseCursorPosition,
};

use super::display;

/// Starts guarding the lane display `id` on a thread of its own, for as long
/// as the process.
pub(super) fn start(id: u32) {
    let spawned = thread::Builder::new()
        .name("hush-lane-guard".into())
        .spawn(move || {
            let Some(tap) = tap(id) else { return };
            let Some(source) = CFMachPort::new_run_loop_source(None, Some(&tap), 0) else {
                return;
            };
            let Some(run_loop) = CFRunLoop::current() else {
                return;
            };
            // SAFETY: the common modes constant is always set.
            run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
            CFRunLoop::run();
        });
    // Without the guard the lane still works; the pointer can just reach it.
    let _ = spawned;
}

fn tap(id: u32) -> Option<CFRetained<CFMachPort>> {
    let mask = [
        CGEventType::MouseMoved,
        CGEventType::LeftMouseDragged,
        CGEventType::RightMouseDragged,
        CGEventType::OtherMouseDragged,
    ]
    .iter()
    .fold(0u64, |mask, kind| mask | 1 << kind.0);
    // The tap lives as long as the process, and so does what it points to.
    let guard = Box::into_raw(Box::new(Guard { id, tap: None }));
    // SAFETY: a valid callback, and `guard` outlives the tap.
    let tap = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            // Listen only: a slow or stuck callback never holds up input.
            CGEventTapOptions::ListenOnly,
            mask,
            Some(callback),
            guard.cast(),
        )
    }?;
    // SAFETY: `guard` is live and only this thread's run loop touches it.
    unsafe { (*guard).tap = Some(tap.clone()) };
    Some(tap)
}

struct Guard {
    id: u32,
    /// For turning the tap back on when macOS turns it off.
    tap: Option<CFRetained<CFMachPort>>,
}

unsafe extern "C-unwind" fn callback(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: NonNull<CGEvent>,
    info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: `info` is the leaked `Guard` from `tap`.
    let guard = unsafe { &*info.cast::<Guard>() };
    if kind == CGEventType::TapDisabledByTimeout || kind == CGEventType::TapDisabledByUserInput {
        if let Some(tap) = &guard.tap {
            CGEvent::tap_enable(tap, true);
        }
        return event.as_ptr();
    }
    // SAFETY: the tap passes a live event for the call.
    let at = CGEvent::location(Some(unsafe { event.as_ref() }));
    let lane = display::frame(guard.id);
    let screens = || {
        display::active()
            .into_iter()
            .filter(|&other| other != guard.id)
            .map(display::frame)
            .collect::<Vec<_>>()
    };
    if within(at.x, at.y, lane)
        && let Some((x, y)) = nearest(at.x, at.y, &screens())
    {
        let _ = CGWarpMouseCursorPosition(CGPoint { x, y });
        // A warp freezes the pointer for a quarter second unless the mouse
        // is joined back to it at once.
        let _ = CGAssociateMouseAndMouseCursorPosition(true);
    }
    event.as_ptr()
}

fn within(x: f64, y: f64, r: Rect) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h
}

/// The point on `screens` closest to `(x, y)`, on the last pixel row or
/// column when it is past an edge.
fn nearest(x: f64, y: f64, screens: &[Rect]) -> Option<(f64, f64)> {
    screens
        .iter()
        .filter(|s| s.w >= 1.0 && s.h >= 1.0)
        .map(|s| {
            let px = x.clamp(s.x, s.x + s.w - 1.0);
            let py = y.clamp(s.y, s.y + s.h - 1.0);
            (px, py, (px - x).powi(2) + (py - y).powi(2))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(px, py, _)| (px, py))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn a_pointer_through_the_corner_stops_on_the_corner_pixel() {
        let main = screen(0.0, 0.0, 1920.0, 1080.0);
        let beside = screen(1920.0, 0.0, 1920.0, 1080.0);
        let lane = screen(3840.0, 1080.0, 1920.0, 1080.0);
        assert!(within(3840.5, 1080.0, lane));
        assert_eq!(
            nearest(3840.5, 1080.0, &[main, beside]),
            Some((3839.0, 1079.0))
        );
    }

    #[test]
    fn a_pointer_deep_in_the_lane_goes_to_the_closest_screen() {
        let main = screen(0.0, 0.0, 1512.0, 982.0);
        let below = screen(-500.0, 982.0, 1000.0, 800.0);
        assert_eq!(
            nearest(1600.0, 1100.0, &[main, below]),
            Some((1511.0, 981.0))
        );
        assert_eq!(
            nearest(520.0, 1500.0, &[main, below]),
            Some((499.0, 1500.0))
        );
    }

    #[test]
    fn with_no_screen_left_the_pointer_stays() {
        assert_eq!(nearest(10.0, 10.0, &[]), None);
    }
}
