//! Key events posted to one process. They never go through the global event
//! stream, so the user's front app never sees them, and they come from a
//! private event source, so keys the user is holding don't leak into them
//! (a held Shift must not turn Hush's typing into capitals or shortcuts).

use hush_core::{Key, KeyCombo, NamedKey};
use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID};

/// Characters per key event. CGEventKeyboardSetUnicodeString takes up to
/// 20; a few less leaves room for surrogate pairs at a chunk's end.
const CHUNK: usize = 16;

pub(crate) fn post_text(pid: i32, text: &str) {
    let source = CGEventSource::new(CGEventSourceStateID::Private);
    let mut pending: Vec<u16> = Vec::with_capacity(CHUNK + 2);
    for c in text.chars() {
        let named = match c {
            '\n' | '\r' => Some(NamedKey::Enter),
            '\t' => Some(NamedKey::Tab),
            _ => None,
        };
        if let Some(named) = named {
            flush(pid, source.as_deref(), &mut pending);
            press(
                pid,
                source.as_deref(),
                code(Key::Named(named)).unwrap_or(0),
                CGEventFlags::empty(),
            );
            continue;
        }
        let mut units = [0u16; 2];
        pending.extend_from_slice(c.encode_utf16(&mut units));
        if pending.len() >= CHUNK {
            flush(pid, source.as_deref(), &mut pending);
        }
    }
    flush(pid, source.as_deref(), &mut pending);
}

/// Posts a key combination. Returns false when the key has no virtual key
/// code on this layout table.
pub(crate) fn post_combo(pid: i32, combo: KeyCombo, repeat: u16) -> bool {
    let Some(key_code) = code(combo.key) else {
        return false;
    };
    let source = CGEventSource::new(CGEventSourceStateID::Private);
    let flags = flags(combo);
    for _ in 0..repeat.max(1) {
        press(pid, source.as_deref(), key_code, flags);
    }
    true
}

fn flush(pid: i32, source: Option<&CGEventSource>, pending: &mut Vec<u16>) {
    if pending.is_empty() {
        return;
    }
    for down in [true, false] {
        let Some(event) = CGEvent::new_keyboard_event(source, 0, down) else {
            continue;
        };
        CGEvent::set_flags(Some(&event), CGEventFlags::empty());
        // SAFETY: `pending` holds `len` UTF-16 units for the whole call.
        unsafe {
            CGEvent::keyboard_set_unicode_string(Some(&event), pending.len() as _, pending.as_ptr())
        };
        CGEvent::post_to_pid(pid, Some(&event));
    }
    pending.clear();
}

fn press(pid: i32, source: Option<&CGEventSource>, key_code: u16, flags: CGEventFlags) {
    for down in [true, false] {
        if let Some(event) = CGEvent::new_keyboard_event(source, key_code, down) {
            CGEvent::set_flags(Some(&event), flags);
            CGEvent::post_to_pid(pid, Some(&event));
        }
    }
}

fn flags(combo: KeyCombo) -> CGEventFlags {
    let m = combo.modifiers;
    let mut flags = CGEventFlags::empty();
    for (on, flag) in [
        (m.cmd, CGEventFlags::MaskCommand),
        (m.shift, CGEventFlags::MaskShift),
        (m.alt, CGEventFlags::MaskAlternate),
        (m.ctrl, CGEventFlags::MaskControl),
        (m.function, CGEventFlags::MaskSecondaryFn),
    ] {
        if on {
            flags |= flag;
        }
    }
    flags
}

/// macOS virtual key codes (ANSI layout positions).
pub(crate) fn code(key: Key) -> Option<u16> {
    use NamedKey::*;
    Some(match key {
        Key::Named(named) => match named {
            Enter => 0x24,
            Tab => 0x30,
            Space => 0x31,
            Backspace => 0x33,
            Escape => 0x35,
            ForwardDelete => 0x75,
            Home => 0x73,
            End => 0x77,
            PageUp => 0x74,
            PageDown => 0x79,
            Left => 0x7B,
            Right => 0x7C,
            Down => 0x7D,
            Up => 0x7E,
            F(n) => *[
                0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B,
                0x71, 0x6A, 0x40, 0x4F, 0x50, 0x5A,
            ]
            .get(usize::from(n).checked_sub(1)?)?,
        },
        Key::Char(c) => match c.to_ascii_lowercase() {
            'a' => 0x00,
            's' => 0x01,
            'd' => 0x02,
            'f' => 0x03,
            'h' => 0x04,
            'g' => 0x05,
            'z' => 0x06,
            'x' => 0x07,
            'c' => 0x08,
            'v' => 0x09,
            'b' => 0x0B,
            'q' => 0x0C,
            'w' => 0x0D,
            'e' => 0x0E,
            'r' => 0x0F,
            'y' => 0x10,
            't' => 0x11,
            '1' => 0x12,
            '2' => 0x13,
            '3' => 0x14,
            '4' => 0x15,
            '6' => 0x16,
            '5' => 0x17,
            '=' | '+' => 0x18,
            '9' => 0x19,
            '7' => 0x1A,
            '-' => 0x1B,
            '8' => 0x1C,
            '0' => 0x1D,
            ']' => 0x1E,
            'o' => 0x1F,
            'u' => 0x20,
            '[' => 0x21,
            'i' => 0x22,
            'p' => 0x23,
            'l' => 0x25,
            'j' => 0x26,
            '\'' => 0x27,
            'k' => 0x28,
            ';' => 0x29,
            '\\' => 0x2A,
            ',' => 0x2B,
            '/' => 0x2C,
            'n' => 0x2D,
            'm' => 0x2E,
            '.' => 0x2F,
            '`' => 0x32,
            ' ' => 0x31,
            _ => return None,
        },
    })
}
