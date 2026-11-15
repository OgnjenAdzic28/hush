//! A Model Context Protocol server over stdio: newline-delimited JSON-RPC
//! 2.0, the tools from `tools.rs`, nothing else. Calls are handled one at a
//! time on the calling thread, which is what the engine expects.

mod base64;
mod tools;

use std::io::{self, BufRead, Write};

use hush_core::Platform;
use hush_engine::Engine;
use serde_json::{Value, json};

/// The newest protocol revision this server implements. A client asking
/// for an older one gets that one back.
const PROTOCOL: &str = "2025-06-18";

/// Serves requests from `input` until it closes.
pub fn serve<P: Platform>(
    engine: &mut Engine<P>,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle(engine, &line) {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The response to one message, or `None` for notifications.
pub fn handle<P: Platform>(engine: &mut Engine<P>, line: &str) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(error) => {
            return Some(failure(
                Value::Null,
                -32700,
                &format!("parse error: {error}"),
            ));
        }
    };
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            json!({
                "protocolVersion": asked.filter(|v| *v <= PROTOCOL).unwrap_or(PROTOCOL),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "hush", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Hush drives desktop apps in the background. observe an app to get refs, \
                    then act with several steps per call; each act answers with what changed, so observe \
                    again only when you need parts you haven't seen."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools::list()}),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(failure(id, -32602, "tools/call needs a tool name"));
            };
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            tools::call(engine, name, &args)
        }
        other => return Some(failure(id, -32601, &format!("unknown method '{other}'"))),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests;
