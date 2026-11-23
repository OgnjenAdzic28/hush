//! Naming an element the way the outline shows it: `button "Save"`,
//! `field[2]`, `row[3] "Inbox"`. A ref is only good for the session that saw
//! it; a locator works on a window nobody has read yet, so an agent can act
//! without observing first and a saved test replays in a fresh session.
//!
//! A locator matches outline lines by their head. The name matches exactly
//! first, then ignoring case, then as part of the name; the first of those
//! that matches anything decides. `[N]` picks the Nth match in outline order.

use std::fmt;

use crate::model::Role;
use crate::outline::Line;
use crate::refs::Ref;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locator {
    pub role: Role,
    /// `None` matches every element of the role, so it needs `nth` unless
    /// there is only one.
    pub name: Option<String>,
    /// 1-based, among the matches.
    pub nth: Option<usize>,
}

/// Why a locator found no single element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Miss {
    /// Nothing matched. Holds up to eight elements of the role, to choose from.
    None(Vec<String>),
    /// Several matched and no `[N]` said which.
    Many(Vec<Ref>),
    /// `[N]` is past the last match.
    Past(usize),
}

impl Locator {
    /// Reads a locator from the start of `text` and returns the rest. `None`
    /// when `text` doesn't start with one: a role word followed by a quoted
    /// name, `[N]`, or both.
    pub fn parse(text: &str) -> Option<(Locator, &str)> {
        let text = text.trim_start();
        let word_end = text
            .find(|c: char| !c.is_ascii_lowercase())
            .unwrap_or(text.len());
        let role = Role::from_word(&text[..word_end])?;
        let mut rest = &text[word_end..];
        let mut nth = None;
        if let Some(after) = rest.strip_prefix('[') {
            let (digits, after) = after.split_once(']')?;
            nth = Some(digits.parse().ok().filter(|&n: &usize| n > 0)?);
            rest = after;
        } else if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let name = match take_quoted(rest.trim_start()) {
            Some((name, after)) => {
                rest = after;
                Some(name)
            }
            None if nth.is_some() => None,
            None => return None,
        };
        Some((Locator { role, name, nth }, rest.trim_start()))
    }

    /// The one line this locator names.
    pub fn find(&self, lines: &[Line]) -> Result<Ref, Miss> {
        let matches = self.matches(lines);
        match (self.nth, matches.as_slice()) {
            (None, []) | (Some(_), []) => Err(Miss::None(
                lines
                    .iter()
                    .filter(|line| {
                        split_head(&line.head).is_some_and(|(role, _)| role == self.role)
                    })
                    .take(8)
                    .map(|line| format!("{} {}", line.r, line.head))
                    .collect(),
            )),
            (None, [one]) => Ok(*one),
            (None, many) => Err(Miss::Many(many.to_vec())),
            (Some(n), found) => found.get(n - 1).copied().ok_or(Miss::Past(found.len())),
        }
    }

    /// The shortest locator that finds `r` in `lines` again: its role and
    /// name, with `[N]` when that alone is ambiguous.
    pub fn of(lines: &[Line], r: Ref) -> Option<Locator> {
        let line = lines.iter().find(|line| line.r == r)?;
        let (role, name) = split_head(&line.head)?;
        let mut locator = Locator {
            role,
            name: (!name.is_empty()).then(|| name.trim_end_matches('…').to_owned()),
            nth: None,
        };
        if locator.find(lines) == Ok(r) {
            return Some(locator);
        }
        let at = locator.matches(lines).iter().position(|&m| m == r)?;
        locator.nth = Some(at + 1);
        Some(locator)
    }

    /// Refs of the matching lines, in outline order, at the strictest level
    /// of name matching that finds any.
    fn matches(&self, lines: &[Line]) -> Vec<Ref> {
        let candidates: Vec<(Ref, String)> = lines
            .iter()
            .filter_map(|line| {
                let (role, name) = split_head(&line.head)?;
                (role == self.role).then_some((line.r, name))
            })
            .collect();
        let Some(wanted) = &self.name else {
            return candidates.into_iter().map(|(r, _)| r).collect();
        };
        let lower = wanted.to_lowercase();
        let levels: [&dyn Fn(&str) -> bool; 3] = [
            // A long name is clipped with `…` in the outline.
            &|name| {
                name == wanted
                    || name
                        .strip_suffix('…')
                        .is_some_and(|start| wanted.starts_with(start))
            },
            &|name| name.to_lowercase() == lower,
            &|name| name.to_lowercase().contains(&lower),
        ];
        for level in levels {
            let found: Vec<Ref> = candidates
                .iter()
                .filter(|(_, name)| level(name))
                .map(|(r, _)| *r)
                .collect();
            if !found.is_empty() {
                return found;
            }
        }
        Vec::new()
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.role.word())?;
        if let Some(n) = self.nth {
            write!(f, "[{n}]")?;
        }
        if let Some(name) = &self.name {
            f.write_str(" \"")?;
            for c in name.chars() {
                match c {
                    '"' => f.write_str("\\\"")?,
                    '\\' => f.write_str("\\\\")?,
                    '\n' => f.write_str("\\n")?,
                    c => write!(f, "{c}")?,
                }
            }
            f.write_str("\"")?;
        }
        Ok(())
    }
}

