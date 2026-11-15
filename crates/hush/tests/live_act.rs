//! Drives the fixture app (bench/fixture) through every act route and
//! checks, from the fixture's own log, that each step landed exactly where
//! it should and that the fixture was never activated, and from the window
//! server that the user's front app never changed.
//!
//! It opens a small window behind everything, so it needs the owner's
//! go-ahead and a Mac on a normal Desktop: windows on an inactive Space (for
//! example while the user is in a full-screen app) aren't visible to
//! Accessibility until they have been on the active Space once.
//!
//! cargo test -p hush --release --test live_act -- --ignored --nocapture

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use hush_core::Platform;
use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

struct Fixture {
    child: Child,
    log: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
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
    let child = Command::new(&binary)
        .arg(&log)
        .spawn()
        .expect("the fixture starts");
    Fixture { child, log }
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
            Err(error) => panic!(
                "the fixture window never became visible to Accessibility ({error}); is the Mac on a normal Desktop \
                 rather than a full-screen Space?"
            ),
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

    let steps: Vec<String> = [
        format!("click {}", reference(&outline, "button \"Increment\"")),
        format!("click {}", reference(&outline, "button \"Increment\"")),
        format!("type {} \"hello\"", reference(&outline, "field")),
        format!("click {}", reference(&outline, "checkbox \"Agree\"")),
        format!("pick {} Menlo", reference(&outline, "popup")),
        "key cmd+k".to_owned(),
        "menu Test > Reset".to_owned(),
        format!("click {canvas}"),
        format!("scroll {canvas} down 2"),
    ]
    .into();
    let started = Instant::now();
    let report = engine.act("HushFixture", None, &steps).expect("act runs");
    println!("{report}\n(act took {:?})", started.elapsed());

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
        "canvas click 1",
        "canvas scroll",
    ] {
        assert!(
            log.contains(expected),
            "missing \"{expected}\" in the fixture log"
        );
    }
    assert!(!log.contains("increment 3"), "a click landed twice");
    assert!(!log.contains("ACTIVATED"), "the fixture was activated");
    assert_eq!(front(&engine), user_app, "the user's front app changed");
}
