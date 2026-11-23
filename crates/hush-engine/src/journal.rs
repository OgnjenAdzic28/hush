//! What a session did, written as a test file (see `hush_core::test_file`)
//! so an agent's run can be replayed with `hush test`. Each step that ran is
//! kept with its refs turned into locators, read from the window as it was
//! just before the step.

use hush_core::outline::Line;
use hush_core::{Locator, Ref, Step};

/// The most steps kept; a session that runs longer keeps its first ones.
const MOST: usize = 2000;

#[derive(Debug, Default)]
pub(crate) struct Journal {
    lines: Vec<String>,
    steps: usize,
    /// The app and window the last kept step ran on.
    at: Option<(String, Option<String>)>,
    full: bool,
}

impl Journal {
    /// Keeps a step that ran on `app`'s `window` (`None`: its main window).
    /// `before` is the window's outline just before the step; `after`, the
    /// latest read, names elements the step itself brought in.
    pub(crate) fn note(
        &mut self,
        app: &str,
        window: Option<&str>,
        step: &Step,
        before: &[Line],
        after: &[Line],
    ) {
        if self.steps >= MOST {
            self.full = true;
            return;
        }
        let here = (app.to_owned(), window.map(str::to_owned));
        if self.at.as_ref().map(|(app, _)| app) != Some(&here.0) {
            self.lines.push(format!("app {app}"));
            if let Some(title) = window {
                self.lines.push(format!("window {}", quoted(title)));
            }
        } else if self.at.as_ref() != Some(&here) {
            self.lines.push(match window {
                Some(title) => format!("window {}", quoted(title)),
                None => "window".into(),
            });
        }
        self.at = Some(here);
        let mut unnamed: Vec<Ref> = Vec::new();
        let line = step.write(&mut |r| {
            Locator::of(before, r)
                .or_else(|| Locator::of(after, r))
                .map_or_else(
                    || {
                        unnamed.push(r);
                        r.to_string()
                    },
                    |locator| locator.to_string(),
                )
        });
        if !unnamed.is_empty() {
            let refs: Vec<String> = unnamed.iter().map(Ref::to_string).collect();
            self.lines.push(format!(
                "# {} wasn't in the outline when this ran; name it as the outline shows it",
                refs.join(", ")
            ));
        }
        self.lines.push(line);
        self.steps += 1;
    }

    /// The test file of everything kept since the last take, or `None`.
    pub(crate) fn take(&mut self) -> Option<String> {
        if self.steps == 0 {
            return None;
        }
        let mut out = String::from(
            "# Saved by hush save. Run it with hush test; add expect lines to check results.\n",
        );
        if self.full {
            out.push_str(&format!("# Only the first {MOST} steps were kept.\n"));
        }
        for line in self.lines.drain(..) {
            out.push_str(&line);
            out.push('\n');
        }
        *self = Journal::default();
        Some(out)
    }
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
