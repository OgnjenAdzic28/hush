//! Where an agent's cursor is drawn in a recording, at any moment.
//!
//! Hush acts without a pointer, so a recording has nothing to show where it
//! acted. While recording, each action leaves a mark (where, when, what kind);
//! afterwards the video gets a cursor that glides to each mark and lands
//! exactly when the action happened, leans toward its direction of travel
//! and settles upright, and dips when it presses. Pacing is real: when
//! actions are 50 ms apart the cursor gets 50 ms to travel, so the video
//! shows Hush's real speed.
//!
//! Everything here is a pure function of the marks and a time, so any frame
//! can be drawn on its own and the motion is tested without a screen.

use std::time::Duration;

/// What an action did at its mark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    /// A click: a quick press and release.
    Click,
    /// Pointed at an element without clicking (typed or set a value in it).
    Point,
    /// Pressed at the mark, moved to `to` (window points) and released at
    /// `until` (time since the recording started).
    Drag { to: (f64, f64), until: Duration },
}

/// One action in a recording, in window points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// Time since the recording started.
    pub at: Duration,
    pub x: f64,
    pub y: f64,
    pub gesture: Gesture,
}

/// How to draw the cursor in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Where the tip is, in window points.
    pub x: f64,
    pub y: f64,
    /// Clockwise lean from upright, in degrees: positive while moving right.
    pub lean: f64,
    /// 1 normally, less while pressing or appearing.
    pub scale: f64,
    pub opacity: f64,
}

/// The longest glide between two marks; shorter when the marks are closer
/// in time.
pub const GLIDE: Duration = Duration::from_millis(560);
/// How long the cursor takes to appear before its first glide.
pub const APPEAR: Duration = Duration::from_millis(300);
/// Where the cursor waits before its first glide, from the first mark.
const ENTRY: (f64, f64) = (150.0, 110.0);
/// The deepest lean, reached at fast horizontal speeds.
const MAX_LEAN: f64 = 28.0;
/// The horizontal speed (points per second) at which the lean is 76% of
/// the deepest.
const LEAN_SPEED: f64 = 900.0;
const PRESSED: f64 = 0.82;
const PRESS_DOWN: Duration = Duration::from_millis(70);
const PRESS_UP: Duration = Duration::from_millis(160);

/// The cursor at `t`, or `None` before it appears (or with no marks).
pub fn pose(marks: &[Mark], t: Duration) -> Option<Pose> {
    let first = marks.first()?;
    let appear_at = first.at.saturating_sub(GLIDE + APPEAR);
    if t < appear_at {
        return None;
    }
    let (x, y) = position(marks, t.as_secs_f64());
    let dt = 0.008;
    let s = t.as_secs_f64();
    let (before, after) = (position(marks, (s - dt).max(0.0)), position(marks, s + dt));
    let speed_x = (after.0 - before.0) / (2.0 * dt);
    let lean = MAX_LEAN * (speed_x / LEAN_SPEED).tanh();
    let appearing = ((t - appear_at).as_secs_f64() / APPEAR.as_secs_f64()).min(1.0);
    let entrance = ease_out(appearing);
    Some(Pose {
        x,
        y,
        lean,
        scale: press(marks, t) * (0.85 + 0.15 * entrance),
        opacity: entrance,
    })
}

/// The tip's position at `s` seconds: waiting, gliding to the next mark, or
/// dragging.
fn position(marks: &[Mark], s: f64) -> (f64, f64) {
    let first = &marks[0];
    let mut from = (first.x + ENTRY.0, first.y + ENTRY.1);
    let mut free_at = 0.0;
    for mark in marks {
        let arrive = mark.at.as_secs_f64();
        let glide = GLIDE.as_secs_f64().min((arrive - free_at).max(0.0));
        let start = arrive - glide;
        if s < start {
            return from;
        }
        if s < arrive {
            let u = ease_glide((s - start) / glide);
            return lerp(from, (mark.x, mark.y), u);
        }
        from = (mark.x, mark.y);
        free_at = arrive;
        if let Gesture::Drag { to, until } = mark.gesture {
            let end = until.as_secs_f64().max(arrive);
            if s < end {
                let u = smoothstep((s - arrive) / (end - arrive).max(1e-6));
                return lerp(from, to, u);
            }
            from = to;
            free_at = end;
        }
    }
    from
}

