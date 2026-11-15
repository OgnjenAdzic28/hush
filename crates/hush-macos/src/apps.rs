//! Running and installed apps.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hush_core::AppInfo;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowLayer,
    kCGWindowOwnerPID,
};

use crate::cf;

/// Apps with a user interface that are running now.
///
/// NSWorkspace's list only refreshes on a main run loop, which a CLI or MCP
/// server doesn't run, so an app launched after Hush started would never
/// appear. The process list is always current; each new pid is asked once
/// whether it is a regular app, and the answer is kept until the pid ends.
pub(crate) fn running() -> Vec<AppInfo> {
    let frontmost = frontmost_pid();
    let pids = all_pids();
    let mut known = KNOWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    known.retain(|pid, _| pids.contains(pid));
    let mut apps = Vec::new();
    let now = Instant::now();
    for pid in pids {
        let entry = known.entry(pid).or_insert_with(|| Seen::ask(pid, now));
        // A process that just started may not have declared itself an app
        // yet (it does so as it finishes launching), so "not an app" is
        // only trusted after a few seconds.
        if entry.app.is_none() && entry.bundled && now.duration_since(entry.first) < SETTLING {
            *entry = Seen {
                first: entry.first,
                ..Seen::ask(pid, now)
            };
        }
        if let Some(app) = &entry.app {
            let mut app = app.clone();
            app.frontmost = frontmost == Some(pid);
            apps.push(app);
        }
    }
    apps.sort_by_key(|app| (!app.frontmost, app.name.to_lowercase()));
    apps
}

/// What each pid seen so far is: a regular app, or not one.
static KNOWN: Mutex<BTreeMap<i32, Seen>> = Mutex::new(BTreeMap::new());

/// How long after first sight a bundled process is asked again.
const SETTLING: Duration = Duration::from_secs(5);

struct Seen {
    first: Instant,
    bundled: bool,
    app: Option<AppInfo>,
}

impl Seen {
    fn ask(pid: i32, now: Instant) -> Seen {
        // Only processes inside an app bundle can be regular apps; the path
        // check is one syscall, the AppKit lookup is not.
        let bundled = in_app_bundle(pid);
        let app = bundled
            .then(|| NSRunningApplication::runningApplicationWithProcessIdentifier(pid))
            .flatten()
            .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
            .map(|app| describe(&app));
        Seen {
            first: now,
            bundled,
            app,
        }
    }
}

fn in_app_bundle(pid: i32) -> bool {
    let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the buffer is PROC_PIDPATHINFO_MAXSIZE bytes, as required.
    let length =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    // `/Contents/MacOS/`, not `.app/Contents/MacOS/`: some apps run from a
    // code-signing clone named `Name.app.bundle`.
    length > 0
        && buffer[..length as usize]
            .windows(16)
            .any(|w| w == b"/Contents/MacOS/")
}

fn all_pids() -> Vec<i32> {
    // SAFETY: a null buffer asks for the count; the second call fills at
    // most `capacity` pids and returns how many it wrote.
    unsafe {
        let count = libc::proc_listallpids(std::ptr::null_mut(), 0).max(0) as usize;
        let mut pids = vec![0i32; count + 64];
        let bytes = (pids.len() * size_of::<i32>()) as libc::c_int;
        let written = libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes).max(0) as usize;
        pids.truncate(written.min(pids.len()));
        pids.retain(|&pid| pid > 0);
        pids
    }
}

/// Installed apps whose name contains `query`, from the standard folders.
pub(crate) fn installed(query: &str) -> Vec<AppInfo> {
    let query = query.to_lowercase();
    let home = std::env::var("HOME").unwrap_or_default();
    let folders = [
        "/Applications".to_owned(),
        "/Applications/Utilities".to_owned(),
        "/System/Applications".to_owned(),
        "/System/Applications/Utilities".to_owned(),
        format!("{home}/Applications"),
    ];
    let mut apps = Vec::new();
    for folder in &folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "app")
                && let Some(name) = path.file_stem().and_then(|n| n.to_str())
                && name.to_lowercase().contains(&query)
            {
                apps.push(AppInfo {
                    name: name.to_owned(),
                    pid: None,
                    id: bundle_id(&path),
                    path: path.to_str().map(str::to_owned),
                    frontmost: false,
                });
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.path == b.path);
    apps
}

/// The app the user is in: the owner of the frontmost normal window. The
/// window server's list is always current and in front-to-back order;
/// asking Accessibility for the focused app instead fails whenever that app
/// is slow to answer (kAXErrorCannotComplete, seen with Electron apps).
pub(crate) fn frontmost_pid() -> Option<i32> {
    window_owners().first().copied()
}

fn describe(app: &NSRunningApplication) -> AppInfo {
    let pid = app.processIdentifier();
    AppInfo {
        name: app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("pid {pid}")),
        pid: u32::try_from(pid).ok(),
        id: app.bundleIdentifier().map(|id| id.to_string()),
        path: app
            .bundleURL()
            .and_then(|url| url.path())
            .map(|p| p.to_string()),
        frontmost: false,
    }
}

/// Owners of on-screen normal windows, front to back.
fn window_owners() -> Vec<i32> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(list) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else {
        return Vec::new();
    };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of dictionaries.
    let list: CFRetained<objc2_core_foundation::CFArray<objc2_core_foundation::CFDictionary>> =
        unsafe { CFRetained::cast_unchecked(list) };
    let mut pids: Vec<i32> = list
        .iter()
        .filter_map(|window| {
            // SAFETY: window info dictionaries map CFString keys to CF values.
            let window: &objc2_core_foundation::CFDictionary<
                objc2_core_foundation::CFString,
                objc2_core_foundation::CFType,
            > = unsafe { window.cast_unchecked() };
            // SAFETY: the keys are framework constants that are always set.
            let (layer_key, pid_key) = unsafe { (kCGWindowLayer, kCGWindowOwnerPID) };
            let layer = window.get(layer_key).and_then(|v| cf::number(&v))?;
            let pid = window.get(pid_key).and_then(|v| cf::number(&v))?;
            (layer == 0.0).then_some(pid as i32)
        })
        .collect();
    // Keep front-to-back order; drop repeats after the first.
    let mut seen = std::collections::HashSet::new();
    pids.retain(|pid| seen.insert(*pid));
    pids
}

fn bundle_id(path: &Path) -> Option<String> {
    // Reading Info.plist as text is enough for the identifier and avoids
    // loading the bundle. Binary plists are skipped (rare for apps).
    let plist = std::fs::read_to_string(path.join("Contents/Info.plist")).ok()?;
    let after = plist.split("<key>CFBundleIdentifier</key>").nth(1)?;
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")? + start;
    Some(after[start..end].trim().to_owned())
}
