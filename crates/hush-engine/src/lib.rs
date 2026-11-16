//! The platform-neutral engine behind every Hush tool. Front ends (MCP, CLI)
//! pass strings in and get strings or images back; everything an agent
//! reads is produced here.

mod act;
mod mirror;
mod record;
mod route;
mod target;
mod timeline;
mod wait;

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use hush_core::{
    AppInfo, Diff, Error, Image, OutlineOptions, Pace, Platform, Ref, Result, ScriptLanguage,
    WindowInfo,
};

use mirror::Mirror;

/// Token budget for a full outline before its largest parts are folded
/// (about one screenshot's worth). Folded parts can be opened with `focus`.
pub const OUTLINE_BUDGET: usize = 1500;

#[derive(Clone, Debug, Default)]
pub struct ObserveOptions<'a> {
    /// A window title (or part of one); the app's main window otherwise.
    pub window: Option<&'a str>,
    /// Only this element and what's inside it, unfolded.
    pub focus: Option<Ref>,
    /// The whole outline even when only a little changed.
    pub full: bool,
}

pub struct Engine<P: Platform> {
    platform: P,
    mirrors: HashMap<(u32, u64), Mirror<P::Handle>>,
    paces: HashMap<u32, Pace>,
    options: OutlineOptions,
    recording: Option<record::Recording>,
}

impl<P: Platform> std::fmt::Debug for Engine<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("windows", &self.mirrors.len())
            .finish_non_exhaustive()
    }
}

impl<P: Platform> Engine<P> {
    pub fn new(platform: P) -> Engine<P> {
        Engine {
            platform,
            mirrors: HashMap::new(),
            paces: HashMap::new(),
            options: OutlineOptions::default(),
            recording: None,
        }
    }

    /// One line per app: name, whether it runs, and where it's installed.
    pub fn apps(&self, query: Option<&str>) -> Result<String> {
        let apps = self.platform.apps(query)?;
        if apps.is_empty() {
            return Ok(match query {
                Some(q) => format!("no app matches \"{q}\""),
                None => "no apps with windows are running".into(),
            });
        }
        let mut out = String::new();
        for app in apps {
            let state = match (app.pid, app.frontmost) {
                (Some(_), true) => "running · front (the user's app)".to_owned(),
                (Some(_), false) => "running".to_owned(),
                (None, _) => format!("installed · {}", app.path.as_deref().unwrap_or("")),
            };
            out.push_str(&format!("{} · {state}\n", app.name));
        }
        Ok(out)
    }

    /// The outline of a window the first time, then only what changed.
    pub fn observe(&mut self, app: &str, options: ObserveOptions<'_>) -> Result<String> {
        let (app, window) = self.target(app, options.window)?;
        let snapshot = self.platform.read(&app, &window)?;
        let (mirror, first_look) = match self.mirrors.entry(key(&app, &window)) {
            Entry::Occupied(entry) => {
                let mirror = entry.into_mut();
                mirror.update(snapshot, &self.options);
                (mirror, false)
            }
            Entry::Vacant(entry) => (entry.insert(Mirror::new(snapshot, &self.options)), true),
        };

        let text = if let Some(r) = options.focus {
            let view = mirror.current.subtree(r).ok_or_else(|| {
                Error::Stale(format!("{r} isn't in the window any more; observe again"))
            })?;
            view.render()
        } else if first_look || options.full {
            mirror.current.render_within(OUTLINE_BUDGET)
        } else {
            let diff = Diff::between(&mirror.seen, &mirror.current);
            if diff.is_empty() {
                format!("# {} · no change since the last look\n", app.name)
            } else if diff.replaces(&mirror.current) {
                mirror.current.render_within(OUTLINE_BUDGET)
            } else {
                diff.render()
            }
        };
        // One part of the window says nothing about the rest, so the next
        // full observe still diffs against what was last seen whole.
        if options.focus.is_none() {
            mirror.mark_seen();
        }
        Ok(text)
    }

    /// Every menu command of the app with its shortcut, one line per menu.
    pub fn menus(&mut self, app: &str) -> Result<String> {
        let (app, _) = self.target(app, None)?;
        let entries = self.platform.menus(&app)?;
        Ok(hush_core::menus::render(&app.name, &entries))
    }

    /// A screenshot of the window, or of one element in it.
    pub fn see(
        &mut self,
        app: &str,
        window: Option<&str>,
        element: Option<Ref>,
        max_width: u32,
    ) -> Result<Image> {
        let (app, window) = self.target(app, window)?;
        let region = match element {
            None => None,
            Some(r) => {
                let mirror = self.mirrors.get(&key(&app, &window)).ok_or_else(|| {
                    Error::Stale(format!("observe the window before asking for {r}"))
                })?;
                let node = mirror
                    .node(r)
                    .ok_or_else(|| Error::Stale(format!("{r} isn't in the window any more")))?;
                Some(
                    node.frame
                        .ok_or_else(|| Error::Unsupported(format!("{r} has no frame")))?,
                )
            }
        };
        self.platform.capture(&window, region, max_width)
    }

    pub fn script(&mut self, app: &str, source: &str, language: ScriptLanguage) -> Result<String> {
        let apps = self.platform.apps(Some(app))?;
        let info = apps.into_iter().next().unwrap_or(AppInfo {
            name: app.to_owned(),
            pid: None,
            id: None,
            path: None,
            frontmost: false,
        });
        self.platform.script(&info, source, language)
    }

    pub fn platform(&self) -> &P {
        &self.platform
    }
}

fn key(app: &AppInfo, window: &WindowInfo) -> (u32, u64) {
    (app.pid.unwrap_or(0), window.id)
}

#[cfg(test)]
mod tests;