/// The press dip around clicks and during drags, as a scale factor.
fn press(marks: &[Mark], t: Duration) -> f64 {
    let mut scale: f64 = 1.0;
    for mark in marks {
        let (depth, held_until) = match mark.gesture {
            Gesture::Click => (PRESSED, mark.at + PRESS_DOWN),
            Gesture::Point => (0.93, mark.at + PRESS_DOWN),
            Gesture::Drag { until, .. } => (PRESSED, until.max(mark.at + PRESS_DOWN)),
        };
        if t < mark.at || t > held_until + PRESS_UP {
            continue;
        }
        let dip = 1.0 - depth;
        let pressed = if t < mark.at + PRESS_DOWN {
            ((t - mark.at).as_secs_f64() / PRESS_DOWN.as_secs_f64()).min(1.0)
        } else if t <= held_until {
            1.0
        } else {
            1.0 - ease_out((t - held_until).as_secs_f64() / PRESS_UP.as_secs_f64())
        };
        scale = scale.min(1.0 - dip * pressed);
    }
    scale
}

fn lerp(a: (f64, f64), b: (f64, f64), u: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u)
}

fn smoothstep(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn ease_out(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    1.0 - (1.0 - u).powi(3)
}

/// `cubic-bezier(0.22, 1, 0.36, 1)`: a fast start and a soft landing.
fn ease_glide(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let bezier = |a: f64, b: f64, s: f64| {
        let r = 1.0 - s;
        3.0 * r * r * s * a + 3.0 * r * s * s * b + s * s * s
    };
    // Find the curve parameter whose x is `u`, then read its y.
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (low + high) / 2.0;
        if bezier(0.22, 0.36, mid) < u {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(1.0, 1.0, (low + high) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn click(at: u64, x: f64, y: f64) -> Mark {
        Mark {
            at: ms(at),
            x,
            y,
            gesture: Gesture::Click,
        }
    }

    #[test]
    fn lands_on_each_mark_exactly_when_it_happened() {
        let marks = [click(2000, 100.0, 100.0), click(2400, 500.0, 300.0)];
        for mark in &marks {
            let p = pose(&marks, mark.at).unwrap();
            assert!((p.x - mark.x).abs() < 1e-6 && (p.y - mark.y).abs() < 1e-6);
        }
    }

    #[test]
    fn waits_until_its_glide_starts_and_appears_before_the_first() {
        let marks = [click(2000, 100.0, 100.0)];
        assert!(pose(&marks, ms(1000)).is_none());
        let waiting = pose(&marks, ms(1300)).unwrap();
        assert_eq!((waiting.x, waiting.y), (250.0, 210.0));
        let halfway = pose(&marks, ms(1720)).unwrap();
        assert!(halfway.x < 250.0 && halfway.x > 100.0);
    }

    #[test]
    fn leans_into_its_direction_and_settles_upright() {
        let right = [click(1000, 100.0, 100.0), click(2000, 800.0, 100.0)];
        assert!(pose(&right, ms(1550)).unwrap().lean > 10.0);
        let left = [click(1000, 800.0, 100.0), click(2000, 100.0, 100.0)];
        assert!(pose(&left, ms(1550)).unwrap().lean < -10.0);
        assert!(pose(&right, ms(2200)).unwrap().lean.abs() < 1e-6);
    }

    #[test]
    fn a_close_follow_up_gets_only_the_time_there_was() {
        // 50 ms apart: the cursor starts the second glide right after the
        // first click and still lands on time.
        let marks = [click(1000, 100.0, 100.0), click(1050, 400.0, 100.0)];
        assert!((pose(&marks, ms(1000)).unwrap().x - 100.0).abs() < 1e-3);
        let mid = pose(&marks, ms(1025)).unwrap();
        assert!(mid.x > 100.0 && mid.x < 400.0);
        assert!((pose(&marks, ms(1050)).unwrap().x - 400.0).abs() < 1e-3);
    }

    #[test]
    fn dips_on_a_click_and_springs_back() {
        let marks = [click(1000, 0.0, 0.0)];
        assert!((pose(&marks, ms(1070)).unwrap().scale - PRESSED).abs() < 1e-6);
        assert_eq!(pose(&marks, ms(1300)).unwrap().scale, 1.0);
    }

    #[test]
    fn a_drag_holds_the_press_and_carries_the_cursor() {
        let marks = [Mark {
            at: ms(1000),
            x: 10.0,
            y: 10.0,
            gesture: Gesture::Drag {
                to: (210.0, 10.0),
                until: ms(1500),
            },
        }];
        let mid = pose(&marks, ms(1250)).unwrap();
        assert!(mid.x > 10.0 && mid.x < 210.0);
        assert!((mid.scale - PRESSED).abs() < 1e-6);
        assert_eq!(pose(&marks, ms(1500)).unwrap().x, 210.0);
        assert_eq!(pose(&marks, ms(2000)).unwrap().scale, 1.0);
    }
}
