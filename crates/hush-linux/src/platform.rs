//! `LinuxPlatform`: the `Platform` for Linux desktops, over AT-SPI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use hush_core::{
    Activity, AppInfo, Error, Image, Op, Platform, Rect, Result, ScriptLanguage, Snapshot,
    WindowInfo,
};

use zbus::block_on;

use crate::apps;
use crate::bus::{Bus, Element};
use crate::{installed, read};

/// Reads apps through the session's accessibility bus. Connecting is lazy
/// and retried, so the server starts (and says what's missing) on a machine
/// without a desktop session.
#[derive(Debug, Default)]
pub struct LinuxPlatform {
    bus: Mutex<Option<Arc<Bus>>>,
    /// Each running app's accessibility root, by pid.
    roots: Mutex<HashMap<u32, Element>>,
    /// Windows seen by `windows`, by id, for `read`.
    windows: Mutex<HashMap<u64, Element>>,
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

fn failed(error: zbus::Error) -> Error {
    match error {
        zbus::Error::MethodError(name, _, _)
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
            Error::Stale("the app quit".into())
        }
        other => Error::Failed(other.to_string()),
    }
}

fn not_yet<T>(what: &str) -> Result<T> {
    Err(Error::Unsupported(format!(
        "Hush on Linux can read windows but can't {what} yet"
    )))
}

impl Platform for LinuxPlatform {
    type Handle = Element;

    fn apps(&self, query: Option<&str>) -> Result<Vec<AppInfo>> {
        let bus = self.bus()?;
        let mut found = self.list(&bus)?;
        if let Some(query) = query {
            let wanted = query.to_lowercase();
            found.retain(|app| app.name.to_lowercase().contains(&wanted));
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
                known.insert(window.info.id, window.element);
                window.info
            })
            .collect())
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<Element>> {
        let bus = self.bus()?;
        let element = lock(&self.windows).get(&window.id).cloned();
        let element = match element {
            Some(element) => element,
            None => {
                self.windows(app)?;
                lock(&self.windows)
                    .get(&window.id)
                    .cloned()
                    .ok_or_else(|| Error::Stale(format!("window \"{}\" closed", window.title)))?
            }
        };
        block_on(read::window(&bus, app, window, &element)).map_err(failed)
    }

    fn activity(&self, _app: &AppInfo) -> Activity {
        Activity::default()
    }

    fn perform(&self, _app: &AppInfo, _window: &WindowInfo, _op: Op<'_, Element>) -> Result<()> {
        not_yet("act on them")
    }

    fn capture(&self, _window: &WindowInfo, _region: Option<Rect>, _max: u32) -> Result<Image> {
        not_yet("capture them")
    }

    fn script(&self, _app: &AppInfo, _source: &str, _language: ScriptLanguage) -> Result<String> {
        Err(Error::Unsupported(
            "AppleScript and JXA only exist on macOS".into(),
        ))
    }
}
