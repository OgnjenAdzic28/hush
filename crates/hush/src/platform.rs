//! The platform for the OS this binary was built for.

#[cfg(not(target_os = "macos"))]
use hush_core::{Platform, ScriptLanguage};

#[cfg(target_os = "macos")]
pub(crate) fn platform() -> Result<hush_macos::MacPlatform, String> {
    hush_macos::MacPlatform::new().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform() -> Result<Unsupported, String> {
    Err("this platform isn't supported yet; Hush runs on macOS for now".into())
}

/// Stands in for the platforms Hush doesn't have yet, so the binary builds
/// everywhere the core does.
#[cfg(not(target_os = "macos"))]
pub(crate) enum Unsupported {}

#[cfg(not(target_os = "macos"))]
impl Platform for Unsupported {
    type Handle = u8;
    fn apps(&self, _: Option<&str>) -> hush_core::Result<Vec<hush_core::AppInfo>> {
        match *self {}
    }
    fn windows(&self, _: &hush_core::AppInfo) -> hush_core::Result<Vec<hush_core::WindowInfo>> {
        match *self {}
    }
    fn read(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
    ) -> hush_core::Result<hush_core::Snapshot<u8>> {
        match *self {}
    }
    fn activity(&self, _: &hush_core::AppInfo) -> hush_core::Activity {
        match *self {}
    }
    fn perform(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
        _: hush_core::Op<'_, u8>,
    ) -> hush_core::Result<()> {
        match *self {}
    }
    fn capture(
        &self,
        _: &hush_core::WindowInfo,
        _: Option<hush_core::Rect>,
        _: u32,
    ) -> hush_core::Result<hush_core::Image> {
        match *self {}
    }
    fn script(
        &self,
        _: &hush_core::AppInfo,
        _: &str,
        _: ScriptLanguage,
    ) -> hush_core::Result<String> {
        match *self {}
    }
}
