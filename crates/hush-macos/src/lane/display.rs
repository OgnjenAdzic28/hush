//! The lane's display: a `CGVirtualDisplay` (the private class DeskPad and
//! BetterDisplay use) placed diagonally off the bottom-right corner of the
//! user's displays, so it touches them at one point only and the pointer
//! can't wander onto it.

use std::ffi::c_void;

use hush_core::{Error, Rect, Result};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_core_foundation::{CGRect, CGSize};
use objc2_foundation::{NSArray, NSString};

use super::{LIMIT, wait};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

const VENDOR: u32 = 0x4855;
const PRODUCT: u32 = 0x5348;

fn class(name: &std::ffi::CStr) -> Result<&'static AnyClass> {
    AnyClass::get(name).ok_or_else(|| {
        Error::Unsupported("this macOS has no virtual displays (CGVirtualDisplay)".into())
    })
}

pub(super) fn create() -> Result<(Retained<AnyObject>, u32)> {
    let descriptor_class = class(c"CGVirtualDisplayDescriptor")?;
    let display_class = class(c"CGVirtualDisplay")?;
    let settings_class = class(c"CGVirtualDisplaySettings")?;
    let mode_class = class(c"CGVirtualDisplayMode")?;
    let name = NSString::from_str("Hush Lane");
    // SAFETY: the selectors and argument types are the ones DeskPad (MIT)
    // declares for these classes (macOS 11 through 27); every object is
    // checked for nil before use.
    unsafe {
        let descriptor: Retained<AnyObject> = msg_send![descriptor_class, new];
        let queue = dispatch_get_global_queue(0, 0);
        let _: () = msg_send![&*descriptor, setDispatchQueue: queue];
        let _: () = msg_send![&*descriptor, setName: &*name];
        let _: () = msg_send![&*descriptor, setMaxPixelsWide: WIDTH];
        let _: () = msg_send![&*descriptor, setMaxPixelsHigh: HEIGHT];
        let size = CGSize {
            width: 600.0,
            height: 340.0,
        };
        let _: () = msg_send![&*descriptor, setSizeInMillimeters: size];
        let _: () = msg_send![&*descriptor, setVendorID: VENDOR];
        let _: () = msg_send![&*descriptor, setProductID: PRODUCT];
        let _: () = msg_send![&*descriptor, setSerialNum: std::process::id()];

        let allocated: *mut AnyObject = msg_send![display_class, alloc];
        let display: Option<Retained<AnyObject>> =
            Retained::from_raw(msg_send![allocated, initWithDescriptor: &*descriptor]);
        let display = display
            .ok_or_else(|| Error::Failed("macOS refused to create the lane display".into()))?;

        let allocated: *mut AnyObject = msg_send![mode_class, alloc];
        let mode: Option<Retained<AnyObject>> = Retained::from_raw(msg_send![
            allocated,
            initWithWidth: WIDTH,
            height: HEIGHT,
            refreshRate: 60.0_f64
        ]);
        let mode = mode.ok_or_else(|| Error::Failed("couldn't describe the lane's mode".into()))?;
        let settings: Retained<AnyObject> = msg_send![settings_class, new];
        let _: () = msg_send![&*settings, setHiDPI: 0_u32];
        let modes = NSArray::from_retained_slice(&[mode]);
        let _: () = msg_send![&*settings, setModes: &*modes];
        let applied: bool = msg_send![&*display, applySettings: &*settings];
        if !applied {
            return Err(Error::Failed(
                "macOS refused the lane display's mode".into(),
            ));
        }
        let id: u32 = msg_send![&*display, displayID];
        Ok((display, id))
    }
}

/// Puts the display at `corner` and returns its frame once macOS reports
/// it there.
pub(super) fn place(id: u32) -> Result<Rect> {
    if !wait(LIMIT, || online(id)) {
        return Err(Error::Failed("the lane display never came online".into()));
    }
    let screens: Vec<Rect> = active()
        .into_iter()
        .filter(|&other| other != id)
        .map(frame)
        .collect();
    let (x, y) = corner(&screens, f64::from(WIDTH), f64::from(HEIGHT));
    let (x, y) = (x as i32, y as i32);
    // SAFETY: display configuration calls with a live display id; the
    // configuration is completed or cancelled.
    unsafe {
        let mut config: *mut c_void = std::ptr::null_mut();
        if CGBeginDisplayConfiguration(&mut config) != 0 {
            return Err(Error::Failed("couldn't arrange the lane display".into()));
        }
        CGConfigureDisplayOrigin(config, id, x, y);
        // kCGConfigureForSession: undone at logout, never saved.
        if CGCompleteDisplayConfiguration(config, 1) != 0 {
            CGCancelDisplayConfiguration(config);
            return Err(Error::Failed("couldn't arrange the lane display".into()));
        }
    }
    wait(LIMIT, || {
        let now = frame(id);
        now.x as i32 == x && now.y as i32 == y
    });
    Ok(frame(id))
}

