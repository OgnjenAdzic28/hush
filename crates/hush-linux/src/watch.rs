//! Counting each app's accessibility events, which is how Hush knows when an
//! app has finished reacting (see `hush_core::settle`).
//!
//! One background thread reads every AT-SPI event signal on the bus and
//! bumps a counter for the app that sent it. Toolkits built on ATK only emit
//! the events some client has registered for, so the watcher registers for
//! all of them first.
//!
//! Elements that change on their own all the time (spinners, pulsing
//! progress bars, clocks) would keep an app from ever looking settled, so an
//! element that has sent events in three of the last four seconds stops
//! counting until it goes quiet. A change a step causes is a burst well
//! under that.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use futures_util::StreamExt;
use hush_core::Activity;
use zbus::message::Type as MessageType;
use zbus::{Connection, MessageStream};

/// The AT-SPI event interfaces. Their signals are what "the UI changed"
/// means: children added or removed, a property, state or text changed,
/// focus moved, a window opened or closed.
const INTERFACES: [&str; 4] = [
    "org.a11y.atspi.Event.Object",
    "org.a11y.atspi.Event.Window",
    "org.a11y.atspi.Event.Focus",
    "org.a11y.atspi.Event.Document",
];

type Counts = Arc<Mutex<HashMap<String, Activity>>>;

/// Seconds (since the watch started) in which one element sent events: its
/// last four distinct ones, newest last.
#[derive(Default)]
struct Recent {
    seconds: [u64; 4],
    len: usize,
}

impl Recent {
    /// Notes an event in `second` and says whether the element is noise.
    fn noisy(&mut self, second: u64) -> bool {
        if self.len == 0 || self.seconds[(self.len - 1).min(3)] != second {
            if self.len == 4 {
                self.seconds.rotate_left(1);
                self.seconds[3] = second;
            } else {
                self.seconds[self.len] = second;
                self.len += 1;
            }
        }
        let window = second.saturating_sub(3);
        self.seconds[..self.len]
            .iter()
            .filter(|s| **s >= window)
            .count()
            >= 3
    }
}

#[derive(Debug)]
pub(crate) struct Watch {
    /// Events by the sending app's unique bus name.
    counts: Counts,
}

impl Watch {
    /// Starts watching. Returns once the match rules are in place, so no
    /// event sent after this call is missed.
    pub(crate) async fn start(connection: &Connection) -> zbus::Result<Watch> {
        for interface in INTERFACES {
            connection
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "AddMatch",
                    &format!("type='signal',interface='{interface}'"),
                )
                .await?;
        }
        for event in ["object:", "window:", "focus:", "document:"] {
            // Older registries don't have this; their toolkits emit
            // everything anyway.
            let _ = connection
                .call_method(
                    Some("org.a11y.atspi.Registry"),
                    "/org/a11y/atspi/registry",
                    Some("org.a11y.atspi.Registry"),
                    "RegisterEvent",
                    &event,
                )
                .await;
        }
        let stream = MessageStream::from(connection);
        let counts: Counts = Arc::default();
        let counting = counts.clone();
        std::thread::Builder::new()
            .name("hush-atspi-events".into())
            .spawn(move || zbus::block_on(count(stream, counting)))
            .map_err(|e| zbus::Error::Failure(format!("can't start the event thread: {e}")))?;
        Ok(Watch { counts })
    }

    pub(crate) fn activity(&self, bus: &str) -> Activity {
        lock(&self.counts).get(bus).copied().unwrap_or_default()
    }
}

async fn count(mut stream: MessageStream, counts: Counts) {
    let started = Instant::now();
    let mut recent: HashMap<(String, String), Recent> = HashMap::new();
    while let Some(message) = stream.next().await {
        let Ok(message) = message else { continue };
        if message.message_type() != MessageType::Signal {
            continue;
        }
        let header = message.header();
        let is_event = header
            .interface()
            .is_some_and(|i| i.as_str().starts_with("org.a11y.atspi.Event."));
        if let (true, Some(sender)) = (is_event, header.sender()) {
            let second = started.elapsed().as_secs();
            let path = header.path().map(|p| p.to_string()).unwrap_or_default();
            if recent.len() > 10_000 {
                recent.retain(|_, r| r.len > 0 && second - r.seconds[r.len - 1] < 10);
            }
            if recent
                .entry((sender.to_string(), path))
                .or_default()
                .noisy(second)
            {
                continue;
            }
            let mut counts = lock(&counts);
            let activity = counts.entry(sender.to_string()).or_default();
            activity.events += 1;
            activity.last = Some(Instant::now());
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::Recent;

    #[test]
    fn a_burst_counts_and_a_steady_source_is_noise() {
        let mut burst = Recent::default();
        assert!(!burst.noisy(5));
        assert!(!burst.noisy(5));
        assert!(!burst.noisy(6));
        let mut steady = Recent::default();
        assert!(!steady.noisy(1));
        assert!(!steady.noisy(2));
        assert!(steady.noisy(3));
        assert!(steady.noisy(4));
        // Quiet for a while, then a fresh burst counts again.
        assert!(!steady.noisy(20));
    }
}
