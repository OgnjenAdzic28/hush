//! `hush_core::Platform` for macOS.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use hush_core::timeline::Input;
use hush_core::{
    Activity, AppInfo, Error, Image, MenuEntry, Op, Platform, Played, Poll, Rect, Result, Rules,
    ScriptLanguage, Settle, Snapshot, Timed, WindowInfo,
};

use crate::ax::{self, Element};
use crate::script;
use crate::seen::{self, Handle};
use crate::watch::Watcher;
use crate::{act, apps, capture, cf, menu, play, read, windows};

/// How long one accessibility call may block on an app that isn't
/// answering. Long enough for a busy app, short enough that a hung one
/// fails a step instead of the whole session.
const AX_TIMEOUT_SECONDS: f32 = 1.5;

/// One platform serves every session in the process, each on its own
/// thread, so its state sits behind locks held only for map lookups.
#[derive(Debug)]
pub struct MacPlatform {
    watcher: Watcher,
    /// Window elements by window id with their app's pid, refreshed for one
    /// app on each of its `windows` calls.
    windows: Mutex<HashMap<u64, (i32, Element)>>,
    /// Windows on other Spaces found without `AXWindows`.
    off_space: Mutex<windows::Known>,
    /// Chromium and Electron apps whose web content was switched on.
    web_enabled: Mutex<HashSet<i32>>,
    /// One lock per app for input: two sessions acting on the same app take
    /// turns, so their clicks and keys never interleave. Different apps run
    /// at the same time.
    acting: Mutex<HashMap<i32, Arc<Mutex<()>>>>,
}

/// Locks `mutex`, ignoring poisoning: a session that panicked mid-call left
/// at worst a stale cache entry, which the next refresh replaces.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
            windows: Mutex::default(),
            off_space: Mutex::default(),
            web_enabled: Mutex::default(),
            acting: Mutex::default(),
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
        if !lock(&self.web_enabled).insert(pid) || !apps::is_chromium(app) {
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
        if let Some((_, element)) = lock(&self.windows).get(&window.id) {
            return Ok(element.clone());
        }
        self.windows(app)?;
        lock(&self.windows)
            .get(&window.id)
            .map(|(_, element)| element.clone())
            .ok_or_else(|| Error::Stale(format!("window \"{}\" is gone", window.title)))
    }

    /// Held for the whole of one action or timeline on the app.
    fn acting_on(&self, app: &AppInfo) -> Arc<Mutex<()>> {
        let pid = app.pid.map_or(0, |p| p as i32);
        Arc::clone(lock(&self.acting).entry(pid).or_default())
    }
}

impl Drop for MacPlatform {
    fn drop(&mut self) {
        crate::lane::release();
    }
}

impl Platform for MacPlatform {
    type Handle = Handle;

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
        // Listing is many round trips to the app, so it works on a copy of
        // this app's entries: a slow app never holds up other sessions.
        let mut known: windows::Known = lock(&self.off_space)
            .iter()
            .filter(|((owner, _), _)| *owner == pid)
            .map(|(key, element)| (*key, element.clone()))
            .collect();
        let listed = windows::list(&Self::app_element(app)?, pid, created, &mut known);
        {
            let mut off_space = lock(&self.off_space);
            off_space.retain(|(owner, _), _| *owner != pid);
            off_space.extend(known);
        }
        let mut cache = lock(&self.windows);
        cache.retain(|_, (owner, _)| *owner != pid);
        Ok(listed
            .into_iter()
            .map(|(info, element)| {
                cache.insert(info.id, (pid, element));
                info
            })
            .collect())
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<Handle>> {
        let element = self.window_element(app, window)?;
        if let Some(pid) = app.pid {
            self.watcher.watch(pid as i32);
        }
        let menus = read::menu_titles(&Self::app_element(app)?);
        Ok(seen::with_pixels(
            read::window(app, window, &element, menus),
            window,
        ))
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

    fn perform(&self, app: &AppInfo, window: &WindowInfo, op: Op<'_, Handle>) -> Result<()> {
        if matches!(op, Op::Lane) {
            let pid = app
                .pid
                .map(|p| p as i32)
                .ok_or_else(|| Error::NotFound(format!("{} isn't running", app.name)))?;
            // Main window first: it is the one raised, so it stays main.
            let order = self.windows(app)?;
            let cache = lock(&self.windows);
            let elements: Vec<Element> = order
                .iter()
                .filter_map(|info| cache.get(&info.id).map(|(_, element)| element.clone()))
                .collect();
            drop(cache);
            return crate::lane::park(pid, &elements);
        }
        let window_element = self.window_element(app, window)?;
        let target = act::Target {
            app,
            window,
            window_element: &window_element,
        };
        let acting = self.acting_on(app);
        let _turn = lock(&acting);
        act::perform(&target, seen::to_ax(op)?)
    }

    fn capture(&self, window: &WindowInfo, region: Option<Rect>, max_width: u32) -> Result<Image> {
        let id = u32::try_from(window.id).map_err(|_| {
            Error::Unsupported("this window has no window-server id to capture".into())
        })?;
        capture::window(id, window.frame, region, max_width)
    }

    fn menus(&self, app: &AppInfo) -> Result<Vec<MenuEntry>> {
        menu::catalog(&Self::app_element(app)?)
    }

    fn play(&self, app: &AppInfo, window: &WindowInfo, events: &[Timed]) -> Result<Played> {
        let window_element = self.window_element(app, window)?;
        let target = act::Target {
            app,
            window,
            window_element: &window_element,
        };
        let acting = self.acting_on(app);
        let _turn = lock(&acting);
        // A press may start a drag-and-drop, whose drop goes to the window
        // on top at the drop point.
        if events
            .iter()
            .any(|e| matches!(e.input, Input::PointerDown { .. }))
        {
            crate::lane::to_top(&window_element)?;
        }
        play::play(&target, events)
    }

    fn record_start(&self, window: &WindowInfo) -> Result<u64> {
        let id = u32::try_from(window.id).map_err(|_| {
            Error::Unsupported("this window has no window-server id to record".into())
        })?;
        crate::record::start(id, window.frame)
    }

    fn record_stop(
        &self,
        id: u64,
        marks: &[hush_core::cursor::Mark],
        label: &str,
        out: &std::path::Path,
    ) -> Result<String> {
        crate::record::stop(id, marks, label, out)
    }

    fn script(&self, _app: &AppInfo, source: &str, language: ScriptLanguage) -> Result<String> {
        match language {
            ScriptLanguage::AppleScript => script::apple_script(source),
            ScriptLanguage::JavaScript => script::javascript(source),
        }
    }
}

// Every session in the process shares one platform.
const _: () = {
    const fn shared<T: Send + Sync>() {}
    shared::<MacPlatform>();
};
