//! Every undocumented macOS API Hush uses, in one place so an OS update has
//! one file to review. Symbols are looked up at runtime; when one is missing
//! the feature that needs it fails with a clear error instead of crashing.
//!
//! Sources for the signatures and record layouts: open-computer-use (MIT)
//! `SkyLightSPI.swift`, verified on macOS 27.0, Cua Driver (MIT)
//! `skylight.rs`, and yabai (MIT) `window_manager_make_key_window`.

use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

use hush_core::{Error, Result};
use objc2_application_services::{AXError, AXUIElement};
use objc2_core_graphics::CGEvent;

use crate::ax::Element;

const APPLICATION_SERVICES: &CStr =
    c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices";

fn symbol(library: &CStr, name: &CStr) -> Option<*mut c_void> {
    // SAFETY: dlopen and dlsym take valid C strings; a null result is handled.
    unsafe {
        let handle = libc::dlopen(library.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL);
        if handle.is_null() {
            return None;
        }
        let pointer = libc::dlsym(handle, name.as_ptr());
        (!pointer.is_null()).then_some(pointer)
    }
}

type GetWindow = unsafe extern "C" fn(*const AXUIElement, *mut u32) -> AXError;

/// The window server id (CGWindowID) behind an AX window element.
pub(crate) fn window_id(window: &Element) -> Option<u32> {
    static FUNCTION: OnceLock<Option<GetWindow>> = OnceLock::new();
    let function = (*FUNCTION.get_or_init(|| {
        // SAFETY: the symbol has this signature on every macOS since 10.x.
        symbol(APPLICATION_SERVICES, c"_AXUIElementGetWindow")
            .map(|p| unsafe { std::mem::transmute::<*mut c_void, GetWindow>(p) })
    }))?;
    let mut id = 0;
    // SAFETY: the element is live for the call and `id` is a valid out-pointer.
    let status = unsafe { function(window.raw(), &mut id) };
    (status == AXError::Success && id != 0).then_some(id)
}

// ---------------------------------------------------------------------------
// SkyLight: background clicks, synthetic key window, window capture.

const SKYLIGHT: &CStr = c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";

type CfRef = *mut c_void;

struct SkyLight {
    post_to_pid: unsafe extern "C" fn(i32, CfRef),
    set_window_location: unsafe extern "C" fn(CfRef, f64, f64),
    post_record: unsafe extern "C" fn(*const u8, *const u8) -> i32,
    process_for_pid: unsafe extern "C" fn(i32, *mut u8) -> i32,
    main_connection: unsafe extern "C" fn() -> u32,
    capture_windows: unsafe extern "C" fn(u32, *const u32, i32, u32) -> CfRef,
}

fn skylight() -> Option<&'static SkyLight> {
    static API: OnceLock<Option<SkyLight>> = OnceLock::new();
    API.get_or_init(|| {
        // SAFETY: each field's type is the symbol's signature, as used by
        // open-computer-use and Cua Driver on macOS 26 and 27.
        unsafe {
            Some(SkyLight {
                post_to_pid: function(SKYLIGHT, c"SLEventPostToPid")?,
                set_window_location: function(SKYLIGHT, c"CGEventSetWindowLocation")?,
                post_record: function(SKYLIGHT, c"SLPSPostEventRecordTo")?,
                process_for_pid: function(APPLICATION_SERVICES, c"GetProcessForPID")?,
                main_connection: function(SKYLIGHT, c"SLSMainConnectionID")?,
                capture_windows: function(SKYLIGHT, c"SLSHWCaptureWindowList")?,
            })
        }
    })
    .as_ref()
}

/// A symbol as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the `extern "C" fn` type matching the symbol's signature.
unsafe fn function<F: Copy>(library: &CStr, name: &CStr) -> Option<F> {
    let pointer = symbol(library, name)?;
    assert_eq!(size_of::<F>(), size_of::<*mut c_void>());
    // SAFETY: the caller guarantees `F` is the symbol's function type.
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&pointer) })
}

fn unavailable() -> Error {
    Error::Unsupported(
        "this macOS version lacks the window server calls background input needs".into(),
    )
}

/// Posts `event` to `pid` through SkyLight, which (unlike the public call)
/// tells the window server the input is live, as Chromium requires.
pub(crate) fn post_to_pid(pid: i32, event: &CGEvent) -> Result<()> {
    let api = skylight().ok_or_else(unavailable)?;
    // SAFETY: the event is live for the call.
    unsafe { (api.post_to_pid)(pid, std::ptr::from_ref(event).cast_mut().cast()) };
    Ok(())
}

/// Stamps where in the window a pointer event lands. macOS 27 reads this
/// as a window-local point; earlier releases as a screen point.
pub(crate) fn set_window_location(event: &CGEvent, x: f64, y: f64) -> Result<()> {
    let api = skylight().ok_or_else(unavailable)?;
    // SAFETY: the event is live for the call.
    unsafe { (api.set_window_location)(std::ptr::from_ref(event).cast_mut().cast(), x, y) };
    Ok(())
}

/// The target app believes it is active and `window` is its key window,
/// while the window server's front app, z-order and Space don't change and
/// the user's app never resigns key or active. Undo with `end`.
#[derive(Debug)]
pub(crate) struct SyntheticFocus {
    psn: [u8; 8],
    window: u32,
}

impl SyntheticFocus {
    pub(crate) fn begin(pid: i32, window: u32) -> Result<SyntheticFocus> {
        let api = skylight().ok_or_else(unavailable)?;
        let mut psn = [0u8; 8];
        // SAFETY: `psn` has room for a ProcessSerialNumber.
        let status = unsafe { (api.process_for_pid)(pid, psn.as_mut_ptr()) };
        if status != 0 {
            return Err(Error::Failed(format!(
                "couldn't find the app's process ({status})"
            )));
        }
        let focus = SyntheticFocus { psn, window };
        focus.post(&activation_record(window, true))?;
        Ok(focus)
    }

