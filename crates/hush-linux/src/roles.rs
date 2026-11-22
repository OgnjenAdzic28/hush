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
    /// Not a sign of being usable (see `states`); tests use it to say so.
    #[cfg(test)]
    pub const ENABLED: u32 = 8;
    pub const EXPANDED: u32 = 10;
    pub const FOCUSED: u32 = 12;
    pub const ICONIFIED: u32 = 15;
    pub const MULTI_LINE: u32 = 17;
    pub const PRESSED: u32 = 20;
    pub const SELECTED: u32 = 23;
    pub const SENSITIVE: u32 = 24;
    pub const SHOWING: u32 = 25;
    pub const VISIBLE: u32 = 30;
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

/// `AtspiRole` numbers, as `GetRole` returns them. The role *name*
/// (`GetRoleName`) isn't reliable: GTK 4 names its windows "application".
pub(crate) mod role {
    pub const ACCELERATOR_LABEL: u32 = 1;
    pub const ALERT: u32 = 2;
    pub const CANVAS: u32 = 6;
    pub const CHECK_BOX: u32 = 7;
    pub const CHECK_MENU_ITEM: u32 = 8;
    pub const COLOR_CHOOSER: u32 = 9;
    pub const COLUMN_HEADER: u32 = 10;
    pub const COMBO_BOX: u32 = 11;
    pub const DIAL: u32 = 15;
    pub const DIALOG: u32 = 16;
    pub const FILE_CHOOSER: u32 = 19;
    pub const FILLER: u32 = 20;
    pub const FONT_CHOOSER: u32 = 22;
    pub const FRAME: u32 = 23;
    pub const GLASS_PANE: u32 = 24;
    pub const HTML_CONTAINER: u32 = 25;
    pub const ICON: u32 = 26;
    pub const IMAGE: u32 = 27;
    pub const INTERNAL_FRAME: u32 = 28;
    pub const LABEL: u32 = 29;
    pub const LAYERED_PANE: u32 = 30;
    pub const LIST: u32 = 31;
    pub const LIST_ITEM: u32 = 32;
    pub const MENU: u32 = 33;
    pub const MENU_BAR: u32 = 34;
    pub const MENU_ITEM: u32 = 35;
    pub const PAGE_TAB: u32 = 37;
    pub const PAGE_TAB_LIST: u32 = 38;
    pub const PANEL: u32 = 39;
    pub const PASSWORD_TEXT: u32 = 40;
    pub const POPUP_MENU: u32 = 41;
    pub const PROGRESS_BAR: u32 = 42;
    pub const PUSH_BUTTON: u32 = 43;
    pub const RADIO_BUTTON: u32 = 44;
    pub const RADIO_MENU_ITEM: u32 = 45;
    pub const ROOT_PANE: u32 = 46;
    pub const SCROLL_BAR: u32 = 48;
    pub const SCROLL_PANE: u32 = 49;
    pub const SEPARATOR: u32 = 50;
    pub const SLIDER: u32 = 51;
    pub const SPIN_BUTTON: u32 = 52;
    pub const SPLIT_PANE: u32 = 53;
    pub const TABLE: u32 = 55;
    pub const TABLE_CELL: u32 = 56;
    pub const TABLE_COLUMN_HEADER: u32 = 57;
    pub const TEAROFF_MENU_ITEM: u32 = 59;
    pub const TEXT: u32 = 61;
    pub const TOGGLE_BUTTON: u32 = 62;
    pub const TOOL_BAR: u32 = 63;
    pub const TOOL_TIP: u32 = 64;
    pub const TREE: u32 = 65;
    pub const TREE_TABLE: u32 = 66;
    pub const VIEWPORT: u32 = 68;
    pub const WINDOW: u32 = 69;
    pub const PARAGRAPH: u32 = 73;
    pub const RULER: u32 = 74;
    pub const APPLICATION: u32 = 75;
    pub const ENTRY: u32 = 79;
    pub const CAPTION: u32 = 81;
    pub const DOCUMENT_FRAME: u32 = 82;
    pub const HEADING: u32 = 83;
    pub const SECTION: u32 = 85;
    pub const REDUNDANT_OBJECT: u32 = 86;
    pub const FORM: u32 = 87;
    pub const LINK: u32 = 88;
    pub const TABLE_ROW: u32 = 90;
    pub const TREE_ITEM: u32 = 91;
    pub const DOCUMENT_SPREADSHEET: u32 = 92;
    pub const DOCUMENT_PRESENTATION: u32 = 93;
    pub const DOCUMENT_TEXT: u32 = 94;
    pub const DOCUMENT_WEB: u32 = 95;
    pub const DOCUMENT_EMAIL: u32 = 96;
    pub const LIST_BOX: u32 = 98;
    pub const GROUPING: u32 = 99;
    pub const IMAGE_MAP: u32 = 100;
    pub const LEVEL_BAR: u32 = 103;
    pub const LANDMARK: u32 = 110;
    pub const STATIC: u32 = 116;
    pub const PUSH_BUTTON_MENU: u32 = 129;
    pub const SWITCH: u32 = 130;
}

