//! Running and installed apps.

use std::collections::BTreeMap;
use std::path::Path;

use hush_core::AppInfo;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowLayer,
    kCGWindowOwnerPID,
};

use crate::ax::Element;
use crate::cf;

/// Apps with a user interface that are running now. NSWorkspace's list is
/// only refreshed by the main run loop, which a CLI or MCP server may not
/// run, so it is merged with the owners of on-screen windows, which is
/// always current.
pub(crate) fn running() -> Vec<AppInfo> {
    let frontmost = frontmost_pid();
    let mut by_pid: BTreeMap<i32, AppInfo> = BTreeMap::new();
    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        if app.activationPolicy() == NSApplicationActivationPolicy::Regular && !app.isTerminated() {
            let info = describe(&app, frontmost);
            by_pid.insert(app.processIdentifier(), info);
        }
    }
    for pid in window_owners() {
        if let std::collections::btree_map::Entry::Vacant(slot) = by_pid.entry(pid)
            && let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            && app.activationPolicy() == NSApplicationActivationPolicy::Regular
        {
            slot.insert(describe(&app, frontmost));
        }
    }
    let mut apps: Vec<AppInfo> = by_pid.into_values().collect();
    apps.sort_by_key(|app| (!app.frontmost, app.name.to_lowercase()));
    apps
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

/// The app the user is in, from Accessibility (always current).
pub(crate) fn frontmost_pid() -> Option<i32> {
    Element::system_wide()
        .element("AXFocusedApplication")
        .and_then(|app| app.pid())
}

fn describe(app: &NSRunningApplication, frontmost: Option<i32>) -> AppInfo {
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
        frontmost: frontmost == Some(pid),
    }
}

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
    pids.sort_unstable();
    pids.dedup();
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
