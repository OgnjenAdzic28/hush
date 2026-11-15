//! Reads the main window of every running app and reports time and size.
//! Read-only: nothing is clicked, typed or focused.
//!
//! cargo test -p hush-macos --release --test live_read -- --ignored --nocapture

#![cfg(target_os = "macos")]

use std::time::Instant;

use hush_core::{OutlineOptions, Platform, RefTable, View, approx_tokens};
use hush_macos::MacPlatform;

#[test]
#[ignore = "reads real apps; needs Accessibility permission"]
fn reads_every_running_app_quickly_and_compactly() {
    let platform = MacPlatform::new().expect("Accessibility permission");
    let apps = platform.apps(None).expect("apps");
    assert!(!apps.is_empty(), "no running apps found");

    println!(
        "{:<28} {:>6} {:>8} {:>8} {:>7} {:>7}",
        "app", "nodes", "read ms", "view ms", "lines", "tokens"
    );
    let mut read_times = Vec::new();
    for app in &apps {
        let Ok(windows) = platform.windows(app) else {
            continue;
        };
        let Some(window) = windows.into_iter().find(|w| !w.minimized) else {
            continue;
        };

        let start = Instant::now();
        let Ok(snapshot) = platform.read(app, &window) else {
            continue;
        };
        let read = start.elapsed();

        let start = Instant::now();
        let mut refs = RefTable::default();
        let (view, _) = View::build(&snapshot, &mut refs, &OutlineOptions::default());
        let text = view.render();
        let built = start.elapsed();

        // A second read of an unchanged window keeps every ref.
        let again = platform.read(app, &window).expect("second read");
        let (second, _) = View::build(&again, &mut refs, &OutlineOptions::default());
        let unchanged = second
            .lines
            .iter()
            .filter(|l| view.lines.iter().any(|o| o.r == l.r))
            .count();

        println!(
            "{:<28} {:>6} {:>8.1} {:>8.2} {:>7} {:>7}   refs kept {}/{}",
            truncate(&app.name, 28),
            snapshot.nodes.len(),
            read.as_secs_f64() * 1000.0,
            built.as_secs_f64() * 1000.0,
            view.lines.len(),
            approx_tokens(&text),
            unchanged,
            second.lines.len(),
        );
        read_times.push(read.as_secs_f64() * 1000.0);
        if std::env::var_os("HUSH_PRINT_OUTLINE").is_some() {
            println!("{text}");
        }
    }
    read_times.sort_by(f64::total_cmp);
    if let Some(median) = read_times.get(read_times.len() / 2) {
        println!(
            "median read {median:.1} ms over {} windows",
            read_times.len()
        );
    }
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
