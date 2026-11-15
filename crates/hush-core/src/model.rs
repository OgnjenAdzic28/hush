//! What a platform reads from a window: apps, windows and elements.

use std::fmt;

/// A running or installed application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    /// `None` when the app is installed but not running.
    pub pid: Option<u32>,
    /// Bundle id on macOS, AppUserModelID on Windows, desktop file id on Linux.
    pub id: Option<String>,
    pub path: Option<String>,
    /// The app the user is working in. Hush never acts on it without being asked.
    pub frontmost: bool,
}

/// One top-level window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    /// The window server's id (CGWindowID on macOS, HWND on Windows).
    pub id: u64,
    pub title: String,
    /// Screen coordinates, points.
    pub frame: Rect,
    /// The app's own main or key window (not system-wide focus).
    pub main: bool,
    pub minimized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn is_empty(&self) -> bool {
        self.w < 1.0 || self.h < 1.0
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

/// Platform-neutral element roles. Platforms map their own roles onto these;
/// anything unrecognised becomes `Group` (if it has children) or `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Window,
    Sheet,
    Dialog,
    Popover,
    Button,
    MenuButton,
    Disclosure,
    Link,
    CheckBox,
    Radio,
    Switch,
    TextField,
    TextArea,
    SearchField,
    SecureField,
    ComboBox,
    PopUp,
    Slider,
    Stepper,
    MenuBar,
    Menu,
    MenuItem,
    TabGroup,
    Tab,
    Toolbar,
    List,
    Table,
    Outline,
    Row,
    Cell,
    Column,
    ScrollArea,
    WebArea,
    Group,
    Heading,
    Text,
    Image,
    Progress,
    Other,
}

impl Role {
    /// The word the outline prints.
    pub fn word(self) -> &'static str {
        use Role::*;
        match self {
            Window => "window",
            Sheet => "sheet",
            Dialog => "dialog",
            Popover => "popover",
            Button => "button",
            MenuButton => "menubutton",
            Disclosure => "disclosure",
            Link => "link",
            CheckBox => "checkbox",
            Radio => "radio",
            Switch => "switch",
            TextField => "field",
            TextArea => "textarea",
            SearchField => "search",
            SecureField => "password",
            ComboBox => "combo",
            PopUp => "popup",
            Slider => "slider",
            Stepper => "stepper",
            MenuBar => "menubar",
            Menu => "menu",
            MenuItem => "menuitem",
            TabGroup => "tabs",
            Tab => "tab",
            Toolbar => "toolbar",
            List => "list",
            Table => "table",
            Outline => "tree",
            Row => "row",
            Cell => "cell",
            Column => "column",
            ScrollArea => "scroll",
            WebArea => "web",
            Group => "group",
            Heading => "heading",
            Text => "text",
            Image => "image",
            Progress => "progress",
            Other => "other",
        }
    }

    /// The letter that starts this role's refs, so `t3` reads as a text input.
    pub fn ref_prefix(self) -> u8 {
        use Role::*;
        match self {
            Button | MenuButton | Disclosure => b'b',
            CheckBox | Radio | Switch => b'c',
            TextField | TextArea | SearchField | SecureField | ComboBox => b't',
            PopUp | Slider | Stepper | Tab => b'p',
            MenuBar | Menu | MenuItem => b'm',
            Row | Cell | Column => b'i',
            Link => b'l',
            _ => b'e',
        }
    }

    pub fn is_text_input(self) -> bool {
        matches!(
            self,
            Role::TextField
                | Role::TextArea
                | Role::SearchField
                | Role::SecureField
                | Role::ComboBox
        )
    }

    /// Roles that only group other elements and are worth a line only when named.
    pub fn is_transparent(self) -> bool {
        matches!(
            self,
            Role::Group
                | Role::ScrollArea
                | Role::WebArea
                | Role::Other
                | Role::Cell
                | Role::Column
        )
    }

    /// Containers that always get a line, so their children read as a unit.
    pub fn is_container(self) -> bool {
        matches!(
            self,
            Role::Window
                | Role::Sheet
                | Role::Dialog
                | Role::Popover
                | Role::Toolbar
                | Role::TabGroup
                | Role::List
                | Role::Table
                | Role::Outline
                | Role::Menu
        )
    }
}

macro_rules! flags {
    ($(#[$meta:meta])* $name:ident { $($flag:ident = $bit:expr),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name(pub u16);

        impl $name {
            $(pub const $flag: $name = $name(1 << $bit);)*

            pub fn has(self, other: $name) -> bool {
                self.0 & other.0 == other.0
            }

            pub fn set(&mut self, other: $name, on: bool) {
                if on { self.0 |= other.0 } else { self.0 &= !other.0 }
            }
        }

        impl std::ops::BitOr for $name {
            type Output = $name;
            fn bitor(self, other: $name) -> $name {
                $name(self.0 | other.0)
            }
        }
    };
}

flags! {
    /// What the element currently is.
    States {
        DISABLED = 0,
        FOCUSED = 1,
        SELECTED = 2,
        CHECKED = 3,
        MIXED = 4,
        EXPANDED = 5,
        HIDDEN = 6,
        EDITABLE = 7,
        BUSY = 8,
    }
}

flags! {
    /// What the platform can do to the element without pixel input.
    Actions {
        PRESS = 0,
        INCREMENT = 1,
        DECREMENT = 2,
        SHOW_MENU = 3,
        CONFIRM = 4,
        CANCEL = 5,
        PICK = 6,
        SCROLL_TO_VISIBLE = 7,
        SET_VALUE = 8,
        FOCUS = 9,
        SELECT = 10,
    }
}

/// One element, as read. `H` is the platform's handle for acting on it later.
#[derive(Clone, Debug)]
pub struct Node<H> {
    pub handle: H,
    pub role: Role,
    /// Title, label or description: whatever names it for a person.
    pub name: String,
    pub value: Option<String>,
    /// Window-relative points. `None` when the platform didn't report one.
    pub frame: Option<Rect>,
    pub states: States,
    pub actions: Actions,
    /// Index of the parent in `Snapshot::nodes`; `None` for the root.
    pub parent: Option<usize>,
}

/// The elements of one window at one moment, in pre-order (parents first).
#[derive(Clone, Debug)]
pub struct Snapshot<H> {
    pub app: AppInfo,
    pub window: WindowInfo,
    pub nodes: Vec<Node<H>>,
    /// Top-level menu titles, for the outline's header.
    pub menus: Vec<String>,
    children: Vec<Vec<usize>>,
}

impl<H> Snapshot<H> {
    /// `nodes` must be in pre-order with each `parent` pointing at an
    /// earlier index; the first node is the root.
    pub fn new(app: AppInfo, window: WindowInfo, nodes: Vec<Node<H>>, menus: Vec<String>) -> Self {
        let mut children = vec![Vec::new(); nodes.len()];
        for (index, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                debug_assert!(parent < index, "nodes must be in pre-order");
                children[parent].push(index);
            }
        }
        Snapshot {
            app,
            window,
            nodes,
            menus,
            children,
        }
    }

    pub fn children(&self, index: usize) -> &[usize] {
        &self.children[index]
    }
}

/// A JPEG image of a window or a region of it.
#[derive(Clone)]
pub struct Image {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Image({}x{}, {} bytes)",
            self.width,
            self.height,
            self.jpeg.len()
        )
    }
}
