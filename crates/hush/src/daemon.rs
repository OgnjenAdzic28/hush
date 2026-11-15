//! One engine per user behind a Unix socket, so shell commands keep refs,
//! answer with diffs and keep watching apps between calls, like an MCP
//! session does.
//!
//! The first shell command starts `hush daemon` in the background; it exits
//! after `IDLE` without a request. A daemon built from another binary than
//! the caller's (after a rebuild or an upgrade) exits on the first request
//! and the caller starts a fresh one. The socket lives in the per-user
//! temporary folder, which only the user can open.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use hush_core::Platform;
use hush_engine::Engine;
use serde_json::{Value, json};

use crate::cli;

/// How long the daemon waits for a request before exiting.
const IDLE: Duration = Duration::from_secs(600);
/// How long a caller waits for a daemon it started to accept connections.
const START_LIMIT: Duration = Duration::from_secs(3);

pub(crate) fn socket() -> PathBuf {
    std::env::temp_dir().join("hush.sock")
}

/// Identifies the binary: a daemon only serves callers built from the same
/// file.
fn build() -> String {
    std::env::current_exe()
        .and_then(|exe| exe.metadata())
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(String::new, |since| since.as_nanos().to_string())
}

/// Runs `args` in the daemon, starting it if needed. `None` when no daemon
/// could be reached, so the caller runs the command itself.
pub(crate) fn request(args: &[String]) -> Option<Result<String, String>> {
    let cwd = std::env::current_dir().ok()?;
    let args = cli::with_absolute_out(args, &cwd);
    let message = json!({ "args": args, "build": build() }).to_string();
    for _ in 0..2 {
        let stream = match UnixStream::connect(socket()) {
            Ok(stream) => stream,
            Err(_) => start()?,
        };
        match exchange(stream, &message)? {
            Reply::Done(result) => return Some(result),
            // An older daemon is exiting; the next round starts a new one.
            Reply::Restart => wait_until_gone(),
        }
    }
    None
}

enum Reply {
    Done(Result<String, String>),
    Restart,
}

/// `None` only when the request never reached the daemon. Once it was sent,
/// the daemon may have acted, so a lost reply is an error, never a reason
/// to run the command again in process.
///
/// The reply has no time limit: a command is bounded by its own steps'
/// limits (`wait ... within`) and every Accessibility call's timeout.
fn exchange(mut stream: UnixStream, message: &str) -> Option<Reply> {
    writeln!(stream, "{message}").ok()?;
    let lost = || {
        Reply::Done(Err(
            "lost the daemon's reply; the command may have run, so look before repeating it".into(),
        ))
    };
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return Some(lost());
    }
    let Ok(reply) = serde_json::from_str::<Value>(&line) else {
        return Some(lost());
    };
    if reply["restart"].as_bool() == Some(true) {
        return Some(Reply::Restart);
    }
    let (Some(ok), Some(text)) = (reply["ok"].as_bool(), reply["text"].as_str()) else {
        return Some(lost());
    };
    Some(Reply::Done(if ok {
        Ok(text.to_owned())
    } else {
        Err(text.to_owned())
    }))
}

/// Starts a daemon and connects to it.
fn start() -> Option<UnixStream> {
    let exe = std::env::current_exe().ok()?;
    Command::new(exe)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group, so the caller's Ctrl-C doesn't reach it.
        .process_group(0)
        .spawn()
        .ok()?;
    let deadline = Instant::now() + START_LIMIT;
    loop {
        if let Ok(stream) = UnixStream::connect(socket()) {
            return Some(stream);
        }
        if Instant::now() >= deadline {
            return None;
        }
        // Waiting for the socket to accept; the daemon is up in ~20 ms.
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_until_gone() {
    let deadline = Instant::now() + START_LIMIT;
    while UnixStream::connect(socket()).is_ok() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}

/// Serves requests until idle. Requests run one at a time on this thread,
/// which owns the engine.
pub(crate) fn serve<P: Platform>(engine: &mut Engine<P>) -> Result<(), String> {
    let path = socket();
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        // A live daemon already answers there: nothing to do.
        Err(_) if UnixStream::connect(&path).is_ok() => return Ok(()),
        // A socket file left by a daemon that didn't exit cleanly.
        Err(_) => {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            UnixListener::bind(&path).map_err(|e| e.to_string())?
        }
    };
    // `None` while a command runs, so a long one is never cut off.
    let last_request = Arc::new(Mutex::new(Some(Instant::now())));
    let stop = Arc::new(AtomicBool::new(false));
    watch_idle(Arc::clone(&last_request), Arc::clone(&stop), path.clone());
    let build = build();
    for stream in listener.incoming() {
        // Returning drops the engine, so the platform cleans up (apps in
        // the lane are hidden before the display goes away).
        if stop.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
        let Ok(stream) = stream else { continue };
        if let Ok(mut last) = last_request.lock() {
            *last = None;
        }
        let handled = handle(engine, stream, &build);
        if let Ok(mut last) = last_request.lock() {
            *last = Some(Instant::now());
        }
        if handled == Handled::Restart {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
    }
    Ok(())
}

#[derive(PartialEq)]
enum Handled {
    Served,
    Restart,
}

fn handle<P: Platform>(engine: &mut Engine<P>, stream: UnixStream, build: &str) -> Handled {
    let mut line = String::new();
    let Ok(mut writer) = stream.try_clone() else {
        return Handled::Served;
    };
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return Handled::Served;
    }
    let Ok(request) = serde_json::from_str::<Value>(&line) else {
        let _ = writeln!(writer, "{}", json!({ "ok": false, "text": "bad request" }));
        return Handled::Served;
    };
    if request["build"].as_str() != Some(build) {
        let _ = writeln!(writer, "{}", json!({ "restart": true }));
        return Handled::Restart;
    }
    let args: Vec<String> = request["args"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let reply = match cli::command(engine, &args) {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(text) => json!({ "ok": false, "text": text }),
    };
    let _ = writeln!(writer, "{reply}");
    Handled::Served
}

/// Stops the server once no request came for `IDLE` since the last one
/// finished: it sets `stop` and wakes the accept loop with a connection.
fn watch_idle(last_request: Arc<Mutex<Option<Instant>>>, stop: Arc<AtomicBool>, path: PathBuf) {
    thread::spawn(move || {
        loop {
            // A coarse timer: idleness is measured in minutes.
            thread::sleep(Duration::from_secs(5));
            let idle = last_request
                .lock()
                .ok()
                .and_then(|last| last.map(|at| at.elapsed()))
                .unwrap_or(Duration::ZERO);
            if idle >= IDLE {
                stop.store(true, Ordering::SeqCst);
                let _ = UnixStream::connect(&path);
                return;
            }
        }
    });
}
