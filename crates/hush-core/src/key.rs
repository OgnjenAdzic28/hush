//! Key combinations like `cmd+shift+z`, named the same way on every platform.

use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub function: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        *self == Modifiers::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    Escape,
    Backspace,
    ForwardDelete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1 to F20.
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable character, lowercase for letters.
    Char(char),
    Named(NamedKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeyCombo {
    /// Parses `cmd+s`, `Shift+Tab`, `enter`, `cmd++`. Modifier aliases:
    /// cmd/command/meta/super/win, alt/opt/option, ctrl/control, fn.
    pub fn parse(text: &str) -> Result<KeyCombo, String> {
        let lower = text.trim().to_lowercase();
        if lower.is_empty() {
            return Err("empty key".into());
        }
        let (mods_part, key_part) = if let Some(rest) = lower.strip_suffix("++") {
            (rest, "+")
        } else {
            match lower.rsplit_once('+') {
                Some((mods, key)) => (mods, key),
                None => ("", lower.as_str()),
            }
        };
        let mut modifiers = Modifiers::default();
        for name in mods_part.split('+').filter(|part| !part.is_empty()) {
            match name {
                "cmd" | "command" | "meta" | "super" | "win" => modifiers.cmd = true,
                "shift" => modifiers.shift = true,
                "alt" | "opt" | "option" => modifiers.alt = true,
                "ctrl" | "control" => modifiers.ctrl = true,
                "fn" => modifiers.function = true,
                other => return Err(format!("unknown modifier '{other}' in '{text}'")),
            }
        }
        Ok(KeyCombo {
            modifiers,
            key: parse_key(key_part)
                .ok_or_else(|| format!("unknown key '{key_part}' in '{text}'"))?,
        })
    }
}

fn parse_key(name: &str) -> Option<Key> {
    use NamedKey::*;
    let named = match name {
        "enter" | "return" => Enter,
        "tab" => Tab,
        "space" => Space,
        "esc" | "escape" => Escape,
        "backspace" | "delete" => Backspace,
        "forwarddelete" | "del" => ForwardDelete,
        "up" => Up,
        "down" => Down,
        "left" => Left,
        "right" => Right,
        "home" => Home,
        "end" => End,
        "pageup" => PageUp,
        "pagedown" => PageDown,
        "plus" => return Some(Key::Char('+')),
        "minus" => return Some(Key::Char('-')),
        _ => {
            if let Some(number) = name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
                return (1..=20).contains(&number).then_some(Key::Named(F(number)));
            }
            let mut chars = name.chars();
            let c = chars.next()?;
            return chars.next().is_none().then_some(Key::Char(c));
        }
    };
    Some(Key::Named(named))
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = &self.modifiers;
        for (on, name) in [
            (m.ctrl, "ctrl"),
            (m.alt, "alt"),
            (m.shift, "shift"),
            (m.cmd, "cmd"),
            (m.function, "fn"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        match self.key {
            Key::Char(c) => write!(f, "{c}"),
            Key::Named(NamedKey::F(n)) => write!(f, "f{n}"),
            Key::Named(named) => write!(f, "{}", format!("{named:?}").to_lowercase()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combos_and_aliases() {
        let undo = KeyCombo::parse("Cmd+Shift+Z").unwrap();
        assert!(undo.modifiers.cmd && undo.modifiers.shift && !undo.modifiers.alt);
        assert_eq!(undo.key, Key::Char('z'));
        assert_eq!(KeyCombo::parse("cmd++").unwrap().key, Key::Char('+'));
        assert_eq!(
            KeyCombo::parse("return").unwrap().key,
            Key::Named(NamedKey::Enter)
        );
        assert_eq!(KeyCombo::parse("opt+f12").unwrap().to_string(), "alt+f12");
    }

    #[test]
    fn rejects_unknown_names_with_the_reason() {
        assert!(KeyCombo::parse("hyper+a").unwrap_err().contains("hyper"));
        assert!(KeyCombo::parse("cmd+f99").is_err());
        assert!(KeyCombo::parse("cmd+ab").is_err());
    }
}
