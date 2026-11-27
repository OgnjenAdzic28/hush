//! Steps of an `act` batch, written as short commands (`click b3`,
//! `type t1 "hello"`, `key cmd+s`). One line each costs a model a few tokens,
//! a third of the equivalent JSON objects.

use std::time::Duration;

use crate::key::KeyCombo;
use crate::locator::Locator;
use crate::model::Role;
use crate::refs::Ref;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Ref(Ref),
    /// Window-relative points, as seen in a `see` screenshot.
    Point(f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// An element a wait looks for: by ref, or by locator when it may not be
/// there yet.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    Ref(Ref),
    Find(Locator),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    /// The app finished reacting (the default after every step).
    Settled,
    Exists(Element),
    Gone(Element),
    /// Some element's name or value contains the text.
    Text(String),
    /// The element's value, as its outline line shows it (`= "Ada"`,
    /// `= on`, `= 40`), is this.
    Value(Element, String),
    Elapsed(Duration),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Click {
        target: Target,
        button: Button,
        count: u8,
    },
    Set {
        r: Ref,
        value: String,
    },
    Type {
        into: Option<Ref>,
        text: String,
    },
    Key {
        combo: KeyCombo,
        repeat: u16,
    },
    Menu {
        path: Vec<String>,
    },
    /// Open a popup or combo box and choose an option by name.
    Pick {
        r: Ref,
        option: String,
    },
    Select {
        r: Ref,
    },
    Focus {
        r: Ref,
    },
    Scroll {
        target: Target,
        direction: Direction,
        amount: u16,
    },
    /// Scroll an element into view.
    Show {
        r: Ref,
    },
    Wait {
        until: Condition,
        within: Duration,
    },
    /// Press on one place, move to another while holding, release there.
    Drag {
        from: Target,
        to: Target,
    },
    /// Move the window to Hush's lane: a display the user can't see, where
    /// it renders and takes pointer input without being on screen. In an app
    /// the user is using, the platform opens the agent a window of its own
    /// there instead.
    Lane,
    /// Put the window back where it was before `Lane`.
    Unlane,
}

/// The longest a `wait` may run unless it says otherwise.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(5);

/// Syntax help for tool descriptions. Kept short: it rides on every request.
pub const SYNTAX: &str = "click REF|X,Y · double REF · right REF · set REF VALUE · type [REF] TEXT · \
key COMBO [xN] · menu A > B · pick REF OPTION · select REF · focus REF · \
scroll REF up|down|left|right [N] · show REF · drag REF|X,Y to REF|X,Y (to a ref: takes its place) · wait gone REF|REF|REF = V|text \"T\"|500ms [within 10s] · \
lane (move the window off screen, where it still renders and takes clicks; in the user's app, opens your own window there) · unlane (put it back) · \
REF can also be an outline line's head: button \"Save\", field[2]";

/// Finds the ref a locator names, in the window the step will run on.
pub type Resolve<'a> = dyn FnMut(&Locator) -> Result<Ref, String> + 'a;

impl Step {
    /// Parses a step that names elements by ref only; a locator is an error.
    pub fn parse(line: &str) -> Result<Step, String> {
        Step::parse_with(line, &mut |locator| {
            Err(format!("{locator} needs a window to look in"))
        })
    }

