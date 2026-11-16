//! The `hush` binary: `hush mcp` for agents that speak MCP, and the same
//! tools as shell commands for agents that prefer a shell.
//!
//! Everything runs in one per-user host process (see `daemon` and `host`):
//! `hush mcp` pipes the agent to a session there, and shell commands share
//! one engine there, so refs, diffs and app watching carry over between
//! commands. When no host can be reached, or `HUSH_NO_DAEMON` is set, a
//! command or MCP session runs in its own process.

mod cli;
mod daemon;
mod host;
mod platform;
mod signals;

use std::io::{self, Write};
use std::process::ExitCode;

use hush_engine::Engine;

const USAGE: &str = "\
hush: background computer use for any agent

usage:
  hush mcp                                  serve MCP over stdio
  hush daemon                               the per-user host every command uses (started for you)
  hush apps [QUERY]                         running apps, or apps matching QUERY
  hush observe APP [--window W] [--focus REF] [--full] [--menus]
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
    let args = match cli::with_stdin_events(&args, io::stdin()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("hush: {message}");
            return ExitCode::FAILURE;
        }
    };
    let alone = std::env::var_os("HUSH_NO_DAEMON").is_some();
    if args[0] == "mcp"
        && !alone
        && let Some(piped) = daemon::pipe_mcp()
    {
        return match piped {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("hush: the connection to the Hush host broke: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args[0] == "daemon" {
        let served = platform::platform().and_then(host::serve);
        return match served {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("hush: {message}");
                ExitCode::FAILURE
            }
        };
    }
    let in_daemon = match args[0].as_str() {
        "mcp" => None,
        _ if alone => None,
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
        _ => cli::command(&mut engine, args),
    }
}
