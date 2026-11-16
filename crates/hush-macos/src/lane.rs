//! The lane: a virtual display nobody can see, where an agent's apps render
//! and take input without being on the user's screen.
//!
//! A window on another Space, behind a full-screen app or in a hidden app
//! is invisible to the user but also half-dead to an agent: AppKit doesn't
//! list it, Chromium drops its page's accessibility tree, a hidden app
//! ignores clicks and a never-shown window has nothing to capture. On the
//! lane display it is on that display's current Space and unoccluded, so
//! all of that works, and the user never sees it.
//!
//! The display is created on first use with `CGVirtualDisplay` (the private
//! class DeskPad and BetterDisplay use) and lives as long as this process.
//! It is placed diagonally off a corner of the main display: displays touch
//! only at that one point, so the pointer can't wander onto it. When the
//! process ends normally the apps in the lane are hidden, so their windows
//! don't land on the user's screen when the display goes away.

use std::ffi::c_void;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use hush_core::{Error, Rect, Result};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSString};

use crate::ax::Element;
use crate::{act, apps, cf};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
/// How long macOS may take to bring the display online, move it, or show a
/// window there. Measured: about 1 s for the display, under 100 ms for a move.
const LIMIT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(10);

const VENDOR: u32 = 0x4855;
const PRODUCT: u32 = 0x5348;

struct Lane {
    /// Holding it keeps the display alive; `None` when another Hush process
    /// (the daemon, an MCP server) owns the lane this one reuses.
    _display: Option<Retained<AnyObject>>,
    bounds: Rect,
    /// Apps with windows in the lane, hidden again on exit.
    parked: Vec<i32>,
}

// SAFETY: the display object is only retained and released, never messaged
// after creation, and CGVirtualDisplay is thread-agnostic.
unsafe impl Send for Lane {}

static LANE: Mutex<Option<Lane>> = Mutex::new(None);

/// Moves the app's windows into the lane, creating it if needed. A hidden
/// app shows itself there: raising one of its windows makes AppKit unhide it
/// without activating it.
pub(crate) fn park(pid: i32, windows: &[Element]) -> Result<()> {
    if windows.is_empty() {
        return Err(Error::NotFound("the app has no windows to move".into()));
    }
    let bounds = bounds()?;
    for (i, window) in windows.iter().enumerate() {
        let step = 30.0 * i as f64;
        let at = CGPoint {
            x: bounds.x + 40.0 + step,
            y: bounds.y + 40.0 + step,
        };
        set_position(window, at)?;
    }
    if apps::is_hidden(pid) {
        // `AXHidden` false would call `-[NSApplication unhide:]`, which
        // activates; ordering a window in calls `unhideWithoutActivation`.
        act::perform_action(&windows[0], "AXRaise")?;
        if !wait(|| !apps::is_hidden(pid)) {
            return Err(Error::Failed(
                "the app stayed hidden; it can't be shown in the lane without activating it".into(),
            ));
        }
    }
    let inside = |window: &Element| {
        position(window).is_some_and(|p| {
            p.x >= bounds.x
                && p.y >= bounds.y
                && p.x < bounds.x + bounds.w
                && p.y < bounds.y + bounds.h
        })
    };
    if !wait(|| windows.iter().all(inside)) {
        return Err(Error::Failed(
            "the app moved its windows back out of the lane".into(),
        ));
    }
    if let Ok(mut lane) = LANE.lock()
        && let Some(lane) = lane.as_mut()
        && !lane.parked.contains(&pid)
    {
        lane.parked.push(pid);
    }
    Ok(())
}

/// Puts `window` above the other windows in the lane, when it's there.
///
/// Every app's windows go to the same corner of the lane, so they overlap,
/// and macOS drops a drag-and-drop on whatever window is at the drop point:
/// an agent's drag in one app landed in another app's window on top of it.
/// Clicks and keys go to their window directly and don't need this. Raising
/// a window orders it front without activating its app; a window outside the
/// lane is left alone, since raising it would put it over the user's.
pub(crate) fn to_top(window: &Element) -> Result<()> {
    let Some(bounds) = LANE
        .lock()
        .ok()
        .and_then(|lane| lane.as_ref().map(|l| l.bounds))
    else {
        return Ok(());
    };
    let inside = position(window).is_some_and(|p| {
        p.x >= bounds.x && p.y >= bounds.y && p.x < bounds.x + bounds.w && p.y < bounds.y + bounds.h
    });
    if inside {
        act::perform_action(window, "AXRaise")?;
    }
    Ok(())
}

