//! The `hush` binary: `hush mcp` for agents that speak MCP, and the same
//! tools as shell commands for agents that prefer a shell.
//!
//! Shell commands run in a per-user daemon (see `daemon`) so refs, diffs and
//! app watching carry over between commands, as in an MCP session. When no
//! daemon can be reached, or `HUSH_NO_DAEMON` is set, a command runs in its
//! own process from a fresh read.

mod cli;
mod daemon;
mod platform;
mod signals;

use std::io::{self, Write};
use std::process::ExitCode;

use hush_engine::Engine;

const USAGE: &str = "\
hush: background computer use for any agent

usage:
  hush mcp                                  serve MCP over stdio
  hush daemon                               serve shell commands (started for you)
  hush apps [QUERY]                         running apps, or apps matching QUERY
  hush observe APP [--window W] [--focus REF] [--full]
  hush act APP [--window W] STEP...         each STEP is one quoted step
  hush timeline APP [--window W] EVENT...   events on a schedule; - reads them from stdin
  hush script APP SOURCE [--js]
  hush see APP [--window W] [--ref REF] [--width N] [--out FILE]

steps: ";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" || args[0] == "help" {
        println!(
            "{USAGE}{}\n\ntimeline: {}",
            hush_core::step::SYNTAX,
            hush_core::timeline::SYNTAX
        );
        return ExitCode::SUCCESS;
    }
    if matches!(args[0].as_str(), "mcp" | "daemon") {
        // Before any thread starts, so every thread inherits the mask.
        signals::clean_up_on_exit(args[0] == "daemon");
    }
    let in_daemon = match args[0].as_str() {
        "mcp" | "daemon" => None,
        _ if std::env::var_os("HUSH_NO_DAEMON").is_some() => None,
        _ => daemon::request(&args),
    };
    let result = match in_daemon {
        Some(result) => result,
        None => match platform::platform() {
            Ok(platform) => run(Engine::new(platform), &args),
            Err(message) => Err(message),
        },
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

fn run<P: hush_core::Platform>(mut engine: Engine<P>, args: &[String]) -> Result<String, String> {
    match args[0].as_str() {
        "mcp" => {
            let stdin = io::stdin();
            hush_mcp::serve(&mut engine, stdin.lock(), io::stdout().lock())
                .map(|()| String::new())
                .map_err(|e| e.to_string())
        }
        "daemon" => daemon::serve(&mut engine).map(|()| String::new()),
        _ => cli::command(&mut engine, args),
    }
}