    /// Parses a step, turning each locator into a ref with `resolve`. Waits
    /// keep their locators: what they wait for may not be there yet.
    pub fn parse_with(line: &str, resolve: &mut Resolve<'_>) -> Result<Step, String> {
        let mut parser = Parser { resolve };
        let line = line.trim();
        let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        let step = match verb.to_lowercase().as_str() {
            "click" => parser.click(rest, Button::Left, 1)?,
            "double" => parser.click(rest, Button::Left, 2)?,
            "right" => parser.click(rest, Button::Right, 1)?,
            "drag" => parser.drag(rest)?,
            "set" => {
                let (r, value) = parser.element_then_value(rest)?;
                Step::Set {
                    r,
                    value: text(value),
                }
            }
            "type" => match (parser.element_then_value(rest), Locator::parse(rest)) {
                (Ok((r, after)), _) if !after.is_empty() => Step::Type {
                    into: Some(r),
                    text: text(after),
                },
                // Text that reads as a locator is never typed as it stands:
                // `type textarea "Hi"` means the textarea, so say how to
                // name it instead of typing the words `textarea "Hi"`.
                (result, Some((locator, after))) => {
                    let hint = match (&locator.name, locator.nth, after.is_empty()) {
                        (Some(name), None, true) => format!(
                            "; to type \"{name}\" into a {role} with no name, write {role}[1] \"{name}\"",
                            role = locator.role.word()
                        ),
                        _ => String::new(),
                    };
                    let error = match result {
                        Err(error) => error,
                        Ok(_) => format!("type {locator} has no text after it"),
                    };
                    return Err(format!("{error}{hint}"));
                }
                _ => Step::Type {
                    into: None,
                    text: text(rest),
                },
            },
            "key" | "press" => {
                let (combo, after) = split_word(rest);
                let repeat = match after {
                    "" => 1,
                    count => count
                        .strip_prefix('x')
                        .and_then(|n| n.parse().ok())
                        .filter(|&n| n > 0)
                        .ok_or_else(|| format!("expected a repeat like x3, got '{count}'"))?,
                };
                Step::Key {
                    combo: KeyCombo::parse(combo)?,
                    repeat,
                }
            }
            "menu" => Step::Menu {
                path: rest
                    .split('>')
                    .map(|part| text(part.trim()))
                    .filter(|part| !part.is_empty())
                    .collect(),
            },
            "pick" => {
                let (r, option) = parser.element_then_value(rest)?;
                Step::Pick {
                    r,
                    option: text(option),
                }
            }
            "select" => Step::Select {
                r: parser.only_element(rest)?,
            },
            "focus" => Step::Focus {
                r: parser.only_element(rest)?,
            },
            "show" => Step::Show {
                r: parser.only_element(rest)?,
            },
            "scroll" => parser.scroll(rest)?,
            "wait" => wait(rest)?,
            "lane" if rest.is_empty() => Step::Lane,
            "unlane" if rest.is_empty() => Step::Unlane,
            "" => return Err("empty step".into()),
            other => return Err(format!("unknown step '{other}'. Steps: {SYNTAX}")),
        };
        match &step {
            Step::Menu { path } if path.is_empty() => {
                Err("menu needs a path like File > Save".into())
            }
            Step::Type { text, .. } if text.is_empty() => Err("type needs text".into()),
            _ => Ok(step),
        }
    }

    /// Checks a step's syntax without a window: locators stand in for any
    /// ref. Says whether it has locators, which [`Step::parse_with`] must
    /// resolve against the window just before the step runs.
    pub fn check(line: &str) -> Result<(Step, bool), String> {
        let stand_in = Ref::parse("e1").expect("a valid ref");
        let mut used = false;
        let step = Step::parse_with(line, &mut |_| {
            used = true;
            Ok(stand_in)
        })?;
        Ok((step, used))
    }
}

