//! The tool list and how each call reaches the engine.
//!
//! The definitions ride on every model request and any change to them
//! throws away the client's prompt cache, so they are short and fixed for
//! the life of the server.

use hush_core::{Platform, Ref, ScriptLanguage, step, timeline};
use hush_engine::{Engine, ObserveOptions};
use serde_json::{Value, json};

use crate::base64;

pub(crate) fn list() -> Value {
    let act = format!(
        "Run steps on an app's window in the background, without touching the user's focus, pointer or \
         clipboard. Steps run in order, each waits until the app has finished reacting, and the batch stops \
         at the first failure. Returns what changed. Put several steps in one call. Steps: {}",
        step::SYNTAX
    );
    let timeline = format!(
        "Play key and pointer events on a millisecond schedule in an app's window, in the background. \
         For input whose timing matters: music, games, drags, holds. Plan the whole sequence, then send it \
         once. {}",
        timeline::SYNTAX
    );
    json!([
        {
            "name": "apps",
            "description": "List running apps, or find running and installed apps by name.",
            "inputSchema": {
                "type": "object",
                "properties": {"query": {"type": "string", "description": "Part of an app name"}}
            }
        },
        {
            "name": "observe",
            "description": "Outline of an app's window: one line per element, each with a ref like b3 or t1 \
                for act. Later calls return only what changed. Long parts are folded; pass focus to open one. \
                In apps that draw their own interface, labels read from the pixels appear as `seen` (v refs): \
                click or drag them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string", "description": "App name, bundle id or pid"},
                    "window": {"type": "string", "description": "Part of a window title (default: main window)"},
                    "focus": {"type": "string", "description": "A ref: show only this part, unfolded"},
                    "full": {"type": "boolean", "description": "The whole outline instead of changes"},
                    "menus": {"type": "boolean", "description": "Instead: every menu command with its shortcut"}
                },
                "required": ["app"]
            }
        },
        {
            "name": "act",
            "description": act,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string"},
                    "steps": {"type": "array", "items": {"type": "string"}},
                    "window": {"type": "string"}
                },
                "required": ["app", "steps"]
            }
        },
        {
            "name": "timeline",
            "description": timeline,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string"},
                    "events": {"type": "array", "items": {"type": "string"}},
                    "window": {"type": "string"}
                },
                "required": ["app", "events"]
            }
        },
        {
            "name": "script",
            "description": "Run AppleScript (default) or JavaScript for Automation. Best for apps with a \
                scripting dictionary. Returns the result as text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string"},
                    "source": {"type": "string"},
                    "language": {"type": "string", "enum": ["applescript", "javascript"]}
                },
                "required": ["app", "source"]
            }
        },
        {
            "name": "see",
            "description": "Screenshot of a window, or of one element by ref. Only for what the outline can't \
                show: canvases, video, images, charts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string"},
                    "window": {"type": "string"},
                    "ref": {"type": "string"},
                    "width": {"type": "integer", "description": "Maximum width in pixels (default 960)"}
                },
                "required": ["app"]
            }
        },
        {
            "name": "record",
            "description": "Record a window to MP4 for a demo: start, act, then call with stop. The video \
                shows an animated cursor at each action, in real time.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "app": {"type": "string"},
                    "window": {"type": "string"},
                    "stop": {"type": "boolean"},
                    "label": {"type": "string", "description": "Cursor name"},
                    "out": {"type": "string", "description": "MP4 path"}
                },
                "required": ["app"]
            }
        }
    ])
}

/// Runs one tool call. Tool errors are results the model reads, not
/// protocol errors.
pub(crate) fn call<P: Platform>(engine: &mut Engine<P>, name: &str, args: &Value) -> Value {
    match run(engine, name, args) {
        Ok(content) => json!({"content": content, "isError": false}),
        Err(message) => json!({"content": [{"type": "text", "text": message}], "isError": true}),
    }
}

fn run<P: Platform>(engine: &mut Engine<P>, name: &str, args: &Value) -> Result<Value, String> {
    let text = |s: String| Ok(json!([{"type": "text", "text": s}]));
    let string = |key: &str| args.get(key).and_then(Value::as_str);
    let app = || string("app").ok_or_else(|| "app is required".to_owned());
    let reference = |key: &str| -> Result<Option<Ref>, String> {
        string(key)
            .map(|r| Ref::parse(r).ok_or_else(|| format!("{key} must be a ref like b3, got '{r}'")))
            .transpose()
    };
    match name {
        "apps" => text(engine.apps(string("query")).map_err(|e| e.to_string())?),
        "observe" if args.get("menus").and_then(Value::as_bool) == Some(true) => {
            text(engine.menus(app()?).map_err(|e| e.to_string())?)
        }
        "observe" => {
            let options = ObserveOptions {
                window: string("window"),
                focus: reference("focus")?,
                full: args.get("full").and_then(Value::as_bool).unwrap_or(false),
            };
            text(engine.observe(app()?, options).map_err(|e| e.to_string())?)
        }
        "act" => {
            let steps = strings(args, "steps")?;
            text(
                engine
                    .act(app()?, string("window"), &steps)
                    .map_err(|e| e.to_string())?,
            )
        }
        "timeline" => {
            let events = strings(args, "events")?;
            text(
                engine
                    .timeline(app()?, string("window"), &events)
                    .map_err(|e| e.to_string())?,
            )
        }
        "script" => {
            let language = match string("language") {
                None | Some("applescript") => ScriptLanguage::AppleScript,
                Some("javascript") => ScriptLanguage::JavaScript,
                Some(other) => {
                    return Err(format!(
                        "language must be applescript or javascript, got '{other}'"
                    ));
                }
            };
            let source = string("source").ok_or("source is required")?;
            text(
                engine
                    .script(app()?, source, language)
                    .map_err(|e| e.to_string())?,
            )
        }
        "see" => {
            let width = args
                .get("width")
                .and_then(Value::as_u64)
                .map_or(960, |w| w.clamp(200, 3840) as u32);
            let image = engine
                .see(app()?, string("window"), reference("ref")?, width)
                .map_err(|e| e.to_string())?;
            Ok(json!([{
                "type": "image",
                "data": base64::encode(&image.jpeg),
                "mimeType": "image/jpeg"
            }]))
        }
        "record" if args.get("stop").and_then(Value::as_bool) == Some(true) => text(
            engine
                .record_stop(string("label"), string("out"))
                .map_err(|e| e.to_string())?,
        ),
        "record" => text(
            engine
                .record_start(app()?, string("window"))
                .map_err(|e| e.to_string())?,
        ),
        other => Err(format!("unknown tool '{other}'")),
    }
}

/// A list-of-strings argument.
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    args.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be a list of strings"))?
        .iter()
        .map(|s| {
            s.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("each of {key} must be a string"))
        })
        .collect()
}
