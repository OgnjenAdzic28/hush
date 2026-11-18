# Hush

Computer use that stays out of your way. Hush lets any coding agent see and drive the apps on
your Mac in the background, while you keep typing in front of them.

> **Status: early development.** Reading, acting, the lane and timelines are measured on a real
> Mac (see [results](docs/results.md) and [the benchmark run](docs/benchmark.md)).

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
| `timeline` | Play key and pointer events on a millisecond schedule (music, games, drags) |
| `script` | Run AppleScript (in process, compiled once) or JXA |
| `see` | A screenshot of one window or element, for what the outline can't show |

Steps: `click REF|X,Y`, `double REF`, `right REF`, `set REF VALUE`, `type [REF] TEXT`,
`key COMBO [xN]`, `menu A > B`, `pick REF OPTION`, `select REF`, `focus REF`,
`scroll REF up|down [N]`, `show REF`, `drag REF|X,Y to REF|X,Y`,
`wait gone REF | REF | text "T" | 500ms [within 10s]`. A reordered list shows in the reply as
`↕ i5 row "Test projector" · now after b1`.

Timeline events, one per line: `MS down KEY`, `MS up KEY`, `MS tap KEY [HOLD]`, `MS down X,Y`,
`MS move X,Y`, `MS up X,Y`, `MS tap X,Y [HOLD]`. The agent plans the whole sequence once and Hush
plays it on a real-time thread: in the live test, 36 notes landed within 0.3 to 0.6 ms of their
times at the median and 3.4 ms at worst, measured where the app received them.

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

## Agent skill

`skills/hush/SKILL.md` teaches any agent harness to use the `hush` CLI directly: read a window, act
in batches, trust the diffs. Copy the folder to wherever your agent loads skills, for example:

```sh
cp -r skills/hush ~/.claude/skills/   # Claude Code
cp -r skills/hush ~/.codex/skills/    # Codex
```

## Many agents at once

Everything runs in one background Hush host per user, started by the first command and gone after
10 minutes with nothing connected. Each agent's `hush mcp` is a thin pipe to its own session
there, with its own refs and diffs; shell commands share one session, so refs carry over between
commands. Agents on different apps work in parallel; two agents on the same app take turns, so
their clicks and keys never interleave. Set `HUSH_NO_DAEMON=1` to run a command or MCP server in
its own process instead.

One host isn't only lighter. macOS lets one process per program file read windows: a second Hush
process started from the same file gets no pixels while the first is alive. Ten MCP servers in ten
processes would leave nine of them blind.

Ten sessions on one host (an M4 Pro, 16 Nov 2026), each repeating observe, `see`, the menu
catalog and an AppleScript on FilmCraft and Finder: 500 calls in 2.2 s with no errors, observe
48 ms and `see` 28 ms at the median, and the host peaked at one core, 101 MB and 23 threads,
idling at 0% CPU afterwards. Five sessions typing into the same field at once left five whole
blocks.

## How it compares

Measured head to head on the same window and the same Mac (details in
[docs/comparison.md](docs/comparison.md)):

| Tool | Read, click, type, read | Works hidden | Works on another Space | Tool list (MCP) |
|---|---|---|---|---|
| **Hush** | **178 ms** | yes | yes | **598 tokens**[^tools] |
| agent-desktop | 653 ms | yes | no | 10,514 tokens |
| cua-driver | 1,134 ms | yes | mostly | 50,397 tokens |
| Peekaboo | 3,506 ms | no | no | 13,847 tokens |
| open-computer-use | 4,297 ms | yes | yes | 2,654 tokens |

[^tools]: Measured with five tools. With `timeline` added it is 755 tokens; with the menu catalog and
    `record`, about 920.

## Cua Driver Bench v2 (quiet run)

The five BenchLab tasks Bench v2 runs in the background, on the owner's Mac while they worked, with
`claude -p` driving Hush and the bench's own checkers grading (details and every difference from
the official runner in [docs/benchmark.md](docs/benchmark.md)):

| Model | Passed | User never disturbed | Median time | Median turns |
|---|---|---|---|---|
| Claude Sonnet 5.5 | 15/15 | 15/15 | 17.8 s | 8 |
| Claude Haiku 5.5 | 15/15 | 15/15 | 21.6 s | 14 |

It's 5 of Bench v2's 8 background tasks; the multi-app ones need apps not installed here.

## Recording demos

`record` films a window while Hush works in it, wherever the window is (the lane included), and
draws an agent cursor into the video: it glides to each click, field or drag and lands exactly when
the action happened, leans into its direction of travel, dips on clicks with a ripple, and carries
a name tag. The cursor exists only in the video, never on your screen. Pacing is real, so the video
shows Hush's real speed.

```sh
hush record Notes                     # start
hush act Notes "click b2" "type t1 \"Groceries\""
hush record Notes --stop --label Claude --out demo.mp4
```

## The lane

Windows an agent works in don't have to be on your screen. The `lane` step moves an app's windows
to Hush's lane: a virtual display (1920x1080) that nobody can see, placed diagonally off a corner
of your main display so the pointer can't wander onto it. There the windows render, take clicks,
scrolls and keys, can be captured, and Chromium keeps its pages' accessibility trees. An app that
was hidden (cmd+H, or a lane tool such as Doorman) shows itself only there, without activating.
Every session shares the host's one lane. It exists while the host runs; when the host exits, even
by a signal, the apps in it are hidden first, so nothing lands on your screen.

Use it for apps the agent launched: it takes their windows off your screen until you bring them
back.

## Platforms

macOS first. The core, the engine and the MCP server are platform-neutral, so Windows and Linux
backends can follow.

## Contributing

See `CONTRIBUTING.md` and `AGENTS.md`. Security reports: `SECURITY.md`.

## License

MIT. See `LICENSE` and `NOTICE`.