impl Step {
    /// Writes the step back as a line that parses to the same step, with
    /// each ref written as `name` says (a locator, to replay it elsewhere).
    pub fn write(&self, name: &mut dyn FnMut(Ref) -> String) -> String {
        let target = |target: &Target, name: &mut dyn FnMut(Ref) -> String| match target {
            Target::Ref(r) => name(*r),
            Target::Point(x, y) => format!("{x},{y}"),
        };
        match self {
            Step::Click {
                target: at,
                button,
                count,
            } => {
                let verb = match (button, count) {
                    (Button::Right, _) => "right",
                    (Button::Left, 2) => "double",
                    (Button::Left, _) => "click",
                };
                format!("{verb} {}", target(at, name))
            }
            Step::Set { r, value } => format!("set {} {}", name(*r), quote(value)),
            Step::Type {
                into: Some(r),
                text,
            } => format!("type {} {}", name(*r), quote(text)),
            Step::Type { into: None, text } => format!("type {}", quote(text)),
            Step::Key { combo, repeat: 1 } => format!("key {combo}"),
            Step::Key { combo, repeat } => format!("key {combo} x{repeat}"),
            Step::Menu { path } => {
                let parts: Vec<String> = path
                    .iter()
                    .map(|part| {
                        // A title with `>` in it can't be written: the
                        // parser splits on every `>`.
                        if part.starts_with('"') || part.trim() != part {
                            quote(part)
                        } else {
                            part.clone()
                        }
                    })
                    .collect();
                format!("menu {}", parts.join(" > "))
            }
            Step::Pick { r, option } => format!("pick {} {}", name(*r), quote(option)),
            Step::Select { r } => format!("select {}", name(*r)),
            Step::Focus { r } => format!("focus {}", name(*r)),
            Step::Show { r } => format!("show {}", name(*r)),
            Step::Scroll {
                target: at,
                direction,
                amount,
            } => {
                let direction = match direction {
                    Direction::Up => "up",
                    Direction::Down => "down",
                    Direction::Left => "left",
                    Direction::Right => "right",
                };
                format!("scroll {} {direction} {amount}", target(at, name))
            }
            Step::Drag { from, to } => {
                let from = target(from, name);
                format!("drag {from} to {}", target(to, name))
            }
            Step::Wait { until, within } => {
                let mut element = |element: &Element| match element {
                    Element::Ref(r) => name(*r),
                    Element::Find(locator) => locator.to_string(),
                };
                let condition = match until {
                    Condition::Settled => "idle".to_owned(),
                    Condition::Exists(e) => element(e),
                    Condition::Gone(e) => format!("gone {}", element(e)),
                    Condition::Text(text) => format!("text {}", quote(text)),
                    Condition::Value(e, value) => format!("{} = {}", element(e), quote(value)),
                    Condition::Elapsed(time) => format!("{}ms", time.as_millis()),
                };
                if *within == DEFAULT_WAIT {
                    format!("wait {condition}")
                } else {
                    format!("wait {condition} within {}ms", within.as_millis())
                }
            }
            Step::Lane => "lane".into(),
            Step::Unlane => "unlane".into(),
        }
    }
}

/// `text` in double quotes, with the escapes `text()` undoes.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Parser<'r, 'a> {
    resolve: &'r mut Resolve<'a>,
}

impl Parser<'_, '_> {
    fn click(&mut self, rest: &str, button: Button, count: u8) -> Result<Step, String> {
        let (target, after) = self.target(rest)?;
        expect_end(after)?;
        Ok(Step::Click {
            target,
            button,
            count,
        })
    }

    fn drag(&mut self, rest: &str) -> Result<Step, String> {
        let shape = || format!("drag needs FROM to TO, got '{rest}'");
        let (from, after) = self
            .target(rest)
            .map_err(|error| match Locator::parse(rest) {
                Some(_) => error,
                None => shape(),
            })?;
        let after = after.strip_prefix("to ").unwrap_or(after).trim_start();
        if after.is_empty() {
            return Err(shape());
        }
        let (to, end) = self.target(after)?;
        expect_end(end)?;
        Ok(Step::Drag { from, to })
    }

    fn scroll(&mut self, rest: &str) -> Result<Step, String> {
        let (target, after) = self.target(rest)?;
        let mut words = after.split_whitespace();
        let direction = match words.next() {
            Some("up") => Direction::Up,
            Some("down") | None => Direction::Down,
            Some("left") => Direction::Left,
            Some("right") => Direction::Right,
            Some(other) => {
                return Err(format!(
                    "scroll direction must be up, down, left or right, got '{other}'"
                ));
            }
        };
        let amount = match words.next() {
            Some(n) => n
                .parse()
                .map_err(|_| format!("scroll amount must be a number, got '{n}'"))?,
            None => 3,
        };
        Ok(Step::Scroll {
            target,
            direction,
            amount,
        })
    }

