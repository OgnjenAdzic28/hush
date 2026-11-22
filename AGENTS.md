# Working in Hush

Hush lets any agent see and drive desktop apps in the background, while the user keeps working.
macOS and Linux today; Windows next on the same core. Hush never calls a model: it answers tool
calls as fast as the target app allows, in as few tokens as possible.

## Layout

| Crate | Owns | Depends on |
|---|---|---|
| `hush-core` | domain model, refs, outline, diff, steps, settle policy, the `Platform` trait | serde |
| `hush-engine` | mirrors per window, running batches, waiting for settle | core |
| `hush-macos` | the macOS `Platform`: Accessibility, background input, capture, AppleScript | core |
| `hush-linux` | the Linux `Platform`: AT-SPI, and a hidden Xvfb lane for input, capture and recording | core |
| `hush-mcp` | the MCP server: tool schemas and dispatch | core, engine |
| `hush` | the `hush` binary: CLI verbs and `hush mcp`, picks the platform for the OS | all of the above |

Dependencies point down this table and never sideways between platform crates. `hush-core` knows
nothing about any OS, process or protocol. A platform crate knows nothing about MCP or the CLI.
Domain words are defined in `CONTEXT.md`.

Crates appear as they are built; the table is the plan they follow.

## Rules that keep it fast and small

- Every OS call lives in a platform crate. Undocumented (private) OS APIs live in one file per
  platform (`hush-macos/src/private.rs`), resolved at runtime, failing closed when missing.
- Never disturb the user: no activation, raise, focus switch, pointer warp, clipboard use or
  global event posting. Events go to the target process only. If a route can't do that, fail.
- No fixed sleeps. Wait on an observable condition (an accessibility event, a value read-back)
  with a deadline. A sleep that remains needs a comment saying what it waits for and the
  measurement behind its length.
- Tool output is for a model: short, stable refs, byte-stable ordering, no decoration. Measure
  token counts when changing the outline or diff format.
- Pure logic (outline, diff, refs, settle rules, step parsing) lives in `hush-core` and is tested
  there with plain data. Platform code stays thin.
- Files stay focused. If one passes about 400 lines or mixes concerns, split it. Use the
  `untangle-codebase` skill for larger cleanups, `codebase-design` for the vocabulary.

## Checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs these on Linux for every PR, and again on macOS when platform code changes. Tests that
read real apps are `#[ignore]` and run with `cargo test -- --ignored` on a Mac with Accessibility
permission. Tests that act on real apps need the owner's go-ahead: they can disturb whoever is
using the machine.
