//! When has an app finished reacting to a step?
//!
//! Platforms count accessibility events per app (value changed, element
//! created or destroyed, focus moved). After a step, the app is settled when
//! events arrived and then stopped for a quiet gap, or when nothing arrived
//! within the reaction window. Both windows adapt per app: a native app that
//! answers in 5 ms isn't made to wait like a web app that answers in 200 ms.
//!
//! Published numbers behind this: fixed waits were 42% of agent interaction
//! time, 63% of it after the screen was already ready (28 Oct 2026), and
//! screenshots taken 13 to 39 ms after an action, before the redraw, made a
//! fast agent slower overall (cua-speedrun, 30 Sep 2026).

use std::time::{Duration, Instant};

/// What a platform reports about one app's accessibility events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// Events seen since the platform started watching the app.
    pub events: u64,
    pub last: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// How long to wait for the first event before calling it "no reaction".
    pub reaction: Duration,
    /// How long events must stop before the app counts as settled.
    pub quiet: Duration,
    /// Give up waiting after this long, settled or not.
    pub limit: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Events arrived, then stopped.
    Quiet,
    /// No event within the reaction window.
    NoReaction,
    /// Still busy at the limit.
    Limit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub reason: Reason,
    /// From the start of the wait to the decision.
    pub after: Duration,
    pub first_event: Option<Duration>,
    pub longest_gap: Duration,
    pub events: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    /// Check again at this instant (or sooner, if an event arrives).
    WaitUntil(Instant),
    Done(Outcome),
}

/// One wait for one step.
#[derive(Debug)]
pub struct Settle {
    started: Instant,
    rules: Rules,
    base_events: u64,
    first: Option<Instant>,
    last: Option<Instant>,
    longest_gap: Duration,
    events: u64,
}

impl Settle {
    /// Call just before the step runs, with the app's activity at that moment.
    pub fn begin(now: Instant, before: Activity, rules: Rules) -> Settle {
        Settle {
            started: now,
            rules,
            base_events: before.events,
            first: None,
            last: None,
            longest_gap: Duration::ZERO,
            events: 0,
        }
    }

    pub fn poll(&mut self, now: Instant, activity: Activity) -> Poll {
        let events = activity.events.saturating_sub(self.base_events);
        if events > self.events {
            let at = activity.last.unwrap_or(now).max(self.started);
            if self.first.is_none() {
                self.first = Some(at);
            }
            if let Some(previous) = self.last {
                self.longest_gap = self.longest_gap.max(at.saturating_duration_since(previous));
            }
            self.last = Some(at);
            self.events = events;
        }

        let deadline = self.started + self.rules.limit;
        let decision = match self.last {
            None if now >= self.started + self.rules.reaction => Some(Reason::NoReaction),
            None => None,
            Some(last) if now >= last + self.rules.quiet => Some(Reason::Quiet),
            Some(_) => None,
        };
        let reason = match decision {
            Some(reason) => reason,
            None if now >= deadline => Reason::Limit,
            None => {
                let next = match self.last {
                    None => self.started + self.rules.reaction,
                    Some(last) => last + self.rules.quiet,
                };
                return Poll::WaitUntil(next.min(deadline));
            }
        };
        Poll::Done(Outcome {
            reason,
            after: now.saturating_duration_since(self.started),
            first_event: self
                .first
                .map(|first| first.saturating_duration_since(self.started)),
            longest_gap: self.longest_gap,
            events: self.events,
        })
    }
}

/// How quickly one app tends to react, learned from its last waits.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pace {
    reaction_ms: Option<f64>,
    gap_ms: Option<f64>,
}

impl Pace {
    const WEIGHT: f64 = 0.3;

    /// Rules for the next wait on this app. Before anything is learned the
    /// windows are wide enough for a web view: about 150 ms to react and
    /// 40 ms of quiet.
    pub fn rules(&self, limit: Duration) -> Rules {
        let reaction = self
            .reaction_ms
            .map_or(150.0, |ms| ms * 2.5 + 15.0)
            .clamp(30.0, 400.0);
        let quiet = self
            .gap_ms
            .map_or(40.0, |ms| ms * 2.0 + 10.0)
            .clamp(15.0, 150.0);
        Rules {
            reaction: Duration::from_secs_f64(reaction / 1000.0),
            quiet: Duration::from_secs_f64(quiet / 1000.0),
            limit,
        }
    }

