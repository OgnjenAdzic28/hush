//! `act` on an app with no windows: only menu bar commands make sense there
//! (File > New, Window > a closed window), and they need only the app.

use std::fmt::Write;
use std::thread;
use std::time::{Duration, Instant};

use hush_core::{AppInfo, Platform, Result, Step};

use crate::{Engine, ObserveOptions};

/// How long a menu command may take to open a window (File > New opens one
/// in tens of milliseconds).
const WINDOW_LIMIT: Duration = Duration::from_secs(2);

/// How often the app's windows are listed while waiting for one; a new
/// window sends no event Hush can wait on before the window is known.
const WINDOW_POLL: Duration = Duration::from_millis(20);

impl<P: Platform> Engine<P> {
    /// Presses each `menu` step on the app, then reads the window they
    /// opened, if any. Every step in `parsed` is a `Step::Menu`.
    pub(crate) fn menus_without_window(
        &mut self,
        app: &AppInfo,
        parsed: &[Step],
        steps: &[String],
    ) -> Result<String> {
        let started = Instant::now();
        let mut done = 0;
        let mut stopped = None;
        for (i, step) in parsed.iter().enumerate() {
            let Step::Menu { path } = step else {
                unreachable!("only menu steps come here")
            };
            match self.platform.press_menu(app, path) {
                Ok(()) => done += 1,
                Err(error) => {
                    stopped = Some(format!(
                        "stopped at step {} (`{}`): {error}",
                        i + 1,
                        steps[i]
                    ));
                    break;
                }
            }
        }
        let deadline = Instant::now() + WINDOW_LIMIT;
        let opened = loop {
            let open = !self.platform.windows(app)?.is_empty();
            if open || stopped.is_some() || Instant::now() >= deadline {
                break open;
            }
            thread::sleep(WINDOW_POLL);
        };

        let mut out = format!(
            "# {done}/{} steps · {:.2} s\n",
            parsed.len(),
            started.elapsed().as_secs_f64()
        );
        if let Some(stopped) = stopped {
            let _ = writeln!(out, "# {stopped}");
        }
        if opened {
            let query = app
                .pid
                .map_or_else(|| app.name.clone(), |pid| pid.to_string());
            out.push_str(&self.observe(&query, ObserveOptions::default())?);
        } else {
            let _ = writeln!(out, "# {} still has no open windows", app.name);
        }
        Ok(out)
    }
}
