//! Installed apps, from the desktop entries in the XDG data directories.

use std::path::{Path, PathBuf};

use hush_core::AppInfo;

/// Installed apps whose name or desktop id contains `query`.
pub(crate) fn matching(query: &str) -> Vec<AppInfo> {
    let wanted = query.to_lowercase();
    let mut found: Vec<AppInfo> = Vec::new();
    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "desktop") {
                continue;
            }
            let Some(app) = desktop_entry(&path) else {
                continue;
            };
            let matches = app.name.to_lowercase().contains(&wanted)
                || app.id.as_deref().is_some_and(|id| id.contains(&wanted));
            // The first directory wins, as the XDG spec says.
            if matches && !found.iter().any(|a| a.id == app.id) {
                found.push(app);
            }
        }
    }
    found.sort_by_key(|app| app.name.to_lowercase());
    found
}

fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")));
    let shared = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(shared.split(':').map(PathBuf::from))
        .map(|dir| dir.join("applications"))
        .collect()
}

fn desktop_entry(path: &Path) -> Option<AppInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = parse(&text)?;
    Some(AppInfo {
        name: fields.name,
        pid: None,
        id: path.file_stem().map(|s| s.to_string_lossy().into_owned()),
        path: fields.exec,
        frontmost: false,
    })
}

#[derive(Debug, PartialEq)]
struct Fields {
    name: String,
    exec: Option<String>,
}

/// The `[Desktop Entry]` group of a desktop file, if it is an app a person
/// would launch.
fn parse(text: &str) -> Option<Fields> {
    let mut in_entry = false;
    let (mut name, mut exec, mut app, mut hidden) = (None, None, false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = Some(value.trim().to_owned()),
            "Exec" => exec = value.split_whitespace().next().map(str::to_owned),
            "Type" => app = value.trim() == "Application",
            "NoDisplay" | "Hidden" => hidden |= value.trim() == "true",
            _ => {}
        }
    }
    (app && !hidden).then_some(Fields { name: name?, exec })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_desktop_entry_group_only() {
        let text = "[Desktop Entry]\nType=Application\nName=Calculator\nExec=gnome-calculator %U\n\n[Desktop Action new]\nName=New Window\n";
        assert_eq!(
            parse(text),
            Some(Fields {
                name: "Calculator".into(),
                exec: Some("gnome-calculator".into()),
            })
        );
    }

    #[test]
    fn skips_hidden_and_non_app_entries() {
        assert_eq!(
            parse("[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n"),
            None
        );
        assert_eq!(parse("[Desktop Entry]\nType=Link\nName=X\n"), None);
    }
}
