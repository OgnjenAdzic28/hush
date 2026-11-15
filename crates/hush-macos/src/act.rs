//! Carrying out operations on a background app without touching the user's
//! focus, pointer, keyboard or clipboard.

use std::ptr::NonNull;

use std::thread;
use std::time::{Duration, Instant};

use hush_core::{AppInfo, Error, Op, Result, WindowInfo};
use objc2_application_services::{AXError, AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRange, CFRetained, CFString, CFType};

use crate::ax::Element;
use crate::private::SyntheticFocus;
use crate::{apps, cf, keys, menu, pointer};

/// What an operation needs to know about where it lands.
pub(crate) struct Target<'a> {
    pub(crate) app: &'a AppInfo,
    pub(crate) window: &'a WindowInfo,
    pub(crate) window_element: &'a Element,
}

impl Target<'_> {
    fn pid(&self) -> Result<i32> {
        self.app
            .pid
            .map(|p| p as i32)
            .ok_or_else(|| Error::NotFound(format!("{} isn't running", self.app.name)))
    }

    fn pointer(&self) -> Result<pointer::Target> {
        Ok(pointer::Target {
            pid: self.pid()?,
            window: u32::try_from(self.window.id)
                .map_err(|_| Error::Unsupported("this window has no window-server id".into()))?,
            origin: (self.window.frame.x, self.window.frame.y),
            frontmost: self.app.frontmost,
        })
    }
}

pub(crate) fn perform(target: &Target<'_>, op: Op<'_, Element>) -> Result<()> {
    let app = target.app;
    let pid = target.pid()?;
    let app_element = Element::application(pid);
    match op {
        Op::Press(element) => perform_action(element, "AXPress"),
        Op::ShowMenu(element) => perform_action(element, "AXShowMenu"),
        Op::ScrollIntoView(element) => perform_action(element, "AXScrollToVisible"),
        Op::Focus(element) => set(element, "AXFocused", CFBoolean::new(true)),
        Op::Select(element) => select(element),
        Op::SetValue(element, value) => set_value(element, value),
        Op::Type { text, into } => type_text(target, &app_element, into, text),
        Op::Key { combo, repeat } => {
            if combo.modifiers.cmd {
                // Background apps ignore Cmd key events; their menu items don't.
                for _ in 0..repeat.max(1) {
                    if !menu::press_shortcut(&app_element, combo)? {
                        return Err(Error::Unsupported(format!(
                            "no menu item has the shortcut {combo}, and {} ignores Cmd keys while in the \
                             background; use menu or click instead",
                            app.name
                        )));
                    }
                }
                return Ok(());
            }
            if keys::code(combo.key).is_none() {
                return Err(Error::Unsupported(format!("no key code for {combo}")));
            }
            with_keys(target, &app_element, || {
                keys::post_combo(pid, combo, repeat);
            })
        }
        Op::Menu(path) => menu::press_path(&app_element, path),
        Op::Click {
            x,
            y,
            button,
            count,
        } => pointer::click(&target.pointer()?, x, y, button, count),
        Op::Scroll { x, y, dx, dy } => pointer::scroll(&target.pointer()?, x, y, dx, dy),
    }
}

pub(crate) fn perform_action(element: &Element, action: &str) -> Result<()> {
    let name = cf::cfstr(action);
    // SAFETY: performing an action on a live element.
    let status = unsafe { element.raw().perform_action(&name) };
    match status {
        AXError::Success => Ok(()),
        // An action that opens a modal dialog often blocks until the
        // messaging timeout and then reports this, although it worked. The
        // settle wait and the diff show what really happened.
        AXError::CannotComplete if action == "AXPress" => Ok(()),
        other => Err(error(other, action)),
    }
}

/// Sets a boolean attribute to true.
pub(crate) fn set_flag(element: &Element, attribute: &str) -> Result<()> {
    set_bool(element, attribute, true)
}

pub(crate) fn set_bool(element: &Element, attribute: &str, value: bool) -> Result<()> {
    set(element, attribute, CFBoolean::new(value))
}

fn set(element: &Element, attribute: &str, value: &CFType) -> Result<()> {
    let name = cf::cfstr(attribute);
    // SAFETY: setting an attribute on a live element with a CF value.
    let status = unsafe { element.raw().set_attribute_value(&name, value) };
    if status == AXError::Success {
        Ok(())
    } else {
        Err(error(status, attribute))
    }
}

fn select(element: &Element) -> Result<()> {
    match set(element, "AXSelected", CFBoolean::new(true)) {
        Err(Error::Unsupported(_)) => {
            // Rows in some tables are selected through their table.
            let table = element
                .element("AXParent")
                .ok_or_else(|| Error::Unsupported("this element can't be selected".into()))?;
            let rows = objc2_core_foundation::CFArray::from_retained_objects(&[element.retained()]);
            set(&table, "AXSelectedRows", &rows)
        }
        other => other,
    }
}

