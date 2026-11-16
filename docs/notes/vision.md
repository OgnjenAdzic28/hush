# Reading apps that draw their own interface

Some apps show a full interface while giving Accessibility almost nothing. FilmCraft, a
Premiere-style editor built on egui and wgpu, lists 141 accessibility elements for its main
window: 135 unnamed groups, a search field, a slider and the window buttons. Its panels, tabs,
timecodes, bins and timeline exist only as pixels. Games, canvas tools and many editors are the
same.

When a window has fewer than 20 accessibility elements that name something and is larger than
500x350 points, Hush also reads its pixels with Apple Vision text recognition and lists each label
as a `seen` element (`v` refs) with its frame. `click`, `drag`, `scroll` and `timeline` work on them
through pointer input at their place; `set` and `type` into them fail as unsupported.

Measured on FilmCraft's 1600x980 window, macOS 27.2, 15 Nov 2026:

| | Result |
|---|---|
| Labels found | 84 |
| Full observe, through the daemon | 172 ms (repeat 114 ms) |
| Text recognition alone | about 30 ms |
| `click v7` on the "Effect Controls" tab | 0.23 s with the settle wait; the diff showed the panel's new text |

How the picture is read:

- **Fast recognition only.** Accurate mode fails on macOS 27.2 with an inference-engine error
  (`e5rtError ... 13`), with or without the CPU forced.
- **At twice the window's size, in gray, with contrast x1.4.** On a 1x display (Hush's lane is
  one), small interface text is too few pixels tall. Against 36 known labels: 16 exact at the
  window's own size, 24 at 2x, 25 at 2x in gray (also about 30 ms), 26 at 3x for nearly twice the
  time.
- **Lines split at wide gaps.** Vision returns labels that sit close together as one line. Word
  boxes show where one label ends: a gap wider than 0.8 of the line height starts a new label.

Limits: clip names on a timeline's colored blocks still come out misspelled or merged
("OcearLSunset.mp"). Icons have no text, and recognition can read a label differently from one
read to the next, which shows up as a change in the diff.

## One capturing process per program file

macOS serves window captures to one live process per executable path. While one Hush process is
alive, a second one started from the same file mostly gets no answer: its request waits in
`SLSHWCaptureWindowListToIOSurfaceProxying`, and sometimes the first process's captures stop too.
It isn't Hush-specific. A 20-line C program calling the same function, and a Swift program using
the public `SCScreenshotManager`, behave the same with two copies from one path, while copies at
different paths don't interfere. Permissions don't explain it: under T3 Code, an MCP server that
Claude Code starts has T3 Code as its responsible process, which has Screen Recording.

So everything runs in one host per user (see the README), and `hush mcp` is a pipe that never
touches the window server. A capture that gets no answer within 2 s fails with a message naming
the other processes running the same file. Later captures fail at once while one is held, with a
fresh try every 10 s; after the other process exits, the host captures again within 5 to 7 s.

An earlier version of this note blamed a missing Screen Recording grant for the binary. That was
this collision: the shell's background daemon was alive whenever the agent's MCP server started.
