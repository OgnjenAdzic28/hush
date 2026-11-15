//! Pointer clicks and scrolling delivered to one window of one app, without
//! moving the real pointer or raising the window.
//!
//! The click recipe is the one open-computer-use measured on macOS 27 (50 of
//! 50 clicks landed exactly once on a covered Chromium window, front app
//! unchanged): a move to the target, a primer press and release at (-1, -1),
//! then the real press and release, every event stamped with the target's
//! pid and window and posted through both SkyLight and the public call.

use std::thread;
use std::time::Duration;

use hush_core::{Button, Error, Result};
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventSource, CGEventSourceStateID, CGEventType, CGMouseButton,
    CGScrollEventUnit,
};

use crate::private::{self, SyntheticFocus};

/// Gaps inside the click recipe. They can't be zero (every click failed at
/// zero in open-computer-use's sweep, all passed at a twentieth of Cua's
/// gaps); these are a fifth of Cua's, its tuned default.
const AFTER_MOVE: Duration = Duration::from_millis(3);
const AFTER_PRIMER: Duration = Duration::from_millis(20);
const BETWEEN_CLICKS: Duration = Duration::from_millis(16);
/// Keeps the synthetic focus until Chromium's renderer has taken the last
/// release, then lets go.
const RENDERER_SETTLE: Duration = Duration::from_millis(20);

/// Raw event fields used by the recipe.
mod field {
    pub const PHASE: u32 = 0;
    pub const CLICK_STATE: u32 = 1;
    pub const BUTTON: u32 = 3;
    pub const SUBTYPE: u32 = 7;
    pub const TARGET_PID: u32 = 40;
    pub const WINDOW: u32 = 51;
    pub const CLICK_GROUP: u32 = 58;
    pub const WINDOW_UNDER_POINTER: u32 = 91;
    pub const WINDOW_FOR_EVENT: u32 = 92;
}

pub(crate) struct Target {
    pub(crate) pid: i32,
    pub(crate) window: u32,
    /// The window's top-left corner on screen, points.
    pub(crate) origin: (f64, f64),
    /// The user's app is the target, so no synthetic focus is needed.
    pub(crate) frontmost: bool,
}

pub(crate) fn click(target: &Target, x: f64, y: f64, button: Button, count: u8) -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let (down, up, mouse_button, button_number) = match button {
        Button::Left => (
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGMouseButton::Left,
            0,
        ),
        Button::Right => (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGMouseButton::Right,
            1,
        ),
    };
    let group = i64::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos()),
    );
    let at = (x, y);
    let primer = (-1.0, -1.0);
    let focus = (!target.frontmost)
        .then(|| SyntheticFocus::begin(target.pid, target.window))
        .transpose()?;

    let result = (|| {
        post(
            target,
            source.as_deref(),
            CGEventType::MouseMoved,
            at,
            0,
            2,
            button_number,
            mouse_button,
            group,
        )?;
        thread::sleep(AFTER_MOVE);
        post(
            target,
            source.as_deref(),
            down,
            primer,
            1,
            1,
            button_number,
            mouse_button,
            group,
        )?;
        post(
            target,
            source.as_deref(),
            up,
            primer,
            1,
            2,
            button_number,
            mouse_button,
            group,
        )?;
        thread::sleep(AFTER_PRIMER);
        for n in 1..=i64::from(count.max(1)) {
            post(
                target,
                source.as_deref(),
                down,
                at,
                n,
                3,
                button_number,
                mouse_button,
                group,
            )?;
            post(
                target,
                source.as_deref(),
                up,
                at,
                n,
                3,
                button_number,
                mouse_button,
                group,
            )?;
            if n < i64::from(count) {
                thread::sleep(BETWEEN_CLICKS);
            }
        }
        Ok(())
    })();
    if let Some(focus) = focus {
        thread::sleep(RENDERER_SETTLE);
        focus.end();
    }
    result
}

/// Scrolls by lines at a window-relative point (positive is down/right).
pub(crate) fn scroll(target: &Target, x: f64, y: f64, dx: i32, dy: i32) -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    // Wheel deltas count up for content moving down, the opposite of "scroll down".
    let event = CGEvent::new_scroll_wheel_event2(
        source.as_deref(),
        CGScrollEventUnit::Line,
        2,
        -dy,
        -dx,
        0,
    )
    .ok_or_else(|| Error::Failed("couldn't create a scroll event".into()))?;
    CGEvent::set_location(Some(&event), screen(target, (x, y)));
    stamp(target, &event, (x, y))?;
    private::post_to_pid(target.pid, &event)?;
    CGEvent::post_to_pid(target.pid, Some(&event));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn post(
    target: &Target,
    source: Option<&CGEventSource>,
    kind: CGEventType,
    at: (f64, f64),
    click_state: i64,
    phase: i64,
    button_number: i64,
    button: CGMouseButton,
    group: i64,
) -> Result<()> {
    let event = CGEvent::new_mouse_event(source, kind, screen(target, at), button)
        .ok_or_else(|| Error::Failed("couldn't create a pointer event".into()))?;
    let set = |field: u32, value: i64| {
        CGEvent::set_integer_value_field(Some(&event), CGEventField(field), value)
    };
    set(field::PHASE, phase);
    set(field::CLICK_STATE, click_state);
    set(field::BUTTON, button_number);
    set(field::SUBTYPE, 3);
    set(field::CLICK_GROUP, group);
    stamp(target, &event, at)?;
    private::post_to_pid(target.pid, &event)?;
    CGEvent::post_to_pid(target.pid, Some(&event));
    Ok(())
}

fn stamp(target: &Target, event: &CGEvent, at: (f64, f64)) -> Result<()> {
    let window = i64::from(target.window);
    for (field, value) in [
        (field::TARGET_PID, i64::from(target.pid)),
        (field::WINDOW, window),
        (field::WINDOW_UNDER_POINTER, window),
        (field::WINDOW_FOR_EVENT, window),
    ] {
        CGEvent::set_integer_value_field(Some(event), CGEventField(field), value);
    }
    // The primer at (-1, -1) is outside the window by design on every release.
    let (x, y) = if private::macos_major() >= 27 || at == (-1.0, -1.0) {
        at
    } else {
        let p = screen(target, at);
        (p.x, p.y)
    };
    private::set_window_location(event, x, y)
}

fn screen(target: &Target, (x, y): (f64, f64)) -> CGPoint {
    if (x, y) == (-1.0, -1.0) {
        return CGPoint { x, y };
    }
    CGPoint {
        x: target.origin.0 + x,
        y: target.origin.1 + y,
    }
}
