//! Waiting for an app to settle, driven by the platform's event counts.

use std::thread;
use std::time::{Duration, Instant};

use hush_core::{Activity, AppInfo, Outcome, Pace, Platform, Poll, Settle};

/// How often the event count is checked while waiting. Short enough that
/// the answer is at most this late, long enough to cost nothing.
const POLL: Duration = Duration::from_millis(2);

/// The longest one step waits for its app to settle.
pub(crate) const STEP_LIMIT: Duration = Duration::from_secs(3);

/// A started wait: take `begin` just before the step's operation.
pub(crate) struct Pending {
    settle: Settle,
}

pub(crate) fn begin<P: Platform>(platform: &P, app: &AppInfo, pace: &Pace) -> Pending {
    let before: Activity = platform.activity(app);
    Pending {
        settle: Settle::begin(Instant::now(), before, pace.rules(STEP_LIMIT)),
    }
}

impl Pending {
    /// Blocks until the app settles, then teaches `pace` how long it took.
    pub(crate) fn finish<P: Platform>(
        mut self,
        platform: &P,
        app: &AppInfo,
        pace: &mut Pace,
    ) -> Outcome {
        loop {
            let now = Instant::now();
            match self.settle.poll(now, platform.activity(app)) {
                Poll::Done(outcome) => {
                    pace.learn(&outcome);
                    return outcome;
                }
                Poll::WaitUntil(at) => thread::sleep(at.saturating_duration_since(now).min(POLL)),
            }
        }
    }
}
