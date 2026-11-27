//! An agent's own window of an app the user is using.
//!
//! Moving the user's window into the lane takes it off their screen. For
//! one of the user's apps (`workspace::is_users`), `lane` instead opens
//! another window of the app, the way the user would with File > New
//! Window, and parks that one. It is the same app working on the same data,
//! so what the agent does there shows up in the user's window as the app
//! syncs its own windows (a folder, a page, a project).
//!
//! The user's window keeps the keyboard. While the user is in the app,
//! AppKit gives keys and clicks to its key window, so a step that needs
//! them in the agent's window would take the user's typing; those steps
//! wait until the user switches away, and steps on refs (Accessibility
//! actions) keep working. When the user comes back to the app, the agent's
//! window stays in the lane and the user's window is made key again.

use hush_core::{AppInfo, Error, Rect, Result};
use objc2_core_foundation::CGPoint;

use super::{LIMIT, Parked, bounds, display, evict, inside, send_back, set_position, state, wait};
use crate::ax::Element;
use crate::{act, menu, private};

/// Opens another window of the user's app `pid` and parks it, leaving the
/// user's window where it is. The engine follows the new window, as it does
/// any window a step opens.
pub(crate) fn give_own(pid: i32, name: &str) -> Result<()> {
    if private::front_pid() == Some(pid) {
        return Err(Error::Unsupported(format!(
            "the user is in {name} right now, and a new window would take their typing; \
             run lane again once they switch away"
        )));
    }
    if let Some(title) = own_windows(pid).first().map(title) {
        return Err(Error::Unsupported(format!(
            "your own {name} window \"{title}\" is already in the lane; act on that one"
        )));
    }
    let app = Element::application(pid);
    let entries = menu::catalog(&app)?;
    let item = hush_core::menus::new_window(&entries).ok_or_else(|| {
        Error::Unsupported(format!(
            "{name} is the user's and has no New Window item, so Hush won't take their \
             window; act on it where it is (Hush works in the background) or ask the user"
        ))
    })?;
    let before = app.elements("AXWindows");
    menu::press_path(&app, &item.path)?;
    let mut opened = None;
    wait(LIMIT, || {
        opened = app
            .elements("AXWindows")
            .into_iter()
            .find(|window| !before.contains(window));
        opened.is_some()
    });
    let window = opened.ok_or_else(|| {
        Error::Failed(format!(
            "{name} didn't open a window after {}",
            item.path.join(" > ")
        ))
    })?;
    park(pid, &window)
}

/// Whether `window` is in the lane.
pub(crate) fn holds(window: &Element) -> bool {
    super::in_use().is_some_and(|bounds| inside(window, bounds))
}

/// Refuses a step that reaches `window` through its app's key window (keys,
/// pointer input, menus) while the user is in the app and the window is in
/// the lane: the step would land in the user's window (see the module
/// comment).
pub(crate) fn check_keyboard(app: &AppInfo, window: &Element) -> Result<()> {
    let pid = app.pid.map(|pid| pid as i32);
    if pid.is_some() && pid == private::front_pid() && holds(window) {
        return Err(Error::Unsupported(format!(
            "the user is in {} right now, so its keyboard, pointer and menus go to their \
             window, not this one in the lane; until they switch away, use steps on refs \
             (click, set, pick)",
            app.name
        )));
    }
    Ok(())
}

fn park(pid: i32, window: &Element) -> Result<()> {
    let bounds = bounds()?;
    let mut guard = state()?;
    let lane = guard.as_mut().expect("bounds() made the lane");
    // Each lane window starts a step down and right of the last.
    let step = 30.0 * (lane.parked.len() % 10) as f64;
    let at = CGPoint {
        x: bounds.x + 40.0 + step,
        y: bounds.y + 40.0 + step,
    };
    lane.parked.push(Parked {
        pid,
        window: window.clone(),
        // `unlane` shows it to the user at the same place on the main display.
        from: super::onto(at, bounds, display::main()),
        own: true,
    });
    drop(guard);
    set_position(window, at)?;
    if wait(LIMIT, || inside(window, bounds)) {
        Ok(())
    } else {
        Err(Error::Failed(
            "the app moved its new window back out of the lane".into(),
        ))
    }
}

/// The user switched to `pid` (and it is in front). Its windows in the lane
/// come back, except the agents' own while the user has a window of the app
/// on their screen; with none left there, those come back too, since that is
/// what the user went to the app to see.
pub(super) fn user_switched_to(pid: i32, bounds: Rect) {
    let app = Element::application(pid);
    let windows = app.elements("AXWindows");
    let own = own_windows(pid);
    let theirs: Vec<&Element> = windows
        .iter()
        .filter(|window| !own.contains(window) && !inside(window, bounds))
        .collect();
    let keep = !theirs.is_empty();
    let _ = send_back(|p| p.pid == pid && !(keep && p.own));
    let others: Vec<Element> = windows
        .iter()
        .filter(|window| !(keep && own.contains(window)))
        .cloned()
        .collect();
    let _ = evict(&others, bounds);
    // An agent's step made its window key inside the app; the user's typing
    // goes to their own window, the front one on their screen.
    if keep
        && let Some(key) = app.element("AXFocusedWindow")
        && own.contains(&key)
        && let Some(front) = theirs.first()
    {
        let _ = act::perform_action(front, "AXRaise");
    }
}

fn own_windows(pid: i32) -> Vec<Element> {
    let Ok(lane) = state() else {
        return Vec::new();
    };
    lane.as_ref()
        .map(|lane| {
            lane.parked
                .iter()
                .filter(|p| p.pid == pid && p.own)
                .map(|p| p.window.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn title(window: &Element) -> String {
    window.string("AXTitle").unwrap_or_default()
}
