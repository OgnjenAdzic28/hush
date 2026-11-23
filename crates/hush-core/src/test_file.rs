//! A desktop test written as text, one line per thing to do:
//!
//! ```text
//! # Saves a note.
//! app Notes
//! click button "New Note"
//! type textarea[1] "Groceries"
//! expect text "Groceries"
//! expect field "Title" = "Groceries"
//! ```
//!
//! `app NAME` picks the app (and starts it in the background when it isn't
//! running), `window TITLE` picks one of its windows (`window` alone goes
//! back to the main one), `expect CONDITION` is a check that waits like
//! `wait` and fails the test when it doesn't come true, and every other line
//! is an `act` step. Locators make a file work in any session; refs (`b3`)
//! only mean something to the session that read them, so they are refused.

use crate::refs::Ref;
use crate::step::{Condition, Element, Step, Target};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestLine {
    /// 1-based line number in the file.
    pub number: usize,
    /// The line as written, for reports.
    pub text: String,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    App(String),
    /// `None` is the app's main window.
    Window(Option<String>),
    /// An `act` step; an `expect` line becomes the `wait` step it means.
    Step(String),
}

/// Reads a test file. Every line's syntax is checked here, so a typo fails
/// before anything runs.
pub fn parse(source: &str) -> Result<Vec<TestLine>, String> {
    let mut lines = Vec::new();
    let mut has_app = false;
    for (index, raw) in source.lines().enumerate() {
        let number = index + 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let at = |message: String| format!("line {number} (`{text}`): {message}");
        let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let rest = rest.trim();
        let kind = match word {
            "app" if rest.is_empty() => return Err(at("app needs a name".into())),
            "app" => {
                has_app = true;
                Kind::App(unquote(rest))
            }
            "window" => Kind::Window((!rest.is_empty()).then(|| unquote(rest))),
            _ if !has_app => return Err(at("say which app first: app NAME".into())),
            "expect" => {
                let step = format!("wait {rest}");
                match Step::check(&step).map_err(at)?.0 {
                    Step::Wait {
                        until: Condition::Settled | Condition::Elapsed(_),
                        ..
                    } => {
                        return Err(at(
                            "expect needs something to check: an element, gone, = VALUE or text"
                                .into(),
                        ));
                    }
                    _ => refuse_refs(&step).map_err(at)?,
                }
                Kind::Step(step)
            }
            _ => {
                Step::check(text).map_err(at)?;
                refuse_refs(text).map_err(at)?;
                Kind::Step(text.to_owned())
            }
        };
        lines.push(TestLine {
            number,
            text: text.to_owned(),
            kind,
        });
    }
    if !has_app {
        return Err("the test names no app; start it with app NAME".into());
    }
    Ok(lines)
}

/// Refs belong to one session, so a file that uses them only works in the
/// session that wrote it.
fn refuse_refs(line: &str) -> Result<(), String> {
    if uses_ref(line) {
        Err(REFS_ARE_SESSION_ONLY.into())
    } else {
        Ok(())
    }
}

const REFS_ARE_SESSION_ONLY: &str = "refs like b3 only work in the session that read them; name the element as the outline shows it, like button \"Save\"";

/// Whether a step names an element by ref rather than by locator: it has
/// more element places than locators filled them.
pub fn uses_ref(line: &str) -> bool {
    let mut locators = 0;
    let Ok(step) = Step::parse_with(line, &mut |_| {
        locators += 1;
        Ok(Ref::parse("e1").expect("a valid ref"))
    }) else {
        return false;
    };
    let is_ref = |target: &Target| matches!(target, Target::Ref(_));
    let places = match &step {
        Step::Click { target, .. } | Step::Scroll { target, .. } => usize::from(is_ref(target)),
        Step::Drag { from, to } => usize::from(is_ref(from)) + usize::from(is_ref(to)),
        Step::Set { .. }
        | Step::Type { into: Some(_), .. }
        | Step::Pick { .. }
        | Step::Select { .. }
        | Step::Focus { .. }
        | Step::Show { .. } => 1,
        Step::Wait { until, .. } => {
            return matches!(
                until,
                Condition::Exists(Element::Ref(_))
                    | Condition::Gone(Element::Ref(_))
                    | Condition::Value(Element::Ref(_), _)
            );
        }
        _ => 0,
    };
    places > locators
}

fn unquote(text: &str) -> String {
    crate::locator::take_quoted(text)
        .filter(|(_, rest)| rest.trim().is_empty())
        .map_or_else(|| text.to_owned(), |(inside, _)| inside)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_apps_windows_steps_and_checks() {
        let lines = parse(
            "# Saves a note.\n\napp Notes\nwindow \"Groceries\"\nclick button \"New Note\"\nexpect field \"Title\" = \"Milk\"\nwindow\n",
        )
        .unwrap();
        let kinds: Vec<&Kind> = lines.iter().map(|line| &line.kind).collect();
        assert_eq!(
            kinds,
            [
                &Kind::App("Notes".into()),
                &Kind::Window(Some("Groceries".into())),
                &Kind::Step("click button \"New Note\"".into()),
                &Kind::Step("wait field \"Title\" = \"Milk\"".into()),
                &Kind::Window(None),
            ]
        );
        assert_eq!(lines[2].number, 5);
    }

    #[test]
    fn mistakes_fail_before_anything_runs_and_name_the_line() {
        let error = |source: &str| parse(source).unwrap_err();
        assert!(error("click button \"Save\"").contains("say which app first"));
        assert!(error("app Notes\nfly away").starts_with("line 2 (`fly away`)"));
        assert!(error("app Notes\nexpect 2s").contains("something to check"));
        assert!(error("app Notes\nexpect b3").contains("only work in the session"));
        assert!(error("").contains("names no app"));
    }

    #[test]
    fn refs_in_steps_are_told_apart_from_locators() {
        assert!(uses_ref("click b3"));
        assert!(uses_ref("drag i2 to row \"A\""));
        assert!(!uses_ref("click button \"Save\""));
        assert!(!uses_ref("click 120,48"));
        assert!(!uses_ref("type field[2] \"Ada\""));
        assert!(!uses_ref("key cmd+s"));
    }
}
