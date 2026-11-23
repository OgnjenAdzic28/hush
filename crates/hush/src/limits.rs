//! The user's limits file, `~/.config/hush/limits` (or under
//! `$XDG_CONFIG_HOME`). It is read on every shell command and when an MCP
//! session starts, so an edit applies to the next call without a restart.

use std::path::PathBuf;

use hush_core::Limits;

pub(crate) fn path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("hush").join("limits"))
}

/// The limits, or none when there is no file. A file that can't be read or
/// has a mistake is an error, so nothing runs unlimited by accident.
pub(crate) fn load() -> Result<Limits, String> {
    let Some(path) = path() else {
        return Ok(Limits::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => Limits::parse(&text).map_err(|mistake| {
            format!(
                "{} has a mistake, so Hush won't run until it's fixed: {mistake}",
                path.display()
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Limits::default()),
        Err(error) => Err(format!("couldn't read {}: {error}", path.display())),
    }
}
