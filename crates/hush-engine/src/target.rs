//! Finding the app and window a call is about.

use std::thread;
use std::time::{Duration, Instant};

use hush_core::{Access, AppInfo, Error, Platform, Result, WindowInfo};

use crate::Engine;

/// How long a started app may take to open its first window.
const START_LIMIT: Duration = Duration::from_secs(10);

/// How often a starting app is checked; Hush can't watch an app for events
/// before it is running.
const START_POLL: Duration = Duration::from_millis(50);

impl<P: Platform> Engine<P> {
    /// `app` is a name, part of a name, a bundle id or a pid. Running apps
    /// win over installed ones, exact names over partial ones.
    pub(crate) fn target(&self, app: &str, window: Option<&str>) -> Result<(AppInfo, WindowInfo)> {
        let app = self.find_app(app)?;
        let windows = self.platform.windows(&app)?;
        let chosen = match window {
            Some(title) => {
                let wanted = title.to_lowercase();
                windows
                    .iter()
                    .find(|w| w.title.to_lowercase().contains(&wanted))
            }
            None => windows
                .iter()
                .find(|w| w.main && !w.minimized)
                .or_else(|| windows.iter().find(|w| !w.minimized)),
        };
        let Some(chosen) = chosen.cloned() else {
            let titles: Vec<String> = windows.iter().map(|w| format!("\"{}\"", w.title)).collect();
            return Err(Error::NotFound(match (window, titles.is_empty()) {
                (_, true) => format!("{} has no open windows", app.name),
                (Some(title), false) => format!(
                    "{} has no window like \"{title}\"; it has {}",
                    app.name,
                    titles.join(", ")
                ),
                (None, false) => format!(
                    "all of {}'s windows are minimized: {}",
                    app.name,
                    titles.join(", ")
                ),
            }));
        };
        Ok((app, chosen))
    }

    /// Starts the installed app named exactly `query` when it isn't
    /// running, and waits for it to open a window (or for the limit).
    /// Returns the app it started, if any.
    pub(crate) fn start_if_needed(&self, query: &str) -> Result<Option<AppInfo>> {
        match self.find_app(query) {
            Ok(_) => return Ok(None),
            Err(refused @ Error::Permission(_)) => return Err(refused),
            Err(_) => {}
        }
        let wanted = query.trim().to_lowercase();
        let Some(app) = self.platform.apps(Some(query))?.into_iter().find(|app| {
            app.pid.is_none()
                && (app.name.to_lowercase() == wanted || app.id.as_deref() == Some(query.trim()))
        }) else {
            return Ok(None);
        };
        self.limits.check(&app, Access::Read)?;
        self.platform.launch(&app)?;
        let deadline = Instant::now() + START_LIMIT;
        while Instant::now() < deadline {
            if let Ok(running) = self.find_app(query)
                && !self.platform.windows(&running)?.is_empty()
            {
                break;
            }
            thread::sleep(START_POLL);
        }
        Ok(Some(app))
    }

    pub(crate) fn find_app(&self, query: &str) -> Result<AppInfo> {
        let query = query.trim();
        let candidates = self.platform.apps(Some(query))?;
        let pid: Option<u32> = query.parse().ok();
        let lower = query.to_lowercase();
        let score = |app: &AppInfo| {
            let running = app.pid.is_some();
            let exact = app.name.to_lowercase() == lower
                || app.id.as_deref() == Some(query)
                || app.pid == pid;
            (running, exact)
        };
        let best = if let Some(pid) = pid {
            self.platform
                .apps(None)?
                .into_iter()
                .find(|app| app.pid == Some(pid))
        } else {
            candidates.into_iter().max_by_key(score)
        };
        if let Some(app) = &best {
            self.limits.check(app, Access::Read)?;
        }
        match best {
            Some(app) if app.pid.is_some() => Ok(app),
            Some(app) => Err(Error::NotFound(format!(
                "{} isn't running; observe {} starts it in the background",
                app.name, app.name
            ))),
            None => Err(Error::NotFound(format!(
                "no running app matches \"{query}\"; try apps"
            ))),
        }
    }
}
