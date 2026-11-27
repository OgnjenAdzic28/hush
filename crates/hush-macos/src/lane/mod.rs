//! The lane: a virtual display nobody can see, where an agent's apps render
//! and take input without being on the user's screen.
//!
//! A window on another Space, behind a full-screen app or in a hidden app
//! is invisible to the user but also half-dead to an agent: AppKit doesn't
//! list it, Chromium drops its page's accessibility tree, a hidden app
//! ignores clicks and a never-shown window has nothing to capture. On the
//! lane display it is on that display's current Space and unoccluded, so
//! all of that works, and the user never sees it.
//!
//! Only the window an agent asks for moves, and Hush remembers where it
//! was. It goes back there on `unlane`, when the user switches to its app
//! (they want it, so they get it), and when the process ends normally. A
//! hidden app is the exception: none of its windows are on the user's
//! screen, and showing it in the lane shows every one of them, so they all
//! move, and the app is hidden again once they are back.
//!
//! The display lives as long as the process that made it (see `display`).

mod display;
mod guard;
mod place;

use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use hush_core::{Error, Rect, Result};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_core_foundation::CGPoint;

use crate::ax::Element;
use crate::{act, apps, private};
use place::{inside, onto, position, set_position, within};

/// How long macOS may take to bring the display online, move it, or show a
/// window there. Measured: about 1 s for the display, under 100 ms for a move.
const LIMIT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(10);

#[derive(Clone)]
struct Parked {
    pid: i32,
    window: Element,
    /// Where it goes back to.
    from: CGPoint,
}

struct Lane {
    /// Holding it keeps the display alive; `None` when another Hush process
    /// (the daemon, an MCP server) owns the lane this one reuses.
    _display: Option<Retained<AnyObject>>,
    bounds: Rect,
    /// Windows this process moved here, until they go back.
    parked: Vec<Parked>,
    /// Apps that were hidden when their windows came here.
    hidden: Vec<i32>,
}

// SAFETY: the display object is only retained and released, never messaged
// after creation, and CGVirtualDisplay is thread-agnostic.
unsafe impl Send for Lane {}

static LANE: Mutex<Option<Lane>> = Mutex::new(None);

/// Moves `target` into the lane, creating the lane if needed. When the app
/// is hidden, its `others` come too (see the module comment).
pub(crate) fn park(
    pid: i32,
    target: &Element,
    others: impl FnOnce() -> Result<Vec<Element>>,
) -> Result<()> {
    let hidden = apps::is_hidden(pid);
    let mut moving = vec![target.clone()];
    if hidden {
        moving.extend(others()?.into_iter().filter(|window| window != target));
    }
    let bounds = bounds()?;
    // Read before taking the lock: a slow app answers slowly.
    let from = moving
        .iter()
        .map(|window| {
            let at = position(window)
                .ok_or_else(|| Error::Failed("couldn't read where the window is".into()))?;
            // One the app reopened in the lane has no place of its own.
            Ok(if within(at, bounds) {
                onto(at, bounds, display::main())
            } else {
                at
            })
        })
        .collect::<Result<Vec<_>>>()?;
    {
        let mut lane = state()?;
        let lane = lane.as_mut().expect("bounds() made the lane");
        for (window, from) in moving.iter().zip(from) {
            // Parked twice, a window keeps the place it had the first time.
            if !lane.parked.iter().any(|p| &p.window == window) {
                lane.parked.push(Parked {
                    pid,
                    window: window.clone(),
                    from,
                });
            }
        }
        if hidden && !lane.hidden.contains(&pid) {
            lane.hidden.push(pid);
        }
    }
    for (i, window) in moving.iter().enumerate() {
        let step = 30.0 * i as f64;
        let at = CGPoint {
            x: bounds.x + 40.0 + step,
            y: bounds.y + 40.0 + step,
        };
        set_position(window, at)?;
    }
    if hidden {
        // `AXHidden` false would call `-[NSApplication unhide:]`, which
        // activates; ordering a window in calls `unhideWithoutActivation`.
        act::perform_action(target, "AXRaise")?;
        if !wait(LIMIT, || !apps::is_hidden(pid)) {
            return Err(Error::Failed(
                "the app stayed hidden; it can't be shown in the lane without activating it".into(),
            ));
        }
    }
    if !wait(LIMIT, || moving.iter().all(|window| inside(window, bounds))) {
        return Err(Error::Failed(
            "the app moved its window back out of the lane".into(),
        ));
    }
    Ok(())
}

