//! macOS accessibility roles onto Hush's platform-neutral ones.

use hush_core::Role;

/// What to do with an element of a given AX role.
pub(crate) enum Mapped {
    Role(Role),
    /// Scroll bars, grow boxes and the like: never worth reading.
    Skip,
}

pub(crate) fn map(role: &str, subrole: Option<&str>) -> Mapped {
    use Role::*;
    let mapped = match (role, subrole.unwrap_or("")) {
        ("AXWindow", "AXDialog" | "AXSystemDialog" | "AXFloatingWindow") => Dialog,
        ("AXWindow", _) => Window,
        ("AXSheet", _) => Sheet,
        ("AXPopover", _) => Popover,
        ("AXButton" | "AXColorWell", _) => Button,
        ("AXMenuButton", _) => MenuButton,
        ("AXDisclosureTriangle", _) => Disclosure,
        ("AXLink", _) => Link,
        ("AXCheckBox", "AXSwitch" | "AXToggle") => Switch,
        ("AXCheckBox", _) => CheckBox,
        ("AXRadioButton", "AXTabButton") => Tab,
        ("AXRadioButton", _) => Radio,
        ("AXTextField", "AXSearchField") => SearchField,
        ("AXTextField", "AXSecureTextField") => SecureField,
        ("AXTextField" | "AXDateField" | "AXTimeField", _) => TextField,
        ("AXTextArea", _) => TextArea,
        ("AXComboBox", _) => ComboBox,
        ("AXPopUpButton", _) => PopUp,
        ("AXSlider", _) => Slider,
        ("AXIncrementor" | "AXStepper", _) => Stepper,
        ("AXMenuBar", _) => MenuBar,
        ("AXMenu", _) => Menu,
        ("AXMenuItem" | "AXMenuBarItem", _) => MenuItem,
        ("AXTabGroup", _) => TabGroup,
        ("AXToolbar", _) => Toolbar,
        ("AXList", _) => List,
        ("AXTable" | "AXBrowser" | "AXGrid", _) => Table,
        ("AXOutline", _) => Outline,
        ("AXRow", _) => Row,
        ("AXCell", _) => Cell,
        ("AXColumn", _) => Column,
        ("AXScrollArea", _) => ScrollArea,
        ("AXWebArea", _) => WebArea,
        ("AXHeading", _) => Heading,
        ("AXStaticText", _) => Text,
        ("AXImage", _) => Image,
        (
            "AXProgressIndicator" | "AXBusyIndicator" | "AXLevelIndicator" | "AXRelevanceIndicator",
            _,
        ) => Progress,
        (
            "AXScrollBar" | "AXGrowArea" | "AXValueIndicator" | "AXSplitter" | "AXHandle"
            | "AXMatte",
            _,
        ) => {
            return Mapped::Skip;
        }
        ("AXGroup" | "AXSplitGroup" | "AXLayoutArea" | "AXLayoutItem" | "AXApplication", _) => {
            Group
        }
        _ => Other,
    };
    Mapped::Role(mapped)
}

/// Names for unlabelled window buttons, which only say what they are in
/// their subrole.
pub(crate) fn subrole_name(subrole: &str) -> Option<&'static str> {
    Some(match subrole {
        "AXCloseButton" => "close",
        "AXMinimizeButton" => "minimize",
        "AXZoomButton" => "zoom",
        "AXFullScreenButton" => "full screen",
        "AXToolbarButton" => "toolbar",
        "AXSortButton" => "sort",
        _ => return None,
    })
}
