//! Input on a millisecond schedule: the agent plans once, the platform plays
//! it locally, so the timing doesn't depend on model or tool round trips.
//!
//! One event per line, `MS VERB ARGS`, where MS is the time from the start:
//!
//! ```text
//! 0 down a          key a goes down (modifiers allowed: shift+a)
//! 400 up a          and comes back up
//! 0 tap e 380       down at 0, up 380 ms later (default 40 ms)
//! 500 down 120,80   pointer press at window points 120,80 (add `right`)
//! 520 move 160,80   pointer moves while pressed (a drag)
//! 540 up 160,80     pointer release
//! 600 tap 120,80    a click: press, release 40 ms later
//! ```

use std::time::Duration;

use crate::key::KeyCombo;
use crate::step::Button;

/// The grammar, for tool descriptions.
pub const SYNTAX: &str = "One event per line, MS VERB ARGS (MS = time from the start): \
MS down KEY · MS up KEY · MS tap KEY [HOLD_MS] · MS down X,Y [right] · MS up X,Y [right] · \
MS move X,Y · MS tap X,Y [HOLD_MS] [right]. KEY like a, shift+a, space, left. \
X,Y are window points. HOLD_MS defaults to 40.";

/// Longest timeline one call plays.
pub const MAX_LENGTH: Duration = Duration::from_secs(600);
/// Most events one call plays.
pub const MAX_EVENTS: usize = 20_000;
const DEFAULT_HOLD: Duration = Duration::from_millis(40);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    KeyDown(KeyCombo),
    KeyUp(KeyCombo),
    PointerDown { x: f64, y: f64, button: Button },
    PointerUp { x: f64, y: f64, button: Button },
    PointerMove { x: f64, y: f64 },
}

