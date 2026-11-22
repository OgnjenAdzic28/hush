//! The lane on Linux: a hidden X server that only Hush uses.
//!
//! On macOS the lane is a virtual display nobody can see. On Linux Hush
//! starts its own X server with no screen attached (Xvfb) and runs the apps
//! it launches there. Keys, clicks and captures on that display can't reach
//! the user's, whether their desktop is X11 or Wayland, and the apps still
//! share the user's D-Bus session, so their accessibility trees are read
//! like any other app's.
//!
//! The server outlives the Hush host that started it, so apps an agent left
//! open keep running and the next host finds them again.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use hush_core::AppInfo;

/// The lane's size. Big enough for any app's default window.
const SCREEN: &str = "2560x1600x24";

/// Display numbers tried for a new lane, well above the ones desktops use.
const DISPLAYS: std::ops::Range<u32> = 64..128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lane {
    pub display: u32,
}

impl Lane {
    /// The `DISPLAY` value for apps on the lane.
    pub(crate) fn name(self) -> String {
        format!(":{}", self.display)
    }

    /// The lane an earlier Hush host left running, if it still answers.
    pub(crate) fn running() -> Option<Lane> {
        let display: u32 = std::fs::read_to_string(state_file()?)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let lane = Lane { display };
        lane.answers().then_some(lane)
    }

    /// The running lane, or a new one.
    pub(crate) fn find_or_start() -> Result<Lane, String> {
        if let Some(lane) = Lane::running() {
            return Ok(lane);
        }
        let display = DISPLAYS
            .clone()
            .find(|n| !lock_file(*n).exists() && !socket(*n).exists())
            .ok_or("every display number Hush tries for its lane is taken")?;
        let lane = Lane { display };
        let mut server = Command::new("Xvfb")
            .args([&lane.name(), "-screen", "0", SCREEN, "-nolisten", "tcp", "-noreset"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Its own process group, so it lives on after this host exits.
            .process_group(0)
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => "Hush runs the apps it starts on its own hidden display, which needs Xvfb: install it (sudo apt install xvfb, or xorg-x11-server-Xvfb on Fedora)".to_owned(),
                _ => format!("can't start Xvfb for Hush's display: {e}"),
            })?;
        // Waits on an observable condition: the server's socket accepting a
        // connection. Xvfb is ready in 50 to 200 ms.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !lane.answers() {
            if let Ok(Some(status)) = server.try_wait() {
                return Err(format!("Xvfb exited at once ({status})"));
            }
            if Instant::now() > deadline {
                let _ = server.kill();
                return Err("Xvfb never came up".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(state) = state_file() {
            if let Some(dir) = state.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&state, display.to_string());
        }
        Ok(lane)
    }

    /// Whether the display is up and is this user's: an X server another
    /// user started on that number must never get this user's apps.
    fn answers(self) -> bool {
        use std::os::unix::fs::MetadataExt;
        let ours = match (
            std::fs::metadata(socket(self.display)),
            crate::runtime::uid(),
        ) {
            (Ok(socket), Ok(uid)) => socket.uid() == uid,
            _ => false,
        };
        ours && x11rb::connect(Some(&self.name())).is_ok()
    }

    /// Whether `pid` runs on this lane, from its environment.
    pub(crate) fn holds(self, pid: u32) -> bool {
        let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ")) else {
            return false;
        };
        let name = self.name();
        environ
            .split(|b| *b == 0)
            .filter_map(|entry| entry.strip_prefix(b"DISPLAY="))
            .any(|value| {
                value == name.as_bytes() || value.starts_with(format!("{name}.").as_bytes())
            })
    }

    /// Starts an installed app on the lane, in its own process group so it
    /// lives as long as the lane, not as long as this host.
    pub(crate) fn launch(self, app: &AppInfo) -> Result<(), String> {
        let exec = app
            .path
            .as_deref()
            .ok_or_else(|| format!("{} has no command to start it", app.name))?;
        Command::new("sh")
            .arg("-c")
            .arg(format!("exec {exec}"))
            .env("DISPLAY", self.name())
            // Toolkits prefer Wayland when it is there; on the lane they
            // must use X.
            .env_remove("WAYLAND_DISPLAY")
            .env("GDK_BACKEND", "x11")
            .env("QT_QPA_PLATFORM", "xcb")
            // Hush reads the app through AT-SPI, so it must publish its
            // tree from the start. Qt, Chromium and Firefox only do when
            // told to, or when they see an assistive tool at startup.
            .env("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1")
            .env("QT_ACCESSIBILITY", "1")
            .env("ACCESSIBILITY_ENABLED", "1")
            .env("GNOME_ACCESSIBILITY", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("can't start {}: {e}", app.name))
    }
}

impl Lane {
    /// Held while sending input to the lane. Every Hush host on this
    /// machine shares the lane, and one host's keys must not land in the
    /// middle of another's drag.
    pub(crate) fn input_lock() -> std::io::Result<std::fs::File> {
        let path = state_file()
            .ok_or_else(|| std::io::Error::other("no private folder for Hush's input lock"))?
            .with_file_name("lane-input.lock");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::File::create(path)?;
        file.lock()?;
        Ok(file)
    }
}

fn socket(display: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/.X11-unix/X{display}"))
}

fn lock_file(display: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/.X{display}-lock"))
}

/// Where the lane's display number is kept between hosts. `None` when the
/// user has no private folder for it, and then no lane is remembered.
fn state_file() -> Option<PathBuf> {
    crate::runtime::dir()
        .ok()
        .map(|dir| dir.join("hush").join("lane-display"))
}
