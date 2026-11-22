//! Keys and pointer input on the lane, through XTest.
//!
//! XTest events are real input on the X server they're sent to, and Hush
//! only ever connects to its own display, so they can't reach the user's.
//! The lane has no window manager: before each batch the target window is
//! raised and given the keyboard focus, so input lands in it even when
//! another app Hush started overlaps it.

use std::fs::File;
use std::time::Instant;

use hush_core::{Button, Error, Input, Key, KeyCombo, NamedKey, Played, Result, Timed};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ConfigureWindowAux, ConnectionExt as _, InputFocus,
    KEY_PRESS_EVENT, KEY_RELEASE_EVENT, Keycode, MOTION_NOTIFY_EVENT, StackMode, Window,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::lane::Lane;
use crate::screen::Screen;

const SHIFT_L: u32 = 0xffe1;
const CONTROL_L: u32 = 0xffe3;
const ALT_L: u32 = 0xffe9;

/// Input for one window on the lane. Holds the lane's input lock, so two
/// Hush hosts never interleave keys or drags on the shared display.
pub(crate) struct Sender<'a> {
    connection: &'a RustConnection,
    root: Window,
    /// The window's top left, in lane coordinates.
    origin: (f64, f64),
    keys: Keymap,
    _lock: File,
}

fn x(error: impl std::fmt::Display) -> Error {
    Error::Failed(format!("Hush's display: {error}"))
}

impl<'a> Sender<'a> {
    /// Raises and focuses `window` (whose top left is `origin`) on the lane.
    pub(crate) fn to(screen: &'a Screen, window: Window, origin: (f64, f64)) -> Result<Self> {
        let lock = Lane::input_lock().map_err(x)?;
        let c = screen.connection();
        c.configure_window(
            window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )
        .map_err(x)?;
        c.set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .map_err(x)?;
        let keys = Keymap::read(c)?;
        Ok(Sender {
            connection: c,
            root: screen.root(),
            origin,
            keys,
            _lock: lock,
        })
    }

    /// Waits until the server has handled everything sent so far.
    pub(crate) fn flush(&self) -> Result<()> {
        self.connection
            .get_input_focus()
            .map_err(x)?
            .reply()
            .map(|_| ())
            .map_err(x)
    }

    fn fake(&self, kind: u8, detail: u8, at: (f64, f64)) -> Result<()> {
        let (rx, ry) = (self.origin.0 + at.0, self.origin.1 + at.1);
        self.connection
            .xtest_fake_input(
                kind,
                detail,
                x11rb::CURRENT_TIME,
                self.root,
                rx.round() as i16,
                ry.round() as i16,
                0,
            )
            .map(|_| ())
            .map_err(x)
    }

    fn key(&self, code: Keycode, down: bool) -> Result<()> {
        let kind = if down {
            KEY_PRESS_EVENT
        } else {
            KEY_RELEASE_EVENT
        };
        self.connection
            .xtest_fake_input(kind, code, x11rb::CURRENT_TIME, self.root, 0, 0, 0)
            .map(|_| ())
            .map_err(x)
    }

    /// The key codes `combo` holds down, modifiers first.
    fn codes(&self, combo: KeyCombo) -> Result<Vec<Keycode>> {
        let (key, shifted) = self
            .keys
            .find(keysym(combo.key))
            .ok_or_else(|| Error::Unsupported(format!("Hush's display has no key for {combo}")))?;
        let m = combo.modifiers;
        let mut held = Vec::new();
        // Linux apps use Control where macOS apps use Command, so cmd+s
        // saves on both.
        for (on, sym) in [
            (m.cmd || m.ctrl, CONTROL_L),
            (m.shift || shifted, SHIFT_L),
            (m.alt, ALT_L),
        ] {
            if on {
                let (code, _) = self.keys.find(sym).ok_or_else(|| x("no modifier keys"))?;
                held.push(code);
            }
        }
        held.push(key);
        Ok(held)
    }

    pub(crate) fn combo(&self, combo: KeyCombo, repeat: u16) -> Result<()> {
        let codes = self.codes(combo)?;
        for _ in 0..repeat {
            self.press(&codes)?;
        }
        self.flush()
    }

    fn press(&self, codes: &[Keycode]) -> Result<()> {
        for code in codes {
            self.key(*code, true)?;
        }
        for code in codes.iter().rev() {
            self.key(*code, false)?;
        }
        Ok(())
    }