    /// A ref, a point or a locator at the start of `rest`, and what follows.
    fn target<'t>(&mut self, rest: &'t str) -> Result<(Target, &'t str), String> {
        let (word, after) = split_word(rest);
        if let Some(point) = point(word) {
            return Ok((point, after));
        }
        // A point may have a space after its comma: `120, 48`.
        if word.ends_with(',') {
            let (y, after) = split_word(after);
            if let Some(point) = point(&format!("{word}{y}")) {
                return Ok((point, after));
            }
        }
        let (r, after) = self.element(rest).map_err(|error| {
            if Locator::parse(rest).is_some() || bare_role(word).is_some() {
                error
            } else {
                format!("expected a ref like b3, a point like 120,48 or a locator like button \"Save\", got '{word}'")
            }
        })?;
        Ok((Target::Ref(r), after))
    }

    /// A ref or a locator at the start of `rest`, and what follows.
    fn element<'t>(&mut self, rest: &'t str) -> Result<(Ref, &'t str), String> {
        self.element_then(rest, false)
    }

    /// Like `element`, for steps that need a value after it (`type`, `set`,
    /// `pick`): in `field[2] "Ada"` the quoted text is the value, not a name.
    fn element_then_value<'t>(&mut self, rest: &'t str) -> Result<(Ref, &'t str), String> {
        self.element_then(rest, true)
    }

    fn element_then<'t>(
        &mut self,
        rest: &'t str,
        value_follows: bool,
    ) -> Result<(Ref, &'t str), String> {
        if let Some((mut locator, mut after)) = Locator::parse(rest) {
            if value_follows
                && after.is_empty()
                && locator.nth.is_some()
                && locator.name.take().is_some()
                && let Some(close) = rest.find(']')
            {
                after = rest[close + 1..].trim();
            }
            return Ok(((self.resolve)(&locator)?, after));
        }
        let (word, after) = split_word(rest);
        let r = Ref::parse(word).ok_or_else(|| {
            bare_role(word).unwrap_or_else(|| {
                format!("expected a ref like b3 or a locator like button \"Save\", got '{word}'")
            })
        })?;
        Ok((r, after))
    }

    fn only_element(&mut self, rest: &str) -> Result<Ref, String> {
        let (r, after) = self.element(rest)?;
        expect_end(after)?;
        Ok(r)
    }
}

/// Says how to fix a role word used alone, like `textarea`, which could
/// be any of the window's textareas.
fn bare_role(word: &str) -> Option<String> {
    Role::from_word(word).map(|_| {
        format!("'{word}' alone names no element; add its name or which one: {word} \"Name\" or {word}[1]")
    })
}

fn point(word: &str) -> Option<Target> {
    let (x, y) = word.split_once(',')?;
    Some(Target::Point(
        x.trim().parse().ok()?,
        y.trim().parse().ok()?,
    ))
}

fn expect_end(rest: &str) -> Result<(), String> {
    match rest.trim() {
        "" => Ok(()),
        extra => Err(format!("unexpected '{extra}' at the end")),
    }
}

