//! Steps of an `act` batch, written as short commands (`click b3`,
//! `type t1 "hello"`, `key cmd+s`). One line each costs a model a few tokens,
//! a third of the equivalent JSON objects.

use std::time::Duration;

use crate::key::KeyCombo;
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

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    /// The app finished reacting (the default after every step).
    Settled,
    Exists(Ref),
    Gone(Ref),
    /// Some element's name or value contains the text.
    Text(String),
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
    /// it renders and takes pointer input without being on screen.
    Lane,
    /// Put the window back where it was before `Lane`.
    Unlane,
}

/// The longest a `wait` may run unless it says otherwise.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(5);

/// Syntax help for tool descriptions. Kept short: it rides on every request.
pub const SYNTAX: &str = "click REF|X,Y · double REF · right REF · set REF VALUE · type [REF] TEXT · \
key COMBO [xN] · menu A > B · pick REF OPTION · select REF · focus REF · \
scroll REF up|down|left|right [N] · show REF · drag REF|X,Y to REF|X,Y (to a ref: takes its place) · wait gone REF|REF|text \"T\"|500ms [within 10s] · \
lane (move the window off screen, where it still renders and takes clicks) · unlane (put it back)";

impl Step {
    pub fn parse(line: &str) -> Result<Step, String> {
        let line = line.trim();
        let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        let step = match verb.to_lowercase().as_str() {
            "click" => click(rest, Button::Left, 1)?,
            "double" => click(rest, Button::Left, 2)?,
            "right" => click(rest, Button::Right, 1)?,
            "drag" => drag(rest)?,
            "set" => {
                let (r, value) = ref_then_rest(rest)?;
                Step::Set {
                    r,
                    value: text(value),
                }
            }
            "type" => {
                let (first, after) = split_word(rest);
                match Ref::parse(first) {
                    Some(r) if !after.is_empty() => Step::Type {
                        into: Some(r),
                        text: text(after),
                    },
                    _ => Step::Type {
                        into: None,
                        text: text(rest),
                    },
                }
            }
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
                let (r, option) = ref_then_rest(rest)?;
                Step::Pick {
                    r,
                    option: text(option),
                }
            }
            "select" => Step::Select { r: only_ref(rest)? },
            "focus" => Step::Focus { r: only_ref(rest)? },
            "show" => Step::Show { r: only_ref(rest)? },
            "scroll" => scroll(rest)?,
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
}

fn click(rest: &str, button: Button, count: u8) -> Result<Step, String> {
    Ok(Step::Click {
        target: target(rest)?,
        button,
        count,
    })
}

fn drag(rest: &str) -> Result<Step, String> {
    let words: Vec<&str> = rest.split_whitespace().filter(|w| *w != "to").collect();
    match words[..] {
        [from, to] => Ok(Step::Drag {
            from: target(from)?,
            to: target(to)?,
        }),
        _ => Err(format!("drag needs FROM to TO, got '{rest}'")),
    }
}

fn scroll(rest: &str) -> Result<Step, String> {
    let mut words = rest.split_whitespace();
    let target = target(words.next().unwrap_or(""))?;
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

fn wait(rest: &str) -> Result<Step, String> {
    let (condition, within) = match rest.rsplit_once(" within ") {
        Some((condition, limit)) => (condition.trim(), duration(limit.trim())?),
        None => (rest, DEFAULT_WAIT),
    };
    let (word, after) = split_word(condition);
    let until = match word {
        "" | "idle" => Condition::Settled,
        "gone" => Condition::Gone(only_ref(after)?),
        "text" => Condition::Text(text(after)),
        _ => match (Ref::parse(word), duration(word)) {
            (Some(r), _) if after.is_empty() => Condition::Exists(r),
            (_, Ok(elapsed)) if after.is_empty() => Condition::Elapsed(elapsed),
            _ => {
                return Err(format!(
                    "wait needs gone REF, REF, text \"T\" or a time, got '{condition}'"
                ));
            }
        },
    };
    Ok(Step::Wait { until, within })
}

fn target(word: &str) -> Result<Target, String> {
    if let Some(r) = Ref::parse(word) {
        return Ok(Target::Ref(r));
    }
    if let Some((x, y)) = word.split_once(',')
        && let (Ok(x), Ok(y)) = (x.trim().parse(), y.trim().parse())
    {
        return Ok(Target::Point(x, y));
    }
    Err(format!(
        "expected a ref like b3 or a point like 120,48, got '{word}'"
    ))
}

fn only_ref(word: &str) -> Result<Ref, String> {
    Ref::parse(word.trim()).ok_or_else(|| format!("expected a ref like b3, got '{word}'"))
}

fn ref_then_rest(rest: &str) -> Result<(Ref, &str), String> {
    let (first, after) = split_word(rest);
    Ok((only_ref(first)?, after))
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
                until: Condition::Gone(r("e4")),
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
