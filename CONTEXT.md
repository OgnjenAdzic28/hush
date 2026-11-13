# Mac computer use: project context

Working name only (`mac-computer-use`). A real name is still to be picked.
This file is the handoff from the planning thread that started under Doorman on Fri 13 Nov 2026.

## The idea

A much faster, cheaper, more reliable, agent-agnostic computer use system for macOS, probably open source.
Motivation: Claude Code's computer use barely works; Codex's is good but slow and locked to Codex.
Build day: Side Quest Sunday, Sun 15 Nov 2026. Rule: no prototyping before the event (the Doorman lesson). Research, setup and benchmark baselines are fine.

## Decisions so far

- Doorman stays in maintenance. Not this project.
- Build from scratch in Swift. Do not fork Cua. Reference repos are for reading and benchmarking only.
- The pitch must be a measured result on Cua Driver Bench v2 (background tasks while a simulated user types), not features. The field has converged on batching, diffs and scripts, so those are table stakes.
- Differentiators: never interfere with the user's typing (background reliability), settle-aware timing (answer the moment the app is ready, no fixed sleeps, no stale screenshots), and making Haiku 5.5 viable as the cheap model.
- Hard demo tests: (1) a full professional video edit where the agent finds the editor itself, learns it, edits and exports; (2) playing Moonlight Sonata on an online piano roll in rhythm (needs a precise `timeline` tool).

## Key numbers (all Aug to Nov 2026, see docs/spec.html section 13 for sources)

- Cua Driver Bench v2 with Opus 5.5, background pass rate while typing: Aside 95% (22 s, 7 calls), cua-driver 45% (35 s, 11), Codex plugin 25% (27 s, 9), agent-desktop 26 s / 12 calls.
- Targets: >=95% background pass, <=5 model calls, <=15 s median, Haiku viable.
- Measured on this Mac (macOS 27.2.1): full AX tree walk 14 to 84 ms (batched ~2x faster than naive), single attribute read 0.01 to 0.05 ms, osascript cold 67 ms vs compiled NSAppleScript ~0 ms, ScreenCaptureKit window capture 30 ms + 1.2 ms JPEG (56 KB at 960x540). Conclusion: the Mac side is fast; model round trips dominate. The live UI mirror is justified by settle detection and diffs, not read speed.
- HCU (Microsoft, open source 30 Oct): -30% steps, -57% tokens, -55% cost vs Cua.
- Caveats: Aside leads the bench and also ran the research (Scale produced the numbers); Cua grades its own driver; small samples. The bench assumes macOS 26 and this Mac runs 27.2.1.

## Files here

- `docs/spec.html` - the spec (also published privately: https://claude.ai/artifact/EnFzaGNq3arZ49Zhe7Lhyy, version "v1: Nov 2026 data"). Architecture: Router, Live UI mirror, Batch runner, Settle detector, Capture, Input guard. Seven tools: apps, observe, act, script, see, timeline (stretch), routine (later).
- `docs/research/v2-nov-2026/` - current research (benchmarks, products, research). Trust these over v1.
- `docs/research/v1-first-pass/` - older first pass; some figures are outdated.
- `bench/axbench.swift`, `bench/capbench.swift` - read-only probes. Build with `swiftc`. capbench needs `_ = NSApplication.shared` before using ScreenCaptureKit.

## Reference repos (~/projects/reference, shallow clones, read-only)

cua (MIT core, FSL bench, AGPL perception; libs/cua-driver, libs/cua-driver-bench), Peekaboo (BackgroundInputDriver, WindowRoutedPointerDriver), open-computer-use (SkyLightSPI.swift), agent-desktop (Apache-2.0), Hybrid-Computer-Use (MIT, closest to our outline-and-diff design).

## Weekend plan (open items)

- Fri/Sat: read the background-input code in the reference repos.
- Sat: get Cua Driver Bench v2 running on macOS 27; record baselines; check the piano roll site; decide whether to buy a Codex month for a real side-by-side; pick a name.
- Sun: build.

## Constraints to keep

Non-disruptive: no foreground windows, focus switches, pointer moves, native keystrokes or clipboard on the owner's Mac unless the current request explicitly asks. Benchmarks that drive the desktop need explicit go-ahead each time. Web research goes through Aside, read-only.
