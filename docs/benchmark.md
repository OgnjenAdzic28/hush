# Cua Driver Bench v2, run quietly with Hush

Cua Driver Bench v2 is the shared benchmark for background computer use: a model drives a tool
layer through real desktop tasks while a user keeps working, and a task passes only if its checker
passes and the user was never disturbed. Its runner is built for a spare Mac or a VM: it puts a
"User activity" window in front that types once a second, parks the pointer and kills apps by
name. Hush's run happens on the owner's own Mac while they work, so `bench/cdb/run.py` keeps what
decides a pass and changes only what would disturb a person.

## Result (15 Nov 2026, macOS 27.2, Hush at main after #64)

The five BenchLab tasks Bench v2 runs in its background condition, three runs each:

| Model | Passed | Undisturbed | Median time | Median turns | Median Hush calls | Cost, all trials |
|---|---|---|---|---|---|---|
| Claude Sonnet 5.5 | **15/15** | **15/15** | **17.8 s** | 8 | 6 | $0.42 |
| Claude Haiku 5.5 | **15/15** | **15/15** | 21.6 s | 14 | 12 | $0.04 |

Per task (Sonnet): MB-09 reorder a list by drag 3/3, IR-01 form through a permission prompt 3/3,
IR-02 settings under a banner 3/3, IR-03 two note edits through a save prompt 3/3, IR-04 web
sign-up through a cookie overlay 3/3. Raw rows are in `bench/cdb/results/`.

For reference, the published Bench v2 background pass rates from 12 Nov 2026 (sources in
`docs/spec.html`, section 13): with Claude Opus 5.5, Aside 95% in a median 22 s and 7 model calls,
cua-driver 45% (35 s, 11), the Codex computer-use plugin 25% (27 s, 9); with Claude Sonnet 5.5,
Aside 93%, cua-driver 57%, the Codex plugin 28%. **These are not like for like**, see below.

## What is the same as Bench v2

- The BenchLab app, built from the bench's own source, and its seeds (the same seed per task and
  run index as every other setup).
- The task briefs (`render_brief.py`), and the checkers (`evaluate.py`), which read BenchLab's own
  state and event log and recompute every expected value from the seed. No model grades anything.
- The system prompt (verbatim), the background paragraph, 360 s and 45 turns per trial, and the
  `claude -p` invocation: the agent gets ToolSearch and the computer-use MCP server only, no
  shell or file tools, no user settings, memory or CLAUDE.md.

## What is different

- **Only the five BenchLab background tasks.** The three multi-app tasks (CDB-G02 to G04) need
  LibreOffice, GnuCash and three Electron apps that aren't installed here. MB-10 and MB-11 run
  in the foreground condition only in Bench v2 too. So the comparison above is 5 of the 8
  background tasks, and the hardest ones are not among them.
- **The user is real.** No simulated "User activity" window: the owner was working in T3 Code (in
  full screen for most trials). Disturbance is measured by `bench/cdb/Watch.swift` every 20 ms:
  BenchLab becoming the front app, or any BenchLab window appearing on a display the user can
  see. Pointer and pasteboard changes are logged but can't be told apart from the user's own.
  The background paragraph drops the simulated window's title.
- **BenchLab starts in Hush's lane.** The runner opens BenchLab in the background and runs
  `lane` before the agent starts, so its window is never on the user's screen. The agent can see
  it and act on it all the same.
- **Three launch changes to BenchLab** (`bench/cdb/build_benchlab.sh`; task logic untouched):
  its window opens behind the user's apps instead of in front, it is made its app's main window
  without activating anything, and when Doorman hides it, it shows itself again without
  activating. Each works around a limit found during the run:
  - A window an app orders while hidden stays off screen after it unhides, and Hush's lane can't
    show an app that was hidden before its first window appeared (#62). Doorman hides every app
    an agent launches, as soon as it launches.
  - When the user is in a full-screen app, a new window opens on another Space, where
    Accessibility reaches a window no one asked about yet only as its app's main window.
- **Hush, not one of the bench's arms.** Hush's MCP server is the tool layer (`hush mcp`, six
  tools, 768 tokens of definitions).

## What the run found and fixed

- **No drag step** (#63, fixed in #64). MB-09 failed in the first run because `act` couldn't drag.
  The agent tried `drag i2 i1` and never found `timeline`. With `drag REF to REF` it passes 3/3.
  The same fix taught diffs to report reordered elements: before it, a successful drag came back
  as "no change".
- **Hidden at launch** (#62, open). See above.

## Reproduce

```sh
bench/cdb/build_benchlab.sh <cua>/libs/cua-driver-bench/automated-eval/macos_bench <build>
swiftc -O bench/cdb/Watch.swift -o <watch>
cargo build --release
python3 bench/cdb/run.py --bench <cua>/libs/cua-driver-bench/automated-eval/macos_bench \
  --build <build> --watch <watch> --runs 3 [--model claude-haiku-5-5]
```

It needs Accessibility for the process running it, `claude` logged in, and no BenchLab running.
