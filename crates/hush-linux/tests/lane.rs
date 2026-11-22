//! Hush's own display: an app an agent observes by name is started there,
//! off the user's display, and `see` captures it there.
//! Ignored by default; the Linux CI job runs it headlessly.
#![cfg(target_os = "linux")]

use hush_core::Platform;
use hush_engine::{Engine, ObserveOptions};
use hush_linux::LinuxPlatform;

/// Kills the app the test started, which outlives the engine on purpose.
struct Started(u32);

impl Drop for Started {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill")
            .arg(self.0.to_string())
            .status();
    }
}

fn display_of(pid: u32) -> Option<String> {
    let environ = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    environ
        .split(|b| *b == 0)
        .find_map(|e| e.strip_prefix(b"DISPLAY="))
        .map(|v| String::from_utf8_lossy(v).into_owned())
}

#[test]
#[ignore]
fn observing_an_installed_app_starts_it_on_hushs_display_and_sees_it() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let outline = engine
        .observe("Calculator", ObserveOptions::default())
        .unwrap();
    println!("{outline}");
    assert!(outline.starts_with("# started Calculator"), "{outline}");
    let app = engine
        .platform()
        .apps(Some("Calculator"))
        .unwrap()
        .into_iter()
        .find(|a| a.pid.is_some())
        .expect("Calculator is running");
    let _started = Started(app.pid.unwrap());

    // Not the display the test runs on: Hush's own.
    let ours = std::env::var("DISPLAY").unwrap();
    let theirs = display_of(app.pid.unwrap()).expect("Calculator has a DISPLAY");
    assert_ne!(theirs, ours);
    let number: u32 = theirs.trim_start_matches(':').parse().unwrap();
    assert!((64..128).contains(&number), "{theirs}");

    let image = engine.see("Calculator", None, None, 800).unwrap();
    println!(
        "see: {}x{}, {} bytes",
        image.width,
        image.height,
        image.jpeg.len()
    );
    assert!(image.width > 100 && image.width <= 800);
    assert_eq!(&image.jpeg[..2], &[0xFF, 0xD8], "a JPEG");
    // A window that drew something has more than one color, so it doesn't
    // compress to almost nothing.
    assert!(image.jpeg.len() > 2000, "{} bytes", image.jpeg.len());
}

#[test]
#[ignore]
fn apps_on_the_users_display_are_not_captured() {
    let mut child = std::process::Command::new("gtk3-widget-factory")
        .spawn()
        .unwrap();
    let mut engine = Engine::new(LinuxPlatform::new());
    // Waits on an observable condition: the window appearing on the bus.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while engine
        .observe("gtk3-widget-factory", ObserveOptions::default())
        .is_err()
    {
        assert!(std::time::Instant::now() < deadline, "no window");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let refused = engine
        .see("gtk3-widget-factory", None, None, 800)
        .unwrap_err();
    let _ = child.kill();
    let _ = child.wait();
    assert!(refused.to_string().contains("own display"), "{refused}");
}

/// Observes until `wanted` shows, for up to 3 s: GNOME Calculator works a
/// result out in the background, about 250 ms after the key.
fn observe_until(engine: &mut Engine<LinuxPlatform>, wanted: &str) -> String {
    let started = std::time::Instant::now();
    loop {
        let outline = engine
            .observe(
                "Calculator",
                ObserveOptions {
                    full: true,
                    ..Default::default()
                },
            )
            .unwrap();
        if outline.contains(wanted) || started.elapsed() > std::time::Duration::from_secs(3) {
            return outline;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
#[ignore]
fn keys_and_pointer_clicks_reach_an_app_on_hushs_display() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let outline = engine
        .observe("Calculator", ObserveOptions::default())
        .unwrap();
    let app = engine
        .platform()
        .apps(Some("Calculator"))
        .unwrap()
        .into_iter()
        .find(|a| a.pid.is_some())
        .expect("Calculator is running");
    let _started = Started(app.pid.unwrap());

    let steps: Vec<String> = [
        "key 1",
        "key 2",
        "key shift+=",
        "key 3",
        "key 0",
        "key enter",
    ]
    .map(String::from)
    .to_vec();
    println!("{}", engine.act("Calculator", None, &steps).unwrap());
    let shown = observe_until(&mut engine, "42");
    assert!(shown.contains("42"), "12+30 didn't make 42:\n{shown}");

    // A double click has no accessibility action, so it goes by pointer.
    let seven = outline
        .lines()
        .map(str::trim)
        .find(|l| l.contains("button \"7\""))
        .and_then(|l| l.split(' ').next())
        .unwrap_or_else(|| panic!("no 7 button in\n{outline}"))
        .to_owned();
    let steps = vec!["key escape".to_owned(), format!("double {seven}")];
    println!("{}", engine.act("Calculator", None, &steps).unwrap());
    let shown = observe_until(&mut engine, "77");
    assert!(
        shown.contains("77"),
        "a double click on 7 didn't show 77:\n{shown}"
    );

    // A timeline plays on its own schedule, here keys 5 and 6 100 ms apart.
    let lines = vec![
        "0 tap escape".to_owned(),
        "100 tap 5".to_owned(),
        "200 tap 6".to_owned(),
    ];
    let played = engine.timeline("Calculator", None, &lines).unwrap();
    println!("{played}");
    let shown = observe_until(&mut engine, "56");
    assert!(
        shown.contains("56"),
        "the timeline didn't type 56:\n{shown}"
    );
}

#[test]
#[ignore]
fn records_an_app_on_hushs_display_with_the_agent_cursor() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let outline = engine
        .observe("Calculator", ObserveOptions::default())
        .unwrap();
    let app = engine
        .platform()
        .apps(Some("Calculator"))
        .unwrap()
        .into_iter()
        .find(|a| a.pid.is_some())
        .expect("Calculator is running");
    let _started = Started(app.pid.unwrap());
    let button = |digit: &str| {
        outline
            .lines()
            .map(str::trim)
            .find(|l| l.contains(&format!("button \"{digit}\"")))
            .and_then(|l| l.split(' ').next())
            .unwrap_or_else(|| panic!("no {digit} button in\n{outline}"))
            .to_owned()
    };

    println!("{}", engine.record_start("Calculator", None).unwrap());
    for digit in ["4", "2"] {
        let steps = vec![format!("click {}", button(digit))];
        println!("{}", engine.act("Calculator", None, &steps).unwrap());
    }
    let out = std::env::temp_dir().join("hush-linux-recording.mp4");
    let wrote = engine
        .record_stop(Some("Claude"), Some(&out.to_string_lossy()))
        .unwrap();
    println!("{wrote}");
    let probe = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-count_frames", "-select_streams", "v:0"])
        .args([
            "-show_entries",
            "stream=codec_name,nb_read_frames,width,height",
        ])
        .args(["-of", "csv=p=0"])
        .arg(&out)
        .output()
        .unwrap();
    let probe = String::from_utf8_lossy(&probe.stdout);
    println!("ffprobe: {probe}");
    let fields: Vec<&str> = probe.trim().split(',').collect();
    assert_eq!(fields.first(), Some(&"h264"), "{probe}");
    let frames: u32 = fields.last().unwrap().parse().unwrap();
    assert!(frames >= 5, "only {frames} frames");
}
