# Hush

Computer use that stays out of your way. Hush lets any coding agent see and drive the apps on
your Mac in the background, while you keep typing in front of them.

> **Status: early development.** Reading, outlines, diffs, scripts and screenshots are measured
> on a real Mac. Acting on real apps is built but not yet verified there (see
> [results](docs/results.md)).

## What it does

Hush is an MCP server and a CLI. An agent asks for the outline of a window, then sends a batch of
steps (click, set a value, type, press a key, pick a menu item, wait for something). Hush runs the
batch against the target app only, waits for the app to settle after each step, and answers with
just what changed. Illustrative output:

```text
observe TextEdit
# TextEdit · window "Notes.rtf" 800x600 · focus t1
e1 window "Notes.rtf"
  e2 toolbar
    b1 button "Bold"
    p1 popup "Font" = "Helvetica"
  t1 textarea = "Meeting notes, 13 Nov. Ship the…" (412 chars)

act TextEdit ["type t1 \" Done.\"", "key cmd+s"]
# 2/2 steps · 0.09 s · settled in 31 ms
~ t1 textarea: = "Meeting notes, 13 Nov. Ship the…" (412 chars) → = "Meeting notes, 13 Nov. Ship the…" (418 chars)
+ e3 sheet "Do you want to save changes?"
+   b2 button "Save"
```

- **Background by default.** Accessibility actions first; when an app needs real input, events go
  to the target process only. Your front app keeps focus, your pointer doesn't move, and the
  clipboard is never touched.
- **No fixed waits.** Hush counts the app's accessibility events and answers when they stop. Each
  app's pace is learned, so a fast native app isn't made to wait like a web view.
- **Few, small calls.** Short refs that survive re-renders, then only the changes. Several steps
  per call, stopping at the first failure. Big windows are folded to about a screenshot's worth of
  tokens. The five tool definitions are 562 tokens.
- **Any agent.** Claude Code, Codex, OpenCode, Gemini CLI or anything that speaks MCP or can run
  a shell command.

## Tools

| Tool | What it does |
|---|---|
| `apps` | Find running and installed apps |
| `observe` | The outline of a window, then only what changed; `focus` opens one part |
| `act` | Run a batch of steps and return the change |
| `script` | Run AppleScript (in process, compiled once) or JXA |
| `see` | A screenshot of one window or element, for what the outline can't show |

Steps: `click REF|X,Y`, `double REF`, `right REF`, `set REF VALUE`, `type [REF] TEXT`,
`key COMBO [xN]`, `menu A > B`, `pick REF OPTION`, `select REF`, `focus REF`,
`scroll REF up|down [N]`, `show REF`, `wait gone REF | REF | text "T" | 500ms [within 10s]`.

## Setup

Hush needs macOS 15 or later and the Rust toolchain (`rustup` installs the pinned version).

```sh
cargo build --release
# the binary is target/release/hush
```

Grant the app that runs Hush (your terminal or agent) **Accessibility**, and **Screen Recording**
for `see`, in System Settings > Privacy & Security.

Claude Code:

```sh
claude mcp add hush -- /path/to/hush mcp
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.hush]
command = "/path/to/hush"
args = ["mcp"]
```

From a shell:

```sh
hush apps
hush observe Notes
hush act Notes "click b2" "type t1 \"Groceries\"" "key enter"
hush see Notes --out notes.jpg
```

Shell commands share one background `hush daemon` per user, started by the first command and
gone after 10 idle minutes, so refs carry over and repeat looks answer with diffs, as in an MCP
session. Set `HUSH_NO_DAEMON=1` to run each command in its own process instead.

## Platforms

macOS first. The core, the engine and the MCP server are platform-neutral, so Windows and Linux
backends can follow.

## Contributing

See `CONTRIBUTING.md` and `AGENTS.md`. Security reports: `SECURITY.md`.

## License

MIT. See `LICENSE` and `NOTICE`.