pub(crate) fn map(atspi_role: u32, states: StateSet) -> Mapped {
    use Role::*;
    use role::*;
    let mapped = match atspi_role {
        FRAME | WINDOW => Window,
        DIALOG | ALERT | FILE_CHOOSER | COLOR_CHOOSER | FONT_CHOOSER => Dialog,
        PUSH_BUTTON | PUSH_BUTTON_MENU | TOGGLE_BUTTON => Button,
        LINK => Link,
        CHECK_BOX => CheckBox,
        RADIO_BUTTON => Radio,
        SWITCH => Switch,
        PASSWORD_TEXT => SecureField,
        ENTRY => TextField,
        TEXT if states.has(state::EDITABLE) => {
            if states.has(state::MULTI_LINE) {
                TextArea
            } else {
                TextField
            }
        }
        // A word processor's paragraphs (LibreOffice Writer) are where its
        // text is typed.
        PARAGRAPH if states.has(state::EDITABLE) => TextArea,
        TEXT | LABEL | STATIC | CAPTION | PARAGRAPH | ACCELERATOR_LABEL => Text,
        COMBO_BOX => ComboBox,
        SPIN_BUTTON => Stepper,
        SLIDER | DIAL => Slider,
        MENU_BAR => MenuBar,
        MENU | POPUP_MENU => Menu,
        MENU_ITEM | CHECK_MENU_ITEM | RADIO_MENU_ITEM | TEAROFF_MENU_ITEM => MenuItem,
        PAGE_TAB_LIST => TabGroup,
        PAGE_TAB => Tab,
        TOOL_BAR => Toolbar,
        LIST | LIST_BOX => List,
        TABLE | TREE_TABLE => Table,
        TREE => Outline,
        LIST_ITEM | TABLE_ROW | TREE_ITEM => Row,
        TABLE_CELL => Cell,
        COLUMN_HEADER | TABLE_COLUMN_HEADER => Column,
        SCROLL_PANE | VIEWPORT => ScrollArea,
        DOCUMENT_WEB | DOCUMENT_FRAME | HTML_CONTAINER => WebArea,
        DOCUMENT_TEXT | DOCUMENT_SPREADSHEET | DOCUMENT_PRESENTATION | DOCUMENT_EMAIL => Group,
        HEADING => Heading,
        IMAGE | ICON | IMAGE_MAP | CANVAS => Image,
        PROGRESS_BAR | LEVEL_BAR => Progress,
        SCROLL_BAR | SEPARATOR | TOOL_TIP | RULER | GLASS_PANE | REDUNDANT_OBJECT => {
            return Mapped::Skip;
        }
        FILLER | PANEL | GROUPING | SECTION | FORM | LANDMARK | SPLIT_PANE | LAYERED_PANE
        | ROOT_PANE | INTERNAL_FRAME | APPLICATION => Group,
        _ => Other,
    };
    Mapped::Role(mapped)
}

