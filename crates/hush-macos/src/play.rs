//! Playing a timeline: key and pointer events on a millisecond schedule,
//! posted to one process the same way single steps are.
//!
//! The schedule runs on its own real-time thread (see `real_time`),
//! sleeping until shortly before each event and spinning the rest of the
//! way, so an event is posted within a fraction of a millisecond of its time
//! instead of whenever the scheduler wakes a sleeping thread (often 1 to 5
//! ms late on macOS). Synthetic focus is taken once for the whole timeline.

use std::thread;
use std::time::{Duration, Instant};

use hush_core::{Error, Input, Played, Result, Timed};
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID};

use crate::act::{self, Target};
use crate::ax::Element;
use crate::pointer::{self, Motion};
use crate::private::SyntheticFocus;
use crate::{apps, keys};

/// Time between finishing setup and the first event, so the first event
/// isn't late by the setup itself.
const LEAD_IN: Duration = Duration::from_millis(20);
/// Sleep until this long before an event, then spin. Longer than the
/// scheduler's usual wake-up delay for a user-interactive thread.
const SPIN: Duration = Duration::from_micros(1500);

pub(crate) fn play(target: &Target<'_>, events: &[Timed]) -> Result<Played> {
    let pointer = target.pointer()?;
    let has_pointer = events.iter().any(|e| e.input.is_pointer());
    let has_keys = events.iter().any(|e| !e.input.is_pointer());
    if has_pointer {
        pointer::check(&pointer)?;
    }
    for event in events {
        if let Input::KeyDown(combo) | Input::KeyUp(combo) = event.input
            && keys::code(combo.key).is_none()
        {
            return Err(Error::Unsupported(format!("no key code for {combo}")));
        }
    }

    let focus = (!target.app.frontmost)
        .then(|| SyntheticFocus::begin(pointer.pid, pointer.window))
        .transpose()?;
    let result = (|| {
        // Chromium takes keys only in the window it considers key.
        if let Some(focus) = &focus
            && has_keys
            && apps::is_chromium(target.app)
        {
            focus.make_key()?;
            let app = Element::application(pointer.pid);
            if !act::wait_for_key_window(&app, target.window_element) {
                return Err(Error::Unsupported(format!(
                    "couldn't make the window key inside {} without bringing it to the front",
                    target.app.name
                )));
            }
        }
        if has_pointer && focus.is_some() {
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
            pointer::prime(&pointer, source.as_deref())?;
        }
        thread::scope(|scope| {
            thread::Builder::new()
                .name("hush-timeline".into())
                .spawn_scoped(scope, || schedule(&pointer, events))
                .map_err(|e| Error::Failed(format!("couldn't start the timeline thread: {e}")))?
                .join()
                .map_err(|_| Error::Failed("the timeline thread panicked".into()))?
        })
    })();
    if let Some(focus) = focus {
        pointer::settle();
        focus.end();
    }
    result
}

fn schedule(pointer: &pointer::Target, events: &[Timed]) -> Result<Played> {
    real_time();
    let keys_source = CGEventSource::new(CGEventSourceStateID::Private);
    let pointer_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let mut held = None;
    let mut late = Vec::with_capacity(events.len());
    let start = Instant::now() + LEAD_IN;
    for event in events {
        let due = start + event.at;
        wait_until(due);
        match event.input {
            Input::KeyDown(combo) => {
                keys::post_key(pointer.pid, keys_source.as_deref(), combo, true);
            }
            Input::KeyUp(combo) => {
                keys::post_key(pointer.pid, keys_source.as_deref(), combo, false);
            }
            Input::PointerDown { x, y, button } => {
                held = Some(button);
                let source = pointer_source.as_deref();
                pointer::post_motion(pointer, source, Motion::Down, button, (x, y))?;
            }
            Input::PointerUp { x, y, button } => {
                held = None;
                let source = pointer_source.as_deref();
                pointer::post_motion(pointer, source, Motion::Up, button, (x, y))?;
            }
            Input::PointerMove { x, y } => {
                let motion = Motion::Move { held };
                let button = held.unwrap_or(hush_core::Button::Left);
                let source = pointer_source.as_deref();
                pointer::post_motion(pointer, source, motion, button, (x, y))?;
            }
        }
        late.push(Instant::now().saturating_duration_since(due));
    }
    Ok(Played { late })
}

/// Makes this thread a time-constraint (real-time) thread, the policy audio
/// threads use: its sleeps end on time instead of being coalesced with
/// other timers, which in a background process made wake-ups 2.7 ms late
/// on average and 9 ms at worst (macOS 27.2, user-interactive QoS alone).
// `mach_timebase_info` is stable; libc only deprecates it in favor of the
// mach2 crate, which isn't worth a dependency for one call.
#[allow(deprecated)]
fn real_time() {
    let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: valid out-pointer.
    unsafe { libc::mach_timebase_info(&mut timebase) };
    if timebase.numer == 0 {
        return;
    }
    let ticks = |d: Duration| {
        (d.as_nanos() as u64 * u64::from(timebase.denom) / u64::from(timebase.numer)) as u32
    };
    let mut policy = libc::thread_time_constraint_policy {
        period: 0,
        // Posting one event takes about 0.2 ms.
        computation: ticks(Duration::from_micros(500)),
        constraint: ticks(Duration::from_millis(1)),
        preemptible: 1,
    };
    // SAFETY: the policy struct matches the flavor and count; the thread
    // port is this thread's own.
    unsafe {
        libc::thread_policy_set(
            libc::pthread_mach_thread_np(libc::pthread_self()),
            libc::THREAD_TIME_CONSTRAINT_POLICY as _,
            (&raw mut policy).cast(),
            libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        );
    }
}

fn wait_until(due: Instant) {
    loop {
        let now = Instant::now();
        if now >= due {
            return;
        }
        let left = due - now;
        if left > SPIN {
            thread::sleep(left - SPIN);
        } else {
            std::hint::spin_loop();
        }
    }
}
