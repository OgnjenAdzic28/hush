//! The fixture app (bench/fixture) and helpers shared by the live tests.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

use hush_core::Platform;
use hush_engine::Engine;
use hush_macos::MacPlatform;

pub struct Fixture {
    pub binary: PathBuf,
    pub log: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill")
            .arg("-f")
            .arg(&self.binary)
            .status();
    }
}

pub fn launch(visible: bool) -> Fixture {
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
        .args(["-g", "-n"])
        .args(if visible {
            &["--env", "FIXTURE_VISIBLE=1"][..]
        } else {
            &[][..]
        })
        .arg("-a")
        .arg(dir.join("HushFixture.app"))
        .arg("--args")
        .arg(&log)
        .status()
        .expect("open runs");
    assert!(opened.success(), "the fixture opens");
    Fixture { binary, log }
}

/// The ref on the first outline line containing `needle`.
pub fn reference(outline: &str, needle: &str) -> String {
    outline
        .lines()
        .find(|line| line.contains(needle))
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("no line with {needle} in\n{outline}"))
        .to_owned()
}

pub fn front(engine: &Engine<MacPlatform>) -> Option<String> {
    engine
        .platform()
        .apps(None)
        .ok()?
        .into_iter()
        .find(|a| a.frontmost)
        .map(|a| a.name)
}