/// Sets a value and checks the app kept it. Text fields that don't take a
/// direct value get their whole text replaced through the selection.
fn set_value(element: &Element, value: &str) -> Result<()> {
    let current = element.value("AXValue").ok();
    let numeric = current
        .as_deref()
        .and_then(|v| v.downcast_ref::<CFNumber>())
        .is_some();
    let direct = match value.trim().parse::<f64>() {
        Ok(number) if numeric => set(element, "AXValue", &CFNumber::new_f64(number)),
        _ => set(element, "AXValue", &CFString::from_str(value)),
    };
    if direct.is_ok() && reads_back(element, value) {
        return Ok(());
    }
    // Replacing through the selection focuses the element, which only makes
    // sense for text; elsewhere it would just move the app's focus.
    let role = element.string("AXRole").unwrap_or_default();
    if !matches!(
        role.as_str(),
        "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField"
    ) {
        return direct.and(Err(Error::Failed(
            "the app didn't keep the new value".into(),
        )));
    }
    let length = element
        .string("AXValue")
        .map_or(0, |v| v.encode_utf16().count());
    let whole = range(0, length)?;
    let replaced = set(element, "AXFocused", CFBoolean::new(true))
        .and_then(|()| set(element, "AXSelectedTextRange", &whole))
        .and_then(|()| set(element, "AXSelectedText", &CFString::from_str(value)));
    match replaced {
        Ok(()) if reads_back(element, value) => Ok(()),
        Ok(()) => Err(Error::Failed("the app didn't keep the new value".into())),
        Err(_) => direct.and(Err(Error::Failed(
            "the app didn't keep the new value".into(),
        ))),
    }
}

fn reads_back(element: &Element, value: &str) -> bool {
    let Some(now) = element.string("AXValue") else {
        return false;
    };
    now.trim() == value.trim()
        || matches!((now.parse::<f64>(), value.parse::<f64>()), (Ok(a), Ok(b)) if (a - b).abs() < 1e-6)
}

/// Inserting through Accessibility comes first: it is one call, works on a
/// background window and sends no key events at all. Key events to the
/// process are the fallback.
fn type_text(
    target: &Target<'_>,
    app_element: &Element,
    into: Option<&Element>,
    text: &str,
) -> Result<()> {
    let field = match into {
        Some(element) => {
            let _ = set(element, "AXFocused", CFBoolean::new(true));
            Some(element.clone())
        }
        None => app_element.element("AXFocusedUIElement"),
    };
    // An app that accepted the insert has the text, even if its value
    // updates a moment later; typing it again as keys would double it.
    if let Some(field) = &field
        && set(field, "AXSelectedText", &CFString::from_str(text)).is_ok()
    {
        return Ok(());
    }
    let pid = target.pid()?;
    with_keys(target, app_element, || keys::post_text(pid, text))
}

/// Runs `post` (which posts key events to the app) so the events are used.
///
/// AppKit apps take key events in the background. Chromium and Electron
/// insert typed text only into the window AppKit considers key, so for them
/// the window is made key inside the app first (the user's app keeps focus),
/// and keys are posted only once Accessibility confirms it: a key that lands
/// in another window or field is worse than a refused step.
fn with_keys(target: &Target<'_>, app_element: &Element, post: impl FnOnce()) -> Result<()> {
    let chromium = apps::is_chromium(target.app);
    if !chromium || target.app.frontmost {
        post();
        return Ok(());
    }
    let pointer = target.pointer()?;
    if apps::is_hidden(pointer.pid) {
        return Err(Error::Unsupported(format!(
            "{} is hidden; Hush doesn't unhide apps on its own, so show it first",
            target.app.name
        )));
    }
    let focus = SyntheticFocus::begin(pointer.pid, pointer.window)?;
    let result = focus.make_key().and_then(|()| {
        if !wait_for_key_window(app_element, target.window_element) {
            return Err(Error::Unsupported(format!(
                "couldn't make the window key inside {} without bringing it to the front",
                target.app.name
            )));
        }
        post();
        // Keys are taken asynchronously; on macOS 26 a zero gap here lost 1
        // key in 50 (open-computer-use), 10 ms lost none.
        thread::sleep(KEY_RELEASE);
        Ok(())
    });
    focus.end();
    result
}

const KEY_RELEASE: Duration = Duration::from_millis(10);
const KEY_WINDOW_LIMIT: Duration = Duration::from_millis(250);

fn wait_for_key_window(app: &Element, window: &Element) -> bool {
    let deadline = Instant::now() + KEY_WINDOW_LIMIT;
    loop {
        if app.element("AXFocusedWindow").as_ref() == Some(window) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn range(location: usize, length: usize) -> Result<CFRetained<AXValue>> {
    let range = CFRange {
        location: location as _,
        length: length as _,
    };
    // SAFETY: the pointer refers to a CFRange, matching the CFRange type tag.
    unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&range).cast()) }
        .ok_or_else(|| Error::Failed("couldn't build a text range".into()))
}

fn error(status: AXError, what: &str) -> Error {
    match status {
        AXError::ActionUnsupported | AXError::AttributeUnsupported | AXError::IllegalArgument => {
            Error::Unsupported(format!("the element doesn't support {what}"))
        }
        AXError::InvalidUIElement => Error::Stale("the element is gone; observe again".into()),
        AXError::APIDisabled => Error::Permission("Accessibility access was turned off".into()),
        AXError::CannotComplete => Error::Failed(format!("the app didn't answer {what} in time")),
        other => Error::Failed(format!("{what} failed (AXError {})", other.0)),
    }
}
