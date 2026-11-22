# Hush domain glossary

Words used in the code and docs. Where the project stands and why: `docs/planning/handoff.md`.

- **Agent**: the model-driven program calling Hush (Claude Code, Codex, any MCP client or shell).
  Hush never calls a model itself.
- **User**: the person at the machine. Hush must never disturb them: no focus change, no raised
  window, no pointer move, no clipboard use, no keystroke landing in their app.
- **Platform**: one OS backend (`hush-macos`, `hush-linux`; Windows later). It implements the
  `Platform` trait from `hush-core` and is the only code that touches OS APIs.
- **App**: a running process with windows, identified by pid.
- **Window**: one top-level window of an app. Hush reads and acts on one window at a time.
- **Element**: one node of a window's accessibility tree (button, text field, row...). The
  platform owns the handle; Hush gives each element a short **ref** like `b3` or `t1`.
- **Ref**: the agent-facing ID of an element. Stable for as long as the element exists, and
  reattached when an app rebuilds an identical element.
- **Snapshot**: the elements of one window read at one moment.
- **Mirror**: the snapshot Hush keeps per window, plus its refs, so later calls can answer with a
  **diff** instead of the whole outline.
- **Outline**: the compact text rendering of a snapshot that the agent reads. Noise is dropped and
  the output is byte-stable so prompt caching keeps working.
- **Diff**: what changed between two snapshots: added, removed and changed elements.
- **Step**: one thing to do inside `act`: click, set, type, key, menu, scroll, wait.
- **Batch**: the steps of one `act` call. They run in order and stop at the first failure.
- **Route**: how a step is carried out: the app's script dictionary, an accessibility action, or a
  background event sent to the target process only.
- **Settle**: the moment the app has finished reacting to a step. Hush waits for it by watching the
  app's accessibility events, never with a fixed sleep.
