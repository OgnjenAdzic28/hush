//! `hush_core::Platform` for macOS.

use std::cell::RefCell;
use std::collections::HashMap;

use hush_core::{
    Activity, AppInfo, Error, Image, Op, Platform, Rect, Result, ScriptLanguage, Snapshot,
    WindowInfo,
};

use crate::ax::{self, Element};
use crate::script::{self, Scripts};
use crate::watch::Watcher;
use crate::{act, apps, read, windows};

/// How long one accessibility call may block on an app that isn't
/// answering. Long enough for a busy app, short enough that a hung one
/// fails a step instead of the whole session.
const AX_TIMEOUT_SECONDS: f32 = 1.5;

#[derive(Debug)]
pub struct MacPlatform {
    watcher: Watcher,
    /// Window elements by window id, refreshed on every `windows` call.
    windows: RefCell<HashMap<u64, Element>>,
    scripts: RefCell<Scripts>,
}

impl MacPlatform {
    /// Fails with a permission error, naming the setting, when this process
    /// isn't allowed to use Accessibility.
    pub fn new() -> Result<MacPlatform> {
        if !ax::trusted() {
            return Err(Error::Permission(
                "Hush needs Accessibility access. Turn it on for the app running Hush (your terminal \
                 or agent) in System Settings > Privacy & Security > Accessibility."
                    .into(),
            ));
        }
        Element::system_wide().set_timeout(AX_TIMEOUT_SECONDS);
        Ok(MacPlatform {
            watcher: Watcher::start(),
            windows: RefCell::default(),
            scripts: RefCell::default(),
        })
    }

    fn app_element(app: &AppInfo) -> Result<Element> {
        let pid = app
            .pid
            .ok_or_else(|| Error::NotFound(format!("{} isn't running", app.name)))?;
        Ok(Element::application(pid as i32))
    }

    fn window_element(&self, app: &AppInfo, window: &WindowInfo) -> Result<Element> {
        if let Some(element) = self.windows.borrow().get(&window.id) {
            return Ok(element.clone());
        }
        self.windows(app)?;
        self.windows
            .borrow()
            .get(&window.id)
            .cloned()
            .ok_or_else(|| Error::Stale(format!("window \"{}\" is gone", window.title)))
    }
}

impl Platform for MacPlatform {
    type Handle = Element;

    fn apps(&self, query: Option<&str>) -> Result<Vec<AppInfo>> {
        let mut found = apps::running();
        if let Some(query) = query {
            let wanted = query.to_lowercase();
            found.retain(|app| {
                app.name.to_lowercase().contains(&wanted)
                    || app.id.as_deref().is_some_and(|id| id.contains(&wanted))
            });
            for installed in apps::installed(query) {
                if !found.iter().any(|app| app.path == installed.path) {
                    found.push(installed);
                }
            }
        }
        Ok(found)
    }

    fn windows(&self, app: &AppInfo) -> Result<Vec<WindowInfo>> {
        let listed = windows::list(&Self::app_element(app)?);
        let mut cache = self.windows.borrow_mut();
        cache.clear();
        Ok(listed
            .into_iter()
            .map(|(info, element)| {
                cache.insert(info.id, element);
                info
            })
            .collect())
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<Element>> {
        let element = self.window_element(app, window)?;
        if let Some(pid) = app.pid {
            self.watcher.watch(pid as i32);
        }
        let menus = read::menu_titles(&Self::app_element(app)?);
        Ok(read::window(app, window, &element, menus))
    }

    fn activity(&self, app: &AppInfo) -> Activity {
        match app.pid {
            Some(pid) => {
                self.watcher.watch(pid as i32);
                self.watcher.activity(pid as i32)
            }
            None => Activity::default(),
        }
    }

    fn perform(&self, app: &AppInfo, _window: &WindowInfo, op: Op<'_, Element>) -> Result<()> {
        act::perform(app, op)
    }

    fn capture(
        &self,
        _window: &WindowInfo,
        _region: Option<Rect>,
        _max_width: u32,
    ) -> Result<Image> {
        Err(Error::Unsupported(
            "screenshots on macOS aren't built yet".into(),
        ))
    }

    fn script(&self, _app: &AppInfo, source: &str, language: ScriptLanguage) -> Result<String> {
        match language {
            ScriptLanguage::AppleScript => self.scripts.borrow_mut().apple_script(source),
            ScriptLanguage::JavaScript => script::javascript(source),
        }
    }
}
