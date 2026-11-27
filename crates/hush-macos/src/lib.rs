//! The macOS platform: reads windows through Accessibility and acts on apps
//! without taking focus from the user. Compiles to nothing on other systems.

#[cfg(target_os = "macos")]
mod act;
#[cfg(target_os = "macos")]
mod apps;
#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod cf;
#[cfg(target_os = "macos")]
mod cursor_art;
#[cfg(target_os = "macos")]
mod keys;
#[cfg(target_os = "macos")]
mod lane;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(target_os = "macos")]
mod permissions;
#[cfg(target_os = "macos")]
mod platform;
#[cfg(target_os = "macos")]
mod play;
#[cfg(target_os = "macos")]
mod pointer;
#[cfg(target_os = "macos")]
mod private;
#[cfg(target_os = "macos")]
mod read;
#[cfg(target_os = "macos")]
mod record;
#[cfg(target_os = "macos")]
mod roles;
#[cfg(target_os = "macos")]
mod script;
#[cfg(target_os = "macos")]
mod seen;
#[cfg(target_os = "macos")]
mod server;
#[cfg(target_os = "macos")]
mod vision;
#[cfg(target_os = "macos")]
mod watch;
#[cfg(target_os = "macos")]
mod windows;
#[cfg(target_os = "macos")]
mod workspace;

#[cfg(target_os = "macos")]
pub use ax::Element;
#[cfg(target_os = "macos")]
pub use permissions::{Permissions, host_app, permissions, sandboxed};
#[cfg(target_os = "macos")]
pub use platform::MacPlatform;
#[cfg(target_os = "macos")]
pub use seen::Handle;

/// Sends the windows this process moved to the lane back. For a process
/// that is about to exit without dropping its platform (a signal).
#[cfg(target_os = "macos")]
pub fn release_lane() {
    lane::release();
}

/// Gives the calling (main) thread to its run loop for good. A process
/// serving sessions on other threads calls this, so work that must happen on
/// the main thread (AppleScript) runs there, and NSWorkspace, which posts on
/// the main run loop only, reports app launches and switches (see
/// `workspace`). The main run loop also drains the main dispatch queue.
#[cfg(target_os = "macos")]
pub fn serve_main_thread() -> ! {
    loop {
        // SAFETY: the default mode constant is always set.
        let mode = unsafe { objc2_core_foundation::kCFRunLoopDefaultMode };
        // The main queue keeps the main run loop from finishing when idle.
        objc2_core_foundation::CFRunLoop::run_in_mode(mode, 1e10, false);
    }
}
