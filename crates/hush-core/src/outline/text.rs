//! How one element is written: its head (role and name) and tail (value
//! and states).

use std::fmt::Write;

use crate::model::{Node, Role, Snapshot, States};

pub(super) fn header<H>(snapshot: &Snapshot<H>) -> String {
    let window = &snapshot.window;
    let mut out = format!(
        "# {} · window \"{}\" {}x{}",
        snapshot.app.name,
        clip(&window.title, 60),
        window.frame.w.round(),
        window.frame.h.round()
    );
    if !snapshot.menus.is_empty() {
        let _ = write!(out, "\n# menus: {}", snapshot.menus.join(" · "));
    }
    out
}

pub(super) fn head(role: Role, name: &str, max: usize) -> String {
    if name.is_empty() {
        role.word().to_owned()
    } else {
        format!("{} \"{}\"", role.word(), clip(name, max))
    }
}

pub(super) fn tail<H>(node: &Node<H>, name: &str, max: usize) -> String {
    let mut out = String::new();
    let value = node.value.as_deref().unwrap_or("").trim();
    match node.role {
        Role::SecureField => out.push_str(if value.is_empty() {
            " = \"\""
        } else {
            " = ••••"
        }),
        Role::CheckBox | Role::Radio | Role::Switch => {
            out.push_str(if node.states.has(States::MIXED) {
                " = mixed"
            } else if node.states.has(States::CHECKED) {
                " = on"
            } else {
                " = off"
            })
        }
        Role::Slider | Role::Stepper | Role::Progress if !value.is_empty() => {
            let _ = write!(out, " = {value}");
        }
        role if role.is_text_input() => {
            let length = value.chars().count();
            if length > max {
                let _ = write!(out, " = \"{}\" ({length} chars)", clip(value, max));
            } else {
                let _ = write!(out, " = \"{}\"", clip(value, max));
            }
        }
        _ if !value.is_empty() && value != name => {
            let _ = write!(out, " = \"{}\"", clip(value, max));
        }
        _ => {}
    }

    let mut states = Vec::new();
    for (flag, word) in [
        (States::DISABLED, "disabled"),
        (States::SELECTED, "selected"),
        (States::EXPANDED, "expanded"),
        (States::BUSY, "busy"),
    ] {
        if node.states.has(flag) {
            states.push(word);
        }
    }
    if !states.is_empty() {
        let _ = write!(out, " [{}]", states.join(", "));
    }
    out
}

/// Cuts to `max` characters on one line, escaping quotes and newlines.
fn clip(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (count, c) in text.chars().enumerate() {
        if count == max {
            out.push('…');
            break;
        }
        match c {
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push_str("\\n"),
            '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}
