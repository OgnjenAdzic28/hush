//! Every undocumented macOS API Hush uses, in one place so an OS update has
//! one file to review. Symbols are looked up at runtime; when one is missing
//! the feature that needs it fails with a clear error instead of crashing.
//!
//! Sources for the signatures: open-computer-use (MIT) `SkyLightSPI.swift`,
//! verified on macOS 27.0, and Cua Driver (MIT) `skylight.rs`.

use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

use objc2_application_services::{AXError, AXUIElement};

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