    /// Learn from a wait that saw the app react. Waits without a reaction
    /// say nothing about the app's speed and are ignored.
    pub fn learn(&mut self, outcome: &Outcome) {
        let Some(first) = outcome.first_event else {
            return;
        };
        if outcome.reason != Reason::Quiet {
            return;
        }
        blend(&mut self.reaction_ms, first.as_secs_f64() * 1000.0);
        blend(&mut self.gap_ms, outcome.longest_gap.as_secs_f64() * 1000.0);
    }
}

fn blend(slot: &mut Option<f64>, sample: f64) {
    *slot = Some(match *slot {
        None => sample,
        Some(old) => old + Pace::WEIGHT * (sample - old),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: Duration = Duration::from_secs(2);

    fn rules() -> Rules {
        Rules {
            reaction: Duration::from_millis(100),
            quiet: Duration::from_millis(30),
            limit: LIMIT,
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn settles_one_quiet_gap_after_the_last_event() {
        let t0 = Instant::now();
        let before = Activity {
            events: 7,
            last: None,
        };
        let mut settle = Settle::begin(t0, before, rules());
        assert_eq!(settle.poll(t0, before), Poll::WaitUntil(t0 + ms(100)));

        let burst = |n: u64, at: u64| Activity {
            events: 7 + n,
            last: Some(t0 + ms(at)),
        };
        assert_eq!(
            settle.poll(t0 + ms(12), burst(1, 10)),
            Poll::WaitUntil(t0 + ms(40))
        );
        assert_eq!(
            settle.poll(t0 + ms(30), burst(3, 25)),
            Poll::WaitUntil(t0 + ms(55))
        );
        let Poll::Done(outcome) = settle.poll(t0 + ms(55), burst(3, 25)) else {
            panic!()
        };
        assert_eq!(outcome.reason, Reason::Quiet);
        assert_eq!(outcome.after, ms(55));
        assert_eq!(outcome.first_event, Some(ms(10)));
        assert_eq!(outcome.longest_gap, ms(15));
        assert_eq!(outcome.events, 3);
    }

    #[test]
    fn no_event_within_the_reaction_window_means_no_reaction() {
        let t0 = Instant::now();
        let mut settle = Settle::begin(t0, Activity::default(), rules());
        let Poll::Done(outcome) = settle.poll(t0 + ms(100), Activity::default()) else {
            panic!()
        };
        assert_eq!(outcome.reason, Reason::NoReaction);
    }

    #[test]
    fn a_busy_app_stops_at_the_limit() {
        let t0 = Instant::now();
        let mut settle = Settle::begin(t0, Activity::default(), rules());
        let busy = Activity {
            events: 500,
            last: Some(t0 + LIMIT),
        };
        let Poll::Done(outcome) = settle.poll(t0 + LIMIT, busy) else {
            panic!()
        };
        assert_eq!(outcome.reason, Reason::Limit);
    }

    #[test]
    fn pace_shrinks_waits_for_a_fast_app_and_widens_them_for_a_slow_one() {
        let quiet = |first: u64, gap: u64| Outcome {
            reason: Reason::Quiet,
            after: ms(first + gap + 40),
            first_event: Some(ms(first)),
            longest_gap: ms(gap),
            events: 4,
        };
        let mut fast = Pace::default();
        let mut slow = Pace::default();
        for _ in 0..10 {
            fast.learn(&quiet(4, 2));
            slow.learn(&quiet(180, 60));
        }
        let (fast, slow, fresh) = (
            fast.rules(LIMIT),
            slow.rules(LIMIT),
            Pace::default().rules(LIMIT),
        );
        assert!(
            fast.reaction < fresh.reaction && fast.quiet < fresh.quiet,
            "{fast:?}"
        );
        assert!(
            slow.reaction > fresh.reaction && slow.quiet > fresh.quiet,
            "{slow:?}"
        );
        assert_eq!(fast.reaction, ms(30));
    }
}