    /// Makes the window key inside the app (yabai's record pair). Must
    /// follow `begin`; the other way round the app ignores it.
    pub(crate) fn make_key(&self) -> Result<()> {
        let mut record = [0u8; 0xF8];
        record[0x04] = 0xF8;
        record[0x3A] = 0x10;
        record[0x3C..0x40].copy_from_slice(&self.window.to_le_bytes());
        record[0x20..0x30].fill(0xFF);
        for kind in [0x01, 0x02] {
            record[0x08] = kind;
            self.post(&record)?;
        }
        Ok(())
    }

    pub(crate) fn end(self) {
        let _ = self.post(&activation_record(self.window, false));
    }

    fn post(&self, record: &[u8; 0xF8]) -> Result<()> {
        let api = skylight().ok_or_else(unavailable)?;
        // SAFETY: both buffers live for the call; the record is 0xF8 bytes.
        let status = unsafe { (api.post_record)(self.psn.as_ptr(), record.as_ptr()) };
        if status == 0 {
            Ok(())
        } else {
            Err(Error::Failed(format!(
                "the window server refused a focus record ({status})"
            )))
        }
    }
}

fn activation_record(window: u32, focused: bool) -> [u8; 0xF8] {
    let mut record = [0u8; 0xF8];
    record[0x04] = 0xF8;
    record[0x08] = 0x0D;
    record[0x3C..0x40].copy_from_slice(&window.to_le_bytes());
    record[0x8A] = if focused { 0x01 } else { 0x02 };
    record
}

/// A +1 CGImage of the window's backing store, or `None`.
pub(crate) fn capture_window(window: u32) -> Option<CfRef> {
    let api = skylight()?;
    // 0x800: ignore the global clip shape, so covered parts are included.
    // SAFETY: one valid window id; the result is a +1 CFArray or null.
    let array = unsafe { (api.capture_windows)((api.main_connection)(), &window, 1, 0x800) };
    if array.is_null() {
        return None;
    }
    // SAFETY: a CFArray of CGImages; the first is retained before the array goes.
    unsafe {
        let image = if CFArrayGetCount(array) > 0 {
            CFArrayGetValueAtIndex(array, 0)
        } else {
            std::ptr::null_mut()
        };
        if !image.is_null() {
            CFRetain(image);
        }
        CFRelease(array);
        (!image.is_null()).then_some(image)
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: CfRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CfRef, index: isize) -> CfRef;
    fn CFRetain(object: CfRef) -> CfRef;
    fn CFRelease(object: CfRef);
}

/// The major macOS version, like 27.
pub(crate) fn macos_major() -> u32 {
    static MAJOR: OnceLock<u32> = OnceLock::new();
    *MAJOR.get_or_init(|| {
        let mut buffer = [0u8; 32];
        let mut length = buffer.len();
        // SAFETY: valid name, buffer and length pointers.
        let status = unsafe {
            libc::sysctlbyname(
                c"kern.osproductversion".as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            return 0;
        }
        String::from_utf8_lossy(&buffer[..length.saturating_sub(1)])
            .split('.')
            .next()
            .and_then(|major| major.parse().ok())
            .unwrap_or(0)
    })
}

// ---------------------------------------------------------------------------
// Windows on other Spaces.

type CreateWithToken = unsafe extern "C" fn(CfRef) -> *mut AXUIElement;

/// Accessibility elements for windows of `pid` whose window ids are in
/// `wanted`, found by building elements from remote tokens.
///
/// AXWindows lists only the windows on the current Space (macOS 27.2), so a
/// window on another Desktop, or behind a full-screen app, is invisible to
/// it. An element can be made from a 20-byte token (pid, 0, "coco", element
/// id); window elements have small ids, so ids are tried in order until every
/// wanted window is found. The technique and token layout come from
/// alt-tab-macos (GPL, technique only). Measured here: a full scan of 1,000
/// ids takes 34 to 88 ms, and windows were found at ids 42 and 61.
pub(crate) fn windows_by_token(pid: i32, wanted: &[u32]) -> Vec<(u32, Element)> {
    static FUNCTION: OnceLock<Option<CreateWithToken>> = OnceLock::new();
    // SAFETY: the symbol takes a CFData and returns a +1 AXUIElement or null.
    let Some(create) = *FUNCTION.get_or_init(|| unsafe {
        function(APPLICATION_SERVICES, c"_AXUIElementCreateWithRemoteToken")
    }) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for element_id in 0u64..1000 {
        if found.len() == wanted.len() {
            break;
        }
        let mut token = [0u8; 20];
        token[0..4].copy_from_slice(&pid.to_le_bytes());
        token[8..12].copy_from_slice(&0x636f_636f_u32.to_le_bytes());
        token[12..20].copy_from_slice(&element_id.to_le_bytes());
        // SAFETY: CFDataCreate copies the bytes; the data is released below.
        let element = unsafe {
            let data = CFDataCreate(std::ptr::null_mut(), token.as_ptr(), token.len() as isize);
            let raw = create(data);
            CFRelease(data);
            match std::ptr::NonNull::new(raw) {
                Some(raw) => Element::from_raw(objc2_core_foundation::CFRetained::from_raw(raw)),
                None => continue,
            }
        };
        if element.string("AXRole").as_deref() != Some("AXWindow") {
            continue;
        }
        if let Some(id) = window_id(&element)
            && wanted.contains(&id)
            && !found.iter().any(|(known, _)| *known == id)
        {
            found.push((id, element));
        }
    }
    found
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(allocator: CfRef, bytes: *const u8, length: isize) -> CfRef;
}