/// Hides the apps in the lane. Called when the platform is dropped, before
/// the display goes away with the process.
pub(crate) fn release() {
    let Ok(mut lane) = LANE.lock() else { return };
    if let Some(lane) = lane.as_mut() {
        for pid in lane.parked.drain(..) {
            // Hiding never activates anything.
            let _ = act::set_bool(&Element::application(pid), "AXHidden", true);
        }
    }
}

/// The lane's frame in global screen coordinates (top-left origin).
fn bounds() -> Result<Rect> {
    let mut lane = LANE.lock().map_err(|_| Error::Failed("lane lock".into()))?;
    if let Some(lane) = lane.as_ref() {
        return Ok(lane.bounds);
    }
    if let Some(id) = existing() {
        let bounds = frame(id);
        *lane = Some(Lane {
            _display: None,
            bounds,
            parked: Vec::new(),
        });
        return Ok(bounds);
    }
    let (display, id) = create()?;
    let bounds = place(id)?;
    *lane = Some(Lane {
        _display: Some(display),
        bounds,
        parked: Vec::new(),
    });
    Ok(bounds)
}

fn class(name: &std::ffi::CStr) -> Result<&'static AnyClass> {
    AnyClass::get(name).ok_or_else(|| {
        Error::Unsupported("this macOS has no virtual displays (CGVirtualDisplay)".into())
    })
}

fn create() -> Result<(Retained<AnyObject>, u32)> {
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
            initWithWidth: WIDTH as usize,
            height: HEIGHT as usize,
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

/// Puts the display diagonally off the main display's bottom-right corner
/// and returns its frame once macOS reports it there.
fn place(id: u32) -> Result<Rect> {
    if !wait(|| online(id)) {
        return Err(Error::Failed("the lane display never came online".into()));
    }
    // SAFETY: plain display configuration calls with a live display id.
    let main = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    let (x, y) = (
        (main.origin.x + main.size.width) as i32,
        (main.origin.y + main.size.height) as i32,
    );
    // SAFETY: as above; the configuration is completed or cancelled.
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
    // SAFETY: reading the bounds of a display id has no preconditions.
    let frame = || unsafe { CGDisplayBounds(id) };
    wait(|| frame().origin.x as i32 == x && frame().origin.y as i32 == y);
    let frame = frame();
    Ok(Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        w: frame.size.width,
        h: frame.size.height,
    })
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
fn existing() -> Option<u32> {
    active().into_iter().find(|&id| {
        // SAFETY: reading a display's identity has no preconditions.
        unsafe { CGDisplayVendorNumber(id) == VENDOR && CGDisplayModelNumber(id) == PRODUCT }
    })
}

fn frame(id: u32) -> Rect {
    // SAFETY: reading the bounds of a display id has no preconditions.
    let frame = unsafe { CGDisplayBounds(id) };
    Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        w: frame.size.width,
        h: frame.size.height,
    }
}

fn wait(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + LIMIT;
    while !done() {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
    true
}

fn set_position(window: &Element, at: CGPoint) -> Result<()> {
    // SAFETY: AXValueCreate copies the point.
    let value = unsafe {
        objc2_application_services::AXValue::new(
            objc2_application_services::AXValueType::CGPoint,
            std::ptr::NonNull::from(&at).cast(),
        )
    }
    .ok_or_else(|| Error::Failed("couldn't build a position".into()))?;
    let value: CFRetained<objc2_core_foundation::CFType> = value.into();
    act::set(window, "AXPosition", &value)
}

fn position(window: &Element) -> Option<CGPoint> {
    window.value("AXPosition").ok().and_then(|v| cf::point(&v))
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
