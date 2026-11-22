//! Reads real apps through AT-SPI. Ignored by default: they need a desktop
//! session with an accessibility bus and the apps installed. The Linux CI
//! job runs them headlessly (Xvfb plus a private D-Bus session):
//!
//! ```sh
//! xvfb-run -a dbus-run-session -- cargo test -p hush-linux -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "linux")]

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use hush_core::{AppInfo, OutlineOptions, Platform, RefTable, Role, View, approx_tokens};
use hush_linux::LinuxPlatform;

/// Kills the app when the test ends, pass or fail.
struct Launched(Child);

impl Drop for Launched {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `command` and waits until it shows a window on the bus.
fn launch(platform: &LinuxPlatform, command: &str) -> (Launched, AppInfo) {
    // Connecting turns accessibility on. Qt only exposes its tree when it
    // is on as the app starts, so connect first.
    platform.apps(None).unwrap();
    let child = Launched(
        Command::new(command)
            .spawn()
            .unwrap_or_else(|e| panic!("can't start {command}: {e}")),
    );
    let pid = child.0.id();
    // Waits on an observable condition: the app's window appearing on the
    // accessibility bus. A cold GTK start under Xvfb takes 1 to 3 s in CI.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let apps = platform.apps(None).unwrap();
        if let Some(app) = apps.into_iter().find(|a| a.pid == Some(pid)) {
            return (child, app);
        }
        assert!(Instant::now() < deadline, "{command} never showed a window");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Reads the app's main window, prints its outline and timing, and returns
/// the snapshot's roles and names.
fn observe(platform: &LinuxPlatform, app: &AppInfo) -> Vec<(Role, String)> {
    let windows = platform.windows(app).unwrap();
    let window = windows.first().expect("a window");
    let started = Instant::now();
    let snapshot = platform.read(app, window).unwrap();
    let first = started.elapsed();
    let started = Instant::now();
    platform.read(app, window).unwrap();
    let repeat = started.elapsed();
    let (view, _) = View::build(
        &snapshot,
        &mut RefTable::default(),
        &OutlineOptions::default(),
    );
    let text = view.render();
    println!(
        "{}: {} elements, read in {first:?} (repeat {repeat:?}), outline ~{} tokens\n{text}",
        app.name,
        snapshot.nodes.len(),
        approx_tokens(&text),
    );
    snapshot
        .nodes
        .iter()
        .map(|n| (n.role, n.name.clone()))
        .collect()
}

#[test]
#[ignore]
fn reads_a_gtk3_window() {
    let platform = LinuxPlatform::new();
    let (_app, info) = launch(&platform, "gtk3-widget-factory");
    let nodes = observe(&platform, &info);
    assert!(nodes.len() > 30, "only {} elements", nodes.len());
    assert!(nodes.iter().any(|(r, _)| *r == Role::Button));
    assert!(nodes.iter().any(|(r, _)| r.is_text_input()));
    assert!(nodes.iter().any(|(r, _)| *r == Role::CheckBox));
}

#[test]
#[ignore]
fn reads_a_gtk4_window() {
    let platform = LinuxPlatform::new();
    let (_app, info) = launch(&platform, "gnome-calculator");
    let nodes = observe(&platform, &info);
    // The keypad's digits are buttons named by their digit.
    for digit in ["1", "7", "9"] {
        assert!(
            nodes
                .iter()
                .any(|(r, n)| *r == Role::Button && n.as_str() == digit),
            "no button {digit}"
        );
    }
}

#[test]
#[ignore]
fn reads_a_qt_window() {
    let platform = LinuxPlatform::new();
    let (_app, info) = launch(&platform, "featherpad");
    let nodes = observe(&platform, &info);
    assert!(
        nodes
            .iter()
            .any(|(r, _)| *r == Role::MenuBar || *r == Role::MenuItem)
    );
    assert!(nodes.iter().any(|(r, _)| r.is_text_input()));
}

#[test]
#[ignore]
fn finds_installed_apps_by_name() {
    let platform = LinuxPlatform::new();
    let apps = platform.apps(Some("calc")).unwrap();
    assert!(
        apps.iter()
            .any(|a| a.pid.is_none() && a.name.to_lowercase().contains("calc")),
        "{apps:?}"
    );
}
