//! The one interface every OS backend implements. The engine above it is
//! the same on every platform; only these calls differ.

use std::hash::Hash;

use crate::key::KeyCombo;
use crate::model::{AppInfo, Image, Rect, Snapshot, WindowInfo};
use crate::settle::Activity;
use crate::step::Button;
use crate::timeline::{Played, Timed};

/// An operation the platform carries out on the target app only. Every
/// variant must leave the user's front app, focus, pointer and clipboard
/// alone; a platform that can't do that for some app returns an error.
#[derive(Debug)]
pub enum Op<'a, H> {
    /// Move the app's windows to the platform's off-screen lane.
    Lane,
    Press(&'a H),
    ShowMenu(&'a H),
    SetValue(&'a H, &'a str),
    Focus(&'a H),
    Select(&'a H),
    ScrollIntoView(&'a H),
    /// Type text, into an element if given, else wherever the app's focus is.
    Type {
        text: &'a str,
        into: Option<&'a H>,
    },
    Key {
        combo: KeyCombo,
        repeat: u16,
    },
    /// A menu bar item by its titles, like `["File", "Export", "PDF…"]`.
    Menu(&'a [String]),
    /// A pointer click at window-relative points, without moving the pointer.
    Click {
        x: f64,
        y: f64,
        button: Button,
        count: u8,
    },
    /// Scroll at window-relative points by lines (positive is down/right).
    Scroll {
        x: f64,
        y: f64,
        dx: i32,
        dy: i32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptLanguage {
    AppleScript,
    JavaScript,
}

pub trait Platform {
    /// The platform's reference to one element. Equal handles must mean the
    /// same live element.
    type Handle: Clone + Eq + Hash;

    /// Running apps, plus installed ones when `query` is given.
    fn apps(&self, query: Option<&str>) -> Result<Vec<AppInfo>>;

    /// The app's windows, main window first.
    fn windows(&self, app: &AppInfo) -> Result<Vec<WindowInfo>>;

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<Self::Handle>>;

    /// Accessibility event counts for the app. Watching starts on the first
    /// call and continues for the life of the platform.
    fn activity(&self, app: &AppInfo) -> Activity;

    fn perform(&self, app: &AppInfo, window: &WindowInfo, op: Op<'_, Self::Handle>) -> Result<()>;

    /// A JPEG of the window, or a window-relative region of it, at most
    /// `max_width` pixels wide.
    fn capture(&self, window: &WindowInfo, region: Option<Rect>, max_width: u32) -> Result<Image>;

    fn script(&self, app: &AppInfo, source: &str, language: ScriptLanguage) -> Result<String>;

    /// Plays `events` on their schedule from now, on the target app only,
    /// and says how late each one was posted.
    fn play(&self, _app: &AppInfo, _window: &WindowInfo, _events: &[Timed]) -> Result<Played> {
        Err(Error::Unsupported(
            "this platform can't play timelines yet".into(),
        ))
    }
}

/// Errors are read by a model, so each says what happened and, where it
/// helps, what to do next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotFound(String),
    /// An OS permission is missing. Says which one and where to grant it.
    Permission(String),
    /// The platform can't do this for this app without disturbing the user.
    Unsupported(String),
    /// A ref that no longer exists, or a window that closed.
    Stale(String),
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, message) = match self {
            Error::NotFound(m) => ("not found", m),
            Error::Permission(m) => ("permission needed", m),
            Error::Unsupported(m) => ("unsupported", m),
            Error::Stale(m) => ("stale", m),
            Error::Failed(m) => ("failed", m),
        };
        write!(f, "{kind}: {message}")
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