impl Input {
    pub fn is_pointer(&self) -> bool {
        !matches!(self, Input::KeyDown(_) | Input::KeyUp(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    /// From the start of the timeline.
    pub at: Duration,
    pub input: Input,
}

/// The events of `lines` in the order they play: by time, and in line order
/// for equal times (so `up` before `down` at the same instant stays that
/// way, which re-strikes a held key).
pub fn parse(lines: &[String]) -> Result<Vec<Timed>, String> {
    let mut events = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        parse_line(line, &mut events).map_err(|e| format!("line {} (`{line}`): {e}", i + 1))?;
        if events.len() > MAX_EVENTS {
            return Err(format!("more than {MAX_EVENTS} events; split the timeline"));
        }
    }
    if events.is_empty() {
        return Err("the timeline has no events".into());
    }
    events.sort_by_key(|e| e.at);
    let end = events.last().map_or(Duration::ZERO, |e| e.at);
    if end > MAX_LENGTH {
        return Err(format!(
            "the timeline runs {:.0} s; the longest one call plays is {} s",
            end.as_secs_f64(),
            MAX_LENGTH.as_secs()
        ));
    }
    Ok(events)
}

fn parse_line(line: &str, events: &mut Vec<Timed>) -> Result<(), String> {
    let mut words = line.split_whitespace();
    let at = millis(words.next().unwrap_or_default())?;
    let verb = words.next().ok_or("missing a verb (down, up, tap, move)")?;
    let target = words.next().ok_or("missing a key or X,Y")?;
    let rest: Vec<&str> = words.collect();

    let point = point(target);
    let button = if rest.contains(&"right") {
        Button::Right
    } else {
        Button::Left
    };
    let hold = rest
        .iter()
        .find(|w| **w != "right")
        .map(|w| millis(w))
        .transpose()?
        .unwrap_or(DEFAULT_HOLD);
    if let Some(extra) = rest
        .iter()
        .filter(|w| **w != "right")
        .nth(usize::from(verb == "tap"))
    {
        return Err(format!("unexpected '{extra}'"));
    }
    let mut push = |at: Duration, input: Input| events.push(Timed { at, input });
    match (verb, point) {
        ("down", Some((x, y))) => push(at, Input::PointerDown { x, y, button }),
        ("up", Some((x, y))) => push(at, Input::PointerUp { x, y, button }),
        ("move", Some((x, y))) => push(at, Input::PointerMove { x, y }),
        ("tap", Some((x, y))) => {
            push(at, Input::PointerDown { x, y, button });
            push(at + hold, Input::PointerUp { x, y, button });
        }
        ("down", None) => push(at, Input::KeyDown(KeyCombo::parse(target)?)),
        ("up", None) => push(at, Input::KeyUp(KeyCombo::parse(target)?)),
        ("tap", None) => {
            let key = KeyCombo::parse(target)?;
            push(at, Input::KeyDown(key));
            push(at + hold, Input::KeyUp(key));
        }
        ("move", None) => return Err("move needs X,Y".into()),
        (other, _) => return Err(format!("unknown verb '{other}' (down, up, tap, move)")),
    }
    Ok(())
}

fn millis(word: &str) -> Result<Duration, String> {
    let number = word.strip_suffix("ms").unwrap_or(word);
    match number.parse::<f64>() {
        // Bounded before converting: a huge number would overflow Duration.
        Ok(ms) if ms > MAX_LENGTH.as_secs_f64() * 1000.0 => Err(format!(
            "{word} ms is past the longest timeline one call plays ({} s)",
            MAX_LENGTH.as_secs()
        )),
        Ok(ms) if ms >= 0.0 => Ok(Duration::from_secs_f64(ms / 1000.0)),
        _ => Err(format!("'{word}' isn't a time in milliseconds")),
    }
}

fn point(word: &str) -> Option<(f64, f64)> {
    let (x, y) = word.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// How late each event was posted against its schedule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Played {
    pub late: Vec<Duration>,
}

impl Played {
    /// One line: how many events, over how long, and the timing error.
    pub fn summary(&self, length: Duration) -> String {
        let mut late = self.late.clone();
        late.sort();
        let at = |q: f64| {
            late.get(((late.len() as f64 - 1.0) * q).round() as usize)
                .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
        };
        format!(
            "played {} events over {:.2} s · late by {:.2} ms median, {:.2} ms at 99%, {:.2} ms worst",
            late.len(),
            length.as_secs_f64(),
            at(0.5),
            at(0.99),
            at(1.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn taps_expand_and_events_sort_by_time() {
        let events = parse(&lines(
            "100 tap e 380\n0 down shift+a\n# a comment\n50 up shift+a",
        ))
        .unwrap();
        let times: Vec<Duration> = events.iter().map(|e| e.at).collect();
        assert_eq!(times, [ms(0), ms(50), ms(100), ms(480)]);
        assert!(matches!(events[2].input, Input::KeyDown(k) if k.key == Key::Char('e')));
        assert!(matches!(events[0].input, Input::KeyDown(k) if k.modifiers.shift));
    }

    #[test]
    fn pointer_events_take_points_and_an_optional_right_button() {
        let events = parse(&lines(
            "0 down 10,20\n20 move 30,20\n40 up 30,20\n60 tap 5,5 right",
        ))
        .unwrap();
        assert_eq!(
            events[0].input,
            Input::PointerDown {
                x: 10.0,
                y: 20.0,
                button: Button::Left
            }
        );
        assert_eq!(events[1].input, Input::PointerMove { x: 30.0, y: 20.0 });
        assert_eq!(events[4].at, ms(100));
        assert!(matches!(
            events[4].input,
            Input::PointerUp {
                button: Button::Right,
                ..
            }
        ));
    }

    #[test]
    fn equal_times_keep_line_order() {
        let events = parse(&lines("0 down c\n500 up c\n500 down c")).unwrap();
        assert!(matches!(events[1].input, Input::KeyUp(_)));
        assert!(matches!(events[2].input, Input::KeyDown(_)));
    }

    #[test]
    fn mistakes_name_the_line() {
        for (text, wanted) in [
            ("x down a", "line 1"),
            ("0 hop a", "unknown verb"),
            ("0 move a", "move needs X,Y"),
            ("0 down a\n5 tap b 10 20", "line 2"),
            ("0 down hyper+a", "unknown modifier"),
            ("700000 down a", "longest"),
            ("1e300 down a", "longest"),
            ("0 tap a 1e300", "longest"),
            ("NaN down a", "isn't a time"),
            ("inf down a", "longest"),
            ("# nothing", "no events"),
        ] {
            let error = parse(&lines(text)).unwrap_err();
            assert!(error.contains(wanted), "{text}: {error}");
        }
    }

    #[test]
    fn the_summary_reports_percentiles() {
        let played = Played {
            late: (1..=100).map(|n| Duration::from_micros(n * 10)).collect(),
        };
        let line = played.summary(Duration::from_secs(2));
        assert!(line.starts_with("played 100 events over 2.00 s"), "{line}");
        assert!(line.contains("0.51 ms median"), "{line}");
        assert!(line.contains("1.00 ms worst"), "{line}");
    }
}
