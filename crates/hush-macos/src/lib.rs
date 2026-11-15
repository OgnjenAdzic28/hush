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
mod keys;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(target_os = "macos")]
mod platform;
#[cfg(target_os = "macos")]
mod pointer;
#[cfg(target_os = "macos")]
mod private;
#[cfg(target_os = "macos")]
mod read;
#[cfg(target_os = "macos")]
mod roles;
#[cfg(target_os = "macos")]
mod script;
#[cfg(target_os = "macos")]
mod server;
#[cfg(target_os = "macos")]
mod watch;
#[cfg(target_os = "macos")]
mod windows;

#[cfg(target_os = "macos")]
pub use ax::Element;
#[cfg(target_os = "macos")]
pub use platform::MacPlatform;