/// Sends `window` back to where it was, with the rest of its app when the
/// app was hidden (it is hidden again). A window in the lane that Hush
/// didn't move here, which an app reopens where it last was, goes to the
/// same place on the main display.
pub(crate) fn restore(pid: i32, window: &Element) -> Result<()> {
    let Some(bounds) = in_use() else {
        return Ok(());
    };
    let whole_app = state()?
        .as_ref()
        .is_some_and(|lane| lane.hidden.contains(&pid));
    if whole_app {
        send_back(|p| p.pid == pid)?;
    } else {
        send_back(|p| &p.window == window)?;
    }
    evict(std::slice::from_ref(window), bounds)
}

/// The user switched to `pid`, so its windows in the lane come back and the
/// app stays shown. Called on an observer thread, once by NSWorkspace and
/// once by the app's own observer when Hush watches it; the moving happens
/// on another thread, once, since it waits on the app.
pub(crate) fn user_switched_to(pid: i32) {
    let Some(bounds) = in_use() else { return };
    {
        let mut switching = lock(&SWITCHING);
        if switching.contains(&pid) {
            return;
        }
        switching.push(pid);
    }
    thread::spawn(move || {
        // Hush's own synthetic focus (`private::SyntheticFocus`) makes an
        // app believe it is active without the window server putting it in
        // front; only a real switch puts it there. Not the front window:
        // `to_top` raises lane windows above the user's. The deadline covers
        // the window server catching up after the app hears it is active.
        if wait(SWITCH, || private::front_pid() == Some(pid)) {
            if let Ok(mut lane) = state()
                && let Some(lane) = lane.as_mut()
            {
                lane.hidden.retain(|&hidden| hidden != pid);
            }
            let _ = send_back(|p| p.pid == pid);
            let _ = evict(&Element::application(pid).elements("AXWindows"), bounds);
        }
        lock(&SWITCHING).retain(|&other| other != pid);
    });
}

/// Apps whose switch is being handled.
static SWITCHING: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// `pid` opened `window`. One that opens in the lane while its app is the
/// user's front app and has no window parked there is the user's: an app
/// they reopened put it back where it was when they quit, which was here.
/// It moves to the main display. Called on the observer thread.
pub(crate) fn window_opened(pid: i32, window: &Element) {
    let Some(bounds) = in_use() else { return };
    let window = window.clone();
    thread::spawn(move || {
        let agents = state()
            .ok()
            .and_then(|lane| {
                lane.as_ref()
                    .map(|lane| lane.parked.iter().any(|p| p.pid == pid))
            })
            .unwrap_or(false);
        if !agents && private::front_pid() == Some(pid) && inside(&window, bounds) {
            let _ = evict(std::slice::from_ref(&window), bounds);
        }
    });
}

/// How long after an app hears it is active the window server may take to
/// put it in front.
const SWITCH: Duration = Duration::from_millis(500);

/// Puts `window` above the other windows in the lane, when it's there.
///
/// Every app's windows go to the same corner of the lane, so they overlap,
/// and macOS drops a drag-and-drop on whatever window is at the drop point:
/// an agent's drag in one app landed in another app's window on top of it.
/// Clicks and keys go to their window directly and don't need this. Raising
/// a window orders it front without activating its app; a window outside the
/// lane is left alone, since raising it would put it over the user's.
pub(crate) fn to_top(window: &Element) -> Result<()> {
    let Some(bounds) = state()?.as_ref().map(|lane| lane.bounds) else {
        return Ok(());
    };
    if inside(window, bounds) {
        act::perform_action(window, "AXRaise")?;
    }
    Ok(())
}

