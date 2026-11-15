//! AppleScript run inside Hush: compiled once per source text and kept, so a
//! repeated script costs no process start and no recompile (osascript takes
//! about 67 ms cold on an M4 Pro; a compiled in-process run adds well under
//! a millisecond). JavaScript for Automation goes through osascript.

use std::collections::HashMap;
use std::process::Command;

use hush_core::{Error, Result};
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2_foundation::{NSAppleScript, NSDictionary, NSString};

#[derive(Debug, Default)]
pub(crate) struct Scripts {
    compiled: HashMap<String, Retained<NSAppleScript>>,
}

impl Scripts {
    pub(crate) fn apple_script(&mut self, source: &str) -> Result<String> {
        let script = match self.compiled.get(source) {
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
                self.compiled.insert(source.to_owned(), script.clone());
                script
            }
        };
        let mut error = None;
        // SAFETY: as above.
        let result = unsafe { script.executeAndReturnError(Some(&mut error)) };
        if error.is_some() {
            return Err(Error::Failed(format!(
                "the script failed: {}",
                message(error)
            )));
        }
        Ok(result
            .stringValue()
            .map(|s| s.to_string())
            .unwrap_or_default())
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

fn message(error: Option<Retained<NSDictionary<NSString, objc2::runtime::AnyObject>>>) -> String {
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
