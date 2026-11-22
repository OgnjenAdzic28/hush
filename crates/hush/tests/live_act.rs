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

use hush_core::Platform;
use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

mod common;

use common::{front, launch, reference};

#[test]
#[ignore = "moves fixture windows onto the lane and back; needs the owner's go-ahead"]
fn lane_isolates_one_window_and_restores_its_original_position() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let _fixture = launch(true);
    let deadline = Instant::now() + Duration::from_secs(5);
    let (app, before) = loop {
        if let Some(app) = engine
            .platform()
            .apps(None)
            .unwrap()
            .into_iter()
            .find(|app| app.name == "HushFixture")
        {
            let windows = engine.platform().windows(&app).unwrap();
            if windows.len() == 2 {
                break (app, windows);
            }
        }
        assert!(Instant::now() < deadline, "both fixture windows must open");
        std::thread::sleep(Duration::from_millis(50));
    };
    let target = before.iter().find(|w| w.title == "Hush Fixture").unwrap();
    let other = before.iter().find(|w| w.title == "Later").unwrap();
    // Re-parking must retain the original visible position, not the lane position.
    for _ in 0..2 {
        let report = engine
            .act("HushFixture", Some(&target.title), &["lane".into()])
            .unwrap();
        assert!(!report.contains("stopped"), "{report}");
        let windows = engine.platform().windows(&app).unwrap();
        assert_ne!(
            windows.iter().find(|w| w.id == target.id).unwrap().frame,
            target.frame
        );
        assert_eq!(
            windows.iter().find(|w| w.id == other.id).unwrap().frame,
            other.frame
        );
    }
    let report = engine
        .act("HushFixture", Some(&target.title), &["unlane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    for original in &before {
        let windows = engine.platform().windows(&app).unwrap();
        assert_eq!(
            windows.iter().find(|w| w.id == original.id).unwrap().frame,
            original.frame
        );
    }
    let report = engine
        .act("HushFixture", Some(&target.title), &["lane".into()])
        .unwrap();
    assert!(!report.contains("stopped"), "{report}");
    drop(engine);
    let platform = MacPlatform::new().unwrap();
    for original in &before {
        let windows = platform.windows(&app).unwrap();
        assert_eq!(
            windows.iter().find(|w| w.id == original.id).unwrap().frame,
            original.frame
        );
    }
    assert!(
        platform.windows(&app).unwrap().len() == 2,
        "cleanup must not hide the app"
    );
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