impl Miss {
    /// Says what went wrong for `locator`, and what to write instead.
    pub fn explain(&self, locator: &Locator) -> String {
        match self {
            Miss::None(others) if others.is_empty() => {
                format!(
                    "no {locator} in the window, and no {} at all",
                    locator.role.word()
                )
            }
            Miss::None(others) => {
                format!("no {locator} in the window; it has {}", others.join(", "))
            }
            Miss::Many(refs) => {
                let list: Vec<String> = refs.iter().take(8).map(Ref::to_string).collect();
                let first = Locator {
                    nth: Some(1),
                    ..locator.clone()
                };
                format!(
                    "{locator} matches {}; say {first} or use a ref",
                    list.join(", ")
                )
            }
            Miss::Past(count) => format!("{locator}: only {count} match"),
        }
    }
}

/// An outline head (`button "Save"`, `textarea`) as its role and name, with
/// the outline's escapes undone.
fn split_head(head: &str) -> Option<(Role, String)> {
    match head.split_once(' ') {
        None => Some((Role::from_word(head)?, String::new())),
        Some((word, quoted)) => {
            let (name, rest) = take_quoted(quoted)?;
            rest.is_empty().then_some((Role::from_word(word)?, name))
        }
    }
}

/// The inside of a double-quoted string at the start of `text`, with `\"`,
/// `\\`, `\n` and `\t` undone, and the text after the closing quote.
pub(crate) fn take_quoted(text: &str) -> Option<(String, &str)> {
    let inner = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = inner.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Some((out, &inner[at + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(heads: &[&str]) -> Vec<Line> {
        heads
            .iter()
            .enumerate()
            .map(|(i, head)| Line {
                r: Ref::parse(&format!("e{}", i + 1)).unwrap(),
                depth: 0,
                head: (*head).to_owned(),
                tail: String::new(),
            })
            .collect()
    }

    fn r(text: &str) -> Ref {
        Ref::parse(text).unwrap()
    }

    fn locator(text: &str) -> Locator {
        let (locator, rest) = Locator::parse(text).unwrap();
        assert!(rest.is_empty(), "{text} left {rest}");
        locator
    }

    #[test]
    fn parses_a_role_with_a_name_an_index_or_both() {
        let (found, rest) = Locator::parse(r#"button "Don't \"Save\"" "after""#).unwrap();
        assert_eq!(found.name.as_deref(), Some("Don't \"Save\""));
        assert_eq!(rest, r#""after""#);
        assert_eq!(locator("field[2]").nth, Some(2));
        assert_eq!(
            locator(r#"row[3] "Inbox""#).to_string(),
            r#"row[3] "Inbox""#
        );
        for not_one in [
            "button",
            "b3",
            "hello world",
            "field[0]",
            "buttons \"x\"",
            "\"Save\"",
        ] {
            assert!(Locator::parse(not_one).is_none(), "{not_one}");
        }
    }

    #[test]
    fn exact_names_win_over_case_and_part_matches() {
        let window = lines(&[
            r#"button "Save as…""#,
            r#"button "save""#,
            r#"button "Save""#,
            r#"field "Save""#,
        ]);
        assert_eq!(locator(r#"button "Save""#).find(&window), Ok(r("e3")));
        assert_eq!(
            locator(r#"button "SAVE""#).find(&window),
            Err(Miss::Many(vec![r("e2"), r("e3")]))
        );
        assert_eq!(locator(r#"button "as""#).find(&window), Ok(r("e1")));
        assert_eq!(locator(r#"button[2] "SAVE""#).find(&window), Ok(r("e3")));
        assert_eq!(locator("field[1]").find(&window), Ok(r("e4")));
        assert_eq!(locator("field[2]").find(&window), Err(Miss::Past(1)));
    }

    #[test]
    fn a_clipped_name_still_matches_the_whole_name() {
        let window = lines(&[r#"text "Meeting notes, 13 Nov…""#]);
        assert_eq!(
            locator(r#"text "Meeting notes, 13 Nov. Ship the beta""#).find(&window),
            Ok(r("e1"))
        );
    }

    #[test]
    fn misses_list_what_is_there() {
        let window = lines(&[r#"button "Cancel""#, r#"button "OK""#, "textarea"]);
        let wanted = locator(r#"button "Save""#);
        let miss = wanted.find(&window).unwrap_err();
        assert_eq!(
            miss.explain(&wanted),
            r#"no button "Save" in the window; it has e1 button "Cancel", e2 button "OK""#
        );
        let twice = lines(&[r#"button "OK""#, r#"button "OK""#]);
        let ok = locator(r#"button "OK""#);
        assert_eq!(
            ok.find(&twice).unwrap_err().explain(&ok),
            r#"button "OK" matches e1, e2; say button[1] "OK" or use a ref"#
        );
    }

    #[test]
    fn of_gives_back_a_locator_that_finds_the_same_element() {
        let window = lines(&[
            r#"button "OK""#,
            "textarea",
            r#"button "OK""#,
            r#"link "Say \"hi\"""#,
            "textarea",
        ]);
        for line in &window {
            let found = Locator::of(&window, line.r).unwrap();
            assert_eq!(found.find(&window), Ok(line.r), "{found}");
        }
        assert_eq!(
            Locator::of(&window, r("e3")).unwrap().to_string(),
            r#"button[2] "OK""#
        );
        assert_eq!(
            Locator::of(&window, r("e2")).unwrap().to_string(),
            "textarea[1]"
        );
        assert_eq!(
            Locator::of(&window, r("e4")).unwrap().to_string(),
            r#"link "Say \"hi\"""#
        );
    }
}
