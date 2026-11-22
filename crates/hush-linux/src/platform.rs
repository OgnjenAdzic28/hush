//! `LinuxPlatform`: the `Platform` for Linux desktops, over AT-SPI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use hush_core::{
    Activity, AppInfo, Error, Image, Op, Platform, Rect, Result, ScriptLanguage, Snapshot,
    WindowInfo,
};

use zbus::block_on;

use crate::bus::{Bus, Element};
use crate::lane::Lane;
use crate::screen::Screen;
use crate::watch::Watch;
use crate::{act, apps, installed, read};

/// Reads apps through the session's accessibility bus. Connecting is lazy
/// and retried, so the server starts (and says what's missing) on a machine
/// without a desktop session.
#[derive(Debug, Default)]
pub struct LinuxPlatform {
    bus: Mutex<Option<Arc<Bus>>>,
    /// Each running app's accessibility root, by pid.
    roots: Mutex<HashMap<u32, Element>>,
    /// Windows seen by `windows`, by id, with their app's pid, for `read`,
    /// `perform` and `capture`.
    windows: Mutex<HashMap<u64, (Element, Option<u32>)>>,
    /// Started on the first `activity` call, then runs for good.
    watch: Mutex<Option<Arc<Watch>>>,
    /// Hush's own display, once found or started, and a connection to it.
    lane: Mutex<Option<(Lane, Arc<Screen>)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl LinuxPlatform {
    pub fn new() -> LinuxPlatform {
        LinuxPlatform::default()
    }

    fn bus(&self) -> Result<Arc<Bus>> {
        let mut bus = lock(&self.bus);
        if let Some(bus) = &*bus {
            return Ok(bus.clone());
        }
        let connected = Arc::new(block_on(Bus::connect()).map_err(Error::Failed)?);
        *bus = Some(connected.clone());
        Ok(connected)
    }

    fn root(&self, bus: &Bus, app: &AppInfo) -> Result<Element> {
        let pid = app
            .pid
            .ok_or_else(|| Error::NotFound(format!("{} isn't running", app.name)))?;
        if let Some(root) = lock(&self.roots).get(&pid) {
            return Ok(root.clone());
        }
        self.list(bus)?;
        lock(&self.roots)
            .get(&pid)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("{} isn't on the accessibility bus", app.name)))
    }

    fn window_element(&self, app: &AppInfo, window: &WindowInfo) -> Result<Element> {
        if let Some((element, _)) = lock(&self.windows).get(&window.id) {
            return Ok(element.clone());
        }
        self.windows(app)?;
        lock(&self.windows)
            .get(&window.id)
            .map(|(element, _)| element.clone())
            .ok_or_else(|| Error::Stale(format!("window \"{}\" closed", window.title)))
    }

    /// The lane an earlier host or this one started, if it is up; with
    /// `start`, one is started when none is.
    fn lane(&self, start: bool) -> Result<Option<(Lane, Arc<Screen>)>> {
        let mut known = lock(&self.lane);
        if let Some(lane) = &*known {
            return Ok(Some(lane.clone()));
        }
        let lane = match (Lane::running(), start) {
            (Some(lane), _) => lane,
            (None, true) => Lane::find_or_start().map_err(Error::Failed)?,
            (None, false) => return Ok(None),
        };
        let screen = Arc::new(Screen::open(lane).map_err(Error::Failed)?);
        *known = Some((lane, screen.clone()));
        Ok(Some((lane, screen)))
    }

    /// Whether the app runs on Hush's display, where input and capture
    /// can't reach the user.
    fn in_lane(&self, pid: Option<u32>) -> bool {
        match (pid, self.lane(false)) {
            (Some(pid), Ok(Some((lane, _)))) => lane.holds(pid),
            _ => false,
        }
    }

    fn watch(&self, bus: &Bus) -> Result<Arc<Watch>> {
        let mut watch = lock(&self.watch);
        if let Some(watch) = &*watch {
            return Ok(watch.clone());
        }
        let started = Arc::new(block_on(Watch::start(bus.connection())).map_err(failed)?);
        *watch = Some(started.clone());
        Ok(started)
    }

    fn list(&self, bus: &Bus) -> Result<Vec<AppInfo>> {
        let running = block_on(apps::running(bus)).map_err(failed)?;
        let mut roots = lock(&self.roots);
        roots.clear();
        for app in &running {
            if let Some(pid) = app.info.pid {
                roots.insert(pid, app.root.clone());
            }
        }
        Ok(running.into_iter().map(|app| app.info).collect())
    }
}

