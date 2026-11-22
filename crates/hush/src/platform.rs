//! The platform for the OS this binary was built for.

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use hush_core::{Error, Platform, ScriptLanguage};

#[cfg(target_os = "macos")]
pub(crate) fn platform() -> Result<hush_macos::MacPlatform, String> {
    hush_macos::MacPlatform::new().map_err(|e| e.to_string())
}

/// Never fails: without a desktop session, each call says what's missing.
#[cfg(target_os = "linux")]
pub(crate) fn platform() -> Result<hush_linux::LinuxPlatform, String> {
    Ok(hush_linux::LinuxPlatform::new())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn platform() -> Result<Unsupported, String> {
    Ok(Unsupported)
}

/// Stands in for the platforms Hush doesn't have yet. `hush mcp` still
/// starts, lists its tools and answers every call, so an agent (or a
/// registry's automated check) learns why instead of seeing the server die.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) struct Unsupported;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn unsupported<T>() -> hush_core::Result<T> {
    Err(Error::Unsupported(
        "this platform isn't supported yet; Hush runs on macOS for now".into(),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl Platform for Unsupported {
    type Handle = u8;
    fn apps(&self, _: Option<&str>) -> hush_core::Result<Vec<hush_core::AppInfo>> {
        unsupported()
    }
    fn windows(&self, _: &hush_core::AppInfo) -> hush_core::Result<Vec<hush_core::WindowInfo>> {
        unsupported()
    }
    fn read(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
    ) -> hush_core::Result<hush_core::Snapshot<u8>> {
        unsupported()
    }
    fn activity(&self, _: &hush_core::AppInfo) -> hush_core::Activity {
        hush_core::Activity::default()
    }
    fn perform(
        &self,
        _: &hush_core::AppInfo,
        _: &hush_core::WindowInfo,
        _: hush_core::Op<'_, u8>,
    ) -> hush_core::Result<()> {
        unsupported()
    }
    fn capture(
        &self,
        _: &hush_core::WindowInfo,
        _: Option<hush_core::Rect>,
        _: u32,
    ) -> hush_core::Result<hush_core::Image> {
        unsupported()
    }
    fn script(
        &self,
        _: &hush_core::AppInfo,
        _: &str,
        _: ScriptLanguage,
    ) -> hush_core::Result<String> {
        unsupported()
    }
}

#[cfg(all(test, not(any(target_os = "macos", target_os = "linux"))))]
mod tests {
    use hush_engine::Engine;
    use serde_json::{Value, json};

    use super::Unsupported;

    #[test]
    fn mcp_lists_tools_and_says_why_calls_fail() {
        let input = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "apps", "arguments": {}}}),
        ]
        .map(|m| m.to_string())
        .join("\n");
        let mut output = Vec::new();
        hush_mcp::serve(&mut Engine::new(Unsupported), input.as_bytes(), &mut output).unwrap();
        let replies: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        assert!(!replies[1]["result"]["tools"].as_array().unwrap().is_empty());
        let text = replies[2]["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("macOS"), "{text}");
        assert_eq!(replies[2]["result"]["isError"], true);
    }
}

/// Registries run MCP servers in containers with no desktop session (Glama
/// does): the server must still start, list its tools and answer calls.
#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use hush_engine::Engine;
    use serde_json::{Value, json};

    #[test]
    fn mcp_lists_tools_and_answers_without_a_desktop() {
        let input = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "apps", "arguments": {}}}),
        ]
        .map(|m| m.to_string())
        .join("\n");
        let mut output = Vec::new();
        let platform = super::platform().unwrap();
        hush_mcp::serve(&mut Engine::new(platform), input.as_bytes(), &mut output).unwrap();
        let replies: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        assert!(!replies[1]["result"]["tools"].as_array().unwrap().is_empty());
        assert!(replies[2]["result"]["content"][0]["text"].is_string());
    }
}
