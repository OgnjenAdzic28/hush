//! `hush_core::Platform` for macOS.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use hush_core::{
    Activity, AppInfo, Error, Image, Op, Platform, Poll, Rect, Result, Rules, ScriptLanguage,
    Settle, Snapshot, WindowInfo,
};

use crate::ax::{self, Element};
use crate::script::{self, Scripts};
use crate::watch::Watcher;
use crate::{act, apps, capture, cf, read, windows};

/// How long one accessibility call may block on an app that isn't
/// answering. Long enough for a busy app, short enough that a hung one
/// fails a step instead of the whole session.
const AX_TIMEOUT_SECONDS: f32 = 1.5;

#[derive(Debug)]
pub struct MacPlatform {
    watcher: Watcher,
    /// Window elements by window id, refreshed on every `windows` call.
    windows: RefCell<HashMap<u64, Element>>,
    /// Windows on other Spaces found without `AXWindows`.
    off_space: RefCell<windows::Known>,
    scripts: RefCell<Scripts>,
    /// Chromium and Electron apps whose web content was switched on.
    web_enabled: RefCell<HashSet<i32>>,
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
            off_space: RefCell::default(),
            scripts: RefCell::default(),
            web_enabled: RefCell::default(),
        })
    }

    fn app_element(app: &AppInfo) -> Result<Element> {
        let pid = app
            .pid
            .ok_or_else(|| Error::NotFound(format!("{} isn't running", app.name)))?;
        Ok(Element::application(pid as i32))
    }

    /// Chromium and Electron build the accessibility tree of their web
    /// content only for assistive apps that ask for it, so a first read sees
    /// little more than the window buttons. `AXManualAccessibility` asks for
    /// it without the side effect of `AXEnhancedUserInterface` (window moves
    /// by other tools animate). The tree builds asynchronously; this waits
    /// until its creation events stop.
    fn enable_web_content(&self, app: &AppInfo) {
        let Some(pid) = app.pid.map(|p| p as i32) else {
            return;
        };
        if !self.web_enabled.borrow_mut().insert(pid) || !apps::is_chromium(app) {
            return;
        }
        self.watcher.watch(pid);
        let before = self.watcher.activity(pid);
        // Chromium Embedded Framework apps (Spotify) only know the older
        // switch, so it is the fallback.
        let app_element = Element::application(pid);
        // Already on (an earlier Hush process or another assistive app):
        // the tree exists, so there is nothing to wait for.
        let on = |attribute: &str| {
            app_element
                .value(attribute)
                .ok()
                .and_then(|v| cf::boolean(&v))
                == Some(true)
        };
        if on("AXManualAccessibility") || on("AXEnhancedUserInterface") {
            return;
        }
        if act::set_flag(&app_element, "AXManualAccessibility").is_err()
            && act::set_flag(&app_element, "AXEnhancedUserInterface").is_err()
        {
            return;
        }
        let rules = Rules {
            // A first switch starts a burst of creation events within tens of
            // milliseconds; no events means the tree was already there.
            reaction: Duration::from_millis(100),
            quiet: Duration::from_millis(60),
            limit: Duration::from_millis(1500),
        };
        let mut settle = Settle::begin(Instant::now(), before, rules);
        while let Poll::WaitUntil(at) = settle.poll(Instant::now(), self.watcher.activity(pid)) {
            std::thread::sleep(
                at.saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(5)),
            );
        }
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
        // From now on, windows these apps open anywhere are caught.
        for pid in found.iter().filter_map(|app| app.pid) {
            self.watcher.watch_windows(pid as i32);
        }
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
        self.enable_web_content(app);
        let pid = app.pid.map_or(0, |p| p as i32);
        self.watcher.watch_windows(pid);
        let created = self.watcher.take_created(pid);
        let listed = windows::list(
            &Self::app_element(app)?,
            pid,
            created,
            &mut self.off_space.borrow_mut(),
        );
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

    fn perform(&self, app: &AppInfo, window: &WindowInfo, op: Op<'_, Element>) -> Result<()> {
        let window_element = self.window_element(app, window)?;
        let target = act::Target {
            app,
            window,
            window_element: &window_element,
        };
        act::perform(&target, op)
    }

    fn capture(&self, window: &WindowInfo, region: Option<Rect>, max_width: u32) -> Result<Image> {
        let id = u32::try_from(window.id).map_err(|_| {
            Error::Unsupported("this window has no window-server id to capture".into())
        })?;
        capture::window(id, window.frame, region, max_width)
    }

    fn script(&self, _app: &AppInfo, source: &str, language: ScriptLanguage) -> Result<String> {
        match language {
            ScriptLanguage::AppleScript => self.scripts.borrow_mut().apple_script(source),
            ScriptLanguage::JavaScript => script::javascript(source),
        }
    }
}