/// Where a `w` by `h` display touches `screens` at one point only: a
/// screen's bottom-right corner that no other screen is next to. With a
/// second monitor beside the main one, the main one's corner is on that
/// monitor's bottom edge, which the pointer would cross.
fn corner(screens: &[Rect], w: f64, h: f64) -> (f64, f64) {
    let mut corners: Vec<(f64, f64)> = screens.iter().map(|s| (s.x + s.w, s.y + s.h)).collect();
    corners.sort_by(|a, b| (b.0 + b.1).total_cmp(&(a.0 + a.1)));
    let alone = |&(x, y): &(f64, f64)| {
        screens.iter().all(|s| {
            let across = (s.x + s.w).min(x + w) - s.x.max(x);
            let down = (s.y + s.h).min(y + h) - s.y.max(y);
            // Apart, or meeting at the corner only.
            across < 0.0 || down < 0.0 || (across == 0.0 && down == 0.0)
        })
    };
    let right = screens.iter().map(|s| s.x + s.w).fold(0.0, f64::max);
    let bottom = screens.iter().map(|s| s.y + s.h).fold(0.0, f64::max);
    corners.into_iter().find(alone).unwrap_or((right, bottom))
}

fn active() -> Vec<u32> {
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    // SAFETY: the buffer holds 16 ids and `count` is a valid out-pointer.
    unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut count) };
    ids[..count as usize].to_vec()
}

fn online(id: u32) -> bool {
    active().contains(&id)
}

/// A lane display another Hush process already made.
pub(super) fn existing() -> Option<u32> {
    active().into_iter().find(|&id| {
        // SAFETY: reading a display's identity has no preconditions.
        unsafe { CGDisplayVendorNumber(id) == VENDOR && CGDisplayModelNumber(id) == PRODUCT }
    })
}

pub(super) fn frame(id: u32) -> Rect {
    // SAFETY: reading the bounds of a display id has no preconditions.
    let frame = unsafe { CGDisplayBounds(id) };
    Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        w: frame.size.width,
        h: frame.size.height,
    }
}

/// The main display's frame in global screen coordinates.
pub(super) fn main() -> Rect {
    // SAFETY: reading the main display id has no preconditions.
    frame(unsafe { CGMainDisplayID() })
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
    fn CGDisplayVendorNumber(display: u32) -> u32;
    fn CGDisplayModelNumber(display: u32) -> u32;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGBeginDisplayConfiguration(config: *mut *mut c_void) -> i32;
    fn CGConfigureDisplayOrigin(config: *mut c_void, display: u32, x: i32, y: i32) -> i32;
    fn CGCompleteDisplayConfiguration(config: *mut c_void, option: u32) -> i32;
    fn CGCancelDisplayConfiguration(config: *mut c_void) -> i32;
}

unsafe extern "C" {
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut AnyObject;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn one_display_gets_the_lane_off_its_corner() {
        let main = screen(0.0, 0.0, 1512.0, 982.0);
        assert_eq!(corner(&[main], 1920.0, 1080.0), (1512.0, 982.0));
    }

    #[test]
    fn a_monitor_beside_the_main_one_moves_the_lane_past_it() {
        let main = screen(0.0, 0.0, 1920.0, 1080.0);
        let beside = screen(1920.0, 0.0, 1920.0, 1080.0);
        assert_eq!(corner(&[main, beside], 1920.0, 1080.0), (3840.0, 1080.0));
    }

    #[test]
    fn a_monitor_below_and_to_the_left_is_stepped_around() {
        let main = screen(0.0, 0.0, 1920.0, 1080.0);
        let below = screen(-500.0, 1080.0, 2560.0, 1440.0);
        assert_eq!(corner(&[main, below], 1920.0, 1080.0), (2060.0, 2520.0));
    }
}
