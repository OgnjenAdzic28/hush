//! The lane's keyboard: which key code types which keysym, and spare key
//! codes to put a character on when no key types it.

use std::cell::RefCell;

use hush_core::{Error, Key, NamedKey, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, Keycode};
use x11rb::rust_connection::RustConnection;

fn x(error: impl std::fmt::Display) -> Error {
    Error::Failed(format!("Hush's display: {error}"))
}

/// The X keysym for a key.
pub(crate) fn keysym(key: Key) -> u32 {
    use NamedKey::*;
    match key {
        // Latin-1 keysyms are the characters' code points; the rest are
        // the code point plus 0x01000000.
        Key::Char(c) if (' '..='~').contains(&c) || ('\u{a0}'..='\u{ff}').contains(&c) => c as u32,
        Key::Char(c) => 0x0100_0000 + c as u32,
        Key::Named(named) => match named {
            Enter => 0xff0d,
            Tab => 0xff09,
            Space => 0x20,
            Escape => 0xff1b,
            Backspace => 0xff08,
            ForwardDelete => 0xffff,
            Left => 0xff51,
            Up => 0xff52,
            Right => 0xff53,
            Down => 0xff54,
            Home => 0xff50,
            End => 0xff57,
            PageUp => 0xff55,
            PageDown => 0xff56,
            F(n) => 0xffbe + u32::from(n) - 1,
        },
    }
}

/// The lane's keyboard: keysyms by key code, plain and shifted.
pub(crate) struct Keymap {
    first: Keycode,
    per_code: usize,
    syms: RefCell<Vec<u32>>,
    /// Key codes with no keysym, taken in turn for characters no key types.
    spare: Vec<Keycode>,
    next_spare: std::cell::Cell<usize>,
}

impl Keymap {
    pub(crate) fn read(c: &RustConnection) -> Result<Keymap> {
        let setup = c.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let reply = c
            .get_keyboard_mapping(first, last - first + 1)
            .map_err(x)?
            .reply()
            .map_err(x)?;
        let per_code = usize::from(reply.keysyms_per_keycode);
        let spare = spare(&reply.keysyms, per_code)
            .map(|i| first + i as u8)
            .collect();
        Ok(Keymap {
            first,
            per_code,
            syms: RefCell::new(reply.keysyms),
            spare,
            next_spare: Default::default(),
        })
    }

    /// The key code that types `sym`, and whether it needs shift.
    pub(crate) fn find(&self, sym: u32) -> Option<(Keycode, bool)> {
        find(&self.syms.borrow(), self.per_code, sym)
            .map(|(i, shifted)| (self.first + i as u8, shifted))
    }

    /// A key code that types `sym`: its own key, or else a spare one given
    /// `sym` for now (as xdotool does). The server tells every client of the
    /// change before the key's next press, so the app reads it as `sym`.
    pub(crate) fn typing(&self, c: &RustConnection, sym: u32) -> Result<(Keycode, bool)> {
        if let Some(found) = self.find(sym) {
            return Ok(found);
        }
        let turn = self.next_spare.get();
        let code = *self
            .spare
            .get(turn % self.spare.len().max(1))
            .ok_or_else(|| {
                Error::Unsupported("Hush's display has no spare key to type that character".into())
            })?;
        self.next_spare.set(turn + 1);
        let row = vec![sym; self.per_code];
        c.change_keyboard_mapping(1, code, self.per_code as u8, &row)
            .map_err(x)?;
        let at = usize::from(code - self.first) * self.per_code;
        self.syms.borrow_mut()[at..at + self.per_code].copy_from_slice(&row);
        Ok((code, false))
    }
}

/// Indexes of keys with no keysym at all.
fn spare(syms: &[u32], per_code: usize) -> impl Iterator<Item = usize> + '_ {
    syms.chunks(per_code.max(1))
        .enumerate()
        .filter(|(_, row)| row.iter().all(|s| *s == 0))
        .map(|(i, _)| i)
}

/// The key index whose plain (else shifted) keysym is `sym`.
fn find(syms: &[u32], per_code: usize, sym: u32) -> Option<(usize, bool)> {
    if per_code == 0 {
        return None;
    }
    let codes = || syms.chunks(per_code).enumerate();
    codes()
        .find(|(_, row)| row.first() == Some(&sym))
        .map(|(i, _)| (i, false))
        .or_else(|| {
            codes()
                .find(|(_, row)| row.get(1) == Some(&sym))
                .map(|(i, _)| (i, true))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_win_over_shifted_ones() {
        // Two keys of two columns: "1 !" and "= +".
        let syms = [0x31, 0x21, 0x3d, 0x2b];
        assert_eq!(find(&syms, 2, 0x31), Some((0, false)));
        assert_eq!(find(&syms, 2, 0x2b), Some((1, true)));
        assert_eq!(find(&syms, 2, 0x7a), None);
    }

    #[test]
    fn spare_keys_are_the_empty_ones() {
        let syms = [0x31, 0x21, 0, 0, 0x3d, 0];
        assert_eq!(spare(&syms, 2).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn keysyms_follow_the_x_tables() {
        assert_eq!(keysym(Key::Char('a')), 0x61);
        assert_eq!(keysym(Key::Named(NamedKey::Enter)), 0xff0d);
        assert_eq!(keysym(Key::Named(NamedKey::F(12))), 0xffc9);
        assert_eq!(keysym(Key::Char('€')), 0x0100_20ac);
    }
}