fn wait(rest: &str) -> Result<Step, String> {
    let (condition, within) = match rest.rsplit_once(" within ") {
        Some((condition, limit)) => (condition.trim(), duration(limit.trim())?),
        None => (rest, DEFAULT_WAIT),
    };
    let (word, after) = split_word(condition);
    let until = match word {
        "" | "idle" => Condition::Settled,
        "gone" => Condition::Gone(only_element(after)?),
        "text" => Condition::Text(text(after)),
        _ if let Some((element, value)) = element_is_value(condition) => {
            Condition::Value(element, value)
        }
        _ => match duration(word) {
            Ok(elapsed) if after.is_empty() => Condition::Elapsed(elapsed),
            _ => Condition::Exists(only_element(condition).map_err(|_| {
                bare_role(word).unwrap_or_else(|| {
                    format!(
                        "wait needs gone REF, REF, REF = VALUE, text \"T\" or a time, got '{condition}'"
                    )
                })
            })?),
        },
    };
    Ok(Step::Wait { until, within })
}

/// `REF = VALUE` or `LOCATOR = VALUE`, the value quoted or bare.
fn element_is_value(condition: &str) -> Option<(Element, String)> {
    let (element, rest) = match Locator::parse(condition) {
        Some((locator, rest)) => (Element::Find(locator), rest),
        None => {
            let (word, rest) = split_word(condition);
            (Element::Ref(Ref::parse(word)?), rest)
        }
    };
    let value = rest.strip_prefix('=')?;
    Some((element, text(value)))
}

/// A ref or a locator, left unresolved, and nothing after it.
fn only_element(text: &str) -> Result<Element, String> {
    let text = text.trim();
    if let Some((locator, after)) = Locator::parse(text) {
        expect_end(after)?;
        return Ok(Element::Find(locator));
    }
    Ref::parse(text).map(Element::Ref).ok_or_else(|| {
        format!("expected a ref like b3 or a locator like button \"Save\", got '{text}'")
    })
}

fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim();
    match text.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim()),
        None => (text, ""),
    }
}

