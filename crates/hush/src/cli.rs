//! The shell commands, run against one engine. The daemon and the in-process
//! fallback both call `command`, so a command answers the same either way.

use std::path::Path;

use hush_core::{Platform, Ref, ScriptLanguage};
use hush_engine::{Engine, ObserveOptions};

/// Runs one shell command (`args[0]` is the verb) and returns its output.
pub(crate) fn command<P: Platform>(
    engine: &mut Engine<P>,
    args: &[String],
) -> Result<String, String> {
    let (verb, rest) = args
        .split_first()
        .ok_or("missing command; run hush --help")?;
    let flags = Flags::parse(rest);
    match verb.as_str() {
        "apps" => engine
            .apps(flags.positional.first().map(String::as_str))
            .map_err(|e| e.to_string()),
        "observe" if flags.switch("--menus") => {
            engine.menus(&flags.app()?).map_err(|e| e.to_string())
        }
        "observe" => {
            let app = flags.app()?;
            let options = ObserveOptions {
                window: flags.value("--window"),
                focus: flags.reference("--focus")?,
                full: flags.switch("--full"),
            };
            engine.observe(&app, options).map_err(|e| e.to_string())
        }
        "act" => {
            let app = flags.app()?;
            let steps: Vec<String> = flags.positional[1..].to_vec();
            engine
                .act(&app, flags.value("--window"), &steps)
                .map_err(|e| e.to_string())
        }
        "timeline" => {
            let app = flags.app()?;
            // `-` was replaced with stdin's lines by `with_stdin_events`.
            let events: Vec<String> = flags.positional[1..].to_vec();
            engine
                .timeline(&app, flags.value("--window"), &events)
                .map_err(|e| e.to_string())
        }
        "script" => {
            let app = flags.app()?;
            let source = flags.positional.get(1).ok_or("script needs SOURCE")?;
            let language = if flags.switch("--js") {
                ScriptLanguage::JavaScript
            } else {
                ScriptLanguage::AppleScript
            };
            engine
                .script(&app, source, language)
                .map_err(|e| e.to_string())
        }
        "see" => {
            let app = flags.app()?;
            let width = flags
                .value("--width")
                .and_then(|w| w.parse().ok())
                .unwrap_or(960);
            let image = engine
                .see(
                    &app,
                    flags.value("--window"),
                    flags.reference("--ref")?,
                    width,
                )
                .map_err(|e| e.to_string())?;
            let path = flags.value("--out").unwrap_or(DEFAULT_OUT);
            std::fs::write(path, &image.jpeg).map_err(|e| format!("couldn't write {path}: {e}"))?;
            Ok(format!(
                "{path} · {}x{} · {} KB\n",
                image.width,
                image.height,
                image.jpeg.len() / 1024
            ))
        }
        other => Err(format!("unknown command '{other}'; run hush --help")),
    }
}

const DEFAULT_OUT: &str = "hush-see.jpg";

/// The arguments with `see`'s output path made absolute against `cwd`, for
/// a daemon whose working directory is not the caller's.
pub(crate) fn with_absolute_out(args: &[String], cwd: &Path) -> Vec<String> {
    let mut args = args.to_vec();
    if args.first().map(String::as_str) != Some("see") {
        return args;
    }
    match args.iter().position(|a| a == "--out") {
        Some(at) if at + 1 < args.len() => {
            args[at + 1] = cwd.join(&args[at + 1]).to_string_lossy().into_owned();
        }
        Some(_) => {}
        None => {
            args.push("--out".into());
            args.push(cwd.join(DEFAULT_OUT).to_string_lossy().into_owned());
        }
    }
    args
}

/// `timeline APP -` with the events read from `input`, one per line. It runs in the calling
/// process, before a request goes to the daemon, which has no stdin of its own.
pub(crate) fn with_stdin_events(
    args: &[String],
    input: impl std::io::Read,
) -> Result<Vec<String>, String> {
    if args.first().map(String::as_str) != Some("timeline")
        || args.last().map(String::as_str) != Some("-")
    {
        return Ok(args.to_vec());
    }
    let text = std::io::read_to_string(input)
        .map_err(|e| format!("couldn't read events from stdin: {e}"))?;
    let mut expanded = args[..args.len() - 1].to_vec();
    expanded.extend(text.lines().map(str::to_owned));
    Ok(expanded)
}

/// `--name value` options, `--name` switches, and the rest in order.
struct Flags {
    positional: Vec<String>,
    named: Vec<(String, Option<String>)>,
}

impl Flags {
    const VALUED: [&'static str; 5] = ["--window", "--focus", "--ref", "--width", "--out"];

    fn parse(args: &[String]) -> Flags {
        let mut flags = Flags {
            positional: Vec::new(),
            named: Vec::new(),
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if Self::VALUED.contains(&arg.as_str()) {
                flags.named.push((arg.clone(), iter.next().cloned()));
            } else if arg.starts_with("--") {
                flags.named.push((arg.clone(), None));
            } else {
                flags.positional.push(arg.clone());
            }
        }
        flags
    }

    fn app(&self) -> Result<String, String> {
        self.positional
            .first()
            .cloned()
            .ok_or_else(|| "missing APP; run hush --help".into())
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }

    fn switch(&self, name: &str) -> bool {
        self.named.iter().any(|(n, _)| n == name)
    }

    fn reference(&self, name: &str) -> Result<Option<Ref>, String> {
        self.value(name)
            .map(|r| Ref::parse(r).ok_or_else(|| format!("{name} needs a ref like b3, got '{r}'")))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn timeline_events_come_from_stdin_with_a_dash() {
        let input = "0 tap a\n100 tap b\n".as_bytes();
        assert_eq!(
            with_stdin_events(
                &strings(&["timeline", "Piano", "--window", "W", "-"]),
                input
            )
            .unwrap(),
            strings(&["timeline", "Piano", "--window", "W", "0 tap a", "100 tap b"])
        );
        let untouched = strings(&["act", "Notes", "-"]);
        assert_eq!(
            with_stdin_events(&untouched, "x".as_bytes()).unwrap(),
            untouched
        );
    }

    #[test]
    fn see_output_is_made_absolute() {
        let cwd = Path::new("/work");
        assert_eq!(
            with_absolute_out(&strings(&["see", "Notes", "--out", "a.jpg"]), cwd),
            strings(&["see", "Notes", "--out", "/work/a.jpg"])
        );
        assert_eq!(
            with_absolute_out(&strings(&["see", "Notes", "--out", "/tmp/a.jpg"]), cwd),
            strings(&["see", "Notes", "--out", "/tmp/a.jpg"])
        );
        assert_eq!(
            with_absolute_out(&strings(&["see", "Notes"]), cwd),
            strings(&["see", "Notes", "--out", "/work/hush-see.jpg"])
        );
        assert_eq!(
            with_absolute_out(&strings(&["act", "Notes", "--out"]), cwd),
            strings(&["act", "Notes", "--out"])
        );
    }
}
