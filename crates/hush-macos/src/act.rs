//! Carrying out operations on a background app without touching the user's
//! focus, pointer, keyboard or clipboard.

use std::path::Path;
use std::ptr::NonNull;

use hush_core::{AppInfo, Error, Op, Result};
use objc2_application_services::{AXError, AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRange, CFRetained, CFString, CFType};

use crate::ax::Element;
use crate::{cf, keys, menu};

pub(crate) fn perform(app: &AppInfo, op: Op<'_, Element>) -> Result<()> {
    let pid = app
        .pid
        .map(|p| p as i32)
        .ok_or_else(|| Error::NotFound(format!("{} isn't running", app.name)))?;
    let app_element = Element::application(pid);
    match op {
        Op::Press(element) => perform_action(element, "AXPress"),
        Op::ShowMenu(element) => perform_action(element, "AXShowMenu"),
        Op::ScrollIntoView(element) => perform_action(element, "AXScrollToVisible"),
        Op::Focus(element) => set(element, "AXFocused", CFBoolean::new(true)),
        Op::Select(element) => select(element),
        Op::SetValue(element, value) => set_value(element, value),
        Op::Type { text, into } => type_text(app, pid, &app_element, into, text),
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
            refuse_keys_for_chromium(app)?;
            if keys::post_combo(pid, combo, repeat) {
                Ok(())
            } else {
                Err(Error::Unsupported(format!("no key code for {combo}")))
            }
        }
        Op::Menu(path) => menu::press_path(&app_element, path),
        Op::Click { .. } | Op::Scroll { .. } => Err(Error::Unsupported(
            "pointer clicks and scrolling in the background aren't built yet".into(),
        )),
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
    set(element, attribute, CFBoolean::new(true))
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
    app: &AppInfo,
    pid: i32,
    app_element: &Element,
    into: Option<&Element>,
    text: &str,
) -> Result<()> {
    let target = match into {
        Some(element) => {
            let _ = set(element, "AXFocused", CFBoolean::new(true));
            Some(element.clone())
        }
        None => app_element.element("AXFocusedUIElement"),
    };
    // An app that accepted the insert has the text, even if its value
    // updates a moment later; typing it again as keys would double it.
    if let Some(target) = &target
        && set(target, "AXSelectedText", &CFString::from_str(text)).is_ok()
    {
        return Ok(());
    }
    refuse_keys_for_chromium(app)?;
    keys::post_text(pid, text);
    Ok(())
}

/// Chromium and Electron only insert typed text into the window AppKit
/// considers key, which a background window isn't. Until Hush can make the
/// window key inside the app (the SkyLight route), key events would be
/// silently dropped there, so say so instead.
fn refuse_keys_for_chromium(app: &AppInfo) -> Result<()> {
    if app
        .path
        .as_deref()
        .is_some_and(|path| is_chromium(Path::new(path)))
    {
        return Err(Error::Unsupported(format!(
            "{} is a Chromium or Electron app, which drops key events sent in the background; use set on \
             the field instead",
            app.name
        )));
    }
    Ok(())
}

pub(crate) fn is_chromium(bundle: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(bundle.join("Contents/Frameworks")) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        name.starts_with("electron framework")
            || name.starts_with("chromium")
            || (name.ends_with(" framework.framework")
                && [
                    "chrome", "edge", "brave", "opera", "vivaldi", "arc", "aside",
                ]
                .iter()
                .any(|browser| name.contains(browser)))
    })
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
