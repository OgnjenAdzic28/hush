# Results

What is measured, what is published by others, and what is still unknown. Every Hush number
below was measured on one machine (M4 Pro, macOS 27.2.1, release build) on 15 Nov 2026, with
read-only operations: nothing was clicked, typed, focused or raised. Rerun with:

```sh
cargo test -p hush --release --test live_bench -- --ignored --nocapture      # observe and see
cargo test -p hush-macos --release --test live_read -- --ignored --nocapture # per-window reads
cargo test -p hush-macos --release --test script -- --ignored --nocapture    # in-process AppleScript
cargo test -p hush-mcp the_tool_definitions_stay_small -- --nocapture        # schema size
```

## Measured: Hush on this Mac

| What | Result |
|---|---|
| Reading a window (one batched round trip per element) | 3 to 33 ms for windows of 6 to 505 elements; 78 to 91 ms for a 1,628-element chat app |
| Building the outline from a snapshot | under 0.5 ms |
| First `observe` through the engine (find app and window, read, outline) | median 8.5 ms (3 to 92 ms) over 7 looks at 5 apps; the first look at a Chromium or Electron app in a process also switches on its web content (about 300 ms once) |
| `observe` of an unchanged window (`no change since the last look`) | median 7.6 ms (2.6 to 92 ms) |
| Refs kept across two reads of an unchanged window | 100% in every window tested |
| First-look size | 38 to 53 tokens for small windows, 1,036 to 1,304 for a full app or web page; anything over 1,500 is folded |
| Chat app outline before and after the folding rules | 8,319 → 5,843 tokens, then folded to the 1,500 budget |
| `see` (find the window, window server copy, scale to 960 wide, JPEG 0.6) | median 57 ms (22 to 61 ms), median 37 KB; covered windows captured correctly |
| AppleScript in process | 103 ms the first time (compile), then 17 µs per run |
| Tool definitions sent with every request | 562 tokens for all five tools |
| CLI end to end (`hush apps`, including process start and permission check) | 73 ms |
| CLI repeat `observe` of an unchanged window, through the daemon | 10 ms, answered "no change" |

## Published: the field (Aug to Nov 2026)

From Cua Driver Bench v2 as run by Scale (12 Nov 2026), Opus 5.5 driving every tool, background
trials while a simulated user types. Sources and caveats are in `docs/spec.html` section 13.

| Tool | Background pass | Median time | Model calls |
|---|---|---|---|
| Aside Computer Use | 95% | 22 s | 7 |
| agent-desktop | not published | 26 s | 12 |
| Codex computer use plugin | 25% | 27 s | 9 |
| cua-driver 0.47.10 | 45% | 35 s | 11 |
| Hush (target) | ≥ 95% | ≤ 15 s | ≤ 5 |

Fixed costs others publish, against Hush's measured ones:

| Cost | Others | Hush |
|---|---|---|
| Tool definitions per request | Anthropic computer use toolset ~4,500 tokens; agent-desktop fixed prefix ~12,800 | 562 |
| First look at a small window | agent-desktop skeleton 273 tokens (Apple Maps); a 960×540 screenshot ~700 | 38 to 53 |
| Running an AppleScript | `osascript` ~67 to 80 ms cold | 17 µs warm, in process |
| Window capture | ScreenCaptureKit 87 to 155 ms (open-computer-use, macOS 27) | see above |

## Measured: acting on a real app

`crates/hush/tests/live_act.rs` drives the fixture app in `bench/fixture`, which logs every input
it gets, including activation. Run on 15 Nov 2026 while the owner worked in a full-screen app:

| Route | Result |
|---|---|
| Two presses on a button | the fixture counted exactly 2 |
| Typing into a text field (Accessibility insert) | `hello` arrived, no key events sent |
| Checkbox press | on |
| `pick` on a popup | the option was chosen (see the settle note below) |
| `key cmd+k` on a background app | ran the menu item with that shortcut |
| `menu Test > Reset` | ran the item, no menu opened |
| 7 steps in one `act` call | 0.24 to 0.25 s in total, including the settle wait after each |
| The fixture activated / the user's front app changed / clipboard changed | never / never / never |

Run again on 15 Nov 2026 on a normal Desktop with Doorman running. Doorman hides every app an
agent launches, so the fixture was hidden the whole time:

| Route | Result |
|---|---|
| The 7 Accessibility steps above | all landed, 0.58 to 0.66 s for the batch |
| A second window the fixture opens after launch | found and pressed |
| Pointer click and scroll on the hidden fixture | refused with a reason: a hidden app gets the events at the right point, but AppKit doesn't hand them to its views |

Then from a full-screen Space, with the fixture showing itself again after Doorman hid it, so its
windows were visible but on the Desktop's Space:

| Route | Result |
|---|---|
| The 7 Accessibility steps and the later window | all landed, 0.87 to 0.90 s for the batch |
| Click, double click, right click, scroll up and down at a point on a canvas | each landed once at the exact window-local point (the first runs clicked twice: fixed by posting through one path) |
| The user's front app changed / the fixture really activated | never / never (it saw only Hush's in-app synthetic focus) |
| The fixture activated or unhidden while Hush acted / the user's front app changed | never / never |

Found by these runs and fixed: apps launched after Hush started were invisible (#37); `pick`
answered before the popup's menu had closed (#39); the "hidden" seen earlier was Doorman, not the
fixture's Space settings; a fixture started as a child process can activate itself at launch, so
the test now opens it through LaunchServices in the background.

## The lane (15 Nov 2026)

| Measured | Result |
|---|---|
| `lane` on the fixture, display created | 0.6 s; reusing the display 0.15 to 0.23 s |
| A fixture Doorman had hidden | shown in the lane without activating; front app unchanged |
| Click, scroll, capture in the lane | each click and scroll landed once; capture is the live window |
| Chrome 156 (throwaway profile) moved to the lane | its page's accessibility tree appeared; before, on another Space, it was missing |
| Typing into a Chrome input, textarea and contenteditable in the lane | landed; 0.09 to 0.11 s per `type` step, `cmd+a` 0.08 s |
| `kill` of the daemon owning the lane | the parked app was hidden before the display went away |

Chromium ignores Accessibility text inserts (it accepts the call and changes nothing), so typing
into Chromium always uses keys in its synthetic key window. Every event Hush posts carries
`kCGEventSourceUserData` = `0x48555348` ("HUSH"), so input watchers like Doorman can tell
background-only input from input the user sees.

## Not measured yet

- **Cua Driver Bench v2.** Pass rate, model calls and median time for Hush need the bench running
  on macOS 27 and the owner's go-ahead. Until then the comparison above is Hush's fixed costs
  against others' published end-to-end results, which are different things.
- **Settle timing on real apps.** The settle rules are tested on a fake clock; how much waiting
  they save on real apps is measured only once acting is.
- **Windows on other Spaces.** Reached by window-created notifications (any window opened after
  Hush started watching the app), the app's main and focused window, earlier finds and remote
  tokens. Still out of reach: an older window there that no assistive app has asked about
  (see `docs/notes/spaces.md`).
