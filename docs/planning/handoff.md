# Planning handoff

Where the project stands and why. Domain words are in `CONTEXT.md` at the repo root; how to work in
the code is in `AGENTS.md`.

## The idea

A much faster, cheaper, more reliable, agent-agnostic computer use system. macOS first, with
Windows and Linux later on the same core. Open source (MIT), private until it is ready.

Motivation (Nov 2026): Claude Code's computer use has no background mode; Codex's is good but
slow and locked to Codex.

## Decisions

- Name: **Hush** (domain candidate `usehush.dev`, free on Porkbun on 14 Nov, not bought yet).
- **Rust**, from scratch. Do not fork Cua. Reference repos are for reading and benchmarking only.
  Rust over Swift because the core (outline, IDs, diffs, batches, settle rules, MCP) is shared with
  future Windows and Linux backends. Runtime speed is the same either way: the time goes to the
  target app and to model round trips.
- The pitch must be a measured result on Cua Driver Bench v2 (background tasks while a simulated
  user types), not features. Batching, diffs and scripts are table stakes.
- Differentiators: never interfere with the user's typing, settle-aware timing (answer the moment
  the app is ready, no fixed sleeps, no stale reads), and making Haiku 5.5 viable.
- Hard demo tests: (1) a full video edit where the agent finds the editor itself, learns it, edits
  and exports; (2) Moonlight Sonata on an online piano roll in rhythm (needs `timeline`).
- Side Quest Sunday became a week of work (from 15 Nov), shared on the server afterwards.

## Key numbers (Aug to Nov 2026, sources in `docs/spec.html` section 13)

- Cua Driver Bench v2, Opus 5.5, background pass rate while typing: Aside 95% (22 s, 7 calls),
  cua-driver 45% (35 s, 11), Codex plugin 25% (27 s, 9), agent-desktop 26 s / 12 calls.
- Targets: >= 95% background pass, <= 5 model calls, <= 15 s median, Haiku viable.
- Measured on an M4 Pro, macOS 27.2.1: full AX tree walk 14 to 84 ms (batched ~2x faster), one
  attribute read 0.01 to 0.05 ms, osascript cold 67 ms vs in-process compiled script ~0 ms,
  ScreenCaptureKit window capture 30 ms + 1.2 ms JPEG (56 KB at 960x540).
- HCU (Microsoft, 30 Oct): -30% steps, -57% tokens, -55% cost vs Cua.
- Caveats: Aside leads the bench and ran the research; Cua grades its own driver; small samples;
  the bench assumes macOS 26.

## Files

- `docs/spec.html`: spec v1 (13 Nov). Architecture and the seven tools.
- `docs/research/v2-nov-2026/`: current research. `v1-first-pass/` is older.
- `docs/notes/background-input.md`: how the reference repos send input in the background.
- `bench/*.swift`: the read-only probes behind the local numbers (`swiftc`).

## Reference repos (`~/projects/reference`, shallow clones, read-only)

cua (MIT core, FSL bench, AGPL perception), Peekaboo (MIT), open-computer-use (MIT),
agent-desktop (Apache-2.0), Hybrid-Computer-Use (MIT).

## Open items

- Build in small PRs: core model → macOS reading → actions, input and settle → MCP and CLI.
- Get Cua Driver Bench v2 running on macOS 27 and record baselines (drives the desktop, so it
  needs the owner's go-ahead each time).
- Check the piano roll site; decide on a Codex month; buy the domain.

## Constraints

Non-disruptive on the owner's Mac: no foreground windows, focus switches, pointer moves, native
keystrokes or clipboard unless the current request asks. Anything that drives real apps needs an
explicit go-ahead each time. Web research goes through Aside, read-only.
