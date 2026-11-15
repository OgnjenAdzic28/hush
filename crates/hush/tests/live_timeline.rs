//! Plays a timeline into the fixture app (bench/fixture) and checks, from
//! the times the fixture logged on arrival, that every event landed once
//! and close to its schedule, without the fixture ever being activated.
//!
//! It opens a window on Hush's lane display, so it needs the owner's
//! go-ahead.
//!
//! cargo test -p hush --release --test live_timeline -- --ignored --nocapture

#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

mod common;

use common::{front, launch};

/// Arrival times (ms since boot) of log lines starting with `prefix`.
fn arrivals(log: &str, prefix: &str) -> Vec<f64> {
    log.lines()
        .filter(|line| line.starts_with(prefix))
        .filter_map(|line| line.rsplit_once("t=")?.1.parse().ok())
        .collect()
}

#[test]
#[ignore = "opens a fixture window and plays input into it; needs the owner's go-ahead"]
fn a_timeline_lands_on_time_without_disturbing_the_user() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let user_app = front(&engine);
    // In the lane, as an agent would play into a browser: a lane tool such
    // as Doorman may hide an app an agent launched, and a hidden app ignores
    // pointer input.
    let fixture = launch(false);
    let launched = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(5);
    let outline = loop {
        match engine.observe("HushFixture", ObserveOptions::default()) {
            Ok(outline) => break outline,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => panic!("the fixture never became reachable ({error})"),
        }
    };
    // Doorman hides an app an agent launched, and hides it again every
    // 0.5 s for 2 s while its first window opens; `lane` brings it back
    // once that's over.
    let since_launch = Instant::now() - launched;
    std::thread::sleep(Duration::from_millis(2500).saturating_sub(since_launch));
    let report = engine
        .act("HushFixture", Some("Hush Fixture"), &["lane".to_owned()])
        .expect("lane runs");
    assert!(!report.contains("stopped"), "lane failed: {report}");
    let height: f64 = outline
        .lines()
        .next()
        .and_then(|header| header.rsplit(' ').find_map(|word| word.split_once('x')))
        .and_then(|(_, h)| h.parse().ok())
        .unwrap_or(452.0);
    let y = height - 110.0;

    // A click on the canvas gives it keyboard focus, then a run of notes
    // with uneven gaps (the opening triplets of a slow piece, sped up),
    // then a drag across the canvas.
    let keys = ['g', 'c', 'e'];
    let mut lines = vec![format!("0 tap 240,{y}")];
    let mut scheduled = Vec::new();
    let mut at = 200.0;
    for i in 0..36 {
        let key = keys[i % 3];
        let hold = 60.0 + (i % 4) as f64 * 15.0;
        lines.push(format!("{at} tap {key} {hold}"));
        scheduled.push(at);
        at += [90.0, 110.0, 140.0, 75.0][i % 4];
    }
    lines.push(format!("{at} down 100,{y}"));
    for step in 1..=5 {
        lines.push(format!(
            "{} move {},{y}",
            at + 20.0 * f64::from(step),
            100 + 40 * step
        ));
    }
    lines.push(format!("{} up 300,{y}", at + 120.0));

    // A freshly launched app takes its first key event 13 to 21 ms longer
    // than the rest (measured on the fixture, wherever in the timeline it
    // comes), so one key goes first, in its own call.
    let warm_up = [format!("0 tap 240,{y}"), "100 tap x".to_owned()];
    engine
        .timeline("HushFixture", Some("Hush Fixture"), &warm_up)
        .expect("warm-up runs");

    let started = Instant::now();
    let report = engine
        .timeline("HushFixture", Some("Hush Fixture"), &lines)
        .expect("timeline runs");
    println!("{report}(the call took {:?})", started.elapsed());

    std::thread::sleep(Duration::from_millis(200));
    let log = std::fs::read_to_string(&fixture.log).expect("fixture log");
    // Everything after the warm-up key.
    let skip = |mut times: Vec<f64>| {
        times.remove(0);
        times
    };
    let downs = skip(arrivals(&log, "canvas key down"));
    let ups = skip(arrivals(&log, "canvas key up"));
    assert_eq!(
        downs.len(),
        scheduled.len(),
        "every key went down once:\n{log}"
    );
    assert_eq!(ups.len(), scheduled.len(), "every key came up once");
    assert!(log.contains("canvas key down g"), "the right keys arrived");

    // How long after its time each note arrived. Rhythm is the spread of
    // these: a delay every note shares (posting, then the app's own
    // delivery) shifts the whole piece without changing it.
    let delays: Vec<f64> = downs
        .iter()
        .zip(&scheduled)
        .map(|(arrived, due)| (arrived - downs[0]) - (due - scheduled[0]))
        .collect();
    let shown: Vec<String> = delays.iter().map(|d| format!("{d:.1}")).collect();
    println!("delays after the first note (ms): {}", shown.join(" "));
    let mut sorted = delays.clone();
    sorted.sort_by(f64::total_cmp);
    let typical = sorted[sorted.len() / 2];
    let mut errors: Vec<f64> = delays.iter().map(|d| (d - typical).abs()).collect();
    errors.sort_by(f64::total_cmp);
    let median = errors[errors.len() / 2];
    let worst = errors[errors.len() - 1];
    println!(
        "note onsets against the schedule: median error {median:.2} ms, worst {worst:.2} ms \
         (first note {:.2} ms off)",
        (delays[0] - typical).abs()
    );
    assert!(median < 2.0, "median onset error {median:.2} ms");
    assert!(worst < 5.0, "worst onset error {worst:.2} ms");

    assert_eq!(
        arrivals(&log, "canvas drag").len(),
        5,
        "every move of the drag arrived as a drag"
    );
    assert!(
        arrivals(&log, "canvas release").len() >= 3,
        "the taps and the drag were all released"
    );
    assert!(!log.contains("ACTIVATED"), "the fixture was activated");
    assert_eq!(front(&engine), user_app, "the user's front app changed");
}
