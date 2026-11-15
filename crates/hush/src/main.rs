//! The `hush` binary: `hush mcp` for agents that speak MCP, and the same
//! tools as shell commands for agents that prefer a shell.
//!
//! Each shell command is its own process, so it starts from a fresh read:
//! refs stay valid between commands only while the window doesn't change.
//! MCP sessions keep their mirrors and answer with diffs.

use std::io::{self, Write};
use std::process::ExitCode;

use hush_core::{Platform, Ref, ScriptLanguage};
use hush_engine::{Engine, ObserveOptions};

const USAGE: &str = "\
hush: background computer use for any agent

usage:
  hush mcp                                  serve MCP over stdio
  hush apps [QUERY]                         running apps, or apps matching QUERY
  hush observe APP [--window W] [--focus REF] [--full]
  hush act APP [--window W] STEP...         each STEP is one quoted step
  hush script APP SOURCE [--js]
  hush see APP [--window W] [--ref REF] [--width N] [--out FILE]

steps: ";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" || args[0] == "help" {
        println!("{USAGE}{}", hush_core::step::SYNTAX);
        return ExitCode::SUCCESS;
    }
    match platform() {
        Ok(platform) => run(Engine::new(platform), &args),
        Err(message) => {
            eprintln!("hush: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "macos")]
fn platform() -> Result<hush_macos::MacPlatform, String> {
    hush_macos::MacPlatform::new().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform() -> Result<Unsupported, String> {
    Err("this platform isn't supported yet; Hush runs on macOS for now".into())
}

/// Stands in for the platforms Hush doesn't have yet, so the binary builds
/// everywhere the core does.
#[cfg(not(target_os = "macos"))]
enum Unsupported {}

#[cfg(not(target_os = "macos"))]
impl Platform for Unsupported {
    type Handle = u8;
    fn apps(&self, _: Option<&str>) -> hush_core::Result<Vec<hush_core::AppInfo>> {
        match *self {}
    }
    fn windows(&self, _: &hush_core::AppInfo) -> hush_core::Result<Vec<hush_core::WindowInfo>> {
        match *self {}
    }
    fn read(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
    ) -> hush_core::Result<hush_core::Snapshot<u8>> {
        match *self {}
    }
    fn activity(&self, _: &hush_core::AppInfo) -> hush_core::Activity {
        match *self {}
    }
    fn perform(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
        _: hush_core::Op<'_, u8>,
    ) -> hush_core::Result<()> {
        match *self {}
    }
    fn capture(
        &self,
        _: &hush_core::WindowInfo,
        _: Option<hush_core::Rect>,
        _: u32,
    ) -> hush_core::Result<hush_core::Image> {
        match *self {}
    }
    fn script(
        &self,
        _: &hush_core::AppInfo,
        _: &str,
        _: ScriptLanguage,
    ) -> hush_core::Result<String> {
        match *self {}
    }
}

fn run<P: Platform>(mut engine: Engine<P>, args: &[String]) -> ExitCode {
    let mut flags = Flags::parse(&args[1..]);
    let result = match args[0].as_str() {
        "mcp" => {
            let stdin = io::stdin();
            return match hush_mcp::serve(&mut engine, stdin.lock(), io::stdout().lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("hush: {error}");
                    ExitCode::FAILURE
                }
            };
        }
        "apps" => engine
            .apps(flags.positional.first().map(String::as_str))
            .map_err(|e| e.to_string()),
        "observe" => flags.app().and_then(|app| {
            let focus = flags.reference("--focus")?;
            let options = ObserveOptions {
                window: flags.value("--window"),
                focus,
                full: flags.switch("--full"),
            };
            engine.observe(&app, options).map_err(|e| e.to_string())
        }),
        "act" => flags.app().and_then(|app| {
            let steps: Vec<String> = flags.positional[1..].to_vec();
            engine
                .act(&app, flags.value("--window"), &steps)
                .map_err(|e| e.to_string())
        }),
        "script" => flags.app().and_then(|app| {
            let source = flags.positional.get(1).ok_or("script needs SOURCE")?;
            let language = if flags.switch("--js") {
                ScriptLanguage::JavaScript
            } else {
                ScriptLanguage::AppleScript
            };
            engine
                .script(&app, source, language)
                .map_err(|e| e.to_string())
        }),
        "see" => flags.app().and_then(|app| {
            let width = flags
                .value("--width")
                .and_then(|w| w.parse().ok())
                .unwrap_or(960);
            let element = flags.reference("--ref")?;
            let image = engine
                .see(&app, flags.value("--window"), element, width)
                .map_err(|e| e.to_string())?;
            let path = flags.value("--out").unwrap_or("hush-see.jpg").to_owned();
            std::fs::write(&path, &image.jpeg)
                .map_err(|e| format!("couldn't write {path}: {e}"))?;
            Ok(format!(
                "{path} · {}x{} · {} KB\n",
                image.width,
                image.height,
                image.jpeg.len() / 1024
            ))
        }),
        other => Err(format!("unknown command '{other}'; run hush --help")),
    };
    match result {
        Ok(text) => {
            let _ = io::stdout().write_all(text.as_bytes());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("hush: {message}");
            ExitCode::FAILURE
        }
    }
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

    fn app(&mut self) -> Result<String, String> {
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
