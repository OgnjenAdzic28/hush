//! Scripts that compute something and touch no app.

#![cfg(target_os = "macos")]

use std::time::Instant;

use hush_core::{AppInfo, Platform, ScriptLanguage};
use hush_macos::MacPlatform;

fn none() -> AppInfo {
    AppInfo {
        name: "none".into(),
        pid: None,
        id: None,
        path: None,
        frontmost: false,
    }
}

#[test]
#[ignore = "needs Accessibility permission to create the platform"]
fn applescript_runs_in_process_and_repeats_without_recompiling() {
    let platform = MacPlatform::new().expect("Accessibility permission");
    let source = "return \"sum \" & (2 + 3)";
    let cold = Instant::now();
    assert_eq!(
        platform
            .script(&none(), source, ScriptLanguage::AppleScript)
            .unwrap(),
        "sum 5"
    );
    let cold = cold.elapsed();
    let warm = Instant::now();
    for _ in 0..20 {
        platform
            .script(&none(), source, ScriptLanguage::AppleScript)
            .unwrap();
    }
    let warm = warm.elapsed() / 20;
    println!("AppleScript cold {cold:?}, warm {warm:?} per run");
    assert!(warm.as_millis() < 5, "warm run took {warm:?}");

    let broken = platform
        .script(&none(), "return (", ScriptLanguage::AppleScript)
        .unwrap_err();
    assert!(broken.to_string().contains("doesn't compile"), "{broken}");
    // A runtime error is an error result, not a crash.
    let failed = platform
        .script(&none(), "error \"boom\"", ScriptLanguage::AppleScript)
        .unwrap_err();
    assert!(failed.to_string().contains("boom"), "{failed}");
    assert_eq!(
        platform
            .script(&none(), "1 + 1", ScriptLanguage::JavaScript)
            .unwrap(),
        "2"
    );
}
