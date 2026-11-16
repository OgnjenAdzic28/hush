//! The protocol against a one-window platform.

use std::io::Cursor;

use hush_core::{
    Actions, Activity, AppInfo, Error, Image, Node, Op, Platform, Rect, Result, Role,
    ScriptLanguage, Snapshot, States, WindowInfo, approx_tokens,
};
use hush_engine::Engine;
use serde_json::{Value, json};

struct One;

impl Platform for One {
    type Handle = u8;

    fn apps(&self, _query: Option<&str>) -> Result<Vec<AppInfo>> {
        Ok(vec![AppInfo {
            name: "Notes".into(),
            pid: Some(5),
            id: None,
            path: None,
            frontmost: false,
        }])
    }

    fn windows(&self, _app: &AppInfo) -> Result<Vec<WindowInfo>> {
        Ok(vec![WindowInfo {
            id: 1,
            title: "Notes".into(),
            frame: Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 300.0,
            },
            main: true,
            minimized: false,
        }])
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<u8>> {
        let node = |handle, role, name: &str, parent| Node {
            handle,
            role,
            name: name.into(),
            value: None,
            frame: None,
            states: States::default(),
            actions: Actions::PRESS,
            parent,
            unread: 0,
        };
        let nodes = vec![
            node(0, Role::Window, "Notes", None),
            node(1, Role::Button, "New", Some(0)),
        ];
        Ok(Snapshot::new(
            app.clone(),
            window.clone(),
            nodes,
            Vec::new(),
        ))
    }

    fn activity(&self, _app: &AppInfo) -> Activity {
        Activity::default()
    }

    fn perform(&self, _app: &AppInfo, _window: &WindowInfo, _op: Op<'_, u8>) -> Result<()> {
        Ok(())
    }

    fn capture(
        &self,
        _window: &WindowInfo,
        _region: Option<Rect>,
        _max_width: u32,
    ) -> Result<Image> {
        Ok(Image {
            jpeg: vec![0xFF, 0xD8, 0xFF],
            width: 1,
            height: 1,
        })
    }

    fn script(&self, _app: &AppInfo, source: &str, _language: ScriptLanguage) -> Result<String> {
        if source.is_empty() {
            Err(Error::Failed("empty".into()))
        } else {
            Ok(source.to_uppercase())
        }
    }
}

fn exchange(messages: &[Value]) -> Vec<Value> {
    let input: String = messages.iter().map(|m| format!("{m}\n")).collect();
    let mut output = Vec::new();
    crate::serve(&mut Engine::new(One), Cursor::new(input), &mut output).unwrap();
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn call(id: u32, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}})
}

#[test]
fn a_session_initializes_lists_tools_and_calls_them() {
    let replies = exchange(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        call(3, "observe", json!({"app": "Notes"})),
        call(4, "act", json!({"app": "Notes", "steps": ["click b1"]})),
        call(5, "see", json!({"app": "Notes"})),
        call(6, "act", json!({"app": "Notes", "steps": ["fly"]})),
        call(
            7,
            "timeline",
            json!({"app": "Notes", "events": ["0 tap a", "x up a"]}),
        ),
    ]);
    assert_eq!(replies.len(), 7, "the notification gets no reply");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "apps", "observe", "act", "timeline", "script", "see", "record"
        ]
    );
    assert!(
        replies[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("b1 button \"New\"")
    );
    assert!(
        replies[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("# 1/1 steps")
    );
    assert_eq!(replies[4]["result"]["content"][0]["data"], "/9j/");
    assert_eq!(replies[5]["result"]["isError"], true);
    assert!(
        replies[5]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown step")
    );
    assert_eq!(replies[6]["result"]["isError"], true);
    assert!(
        replies[6]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("line 2")
    );
}

#[test]
fn the_tool_definitions_stay_small() {
    let tools = serde_json::to_string(&crate::tools::list()).unwrap();
    let tokens = approx_tokens(&tools);
    println!("tool definitions: {tokens} tokens");
    assert!(tokens < 1500, "tool definitions grew to {tokens} tokens");
}

#[test]
fn unknown_methods_and_bad_json_are_protocol_errors() {
    let replies = exchange(&[json!({"jsonrpc": "2.0", "id": 9, "method": "resources/list"})]);
    assert_eq!(replies[0]["error"]["code"], -32601);
    let mut engine = Engine::new(One);
    assert_eq!(
        crate::handle(&mut engine, "{not json").unwrap()["error"]["code"],
        -32700
    );
}
