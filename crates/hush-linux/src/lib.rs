//! The Linux platform: reads windows through AT-SPI, the accessibility
//! interface GTK, Qt, Chromium, Firefox and LibreOffice share on X11 and
//! Wayland alike. Compiles to nothing on other systems.

#[cfg(target_os = "linux")]
mod act;
#[cfg(target_os = "linux")]
mod apps;
#[cfg(target_os = "linux")]
mod bus;
#[cfg(target_os = "linux")]
mod installed;
#[cfg(target_os = "linux")]
mod lane;
#[cfg(target_os = "linux")]
mod platform;
#[cfg(target_os = "linux")]
mod read;
#[cfg(any(target_os = "linux", test))]
mod roles;
#[cfg(target_os = "linux")]
mod screen;
#[cfg(target_os = "linux")]
mod watch;

#[cfg(target_os = "linux")]
pub use bus::Element;
#[cfg(target_os = "linux")]
pub use platform::LinuxPlatform;
