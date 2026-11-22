//! The platform for the OS this binary was built for.

#[cfg(not(target_os = "macos"))]
use hush_core::{Error, Platform, ScriptLanguage};

#[cfg(target_os = "macos")]
pub(crate) fn platform() -> Result<hush_macos::MacPlatform, String> {
    hush_macos::MacPlatform::new().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform() -> Result<Unsupported, String> {
    Ok(Unsupported)
}

/// Stands in for the platforms Hush doesn't have yet. `hush mcp` still
/// starts, lists its tools and answers every call, so an agent (or a
/// registry's automated check) learns why instead of seeing the server die.
#[cfg(not(target_os = "macos"))]
pub(crate) struct Unsupported;

#[cfg(not(target_os = "macos"))]
fn unsupported<T>() -> hush_core::Result<T> {
    Err(Error::Unsupported(
        "this platform isn't supported yet; Hush runs on macOS for now".into(),
    ))
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(all(test, not(target_os = "macos")))]
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