    pub(crate) fn click(&self, at: (f64, f64), button: Button, count: u8) -> Result<()> {
        self.fake(MOTION_NOTIFY_EVENT, 0, at)?;
        for _ in 0..count.max(1) {
            self.fake(BUTTON_PRESS_EVENT, number(button), at)?;
            self.fake(BUTTON_RELEASE_EVENT, number(button), at)?;
        }
        self.flush()
    }

    /// Scrolls by lines: X buttons 4 and 5 are one line up and down, 6 and
    /// 7 one column left and right.
    pub(crate) fn scroll(&self, at: (f64, f64), dx: i32, dy: i32) -> Result<()> {
        self.fake(MOTION_NOTIFY_EVENT, 0, at)?;
        for (amount, back, forward) in [(dy, 4, 5), (dx, 6, 7)] {
            let button = if amount < 0 { back } else { forward };
            for _ in 0..amount.unsigned_abs() {
                self.fake(BUTTON_PRESS_EVENT, button, at)?;
                self.fake(BUTTON_RELEASE_EVENT, button, at)?;
            }
        }
        self.flush()
    }

    /// Plays `events` on their schedule from now.
    pub(crate) fn play(&self, events: &[Timed]) -> Result<Played> {
        // Resolve every key first, so a bad one fails before anything plays.
        let codes: Vec<Option<Vec<Keycode>>> = events
            .iter()
            .map(|e| match e.input {
                Input::KeyDown(c) | Input::KeyUp(c) => self.codes(c).map(Some),
                _ => Ok(None),
            })
            .collect::<Result<_>>()?;
        let start = Instant::now();
        let mut late = Vec::with_capacity(events.len());
        let mut at = (0.0, 0.0);
        for (event, codes) in events.iter().zip(codes) {
            let due = start + event.at;
            let now = Instant::now();
            if due > now {
                // Waits for the event's scheduled time, which is the point
                // of a timeline; flushed first so it goes out on time.
                self.flush()?;
                std::thread::sleep(due - now);
            }
            late.push(Instant::now().saturating_duration_since(due));
            match (event.input, codes) {
                (Input::KeyDown(_), Some(codes)) => {
                    for code in &codes {
                        self.key(*code, true)?;
                    }
                }
                (Input::KeyUp(_), Some(codes)) => {
                    for code in codes.iter().rev() {
                        self.key(*code, false)?;
                    }
                }
                (Input::PointerDown { x, y, button }, _) => {
                    at = (x, y);
                    self.fake(MOTION_NOTIFY_EVENT, 0, at)?;
                    self.fake(BUTTON_PRESS_EVENT, number(button), at)?;
                }
                (Input::PointerUp { x, y, button }, _) => {
                    if (x, y) != at {
                        self.fake(MOTION_NOTIFY_EVENT, 0, (x, y))?;
                    }
                    at = (x, y);
                    self.fake(BUTTON_RELEASE_EVENT, number(button), at)?;
                }
                (Input::PointerMove { x, y }, _) => {
                    at = (x, y);
                    self.fake(MOTION_NOTIFY_EVENT, 0, at)?;
                }
                _ => {}
            }
        }
        self.flush()?;
        Ok(Played { late })
    }
}

fn number(button: Button) -> u8 {
    match button {
        Button::Left => 1,
        Button::Right => 3,
    }
}

/// The X keysym for a key.
fn keysym(key: Key) -> u32 {
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
struct Keymap {
    first: Keycode,
    per_code: usize,
    syms: Vec<u32>,
}

impl Keymap {
    fn read(c: &RustConnection) -> Result<Keymap> {
        let setup = c.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let reply = c
            .get_keyboard_mapping(first, last - first + 1)
            .map_err(x)?
            .reply()
            .map_err(x)?;
        Ok(Keymap {
            first,
            per_code: usize::from(reply.keysyms_per_keycode),
            syms: reply.keysyms,
        })
    }

    /// The key code that types `sym`, and whether it needs shift.
    fn find(&self, sym: u32) -> Option<(Keycode, bool)> {
        find(&self.syms, self.per_code, sym).map(|(i, shifted)| (self.first + i as u8, shifted))
    }
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
    fn keysyms_follow_the_x_tables() {
        assert_eq!(keysym(Key::Char('a')), 0x61);
        assert_eq!(keysym(Key::Named(NamedKey::Enter)), 0xff0d);
        assert_eq!(keysym(Key::Named(NamedKey::F(12))), 0xffc9);
        assert_eq!(keysym(Key::Char('€')), 0x0100_20ac);
    }
}