/// A value: the inside of one pair of double quotes (with `\"`, `\\`, `\n`,
/// `\t` escapes), or the raw text when it isn't quoted.
fn text(raw: &str) -> String {
    let raw = raw.trim();
    let Some(inner) = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .filter(|_| raw.len() >= 2)
    else {
        return raw.to_owned();
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn duration(text: &str) -> Result<Duration, String> {
    let parse = |n: &str| n.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0);
    let seconds = if let Some(ms) = text.strip_suffix("ms") {
        parse(ms).map(|v| v / 1000.0)
    } else if let Some(s) = text.strip_suffix('s') {
        parse(s)
    } else {
        None
    };
    seconds
        .filter(|&s| s <= 600.0)
        .map(Duration::from_secs_f64)
        .ok_or_else(|| format!("expected a time like 500ms or 2s, got '{text}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    fn r(text: &str) -> Ref {
        Ref::parse(text).unwrap()
    }

    #[test]
    fn lane_and_unlane_target_the_selected_window() {
        assert_eq!(Step::parse("lane").unwrap(), Step::Lane);
        assert_eq!(Step::parse("unlane").unwrap(), Step::Unlane);
        assert!(Step::parse("unlane all").is_err());
    }

    #[test]
    fn drags_take_two_targets_with_an_optional_to() {
        assert_eq!(
            Step::parse("drag i2 to i1").unwrap(),
            Step::Drag {
                from: Target::Ref(r("i2")),
                to: Target::Ref(r("i1")),
            }
        );
        assert_eq!(
            Step::parse("drag 10,20 30,40").unwrap(),
            Step::Drag {
                from: Target::Point(10.0, 20.0),
                to: Target::Point(30.0, 40.0),
            }
        );
        assert!(Step::parse("drag i2").unwrap_err().contains("FROM to TO"));
    }

    #[test]
    fn parses_every_step_form() {
        assert_eq!(
            Step::parse("click b3").unwrap(),
            Step::Click {
                target: Target::Ref(r("b3")),
                button: Button::Left,
                count: 1
            }
        );
        assert_eq!(
            Step::parse("double 120,48.5").unwrap(),
            Step::Click {
                target: Target::Point(120.0, 48.5),
                button: Button::Left,
                count: 2
            }
        );
        assert_eq!(
            Step::parse("type t1 \"say \\\"hi\\\"\\n\"").unwrap(),
            Step::Type {
                into: Some(r("t1")),
                text: "say \"hi\"\n".into()
            }
        );
        assert_eq!(
            Step::parse("type hello world").unwrap(),
            Step::Type {
                into: None,
                text: "hello world".into()
            }
        );
        assert_eq!(
            Step::parse("set t2 42").unwrap(),
            Step::Set {
                r: r("t2"),
                value: "42".into()
            }
        );
        let Step::Key { combo, repeat } = Step::parse("key down x3").unwrap() else {
            panic!()
        };
        assert_eq!((combo.to_string(), repeat), ("down".into(), 3));
        assert_eq!(
            Step::parse("menu File > Export > \"PDF…\"").unwrap(),
            Step::Menu {
                path: vec!["File".into(), "Export".into(), "PDF…".into()]
            }
        );
        assert_eq!(
            Step::parse("pick p1 Helvetica Neue").unwrap(),
            Step::Pick {
                r: r("p1"),
                option: "Helvetica Neue".into()
            }
        );
        assert_eq!(
            Step::parse("wait gone e4 within 10s").unwrap(),
            Step::Wait {
                until: Condition::Gone(Element::Ref(r("e4"))),
                within: Duration::from_secs(10)
            }
        );
        assert_eq!(
            Step::parse("wait text \"Saved\"").unwrap(),
            Step::Wait {
                until: Condition::Text("Saved".into()),
                within: DEFAULT_WAIT
            }
        );
        assert_eq!(
            Step::parse("wait 250ms").unwrap(),
            Step::Wait {
                until: Condition::Elapsed(Duration::from_millis(250)),
                within: DEFAULT_WAIT
            }
        );
        let Step::Key { combo, .. } = Step::parse("key cmd+s").unwrap() else {
            panic!()
        };
        assert!(combo.modifiers.cmd && combo.key == Key::Char('s'));
    }

    #[test]
    fn locators_resolve_where_refs_go_and_waits_keep_them() {
        let mut asked = Vec::new();
        let mut resolve = |locator: &Locator| {
            asked.push(locator.to_string());
            Ok(r("b9"))
        };
        let steps = [
            r#"click button "Save""#,
            r#"type field[2] "Ada Lovelace""#,
            r#"set slider "Volume" 40"#,
            r#"drag row "A" to row[3]"#,
            r#"pick popup "Font" Helvetica"#,
        ];
        let parsed: Vec<Step> = steps
            .iter()
            .map(|line| Step::parse_with(line, &mut resolve).unwrap())
            .collect();
        assert_eq!(
            parsed[1],
            Step::Type {
                into: Some(r("b9")),
                text: "Ada Lovelace".into()
            }
        );
        assert_eq!(
            asked,
            [
                r#"button "Save""#,
                r#"field[2]"#,
                r#"slider "Volume""#,
                r#"row "A""#,
                "row[3]",
                r#"popup "Font""#
            ]
        );
        let (wait, used) = Step::check(r#"wait gone sheet "Save changes?""#).unwrap();
        assert!(!used);
        assert!(matches!(
            wait,
            Step::Wait {
                until: Condition::Gone(Element::Find(_)),
                ..
            }
        ));
        assert!(Step::check(r#"click button "Save""#).unwrap().1);
        assert!(!Step::check("click b3").unwrap().1);
        // Words that happen to be roles are still text to type.
        assert_eq!(
            Step::parse("type text").unwrap(),
            Step::Type {
                into: None,
                text: "text".into()
            }
        );
        assert!(
            Step::parse(r#"click button "Save""#)
                .unwrap_err()
                .contains("needs a window")
        );
        // A missing element is reported, never typed as text.
        let mut missing = |_: &Locator| Err("not here".to_owned());
        for line in [r#"type field[2] "Ada""#, r#"type field "Name" "Ada""#] {
            assert_eq!(
                Step::parse_with(line, &mut missing).unwrap_err(),
                "not here",
                "{line}"
            );
        }
        // A role and a quoted name is a locator, so it is never typed as it
        // stands; the error says how to reach an unnamed element.
        let hint = r#"to type "Hi" into a textarea with no name, write textarea[1] "Hi""#;
        let missed = Step::parse_with(r#"type textarea "Hi""#, &mut missing).unwrap_err();
        assert_eq!(missed, format!("not here; {hint}"));
        let checked = Step::check(r#"type textarea "Hi""#).unwrap_err();
        assert!(
            checked.contains("has no text after it") && checked.contains(hint),
            "{checked}"
        );
        for bare in ["wait textarea", r#"wait textarea = "Hi""#, "click textarea"] {
            let error = Step::check(bare).unwrap_err();
            assert!(error.contains("textarea[1]"), "{bare}: {error}");
        }
    }

    #[test]
    fn points_may_have_a_space_after_the_comma() {
        assert_eq!(
            Step::parse("click 1, 1").unwrap(),
            Step::Click {
                target: Target::Point(1.0, 1.0),
                button: Button::Left,
                count: 1
            }
        );
        assert_eq!(
            Step::parse("scroll 10, 20 up 2").unwrap(),
            Step::Scroll {
                target: Target::Point(10.0, 20.0),
                direction: Direction::Up,
                amount: 2
            }
        );
    }

    #[test]
    fn waits_for_a_value_by_ref_or_locator() {
        assert_eq!(
            Step::parse(r#"wait t1 = "Ada""#).unwrap(),
            Step::Wait {
                until: Condition::Value(Element::Ref(r("t1")), "Ada".into()),
                within: DEFAULT_WAIT
            }
        );
        let Step::Wait {
            until: Condition::Value(Element::Find(found), value),
            within,
        } = Step::parse("wait checkbox \"Wrap\" = on within 2s").unwrap()
        else {
            panic!()
        };
        assert_eq!(
            (found.to_string(), value.as_str(), within),
            (r#"checkbox "Wrap""#.into(), "on", Duration::from_secs(2))
        );
    }

    #[test]
    fn written_steps_parse_back_to_the_same_step() {
        let lines = [
            "click b3",
            "double 120,48.5",
            "right b1",
            r#"set t2 "42""#,
            r#"type t1 "say \"hi\"\n""#,
            r#"type "hello world""#,
            "key cmd+s",
            "key down x3",
            "menu File > Export > PDF…",
            r#"pick p1 "Helvetica Neue""#,
            "select i2",
            "focus t1",
            "show i9",
            "scroll e2 up 5",
            "drag i2 to 30,40",
            "wait gone e4 within 10000ms",
            r#"wait text "Saved""#,
            r#"wait t1 = "Ada""#,
            r#"wait sheet "Saved""#,
            "wait 250ms",
            "wait idle",
            "lane",
            "unlane",
        ];
        for line in lines {
            let step = Step::parse(line).unwrap();
            let written = step.write(&mut |r| r.to_string());
            assert_eq!(written, line);
            assert_eq!(Step::parse(&written).unwrap(), step, "{line}");
        }
        let step = Step::parse("click b3").unwrap();
        assert_eq!(
            step.write(&mut |_| r#"button "Save""#.into()),
            r#"click button "Save""#
        );
    }

    #[test]
    fn errors_name_what_was_expected() {
        assert!(Step::parse("fly b1").unwrap_err().contains("unknown step"));
        assert!(
            Step::parse("click save")
                .unwrap_err()
                .contains("ref like b3")
        );
        assert!(Step::parse("menu").unwrap_err().contains("path"));
        assert!(Step::parse("key cmd+s twice").unwrap_err().contains("x3"));
        assert!(Step::parse("wait forever").is_err());
        assert!(Step::parse("type").is_err());
    }
}
