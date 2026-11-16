//! The per-user Hush host: one process, one platform, many sessions.
//!
//! Each connection gets a thread. An MCP connection (`hush mcp` piping an
//! agent through) gets a session with its own engine, so its refs and diffs
//! are its own; every engine shares the one platform, so captures, the lane
//! and app watching happen in this process only. Shell commands share one
//! engine, so refs carry over from one command to the next. The main thread
//! is kept for work that must run there (AppleScript).
//!
//! The host exits after `IDLE` with no connection open, hiding the apps it
//! moved to the lane.

use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use hush_core::Platform;
use hush_engine::Engine;
use serde_json::{Value, json};

use crate::{cli, daemon};

/// How long the host waits with no connection open before exiting.
const IDLE: Duration = Duration::from_secs(600);

/// A platform every session thread can use at once.
pub(crate) trait Shared: Platform<Handle: Send> + Send + Sync + 'static {}
impl<P: Platform<Handle: Send> + Send + Sync + 'static> Shared for P {}

struct Host<P: Platform> {
    platform: Arc<P>,
    /// Shell commands share one engine and take turns on it.
    shell: Mutex<Engine<Arc<P>>>,
    build: String,
    path: PathBuf,
    /// Connections being served right now (open sessions included).
    open: AtomicUsize,
    /// When the last connection closed.
    last: Mutex<Instant>,
    /// A newer binary asked while sessions were open: exit once they close.
    outdated: AtomicBool,
}

/// Serves connections until idle. Doesn't return while serving: the main
/// thread goes to the main queue.
pub(crate) fn serve<P: Shared>(platform: P) -> Result<(), String> {
    let path = daemon::socket();
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        // A live host already answers there: nothing to do.
        Err(_) if UnixStream::connect(&path).is_ok() => return Ok(()),
        // A socket file left by a host that didn't exit cleanly.
        Err(_) => {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            UnixListener::bind(&path).map_err(|e| e.to_string())?
        }
    };
    let platform = Arc::new(platform);
    let host = Arc::new(Host {
        shell: Mutex::new(Engine::new(Arc::clone(&platform))),
        platform,
        build: daemon::build(),
        path,
        open: AtomicUsize::new(0),
        last: Mutex::new(Instant::now()),
        outdated: AtomicBool::new(false),
    });
    watch_idle(Arc::clone(&host));
    let accepting = thread::Builder::new()
        .name("hush-accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                host.open.fetch_add(1, Ordering::SeqCst);
                let session = Arc::clone(&host);
                let spawned = thread::Builder::new()
                    .name("hush-session".into())
                    .spawn(move || {
                        handle(&session, stream);
                        closed(&session);
                    });
                if spawned.is_err() {
                    // Out of threads: the stream closed with the closure.
                    closed(&host);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let _ = accepting;
        hush_macos::serve_main_thread()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = accepting.join();
        Ok(())
    }
}

/// Counts a connection as closed, and leaves if it was the last one open
/// and a newer binary is waiting.
fn closed<P: Platform>(host: &Host<P>) {
    *host.last.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    let left = host.open.fetch_sub(1, Ordering::SeqCst) - 1;
    if left == 0 && host.outdated.load(Ordering::SeqCst) {
        exit(&host.path);
    }
}

fn handle<P: Shared>(host: &Host<P>, stream: UnixStream) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let Some(request) = daemon::first_line(&mut reader) else {
        let _ = writeln!(writer, "{}", json!({ "ok": false, "text": "bad request" }));
        return;
    };
    if request["build"].as_str() != Some(host.build.as_str()) {
        // This connection is the only one open: make way for the newer
        // binary now. Otherwise serve it and leave once the sessions close.
        if host.open.load(Ordering::SeqCst) == 1 {
            let _ = writeln!(writer, "{}", json!({ "restart": true }));
            exit(&host.path);
        }
        host.outdated.store(true, Ordering::SeqCst);
    }
    if request["mcp"].as_bool() == Some(true) {
        if writeln!(writer, "{}", json!({ "ok": true })).is_err() {
            return;
        }
        let mut engine = Engine::new(Arc::clone(&host.platform));
        let _ = hush_mcp::serve(&mut engine, reader, writer);
        return;
    }
    let args: Vec<String> = request["args"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let result = {
        let mut engine = host.shell.lock().unwrap_or_else(PoisonError::into_inner);
        cli::command(&mut engine, &args)
    };
    let reply = match result {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(text) => json!({ "ok": false, "text": text }),
    };
    let _ = writeln!(writer, "{reply}");
}

/// Exits once no connection was open for `IDLE`.
fn watch_idle<P: Shared>(host: Arc<Host<P>>) {
    thread::spawn(move || {
        loop {
            // A coarse timer: idleness is measured in minutes.
            thread::sleep(Duration::from_secs(5));
            let idle = host
                .last
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed();
            if host.open.load(Ordering::SeqCst) == 0 && idle >= IDLE {
                exit(&host.path);
            }
        }
    });
}

/// Leaves for good: frees the socket and hides the apps in the lane before
/// the lane display goes away with the process.
fn exit(path: &PathBuf) -> ! {
    let _ = std::fs::remove_file(path);
    #[cfg(target_os = "macos")]
    hush_macos::release_lane();
    std::process::exit(0)
}
