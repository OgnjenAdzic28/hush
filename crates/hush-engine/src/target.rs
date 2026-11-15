//! Finding the app and window a call is about.

use hush_core::{AppInfo, Error, Platform, Result, WindowInfo};

use crate::Engine;

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

    fn find_app(&self, query: &str) -> Result<AppInfo> {
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
        match best {
            Some(app) if app.pid.is_some() => Ok(app),
            Some(app) => Err(Error::NotFound(format!(
                "{} is installed but not running{}",
                app.name,
                app.path.map(|p| format!(" ({p})")).unwrap_or_default()
            ))),
            None => Err(Error::NotFound(format!(
                "no running app matches \"{query}\"; try apps"
            ))),
        }
    }
}
