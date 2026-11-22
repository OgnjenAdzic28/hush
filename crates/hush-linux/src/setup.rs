//! What `hush setup` checks on Linux.

use std::path::Path;

/// Whether this machine is ready for Hush, part by part.
#[derive(Debug)]
pub struct Readiness {
    /// The accessibility bus answers. Connecting also switches
    /// accessibility on for apps started from now on.
    pub accessibility: Result<(), String>,
    /// Xvfb, for Hush's own display: starting apps, `see`, keys, pointer
    /// input and `record`.
    pub xvfb: bool,
    /// ffmpeg, for `record`.
    pub ffmpeg: bool,
}

pub fn readiness() -> Readiness {
    Readiness {
        accessibility: zbus::block_on(crate::bus::Bus::connect()).map(|_| ()),
        xvfb: on_path("Xvfb"),
        ffmpeg: on_path("ffmpeg"),
    }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| executable(&dir.join(program))))
}

fn executable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    file.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
