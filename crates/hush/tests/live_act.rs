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
//! cargo test -p hush --release --test live_act -- --ignored --nocapture --test-threads=1
//! (one at a time: each test launches and quits the same fixture app)

#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use hush_core::{AppInfo, Platform, Rect, WindowInfo};
use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

mod common;

use common::{front, launch, launch_as_user, reference};

#[test]
#[ignore = "moves fixture windows onto the lane and back; needs the owner's go-ahead"]
fn lane_moves_only_its_window_and_gives_it_back() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let fixture = launch_as_user();
    let (app, before) = both_windows(&engine);
    let target = before.iter().find(|w| w.title == "Hush Fixture").unwrap();
    let other = before.iter().find(|w| w.title == "Later").unwrap();
    // Parked twice, it still goes back to where it was the first time.
    for _ in 0..2 {
        let report = engine
            .act("HushFixture", Some(&target.title), &["lane".into()])
            .unwrap();
        assert!(!report.contains("stopped"), "{report}");
        let now = engine.platform().windows(&app).unwrap();
        assert!(
            off_main(frame_of(&now, target.id)),
            "the window is in the lane"
        );
        assert_eq!(
            frame_of(&now, other.id),
            other.frame,
            "the other window stays"
        );
    }
    // Acting in it doesn't count as the user switching to the app.
    let outline = engine
        .observe(
            "HushFixture",
            ObserveOptions {
                window: Some(&target.title),
                full: true,
                ..ObserveOptions::default()
            },
        )
        .unwrap();
    let steps = [
        format!("click {}", reference(&outline, "button \"Increment\"")),
        format!("type {} \"lane\"", reference(&outline, "field")),
        "key cmd+k".to_owned(),
    ];
    let report = engine
        .act("HushFixture", Some(&target.title), &steps)
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    let now = engine.platform().windows(&app).unwrap();
    assert!(
        off_main(frame_of(&now, target.id)),
        "acting kept it in the lane"
    );
    let report = engine
        .act("HushFixture", Some(&target.title), &["unlane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    assert_same_frames(&engine.platform().windows(&app).unwrap(), &before);
    engine
        .act("HushFixture", Some(&target.title), &["lane".into()])
        .unwrap();
    drop(engine);
    let platform = MacPlatform::new().unwrap();
    assert_same_frames(&platform.windows(&app).unwrap(), &before);
    assert!(
        !hidden("HushFixture"),
        "going back doesn't hide a shown app"
    );
    let log = std::fs::read_to_string(&fixture.log).unwrap_or_default();
    assert!(
        !log.contains("ACTIVATED"),
        "the fixture was activated:\n{log}"
    );
}

#[test]
#[ignore = "moves fixture windows onto the lane and back; needs the owner's go-ahead"]
fn a_hidden_app_moves_whole_and_is_hidden_again() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let fixture = launch(false);
    let (app, _) = both_windows(&engine);
    // Hiding never activates. Doorman may have hidden it already.
    if !hidden("HushFixture") {
        osascript(
            "tell application \"System Events\" to set visible of process \"HushFixture\" to false",
        );
    }
    // Doorman moves the windows of a hidden app it took in; where they are
    // once it's done is where they must go back to.
    let before = settled(&engine, &app);
    let target = before.iter().find(|w| w.title == "Hush Fixture").unwrap();
    let report = engine
        .act("HushFixture", Some(&target.title), &["lane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    assert!(!hidden("HushFixture"), "the app shows itself in the lane");
    let now = engine.platform().windows(&app).unwrap();
    for window in &before {
        assert!(
            off_main(frame_of(&now, window.id)),
            "\"{}\" would show on the user's screen",
            window.title
        );
    }
    let report = engine
        .act("HushFixture", Some(&target.title), &["unlane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    assert!(hidden("HushFixture"), "the app is hidden again");
    assert_same_frames(&engine.platform().windows(&app).unwrap(), &before);
    // The same on exit.
    engine
        .act("HushFixture", Some(&target.title), &["lane".into()])
        .unwrap();
    drop(engine);
    assert!(hidden("HushFixture"), "the app is hidden again on exit");
    let platform = MacPlatform::new().unwrap();
    assert_same_frames(&platform.windows(&app).unwrap(), &before);
    let log = std::fs::read_to_string(&fixture.log).unwrap_or_default();
    assert!(
        !log.contains("ACTIVATED"),
        "the fixture was activated:\n{log}"
    );
}

/// The fixture, once both of its windows are open.
fn both_windows(engine: &Engine<MacPlatform>) -> (AppInfo, Vec<WindowInfo>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let app = engine
            .platform()
            .apps(None)
            .unwrap()
            .into_iter()
            .find(|app| app.name == "HushFixture");
        if let Some(app) = app {
            let windows = engine.platform().windows(&app).unwrap();
            if windows.len() == 2 {
                return (app, windows);
            }
        }
        assert!(Instant::now() < deadline, "both fixture windows must open");
        // Polls the fixture starting up; it opens in about half a second.
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The app's windows once they stopped moving.
fn settled(engine: &Engine<MacPlatform>, app: &AppInfo) -> Vec<WindowInfo> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = engine.platform().windows(app).unwrap();
    loop {
        // Long enough for a window manager's next move to show.
        std::thread::sleep(Duration::from_millis(300));
        let now = engine.platform().windows(app).unwrap();
        let same = now.len() == last.len()
            && now
                .iter()
                .all(|w| last.iter().any(|l| l.id == w.id && l.frame == w.frame));
        if same || Instant::now() > deadline {
            return now;
        }
        last = now;
    }
}

fn frame_of(windows: &[WindowInfo], id: u64) -> Rect {
    windows
        .iter()
        .find(|w| w.id == id)
        .expect("the window is still open")
        .frame
}

fn assert_same_frames(now: &[WindowInfo], before: &[WindowInfo]) {
    for window in before {
        assert_eq!(
            frame_of(now, window.id),
            window.frame,
            "\"{}\" is back",
            window.title
        );
    }
}

/// Whether the frame is past the main display's bottom-right corner, where
/// the lane sits.
fn off_main(frame: Rect) -> bool {
    #[repr(C)]
    struct CGRect([f64; 4]);
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }
    // SAFETY: reading the main display's bounds has no preconditions.
    let CGRect([x, y, w, h]) = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    frame.x >= x + w && frame.y >= y + h
}

fn hidden(name: &str) -> bool {
    osascript(&format!(
        "tell application \"System Events\" to get visible of process \"{name}\""
    )) == "false"
}

fn osascript(script: &str) -> String {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .expect("osascript runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
#[ignore = "opens a fixture window and acts on it; needs the owner's go-ahead"]
fn every_route_lands_without_disturbing_the_user() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let user_app = front(&engine);
    let fixture = launch(true);

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

#[test]
#[ignore = "creates Hush's lane display and acts on a fixture there; needs the owner's go-ahead"]
fn lane_makes_a_background_app_fully_usable() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let user_app = front(&engine);
    // Not told to show itself: if a lane tool hides it, `lane` must bring it
    // back without activating it.
    let fixture = launch(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    while engine
        .observe("HushFixture", ObserveOptions::default())
        .is_err()
    {
        assert!(
            Instant::now() < deadline,
            "the fixture never became reachable"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let started = Instant::now();
    let report = engine
        .act("HushFixture", Some("Hush Fixture"), &["lane".to_owned()])
        .expect("lane runs");
    println!("{report}\n(lane took {:?})", started.elapsed());
    assert!(!report.contains("stopped"), "lane failed: {report}");

    let steps = [
        "click 240,342".to_owned(),
        "scroll 240,342 down 2".to_owned(),
    ];
    let report = engine
        .act("HushFixture", Some("Hush Fixture"), &steps)
        .expect("act runs");
    println!("{report}");
    let shot = engine
        .see("HushFixture", Some("Hush Fixture"), None, 480)
        .expect("see runs");
    assert!(
        shot.jpeg.len() > 6000,
        "the capture looks empty ({} bytes)",
        shot.jpeg.len()
    );

    std::thread::sleep(Duration::from_millis(200));
    let log = std::fs::read_to_string(&fixture.log).expect("fixture log");
    println!("fixture log:\n{log}");
    assert!(
        log.contains("canvas click 1"),
        "the click in the lane didn't land"
    );
    assert!(!log.contains("canvas click 2"), "the click landed twice");
    assert!(
        log.contains("canvas scroll"),
        "the scroll in the lane didn't land"
    );
    assert!(!log.contains("ACTIVATED"), "the fixture was activated");
    assert_eq!(front(&engine), user_app, "the user's front app changed");
}

#[test]
#[ignore = "opens a fixture window on the lane; needs the owner's go-ahead"]
fn an_agent_gets_its_own_window_of_the_users_app() {
    // Running before Hush starts, the fixture is one of the user's apps.
    let fixture = launch_as_user();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !std::fs::read_to_string(&fixture.log)
        .unwrap_or_default()
        .contains("later window")
    {
        assert!(Instant::now() < deadline, "the fixture must open");
        // Polls the fixture's log while it starts, in about a second.
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let (app, before) = both_windows(&engine);
    let report = engine
        .act("HushFixture", Some("Hush Fixture"), &["lane".into()])
        .unwrap();
    assert!(
        report.contains("opened window \"Agent Copy\""),
        "the agent works in a window of its own:\n{report}"
    );
    let now = engine.platform().windows(&app).unwrap();
    assert_same_frames(&now, &before);
    let copy = now.iter().find(|w| w.title == "Agent Copy").unwrap();
    assert!(off_main(copy.frame), "its window is in the lane");

    let outline = engine
        .observe(
            "HushFixture",
            ObserveOptions {
                window: Some("Agent Copy"),
                full: true,
                ..ObserveOptions::default()
            },
        )
        .unwrap();
    let press = format!("click {}", reference(&outline, "Copy button"));
    let report = engine
        .act("HushFixture", Some("Agent Copy"), &[press])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");

    // Asked again, it gets the same window back, not a second one.
    let report = engine
        .act("HushFixture", Some("Hush Fixture"), &["lane".into()])
        .unwrap();
    assert!(report.contains("already in the lane"), "{report}");

    let report = engine
        .act("HushFixture", Some("Agent Copy"), &["unlane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    let now = engine.platform().windows(&app).unwrap();
    let copy = now.iter().find(|w| w.title == "Agent Copy").unwrap();
    assert!(!off_main(copy.frame), "unlane shows it to the user");
    assert_same_frames(&now, &before);

    let log = std::fs::read_to_string(&fixture.log).unwrap_or_default();
    assert!(log.contains("copy pressed"), "the press landed:\n{log}");
    assert!(
        !log.contains("ACTIVATED"),
        "the fixture was activated:\n{log}"
    );
}
