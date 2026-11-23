//! Installed apps, from the desktop entries in the XDG data directories.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use hush_core::AppInfo;

/// How long a scan of the desktop entries is reused. Listing apps happens
/// several times a step while an app starts; installs are rare.
const FRESH: Duration = Duration::from_secs(30);

static SCANNED: Mutex<Option<(Instant, Vec<AppInfo>)>> = Mutex::new(None);

/// Installed apps whose name or desktop id contains `query`.
pub(crate) fn matching(query: &str) -> Vec<AppInfo> {
    let wanted = query.to_lowercase();
    all()
        .into_iter()
        .filter(|app| {
            app.name.to_lowercase().contains(&wanted)
                || app.id.as_deref().is_some_and(|id| id.contains(&wanted))
        })
        .collect()
}

/// The installed app whose command runs the process, so a running
/// `/usr/bin/gnome-calculator` is "Calculator", as its menu entry is.
/// Launchers that are scripts are followed to the program they start:
/// `libreoffice --writer` runs `program/soffice` then `soffice.bin
/// --writer`. Among entries for one program, the one whose arguments the
/// process has wins, so that process is "LibreOffice Writer", not Calc.
pub(crate) fn by_process(executable: &str, args: &[String]) -> Option<AppInfo> {
    best(all(), args, |program| runs(program, Path::new(executable)))
}

fn best(entries: Vec<AppInfo>, args: &[String], runs: impl Fn(&str) -> bool) -> Option<AppInfo> {
    entries
        .into_iter()
        .filter_map(|app| {
            let mut words = app.path.as_deref()?.split_whitespace();
            if !runs(words.next()?) {
                return None;
            }
            let wanted: Vec<&str> = words.collect();
            let has_args = wanted.iter().all(|w| args.iter().any(|a| a == w));
            Some(((has_args, if has_args { wanted.len() } else { 0 }), app))
        })
        // The first of equally good entries, in scan order.
        .rev()
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, app)| app)
}

/// Whether running `program` from an Exec line ends up as `exe`.
fn runs(program: &str, exe: &Path) -> bool {
    if Path::new(program).file_name() == exe.file_name() {
        return true;
    }
    let Some(found) = resolved(program) else {
        return false;
    };
    found == exe || starts_beside(&found, exe)
}

/// Whether the program an Exec line runs is built on Chromium (Chrome,
/// Edge, Brave, and Electron apps such as VS Code), by the helpers Chromium
/// ships beside its binary.
pub(crate) fn chromium_based(exec: &str) -> bool {
    let Some(found) = exec.split_whitespace().next().and_then(resolved) else {
        return false;
    };
    found.parent().is_some_and(|folder| {
        ["chrome-sandbox", "chrome_crashpad_handler"]
            .iter()
            .any(|helper| folder.join(helper).exists())
    })
}

/// Whether `launcher` is a script that starts `exe` from its own folder, as
/// `program/soffice` starts `soffice.bin` and `google-chrome` starts
/// `chrome`. Not in a shared bin folder, where a script's neighbour is any
/// other program (`/usr/bin/python3`).
fn starts_beside(launcher: &Path, exe: &Path) -> bool {
    let Some(folder) = launcher.parent() else {
        return false;
    };
    exe.parent() == Some(folder)
        && !folder.ends_with("bin")
        && std::fs::read(launcher).is_ok_and(|head| head.starts_with(b"#!"))
}

/// Where `program` is on disk, links followed. Remembered, since every
/// listing of running apps asks this for each entry.
fn resolved(program: &str) -> Option<PathBuf> {
    static RESOLVED: Mutex<Option<HashMap<String, Option<PathBuf>>>> = Mutex::new(None);
    let mut cache = RESOLVED.lock().unwrap_or_else(PoisonError::into_inner);
    cache
        .get_or_insert_with(HashMap::new)
        .entry(program.to_owned())
        .or_insert_with(|| {
            let candidates: Vec<PathBuf> = if program.contains('/') {
                vec![PathBuf::from(program)]
            } else {
                std::env::var_os("PATH")
                    .map(|path| {
                        std::env::split_paths(&path)
                            .map(|dir| dir.join(program))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            candidates
                .iter()
                .find_map(|c| std::fs::canonicalize(c).ok())
        })
        .clone()
}

fn all() -> Vec<AppInfo> {
    let mut scanned = SCANNED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((at, apps)) = &*scanned
        && at.elapsed() < FRESH
    {
        return apps.clone();
    }
    let apps = scan();
    *scanned = Some((Instant::now(), apps.clone()));
    apps
}

fn scan() -> Vec<AppInfo> {
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
            // The first directory wins, as the XDG spec says.
            if !found.iter().any(|a| a.id == app.id) {
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
            "Exec" => exec = Some(without_field_codes(value)),
            "Type" => app = value.trim() == "Application",
            "NoDisplay" | "Hidden" => hidden |= value.trim() == "true",
            _ => {}
        }
    }
    (app && !hidden).then_some(Fields { name: name?, exec })
}

/// An Exec line without the `%f`, `%U` and similar codes a launcher fills
/// in with files: Hush starts apps without any. `%%` stays a percent sign.
fn without_field_codes(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|word| !(word.len() == 2 && word.starts_with('%') && word != &"%%"))
        .map(|word| word.replace("%%", "%"))
        .collect::<Vec<_>>()
        .join(" ")
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
    fn keeps_arguments_but_not_field_codes() {
        assert_eq!(
            without_field_codes("flatpak run org.gnome.Calculator %U --x=50%%"),
            "flatpak run org.gnome.Calculator --x=50%"
        );
    }

    fn entry(name: &str, exec: &str) -> AppInfo {
        AppInfo {
            name: name.into(),
            pid: None,
            id: None,
            path: Some(exec.into()),
            frontmost: false,
        }
    }

    #[test]
    fn a_process_is_the_entry_whose_arguments_it_has() {
        let entries = || {
            vec![
                entry("LibreOffice", "libreoffice"),
                entry("LibreOffice Calc", "libreoffice --calc"),
                entry("LibreOffice Writer", "libreoffice --writer"),
                entry("Calculator", "gnome-calculator"),
            ]
        };
        let soffice = |program: &str| program == "libreoffice";
        let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        let named = |a: &[&str]| best(entries(), &args(a), soffice).map(|app| app.name);
        assert_eq!(
            named(&["--writer", "--splash-pipe=5"]).as_deref(),
            Some("LibreOffice Writer")
        );
        assert_eq!(named(&["--calc"]).as_deref(), Some("LibreOffice Calc"));
        assert_eq!(named(&[]).as_deref(), Some("LibreOffice"));
        assert_eq!(best(entries(), &[], |_| false), None);
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
