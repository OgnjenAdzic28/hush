//! Running a test file (see `hush_core::test_file`) in a fresh session:
//! the steps run as batches, each `expect` waits like `wait`, and the first
//! failure stops the test with what the window looked like at that moment.

use std::time::{Duration, Instant};

use hush_core::test_file::{self, Kind, TestLine};
use hush_core::{Condition, Element, Error, Image, Result, Step};

use crate::{Engine, ObserveOptions, Platform};

/// The label on the agent cursor in a failed test's movie.
const MOVIE_LABEL: &str = "hush test";

#[derive(Debug)]
pub struct TestRun {
    /// Lines that ran (comments and blank lines aren't counted).
    pub ran: usize,
    pub of: usize,
    pub elapsed: Duration,
    pub failure: Option<Failure>,
}

#[derive(Debug)]
pub struct Failure {
    /// 1-based line number in the file.
    pub line: usize,
    pub text: String,
    pub reason: String,
    /// The whole outline of the window when the test failed.
    pub outline: Option<String>,
    pub picture: Option<Image>,
    /// What writing the movie said, when one was asked for.
    pub movie: Option<String>,
}

/// Where the test is: the app and window the next steps act on.
struct At {
    app: String,
    window: Option<String>,
}

impl<P: Platform> Engine<P> {
    /// Runs the test in `source` from a fresh start: no refs or diffs carry
    /// over from earlier calls. With `movie`, the test is recorded and the
    /// movie kept there when it fails. A file that doesn't parse is an error;
    /// a test that fails is a `TestRun` with a `Failure`.
    pub fn test(&mut self, source: &str, movie: Option<&str>) -> Result<TestRun> {
        let lines = test_file::parse(source).map_err(Error::Failed)?;
        self.forget();
        let started = Instant::now();
        let mut at: Option<At> = None;
        let mut batch: Vec<&TestLine> = Vec::new();
        let mut ran = 0;
        let mut failure = None;
        for line in &lines {
            if !matches!(line.kind, Kind::Step(_)) {
                failure = self.flush(at.as_ref(), &mut batch, movie, &mut ran);
                if failure.is_some() {
                    break;
                }
            }
            match &line.kind {
                Kind::Step(_) => batch.push(line),
                Kind::App(name) => {
                    let here = At {
                        app: name.clone(),
                        window: None,
                    };
                    if let Err(error) = self.start_if_needed(name) {
                        failure = Some(self.failure(&here, line, error.to_string(), None));
                        break;
                    }
                    at = Some(here);
                    ran += 1;
                }
                Kind::Window(title) => {
                    if let Some(at) = at.as_mut() {
                        at.window = title.clone();
                    }
                    ran += 1;
                }
            }
        }
        if failure.is_none() {
            failure = self.flush(at.as_ref(), &mut batch, movie, &mut ran);
        }
        if failure.is_none() {
            // The movie of a passing test isn't worth keeping.
            self.forget();
        }
        Ok(TestRun {
            ran,
            of: lines.len(),
            elapsed: started.elapsed(),
            failure,
        })
    }

    /// Runs the step lines collected so far, counting those that ran.
    fn flush(
        &mut self,
        at: Option<&At>,
        batch: &mut Vec<&TestLine>,
        movie: Option<&str>,
        ran: &mut usize,
    ) -> Option<Failure> {
        let at = at.filter(|_| !batch.is_empty())?;
        let done = self.run_steps(at, batch, movie);
        let failure = match done {
            Ok(()) => {
                *ran += batch.len();
                None
            }
            Err((i, reason)) => {
                *ran += i;
                Some(self.failure(at, batch[i], reason, movie))
            }
        };
        batch.clear();
        failure
    }

    /// Runs one batch of step lines. On failure, says which of them stopped
    /// it and why.
    fn run_steps(
        &mut self,
        at: &At,
        lines: &[&TestLine],
        movie: Option<&str>,
    ) -> std::result::Result<(), (usize, String)> {
        if movie.is_some() && !self.is_recording_any() {
            // A movie that can't start shouldn't fail the test it was for.
            let _ = self.record_start(&at.app, at.window.as_deref());
        }
        let steps: Vec<String> = lines
            .iter()
            .map(|line| match &line.kind {
                Kind::Step(step) => step.clone(),
                _ => unreachable!("only steps are batched"),
            })
            .collect();
        match self.batch(&at.app, at.window.as_deref(), &steps) {
            Err(error) => Err((0, error.to_string())),
            Ok(batch) => match batch.stopped {
                None => Ok(()),
                Some((i, error)) => Err((i, self.why(at, &steps[i], error))),
            },
        }
    }

    /// Says why a step failed. A failed `expect` says what the window shows
    /// instead, which is what the person fixing the test needs.
    fn why(&mut self, at: &At, step: &str, error: Error) -> String {
        let Ok((Step::Wait { until, .. }, _)) = Step::check(step) else {
            return error.to_string();
        };
        let Ok(lines) = self.lines(&at.app, at.window.as_deref()) else {
            return error.to_string();
        };
        let line_of = |element: &Element| match element {
            Element::Ref(r) => lines.iter().find(|line| line.r == *r),
            Element::Find(locator) => locator
                .find(&lines)
                .ok()
                .and_then(|r| lines.iter().find(|line| line.r == r)),
        };
        match &until {
            Condition::Exists(Element::Find(locator)) => match locator.find(&lines) {
                Err(miss) => miss.explain(locator),
                Ok(_) => error.to_string(),
            },
            Condition::Value(element, wanted) => match line_of(element) {
                Some(line) => format!(
                    "expected = \"{wanted}\", but {} {}{}",
                    line.r,
                    line.head,
                    if line.tail.is_empty() {
                        " shows no value"
                    } else {
                        &line.tail
                    }
                ),
                None => match element {
                    Element::Find(locator) => locator
                        .find(&lines)
                        .err()
                        .map_or_else(|| error.to_string(), |miss| miss.explain(locator)),
                    Element::Ref(r) => format!("{r} isn't in the window"),
                },
            },
            Condition::Gone(element) => match line_of(element) {
                Some(line) => format!("{} {} is still there", line.r, line.head),
                None => error.to_string(),
            },
            Condition::Text(text) => format!("nothing in the window shows \"{text}\""),
            _ => error.to_string(),
        }
    }

    fn failure(
        &mut self,
        at: &At,
        line: &TestLine,
        reason: String,
        movie: Option<&str>,
    ) -> Failure {
        let options = ObserveOptions {
            window: at.window.as_deref(),
            focus: None,
            full: true,
        };
        let outline = self.observe(&at.app, options).ok();
        let picture = self.see(&at.app, at.window.as_deref(), None, 960).ok();
        let movie = match (movie, self.is_recording_any()) {
            (Some(path), true) => Some(
                self.record_stop(Some(MOVIE_LABEL), Some(path))
                    .unwrap_or_else(|error| format!("the movie couldn't be written: {error}")),
            ),
            _ => None,
        };
        Failure {
            line: line.number,
            text: line.text.clone(),
            reason,
            outline,
            picture,
            movie,
        }
    }
}
