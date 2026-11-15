# Hush against other computer-use tools

Measured on 15 Nov 2026 on one Mac (macOS 27.2), with `bench/compare/compare.py`. Every tool
drove the same fixture window through its own interface. Each round:

1. read the window;
2. click "Increment";
3. type into "Name";
4. read it again.

A round counts only when the fixture's own log shows both the click and the typing landed. Raw
results are in `bench/compare/results-*.json`.

## Speed: one full round (read, click, type, read)

Medians over 10 rounds, with the window on Hush's lane display, where every tool can see it:

| Tool | Round | vs Hush | Read | Click | Type | Rounds landed |
|---|---|---|---|---|---|---|
| **Hush** (daemon, as an agent uses it) | **178 ms** | 1x | 14 ms | 117 ms | 31 ms | 10/10 |
| Hush, no daemon (a fresh process per call) | 630 ms | 3.5x | 117 ms | 228 ms | 164 ms | 10/10 |
| agent-desktop 1.3.1 (Vercel) | 653 ms | 3.7x | 185 ms | 205 ms | 75 ms | 10/10 |
| cua-driver 0.49.4 | 1,134 ms | 6.4x | 77 ms | 569 ms | 392 ms | 10/10 |
| Peekaboo (main) | 3,506 ms | 19.7x | 867 ms | 370 ms | 1,382 ms | 10/10 |
| open-computer-use (main) | 4,297 ms | 24.2x | 207 ms | 2,148 ms | 1,735 ms | 10/10 |

Hush's click includes waiting for the app to settle, and its answer reports what changed. The
other tools answer as soon as the event is sent, and the agent has to read again to find out what
happened.

## Background: does it work while you're elsewhere?

| Window state | Hush | agent-desktop | cua-driver | open-computer-use | Peekaboo |
|---|---|---|---|---|---|
| On the lane display | 10/10 | 10/10 | 10/10 | 10/10 | 10/10 |
| App hidden (cmd+H, or Doorman's agent lane) | **5/5**, 206 ms | 5/5, 487 ms | 5/5, 2,476 ms | 5/5, 4,867 ms | 0/5: can't capture a hidden window |
| Visible, but on another Space (you in a full-screen app) | **5/5**, 173 ms | 0/5: lists no windows | 4/5, 1,483 ms: lost its own element handle once | 5/5, 4,352 ms | 0/5: snapshots went stale |
| Typing into a Chrome page on another Space | works after `lane` | not tested | not tested | not tested | not tested |

No tool took the front from the user in these runs.

## Tokens: what the agent pays to read

| Tool | First read of the window | Each later read | Tool list on every request (MCP) |
|---|---|---|---|
| **Hush** | **~94** | **12** when nothing changed (it sends a diff) | **598** (5 tools; 755 with `timeline`, added later) |
| open-computer-use | ~178 | ~128 | 2,654 (11 tools) |
| agent-desktop | ~306 | ~308 | 10,514 (75 tools) |
| Peekaboo | ~750 | ~752 | 13,847 (26 tools) |
| cua-driver | ~502, including a ~290-token screenshot on every read | ~521 | 50,397 (60 tools) |

Tokens are characters divided by four, for every tool. Screenshot tokens use Anthropic's rule
(width x height / 750).

## End to end with a model (published, not run here)

Codex's computer-use plugin and Aside are closed. Their numbers come from Scale's background
computer-use report of 12 Nov 2026 (model calls, wall time and background pass rate per task):

| Tool | Background pass | Median task time | Model calls |
|---|---|---|---|
| Aside | 95% | 22 s | 7 |
| cua-driver | 45% | 35 s | 11 |
| Codex plugin | 25% | 27 s | 9 |
| agent-desktop | not published | 26 s | 12 |

Hush hasn't run that suite yet: Cua Driver Bench v2 takes focus by design. What these numbers
suggest is an expectation, not a result. A task's time is mostly model calls. Hush cuts their
cost twice: the tool list is 4x to 84x smaller, and repeat reads are diffs. It may also cut how
many calls there are, because each action answers with what changed instead of needing another
read.

## How it was run, and what to be careful with

- The CLIs ran one process per call; Hush also ran that way ("no daemon"). cua-driver ran as a
  long-lived MCP server in `--direct` mode, because its daemon needs its own app's permission
  grant; its numbers therefore include no process start.
- open-computer-use and cua-driver were built from source; agent-desktop is the npm release;
  Peekaboo was built from source (its CLI).
- Typing used each tool's own advice: open-computer-use's `set_value` for a text field;
  Peekaboo needs a fresh `see` before typing after a click, which is counted in its typing time.
- One Mac, one fixture, a few rounds per condition: these are directional numbers, not a
  benchmark suite. Rerun with `bench/compare/compare.py`.
