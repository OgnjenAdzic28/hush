//! Acts on real apps through the engine, with the steps an agent sends.
//! Ignored by default; the Linux CI job runs them headlessly:
//!
//! ```sh
//! xvfb-run -a dbus-run-session -- cargo test -p hush-linux -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "linux")]

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use hush_core::Platform;
use hush_engine::{Engine, ObserveOptions};
use hush_linux::LinuxPlatform;

struct Launched(Child);

impl Drop for Launched {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `command` and waits for its window on the accessibility bus.
fn launch(engine: &Engine<LinuxPlatform>, command: &str) -> Launched {
    engine.platform().apps(None).unwrap();
    let child = Launched(Command::new(command).spawn().unwrap());
    let pid = child.0.id();
    // Waits on an observable condition: the window appearing on the bus.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !engine
        .platform()
        .apps(None)
        .unwrap()
        .iter()
        .any(|a| a.pid == Some(pid))
    {
        assert!(Instant::now() < deadline, "{command} never showed a window");
        std::thread::sleep(Duration::from_millis(100));
    }
    child
}

/// The ref of the first outline line with this role word and name (`None`
/// name: any line of that role that isn't disabled).
fn ref_of(outline: &str, role: &str, name: Option<&str>) -> String {
    outline
        .lines()
        .map(str::trim)
        .find(|line| {
            let mut parts = line.splitn(3, ' ');
            let (_, r, rest) = (parts.next(), parts.next(), parts.next().unwrap_or(""));
            r == Some(role)
                && match name {
                    Some(name) => rest.starts_with(&format!("\"{name}\"")),
                    None => !rest.contains("[disabled]"),
                }
        })
        .and_then(|line| line.split(' ').next())
        .unwrap_or_else(|| panic!("no {role} {name:?} in\n{outline}"))
        .to_owned()
}

fn act(engine: &mut Engine<LinuxPlatform>, app: &str, steps: &[&str]) -> String {
    let steps: Vec<String> = steps.iter().map(|s| s.to_string()).collect();
    let out = engine.act(app, None, &steps).unwrap();
    println!("act {steps:?}\n{out}");
    out
}

#[test]
#[ignore]
fn presses_calculator_buttons_and_settles_on_events() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let _app = launch(&engine, "gnome-calculator");
    let outline = engine
        .observe("gnome-calculator", ObserveOptions::default())
        .unwrap();
    let mut out = String::new();
    for digit in ["7", "8", "9"] {
        let r = ref_of(&outline, "button", Some(digit));
        out += &act(&mut engine, "gnome-calculator", &[&format!("click {r}")]);
    }
    // GNOME Calculator works the result out in the background and updates
    // its display about 250 ms after a press, so the last press can land
    // after its step settled. Reads until it shows, for up to 3 s.
    let started = Instant::now();
    let display = loop {
        let display = engine
            .observe(
                "gnome-calculator",
                ObserveOptions {
                    full: true,
                    ..Default::default()
                },
            )
            .unwrap();
        if display.contains("789") || started.elapsed() > Duration::from_secs(3) {
            break display;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(
        display.contains("789"),
        "the display didn't show 789:\n{display}"
    );
    assert!(
        out.contains("settled"),
        "no press settled on events:\n{out}"
    );
}

#[test]
#[ignore]
fn sets_and_types_text_and_toggles_a_checkbox() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let _app = launch(&engine, "gtk3-widget-factory");
    let outline = engine
        .observe("gtk3-widget-factory", ObserveOptions::default())
        .unwrap();
    let field = ref_of(&outline, "field", None);
    let out = act(
        &mut engine,
        "gtk3-widget-factory",
        &[
            &format!("set {field} \"hello hush\""),
            &format!("type {field} \" again\""),
        ],
    );
    assert!(
        out.contains("hello hush again"),
        "text wasn't set and typed"
    );

    // A plain one: clicking the mixed checkbox toggles it underneath, but
    // it still reads as mixed.
    let checkbox = outline
        .lines()
        .map(str::trim)
        .find(|l| l.contains(" checkbox ") && !l.contains("[disabled]") && !l.contains("= mixed"))
        .and_then(|l| l.split(' ').next())
        .unwrap_or_else(|| panic!("no plain checkbox in\n{outline}"))
        .to_owned();
    let out = act(
        &mut engine,
        "gtk3-widget-factory",
        &[&format!("click {checkbox}")],
    );
    assert!(
        out.contains("→ = on") || out.contains("→ = off"),
        "the checkbox didn't toggle:\n{out}"
    );
}

#[test]
#[ignore]
fn refuses_keys_and_focus_for_apps_on_the_users_display() {
    let mut engine = Engine::new(LinuxPlatform::new());
    let _app = launch(&engine, "gtk3-widget-factory");
    let outline = engine
        .observe("gtk3-widget-factory", ObserveOptions::default())
        .unwrap();
    // A refused step stops the batch and says why; the call itself succeeds.
    let out = act(&mut engine, "gtk3-widget-factory", &["key ctrl+a"]);
    assert!(out.contains("own display"), "{out}");
    let field = ref_of(&outline, "field", None);
    let out = act(
        &mut engine,
        "gtk3-widget-factory",
        &[&format!("focus {field}")],
    );
    assert!(out.contains("take your focus"), "{out}");
}
