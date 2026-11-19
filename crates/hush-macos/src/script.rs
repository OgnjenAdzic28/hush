//! AppleScript run inside Hush: compiled once per source text and kept, so a
//! repeated script costs no process start and no recompile (osascript takes
//! about 67 ms cold on an M4 Pro; a compiled in-process run adds well under
//! a millisecond). JavaScript for Automation goes through osascript.
//!
//! NSAppleScript may only be used on the main thread, while sessions run on
//! threads of their own, so every AppleScript runs on the main queue and the
//! compiled scripts live there.

use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;

use hush_core::{Error, Result};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, msg_send};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleScript, NSDictionary, NSString};

thread_local! {
    /// Only the main thread's copy is ever filled.
    static COMPILED: RefCell<HashMap<String, Retained<NSAppleScript>>> = RefCell::default();
}

/// Runs `source` on the main thread: directly when called there (a process
/// with one session), otherwise through the main queue, which the process
/// serving several sessions keeps running.
pub(crate) fn apple_script(source: &str) -> Result<String> {
    // SAFETY: a plain query of the calling thread.
    if unsafe { libc::pthread_main_np() } != 0 {
        return COMPILED.with(|compiled| run(&mut compiled.borrow_mut(), source));
    }
    let mut result = Err(Error::Failed("the script didn't run".into()));
    dispatch2::DispatchQueue::main().exec_sync(|| {
        result = COMPILED.with(|compiled| run(&mut compiled.borrow_mut(), source));
    });
    result
}

fn run(compiled: &mut HashMap<String, Retained<NSAppleScript>>, source: &str) -> Result<String> {
    {
        let script = match compiled.get(source) {
            Some(script) => script.clone(),
            None => {
                let script = NSAppleScript::initWithSource(
                    NSAppleScript::alloc(),
                    &NSString::from_str(source),
                )
                .ok_or_else(|| Error::Failed("couldn't create the script".into()))?;
                let mut error = None;
                // SAFETY: `error` is a valid out-slot for the error dictionary.
                if !unsafe { script.compileAndReturnError(Some(&mut error)) } {
                    return Err(Error::Failed(format!(
                        "the script doesn't compile: {}",
                        message(error)
                    )));
                }
                compiled.insert(source.to_owned(), script.clone());
                script
            }
        };
        let mut error: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
        // The generated binding promises a non-nil result, but the method
        // returns nil when the script fails at runtime, so it is called
        // directly with an optional result.
        // SAFETY: `executeAndReturnError:` takes an error out-slot and
        // returns an autoreleased descriptor or nil.
        let result: Option<Retained<NSAppleEventDescriptor>> =
            unsafe { msg_send![&*script, executeAndReturnError: Some(&mut error)] };
        let Some(result) = result else {
            return Err(Error::Failed(format!(
                "the script failed: {}",
                message(error)
            )));
        };
        Ok(text(&result))
    }
}

/// typeAEList, a list result such as `name of every window`.
const LIST: u32 = u32::from_be_bytes(*b"list");
/// typeNull, what a script that returns nothing gives back.
const NULL: u32 = u32::from_be_bytes(*b"null");

/// A result as osascript prints it: lists flattened and joined with ", ".
fn text(result: &NSAppleEventDescriptor) -> String {
    // SAFETY: `descriptorType` takes no arguments and returns a FourCharCode.
    // (The generated binding needs a Core Services feature Hush doesn't use.)
    let kind: u32 = unsafe { msg_send![result, descriptorType] };
    if kind == LIST {
        return (1..=result.numberOfItems())
            .filter_map(|i| result.descriptorAtIndex(i))
            .map(|item| text(&item))
            .collect::<Vec<_>>()
            .join(", ");
    }
    match result.stringValue() {
        Some(value) => value.to_string(),
        None if kind == NULL => String::new(),
        None => {
            "(a result that isn't text; add `as text`, or ask for one of its properties)".into()
        }
    }
}

pub(crate) fn javascript(source: &str) -> Result<String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", source])
        .output()
        .map_err(|e| Error::Failed(format!("couldn't run osascript: {e}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        Err(Error::Failed(format!(
            "the script failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn message(error: Option<Retained<NSDictionary<NSString, AnyObject>>>) -> String {
    let Some(error) = error else {
        return "unknown error".into();
    };
    let key = NSString::from_str("NSAppleScriptErrorMessage");
    error
        .objectForKey(&key)
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|text| text.to_string())
        .unwrap_or_else(|| "unknown error".into())
}
