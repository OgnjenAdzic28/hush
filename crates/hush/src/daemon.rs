//! The client side of the per-user Hush host (see `host`): shell commands
//! send their arguments to it, and `hush mcp` becomes a pipe between the
//! agent and a session in it.
//!
//! One host serves everyone because macOS lets only one process per
//! executable capture windows: a second Hush process started from the same
//! file waits forever for pixels while the first is alive. One host also
//! keeps one lane display and one set of app watchers for every agent.
//!
//! The first caller starts `hush daemon` in the background. A host built from
//! another binary than the caller's (after a rebuild or an upgrade) exits on
//! the caller's first request when no session is open, and the caller starts
//! a fresh one. The socket lives in the per-user temporary folder, which only
//! the user can open.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::cli;

/// How long a caller waits for a host it started to accept connections.
const START_LIMIT: Duration = Duration::from_secs(3);

pub(crate) fn socket() -> PathBuf {
    std::env::temp_dir().join("hush.sock")
}

/// Identifies the binary: a host only serves callers built from the same
/// file.
pub(crate) fn build() -> String {
    std::env::current_exe()
        .and_then(|exe| exe.metadata())
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(String::new, |since| since.as_nanos().to_string())
}

/// Runs `args` in the host, starting it if needed. `None` when no host
/// could be reached, so the caller runs the command itself.
pub(crate) fn request(args: &[String]) -> Option<Result<String, String>> {
    let cwd = std::env::current_dir().ok()?;
    let args = cli::with_absolute_out(args, &cwd);
    let message = json!({ "args": args, "build": build() }).to_string();
    let (stream, reply) = open(&message)?;
    drop(stream);
    Some(answer(reply))
}

/// Pipes this process's stdin and stdout to a new MCP session in the host
/// until either side closes. `None` when no host could be reached, so the
/// caller serves MCP itself.
///
/// The pipe never touches the window server, so it doesn't count as a
/// second Hush process for capture.
pub(crate) fn pipe_mcp() -> Option<io::Result<()>> {
    let message = json!({ "mcp": true, "build": build() }).to_string();
    let (stream, reply) = open(&message)?;
    if reply["ok"].as_bool() != Some(true) {
        return None;
    }
    let mut to_host = stream.try_clone().ok()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut to_host);
        // The agent closed its side: the host ends the session.
        let _ = to_host.shutdown(Shutdown::Write);
    });
    let mut from_host = stream;
    Some(io::copy(&mut from_host, &mut io::stdout().lock()).map(|_| ()))
}

/// Connects, sends `message` and reads the first reply line, starting a host
/// or replacing an older one as needed. `None` only when the message never
/// reached a host that answered it.
fn open(message: &str) -> Option<(UnixStream, Value)> {
    for _ in 0..2 {
        let mut stream = match UnixStream::connect(socket()) {
            Ok(stream) => stream,
            Err(_) => start()?,
        };
        writeln!(stream, "{message}").ok()?;
        let reply = match read_line(&mut stream) {
            Some(line) => serde_json::from_str::<Value>(&line).unwrap_or(Value::Null),
            None => Value::Null,
        };
        if reply["restart"].as_bool() == Some(true) {
            // An older host is exiting; the next round starts a new one.
            wait_until_gone();
            continue;
        }
        return Some((stream, reply));
    }
    None
}

/// One line, read a byte at a time so nothing after it is taken from the
/// stream (an MCP session's first messages follow the handshake).
fn read_line(stream: &mut UnixStream) -> Option<String> {
    let mut line = Vec::new();
    let mut byte = [0u8];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return (!line.is_empty()).then(|| String::from_utf8_lossy(&line).into()),
            Ok(_) if byte[0] == b'\n' => return Some(String::from_utf8_lossy(&line).into()),
            Ok(_) => line.push(byte[0]),
            Err(_) => return None,
        }
    }
}

/// A shell command's reply. Once a command was sent the host may have
/// acted, so a lost reply is an error, never a reason to run it again in
/// process.
///
/// The reply has no time limit: a command is bounded by its own steps'
/// limits (`wait ... within`) and every Accessibility call's timeout.
fn answer(reply: Value) -> Result<String, String> {
    match (reply["ok"].as_bool(), reply["text"].as_str()) {
        (Some(true), Some(text)) => Ok(text.to_owned()),
        (Some(false), Some(text)) => Err(text.to_owned()),
        _ => Err(
            "lost the host's reply; the command may have run, so look before repeating it".into(),
        ),
    }
}

/// Starts a host and connects to it.
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
        // Waiting for the socket to accept; the host is up in ~20 ms.
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_until_gone() {
    let deadline = Instant::now() + START_LIMIT;
    while UnixStream::connect(socket()).is_ok() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}

/// The first line of a connection, for the host.
pub(crate) fn first_line(reader: &mut BufReader<UnixStream>) -> Option<Value> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}
