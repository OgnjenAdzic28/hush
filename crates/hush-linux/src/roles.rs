//! AT-SPI roles and states onto Hush's platform-neutral ones. Pure, so it is
//! tested on every system.

// Off Linux this module only exists for its tests.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use hush_core::{Actions, Role, States};

/// What to do with an element of a given AT-SPI role.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mapped {
    Role(Role),
    /// Scroll bars, separators and the like: never worth reading.
    Skip,
}

/// AT-SPI state numbers (`AtspiStateType`) Hush reads.
pub(crate) mod state {
    pub const ACTIVE: u32 = 1;
    pub const BUSY: u32 = 3;
    pub const CHECKED: u32 = 4;
    pub const EDITABLE: u32 = 7;
    pub const ENABLED: u32 = 8;
    pub const EXPANDED: u32 = 10;
    pub const FOCUSED: u32 = 12;
    pub const ICONIFIED: u32 = 15;
    pub const MULTI_LINE: u32 = 17;
    pub const PRESSED: u32 = 20;
    pub const SELECTED: u32 = 23;
    pub const SENSITIVE: u32 = 24;
    pub const SHOWING: u32 = 25;
    pub const INDETERMINATE: u32 = 32;
}

/// The state set `GetState` returns: a bit field split over 32-bit words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StateSet(pub u64);

impl StateSet {
    pub fn from_words(words: &[u32]) -> StateSet {
        let low = words.first().copied().unwrap_or(0) as u64;
        let high = words.get(1).copied().unwrap_or(0) as u64;
        StateSet(low | high << 32)
    }

    pub fn has(self, state: u32) -> bool {
        state < 64 && self.0 & (1 << state) != 0
    }
}

/// `role` is the unlocalized name `GetRoleName` returns, like "push button".
pub(crate) fn map(role: &str, states: StateSet) -> Mapped {
    use Role::*;
    let mapped = match role {
        "frame" | "window" => Window,
        "dialog" | "alert" | "file chooser" | "color chooser" | "font chooser" => Dialog,
        "push button" | "push button menu" => Button,
        "toggle button" => Button,
        "link" => Link,
        "check box" => CheckBox,
        "radio button" => Radio,
        "switch" => Switch,
        "password text" => SecureField,
        "entry" => TextField,
        "text" if states.has(state::EDITABLE) => {
            if states.has(state::MULTI_LINE) {
                TextArea
            } else {
                TextField
            }
        }
        "text" | "label" | "static" | "caption" | "paragraph" | "accelerator label" => Text,
        "combo box" => ComboBox,
        "spin button" => Stepper,
        "slider" | "dial" => Slider,
        "menu bar" => MenuBar,
        "menu" | "popup menu" => Menu,
        "menu item" | "check menu item" | "radio menu item" | "tearoff menu item" => MenuItem,
        "page tab list" => TabGroup,
        "page tab" => Tab,
        "tool bar" => Toolbar,
        "list" | "list box" => List,
        "table" | "tree table" => Table,
        "tree" => Outline,
        "list item" | "table row" | "tree item" => Row,
        "table cell" => Cell,
        "column header" | "table column header" => Column,
        "scroll pane" | "viewport" => ScrollArea,
        "document web" | "document frame" | "html container" => WebArea,
        "heading" => Heading,
        "image" | "icon" | "image map" | "canvas" => Image,
        "progress bar" | "level bar" => Progress,
        "scroll bar" | "separator" | "tool tip" | "ruler" | "glass pane" | "redundant object" => {
            return Mapped::Skip;
        }
        "filler" | "panel" | "grouping" | "section" | "form" | "landmark" | "split pane"
        | "layered pane" | "root pane" | "internal frame" | "application" => Group,
        _ => Other,
    };
    Mapped::Role(mapped)
}

pub(crate) fn states(role: Role, set: StateSet) -> States {
    let mut states = States::default();
    // Toolkits report "sensitive" and "enabled" together; either one missing
    // means the user can't use it.
    states.set(
        States::DISABLED,
        !(set.has(state::ENABLED) && set.has(state::SENSITIVE)),
    );
    states.set(States::FOCUSED, set.has(state::FOCUSED));
    states.set(States::SELECTED, set.has(state::SELECTED));
    states.set(States::EXPANDED, set.has(state::EXPANDED));
    states.set(States::BUSY, set.has(state::BUSY));
    states.set(States::EDITABLE, set.has(state::EDITABLE));
    let checkable = matches!(
        role,
        Role::CheckBox | Role::Radio | Role::Switch | Role::Button | Role::MenuItem
    );
    if checkable {
        states.set(States::MIXED, set.has(state::INDETERMINATE));
        states.set(
            States::CHECKED,
            set.has(state::CHECKED) || (role == Role::Button && set.has(state::PRESSED)),
        );
    }
    states
}

/// What the role normally supports through AT-SPI, without pixel input.
pub(crate) fn actions(role: Role) -> Actions {
    use Role::*;
    match role {
        Button | MenuButton | Disclosure | Link | CheckBox | Radio | Switch | Tab | MenuItem => {
            Actions::PRESS
        }
        PopUp | ComboBox => Actions::PRESS | Actions::SHOW_MENU,
        Slider | Stepper => Actions::SET_VALUE,
        TextField | TextArea | SearchField | SecureField => Actions::SET_VALUE | Actions::FOCUS,
        Row => Actions::SELECT,
        _ => Actions::default(),
    }
}

/// Roles whose value is their text contents (the `Text` interface).
pub(crate) fn has_text_value(role: Role) -> bool {
    role.is_text_input()
}

/// Roles whose value is a number (the `Value` interface).
pub(crate) fn has_number_value(role: Role) -> bool {
    matches!(role, Role::Slider | Role::Stepper | Role::Progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(states: &[u32]) -> StateSet {
        StateSet(states.iter().fold(0, |bits, s| bits | 1 << s))
    }

    #[test]
    fn text_is_an_input_only_when_editable() {
        assert_eq!(map("text", set(&[])), Mapped::Role(Role::Text));
        assert_eq!(
            map("text", set(&[state::EDITABLE])),
            Mapped::Role(Role::TextField)
        );
        assert_eq!(
            map("text", set(&[state::EDITABLE, state::MULTI_LINE])),
            Mapped::Role(Role::TextArea)
        );
    }

    #[test]
    fn chrome_is_skipped_and_unknown_roles_are_other() {
        assert_eq!(map("scroll bar", set(&[])), Mapped::Skip);
        assert_eq!(map("something new", set(&[])), Mapped::Role(Role::Other));
    }

    #[test]
    fn state_words_split_at_32() {
        let s = StateSet::from_words(&[1 << state::FOCUSED, 1 << (state::INDETERMINATE - 32)]);
        assert!(s.has(state::FOCUSED));
        assert!(s.has(state::INDETERMINATE));
        assert!(!s.has(state::CHECKED));
    }

    #[test]
    fn disabled_unless_enabled_and_sensitive() {
        let usable = set(&[state::ENABLED, state::SENSITIVE]);
        assert!(!states(Role::Button, usable).has(States::DISABLED));
        assert!(states(Role::Button, set(&[state::ENABLED])).has(States::DISABLED));
    }

    #[test]
    fn a_pressed_toggle_button_reads_as_checked() {
        let on = set(&[state::ENABLED, state::SENSITIVE, state::PRESSED]);
        assert!(states(Role::Button, on).has(States::CHECKED));
        assert!(!states(Role::Text, on).has(States::CHECKED));
    }
}