/// Sends every window back before the display goes away with the process.
pub(crate) fn release() {
    let _ = send_back(|_| true);
}

/// Moves the picked parked windows back, forgets the ones that left the
/// lane, and hides again each hidden app whose windows are all back.
fn send_back(pick: impl Fn(&Parked) -> bool) -> Result<()> {
    let (going, bounds) = match state()?.as_ref() {
        Some(lane) => (
            lane.parked
                .iter()
                .filter(|p| pick(p))
                .cloned()
                .collect::<Vec<_>>(),
            lane.bounds,
        ),
        None => return Ok(()),
    };
    for parked in &going {
        // One the user or its app already moved out stays where it is now.
        if inside(&parked.window, bounds) {
            let _ = set_position(&parked.window, parked.from);
        }
    }
    let all_out = wait(LIMIT, || going.iter().all(|p| !inside(&p.window, bounds)));
    let out: Vec<&Parked> = going
        .iter()
        .filter(|p| !inside(&p.window, bounds))
        .collect();
    let rehide = {
        let mut lane = state()?;
        let Some(lane) = lane.as_mut() else {
            return Ok(());
        };
        lane.parked
            .retain(|kept| !out.iter().any(|p| p.window == kept.window));
        let parked = &lane.parked;
        let (done, waiting) = lane
            .hidden
            .iter()
            .partition::<Vec<i32>, _>(|&&pid| !parked.iter().any(|p| p.pid == pid));
        lane.hidden = waiting;
        done
    };
    for pid in rehide {
        // Hiding never activates anything.
        let _ = act::set_bool(&Element::application(pid), "AXHidden", true);
    }
    if all_out {
        Ok(())
    } else {
        Err(Error::Failed(
            "the app kept its window in the lane; it stays parked".into(),
        ))
    }
}

/// Moves whichever of `windows` are in the lane onto the main display.
fn evict(windows: &[Element], bounds: Rect) -> Result<()> {
    let main = display::main();
    let stuck: Vec<&Element> = windows.iter().filter(|w| inside(w, bounds)).collect();
    for window in &stuck {
        if let Some(at) = position(window) {
            let _ = set_position(window, onto(at, bounds, main));
        }
    }
    if wait(LIMIT, || stuck.iter().all(|w| !inside(w, bounds))) {
        Ok(())
    } else {
        Err(Error::Failed("the app kept its window in the lane".into()))
    }
}

/// The lane's frame in global screen coordinates (top-left origin), made
/// on first use.
fn bounds() -> Result<Rect> {
    let mut lane = state()?;
    if let Some(lane) = lane.as_ref() {
        return Ok(lane.bounds);
    }
    let (display, bounds) = match display::existing() {
        Some(id) => (None, display::frame(id)),
        None => {
            let (display, id) = display::create()?;
            let bounds = display::place(id)?;
            // The process that made the display guards it, for as long as
            // the display lives.
            guard::start(id);
            (Some(display), bounds)
        }
    };
    *lane = Some(Lane {
        _display: display,
        bounds,
        parked: Vec::new(),
        hidden: Vec::new(),
    });
    Ok(bounds)
}

/// The lane's frame when this or another Hush process made it, without
/// making one.
fn in_use() -> Option<Rect> {
    let made = state().ok()?.as_ref().map(|lane| lane.bounds);
    made.or_else(|| display::existing().map(display::frame))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn state() -> Result<MutexGuard<'static, Option<Lane>>> {
    LANE.lock().map_err(|_| Error::Failed("lane lock".into()))
}

fn wait(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    while !done() {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
    true
}