pub(crate) fn failed(error: zbus::Error) -> Error {
    match error {
        zbus::Error::MethodError(name, _, _)
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
            Error::Stale("the app quit".into())
        }
        other => Error::Failed(other.to_string()),
    }
}

/// Capturing is only done on Hush's display: on the user's it could show
/// whatever covers the window, and on Wayland it isn't allowed at all.
const CAPTURE_NEEDS_LANE: &str = "Hush only captures apps it started on its own display; quit the app and observe it again, and Hush starts it there";

impl Platform for LinuxPlatform {
    type Handle = Element;

    fn apps(&self, query: Option<&str>) -> Result<Vec<AppInfo>> {
        let bus = self.bus()?;
        let mut found = self.list(&bus)?;
        if let Some(query) = query {
            let wanted = query.to_lowercase();
            // By the name people see, the desktop id, or the command.
            found.retain(|app| {
                let command = app.path.as_deref().and_then(|p| p.rsplit('/').next());
                [Some(app.name.as_str()), app.id.as_deref(), command]
                    .into_iter()
                    .flatten()
                    .any(|n| n.to_lowercase().contains(&wanted))
            });
            for installed in installed::matching(query) {
                if !found.iter().any(|app| app.name == installed.name) {
                    found.push(installed);
                }
            }
        }
        Ok(found)
    }

    fn windows(&self, app: &AppInfo) -> Result<Vec<WindowInfo>> {
        let bus = self.bus()?;
        let root = self.root(&bus, app)?;
        let windows = block_on(apps::windows(&bus, &root)).map_err(failed)?;
        let mut known = lock(&self.windows);
        Ok(windows
            .into_iter()
            .map(|window| {
                known.insert(window.info.id, (window.element, app.pid));
                window.info
            })
            .collect())
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<Element>> {
        let bus = self.bus()?;
        let element = self.window_element(app, window)?;
        block_on(read::window(&bus, app, window, &element)).map_err(failed)
    }

    fn activity(&self, app: &AppInfo) -> Activity {
        let Ok(bus) = self.bus() else {
            return Activity::default();
        };
        let (Ok(watch), Ok(root)) = (self.watch(&bus), self.root(&bus, app)) else {
            return Activity::default();
        };
        watch.activity(&root.bus)
    }

    fn perform(&self, app: &AppInfo, window: &WindowInfo, op: Op<'_, Element>) -> Result<()> {
        let bus = self.bus()?;
        let element = self.window_element(app, window)?;
        let in_lane = self.in_lane(app.pid);
        block_on(act::perform(&bus, &element, in_lane, op))
    }

    fn capture(&self, window: &WindowInfo, region: Option<Rect>, max: u32) -> Result<Image> {
        let pid = lock(&self.windows)
            .get(&window.id)
            .and_then(|(_, pid)| *pid);
        let Some(pid) = pid else {
            return Err(Error::Stale(format!("window \"{}\" closed", window.title)));
        };
        let Some((_, screen)) = self.lane(false)?.filter(|(lane, _)| lane.holds(pid)) else {
            return Err(Error::Unsupported(CAPTURE_NEEDS_LANE.into()));
        };
        let (found, frame) = screen.window_of(pid, window.frame).map_err(Error::Failed)?;
        let size = (frame.w as u32, frame.h as u32);
        screen
            .capture(found, size, region, max)
            .map_err(Error::Failed)
    }

    fn launch(&self, app: &AppInfo) -> Result<()> {
        let (lane, _) = self
            .lane(true)?
            .ok_or_else(|| Error::Failed("Hush's display didn't start".into()))?;
        lane.launch(app).map_err(Error::Failed)
    }

    fn script(&self, _app: &AppInfo, _source: &str, _language: ScriptLanguage) -> Result<String> {
        Err(Error::Unsupported(
            "AppleScript and JXA only exist on macOS".into(),
        ))
    }
}
