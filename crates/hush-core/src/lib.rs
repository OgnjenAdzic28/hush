//! Platform-neutral core of Hush: the element model, refs, outline, diff,
//! steps and settle rules, plus the `Platform` trait each OS backend
//! implements. Nothing here touches an OS API.

pub mod diff;
pub mod key;
pub mod menus;
pub mod model;
pub mod outline;
pub mod platform;
pub mod refs;
pub mod settle;
pub mod step;
pub mod timeline;

#[cfg(test)]
mod test_support;

pub use diff::Diff;
pub use key::{Key, KeyCombo, Modifiers, NamedKey};
pub use menus::MenuEntry;
pub use model::{Actions, AppInfo, Image, Node, Rect, Role, Snapshot, States, WindowInfo};
pub use outline::{OutlineOptions, View, approx_tokens};
pub use platform::{Error, Op, Platform, Result, ScriptLanguage};
pub use refs::{Ref, RefTable};
pub use settle::{Activity, Outcome, Pace, Poll, Reason, Rules, Settle};
pub use step::{Button, Condition, Direction, Step, Target};
pub use timeline::{Input, Played, Timed};