pub(crate) fn states(role: Role, atspi_role: u32, set: StateSet) -> States {
    let mut states = States::default();
    // "Sensitive" is the state every toolkit drops for a disabled control.
    // "Enabled" isn't: GTK 3 keeps it on insensitive widgets, and GTK 4
    // never reports it at all. A document and its paragraphs are content,
    // not controls, and LibreOffice marks them neither.
    let content = matches!(
        atspi_role,
        role::PARAGRAPH
            | role::DOCUMENT_TEXT
            | role::DOCUMENT_SPREADSHEET
            | role::DOCUMENT_PRESENTATION
            | role::DOCUMENT_EMAIL
    );
    states.set(States::DISABLED, !content && !set.has(state::SENSITIVE));
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
pub(crate) fn has_text_value(role: Role, set: StateSet) -> bool {
    // A GTK combo box's text lives in its entry child; an editable combo
    // (Firefox's address bar) holds its own.
    role.is_text_input() && (role != Role::ComboBox || set.has(state::EDITABLE))
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
        assert_eq!(map(role::TEXT, set(&[])), Mapped::Role(Role::Text));
        assert_eq!(
            map(role::TEXT, set(&[state::EDITABLE])),
            Mapped::Role(Role::TextField)
        );
        assert_eq!(
            map(role::TEXT, set(&[state::EDITABLE, state::MULTI_LINE])),
            Mapped::Role(Role::TextArea)
        );
    }

    #[test]
    fn chrome_is_skipped_and_unknown_roles_are_other() {
        assert_eq!(map(role::SCROLL_BAR, set(&[])), Mapped::Skip);
        assert_eq!(map(1000, set(&[])), Mapped::Role(Role::Other));
    }

    #[test]
    fn state_words_split_at_32() {
        let s = StateSet::from_words(&[1 << state::FOCUSED, 1 << (state::INDETERMINATE - 32)]);
        assert!(s.has(state::FOCUSED));
        assert!(s.has(state::INDETERMINATE));
        assert!(!s.has(state::CHECKED));
    }

    #[test]
    fn disabled_means_not_sensitive() {
        let gtk3 = set(&[state::ENABLED, state::SENSITIVE]);
        assert!(!states(Role::Button, role::PUSH_BUTTON, gtk3).has(States::DISABLED));
        let gtk4 = set(&[state::SENSITIVE]);
        assert!(!states(Role::Button, role::PUSH_BUTTON, gtk4).has(States::DISABLED));
        let gtk3_insensitive = set(&[state::ENABLED]);
        assert!(states(Role::Button, role::PUSH_BUTTON, gtk3_insensitive).has(States::DISABLED));
    }

    #[test]
    fn a_word_processor_document_is_editable_content() {
        let paragraph = set(&[state::EDITABLE, state::FOCUSED, state::SHOWING]);
        assert_eq!(
            map(role::PARAGRAPH, paragraph),
            Mapped::Role(Role::TextArea)
        );
        assert_eq!(map(role::PARAGRAPH, set(&[])), Mapped::Role(Role::Text));
        assert!(!states(Role::TextArea, role::PARAGRAPH, paragraph).has(States::DISABLED));
        assert!(states(Role::TextArea, role::TEXT, paragraph).has(States::DISABLED));
    }

    #[test]
    fn a_pressed_toggle_button_reads_as_checked() {
        let on = set(&[state::ENABLED, state::SENSITIVE, state::PRESSED]);
        assert!(states(Role::Button, role::PUSH_BUTTON, on).has(States::CHECKED));
        assert!(!states(Role::Text, role::LABEL, on).has(States::CHECKED));
    }
}
