//! Whether the app running Hush may use Accessibility and Screen Recording.
//! macOS grants both to the app that started Hush (a terminal, an agent's
//! desktop app), never to the `hush` binary itself.

use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString, CFType};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

// libsystem_sandbox, part of libSystem.
unsafe extern "C" {
    fn sandbox_check(
        pid: libc::pid_t,
        operation: *const libc::c_char,
        kind: libc::c_int,
    ) -> libc::c_int;
}

/// True when this process runs in a sandbox (a coding agent's, say). A
/// sandbox hides Accessibility and Screen Recording even when they are on.
pub fn sandboxed() -> bool {
    // SAFETY: a null operation with SANDBOX_FILTER_NONE (0) only asks
    // whether the process is sandboxed at all.
    unsafe { sandbox_check(libc::getpid(), std::ptr::null(), 0) != 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub accessibility: bool,
    pub screen_recording: bool,
}

/// Reads both permissions. With `ask`, each missing one shows the system
/// prompt that adds the running app to its list in System Settings (macOS
/// shows each prompt only once per app).
pub fn permissions(ask: bool) -> Permissions {
    // SAFETY: kAXTrustedCheckOptionPrompt is a constant CFString.
    let key: &CFString = unsafe { kAXTrustedCheckOptionPrompt };
    let options =
        CFDictionary::<CFString, CFType>::from_slices(&[key], &[CFBoolean::new(ask).as_ref()]);
    // SAFETY: `options` is a valid dictionary with the documented key.
    let accessibility = unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
    // SAFETY: neither function has preconditions.
    let screen_recording = unsafe {
        if ask {
            CGRequestScreenCaptureAccess()
        } else {
            CGPreflightScreenCaptureAccess()
        }
    };
    Permissions {
        accessibility,
        screen_recording,
    }
}

/// The name of the app this process was started from (Terminal, Ghostty, an
/// agent's desktop app), which is the one the permissions belong to. macOS
/// passes its bundle id down to every process it starts.
pub fn host_app() -> Option<String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let id = std::env::var("__CFBundleIdentifier").ok()?;
    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(&id))?;
    let name = url.lastPathComponent()?.to_string();
    Some(name.trim_end_matches(".app").to_owned())
}
