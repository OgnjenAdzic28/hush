//! Drives the fixture app (bench/fixture) through every act route and
//! checks, from the fixture's own log, that each step landed exactly where
//! it should and that the fixture was never activated, and from the window
//! server that the user's front app never changed.
//!
//! It opens two small windows behind everything, so it needs the owner's
//! go-ahead. It runs on a normal Desktop and in a full-screen app; in the
//! second case the windows are on another Space, which `AXWindows` doesn't
//! list, so it also tests Hush's off-Space routes. When a tool that hides
//! apps an agent launches (Doorman's lane) hides the fixture, it shows
//! itself again without activating, so the pointer routes are tested too.
//!
//! cargo test -p hush --release --test live_act -- --ignored --nocapture

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use hush_core::Platform;
use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

struct Fixture {
    binary: PathBuf,
    log: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill")
            .arg("-f")
            .arg(&self.binary)
            .status();
    }
}

fn launch() -> Fixture {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/fixture/HushFixture.swift");
    // A real bundle, so it is found the way any app is.
    let contents = dir.join("HushFixture.app/Contents");
    std::fs::create_dir_all(contents.join("MacOS")).expect("bundle folders");
    std::fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.hush.fixture</string>
<key>CFBundleName</key><string>HushFixture</string>
<key>CFBundleExecutable</key><string>HushFixture</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#,
    )
    .expect("Info.plist");
    let binary = contents.join("MacOS/HushFixture");
    let built = Command::new("swiftc")
        .args(["-O", "-o"])
        .arg(&binary)
        .arg(&source)
        .status()
        .expect("swiftc runs");
    assert!(built.success(), "the fixture builds");
    let log = dir.join("fixture.log");
    let _ = std::fs::remove_file(&log);
    // Through LaunchServices in the background, the way agents open apps. A
    // child process started directly may activate itself at launch.
    let opened = Command::new("/usr/bin/open")
        .args(["-g", "-n", "--env", "FIXTURE_VISIBLE=1", "-a"])
        .arg(dir.join("HushFixture.app"))
        .arg("--args")
        .arg(&log)
        .status()
        .expect("open runs");
    assert!(opened.success(), "the fixture opens");
    Fixture { binary, log }
}

/// The ref on the first outline line containing `needle`.
fn reference(outline: &str, needle: &str) -> String {
    outline
        .lines()
        .find(|line| line.contains(needle))
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("no line with {needle} in\n{outline}"))
        .to_owned()
}

fn front(engine: &Engine<MacPlatform>) -> Option<String> {
    engine
        .platform()
        .apps(None)
        .ok()?
        .into_iter()
        .find(|a| a.frontmost)
        .map(|a| a.name)
}

#[test]
#[ignore = "opens a fixture window and acts on it; needs the owner's go-ahead"]
fn every_route_lands_without_disturbing_the_user() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let user_app = front(&engine);
    let fixture = launch();

    let deadline = Instant::now() + Duration::from_secs(5);
    let outline = loop {
        match engine.observe("HushFixture", ObserveOptions::default()) {
            Ok(outline) => break outline,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                panic!("the fixture window never became visible to Accessibility ({error})")
            }
        }
    };
    println!("{outline}");
    let height: f64 = outline
        .lines()
        .next()
        .and_then(|header| header.rsplit(' ').find_map(|word| word.split_once('x')))
        .and_then(|(_, h)| h.parse().ok())
        .unwrap_or(452.0);
    let canvas = format!("240,{}", height - 110.0);

    // Opening a window unhides an app, and a lane tool may hide it again;
    // only what happens once acting starts counts.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !std::fs::read_to_string(&fixture.log).is_ok_and(|log| log.contains("later window")) {
        assert!(
            Instant::now() < deadline,
            "the fixture never opened its later window"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300));
    let before_acting = std::fs::read_to_string(&fixture.log).map_or(0, |log| log.len());

    let steps: Vec<String> = [
        format!("click {}", reference(&outline, "button \"Increment\"")),
        format!("click {}", reference(&outline, "button \"Increment\"")),
        format!("type {} \"hello\"", reference(&outline, "field")),
        format!("click {}", reference(&outline, "checkbox \"Agree\"")),
        format!("pick {} Menlo", reference(&outline, "popup")),
        "key cmd+k".to_owned(),
        "menu Test > Reset".to_owned(),
    ]
    .into();
    let started = Instant::now();
    let report = engine.act("HushFixture", None, &steps).expect("act runs");
    println!("{report}\n(act took {:?})", started.elapsed());

    let pointer = [format!("click {canvas}"), format!("scroll {canvas} down 2")];
    let pointer_report = engine.act("HushFixture", None, &pointer).expect("act runs");
    println!("{pointer_report}");

    // The window the fixture opens a second after launch.
    let deadline = Instant::now() + Duration::from_secs(5);
    let later = loop {
        let options = ObserveOptions {
            window: Some("Later"),
            ..ObserveOptions::default()
        };
        match engine.observe("HushFixture", options) {
            Ok(outline) if outline.contains("Later button") => break outline,
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            other => panic!("the later window never became reachable: {other:?}"),
        }
    };
    let press = [format!("click {}", reference(&later, "Later button"))];
    engine
        .act("HushFixture", Some("Later"), &press)
        .expect("act runs");

    std::thread::sleep(Duration::from_millis(200));
    let log = std::fs::read_to_string(&fixture.log).expect("fixture log");
    println!("fixture log:\n{log}");
    for expected in [
        "increment 2",
        "field hello",
        "checkbox on",
        "picked Menlo",
        "menu mark",
        "menu reset",
        "later pressed",
    ] {
        assert!(
            log.contains(expected),
            "missing \"{expected}\" in the fixture log"
        );
    }
    for expected in ["canvas click 1", "canvas scroll"] {
        assert!(
            log.contains(expected),
            "missing \"{expected}\" in the fixture log"
        );
    }
    assert!(
        !log.contains("canvas click 2"),
        "a pointer click landed twice"
    );
    assert!(!log.contains("increment 3"), "a click landed twice");
    assert!(!log.contains("ACTIVATED"), "the fixture was activated");
    // The fixture shows itself again after each hide; any other unhide
    // while acting would be Hush's.
    let acting = &log[before_acting..];
    assert!(
        acting.matches("UNHIDDEN").count() <= acting.matches("hidden\n").count(),
        "the fixture was unhidden while Hush acted"
    );
    assert_eq!(front(&engine), user_app, "the user's front app changed");
}
