//! The per-user folder for Hush's socket and lane state.
//!
//! On macOS the temporary folder is already per user. On Linux `/tmp` is
//! shared by everyone, so a file there could be planted or read by another
//! user: a socket they answer, or a lane display number pointing at their
//! X server. `XDG_RUNTIME_DIR` is the user's own (systemd makes it 0700);
//! without it Hush makes `/tmp/hush-UID` itself and checks it is still the
//! user's, private, and not a link.

use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;

/// The user's id, from the owner of this process's /proc entry.
pub fn uid() -> io::Result<u32> {
    std::fs::metadata("/proc/self").map(|m| m.uid())
}

pub fn dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let uid = uid()?;
    let dir = std::env::temp_dir().join(format!("hush-{uid}"));
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let meta = std::fs::symlink_metadata(&dir)?;
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other(format!(
            "{} isn't a private folder of yours; remove it, or set XDG_RUNTIME_DIR",
            dir.display()
        )));
    }
    Ok(dir)
}
