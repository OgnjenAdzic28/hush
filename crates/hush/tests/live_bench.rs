//! Read-only timings for every running app with a window: the numbers in
//! docs/results.md. Nothing is clicked, typed, focused or raised.
//!
//! cargo test -p hush --release --test live_bench -- --ignored --nocapture

#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use hush_core::{Platform, approx_tokens};
use hush_engine::{Engine, ObserveOptions};
use hush_macos::MacPlatform;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

#[test]
#[ignore = "reads real apps; needs Accessibility and Screen Recording permission"]
fn read_only_timings() {
    let mut engine = Engine::new(MacPlatform::new().expect("Accessibility permission"));
    let apps = engine.platform().apps(None).expect("apps");
    let (mut first, mut again, mut tokens, mut captures, mut sizes) =
        (vec![], vec![], vec![], vec![], vec![]);
    println!(
        "{:<22} {:>9} {:>9} {:>8} {:>9} {:>8}",
        "app", "first ms", "again ms", "tokens", "see ms", "see KB"
    );
    for app in apps {
        let Ok(windows) = engine.platform().windows(&app) else {
            continue;
        };
        if !windows.iter().any(|w| !w.minimized) {
            continue;
        }
        let start = Instant::now();
        let Ok(outline) = engine.observe(&app.name, ObserveOptions::default()) else {
            continue;
        };
        let first_look = start.elapsed();
        let start = Instant::now();
        let Ok(unchanged) = engine.observe(&app.name, ObserveOptions::default()) else {
            continue;
        };
        let second_look = start.elapsed();
        let start = Instant::now();
        let shot = engine.see(&app.name, None, None, 960);
        let capture = start.elapsed();
        let (see_ms, see_kb) = match &shot {
            Ok(image) => (
                format!("{:.1}", ms(capture)),
                format!("{}", image.jpeg.len() / 1024),
            ),
            Err(_) => ("-".into(), "-".into()),
        };
        println!(
            "{:<22} {:>9.1} {:>9.1} {:>8} {:>9} {:>8}   {}",
            app.name.chars().take(22).collect::<String>(),
            ms(first_look),
            ms(second_look),
            approx_tokens(&outline),
            see_ms,
            see_kb,
            if unchanged.contains("no change") {
                "unchanged → no change"
            } else {
                "second look differed"
            },
        );
        first.push(ms(first_look));
        again.push(ms(second_look));
        tokens.push(approx_tokens(&outline) as f64);
        if let Ok(image) = shot {
            captures.push(ms(capture));
            sizes.push(image.jpeg.len() as f64 / 1024.0);
        }
    }
    println!(
        "median over {} windows: first look {:.1} ms, unchanged look {:.1} ms, outline {:.0} tokens, see {:.1} ms ({:.0} KB)",
        first.len(),
        median(first),
        median(again),
        median(tokens),
        median(captures),
        median(sizes),
    );
}
