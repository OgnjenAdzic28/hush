# Hush

Computer use that stays out of your way. Hush lets any coding agent see and drive the apps on
your Mac in the background, while you keep typing in front of them.

> **Status: early development.** Nothing here is ready to install yet.

## What it does

Hush is an MCP server and a CLI. An agent asks for the outline of a window, then sends a batch of
steps (click, set a value, type, press a key, pick a menu item, wait for something). Hush runs the
batch against the target app only, waits for the app to settle after each step, and answers with
just what changed.

- **Background by default.** Input goes to the target process. Your front app keeps focus, your
  pointer doesn't move, and the clipboard is never touched.
- **No fixed waits.** Hush watches the app's accessibility events and answers the moment it has
  finished reacting, never before.
- **Few, small calls.** A compact outline with short stable refs, then only the changes. Several
  steps per call, stopping at the first failure.
- **Any agent.** Claude Code, Codex, OpenCode, Gemini CLI or anything that speaks MCP or can run
  a shell command.

## Tools

| Tool | What it does |
|---|---|
| `apps` | Find running and installed apps |
| `observe` | The outline of a window, then only what changed |
| `act` | Run a batch of steps and return the change |
| `script` | Run AppleScript or JXA inside Hush |
| `see` | A screenshot of one window, for what the outline can't show |

## Platforms

macOS 15 or later first. The core is platform-neutral so Windows and Linux backends can follow.

## Building

```sh
cargo build --release
```

Hush needs the Accessibility permission, plus Screen Recording for `see`.

## Contributing

See `CONTRIBUTING.md`. Security reports: `SECURITY.md`.

## License

MIT. See `LICENSE` and `NOTICE`.
