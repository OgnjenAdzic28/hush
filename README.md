# Hush

**The fastest computer use for AI agents.**

https://github.com/user-attachments/assets/d415d27b-6906-4f0c-a700-dbc79e826e34

Hush lets any agent see and drive the apps on your computer, in the background, while you keep working.
Read a window, click, type, read it again: **178 ms**. The next fastest tool takes 653 ms.

That's fast enough to win one-minute bullet against a 2200 chess.com bot, to play the third
movement of the Moonlight Sonata at full tempo with every note on time, and to run ten agents on ten
apps at once on one machine, all without anything coming to the front.

## How it compares

Same window, same machine, one round of read, click, type, read
([details](docs/comparison.md)):

| Tool | Round | Works hidden | Works on another Space | Tool list (MCP) |
|---|---|---|---|---|
| **Hush** | **178 ms** | yes | yes | **598 tokens**[^tools] |
| agent-desktop | 653 ms | yes | no | 10,514 tokens |
| cua-driver | 1,134 ms | yes | mostly | 50,397 tokens |
| Peekaboo | 3,506 ms | no | no | 13,847 tokens |
| open-computer-use | 4,297 ms | yes | yes | 2,654 tokens |

On [Cua Driver Bench v2](docs/benchmark.md)'s background tasks, agents on Hush went **15/15** with
Claude Sonnet 5.5 and with Claude Haiku 5.5, the user never disturbed once. Sonnet took a median
17.8 s per task.

[^tools]: Five tools. With `timeline`, 755 tokens; with the menu catalog and `record`, about 920.

## What an agent sees

An agent reads a window as a short outline, sends a batch of steps, and gets back only what changed:

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

- **Never waits for nothing.** Hush listens to the app's accessibility events and answers the
  moment they stop. It learns each app's pace, so a fast native app is never made to wait like a
  web view.
- **Tiny on tokens.** Short refs that survive re-renders, then only the diffs. A repeat read of an
  unchanged window is 12 tokens.
- **Stays in the background.** Events go to the target app only. Your front app keeps focus, your
  pointer stays put, your clipboard is untouched.
- **Any agent harness.** Claude Code, Codex, OpenCode, Gemini CLI, your own loop: as a CLI, an
  MCP server or a skill.

## Tools

| Tool | What it does |
|---|---|
| `apps` | Find running and installed apps |
| `observe` | The outline of a window, then only what changed; `focus` opens one part |
| `act` | Run a batch of steps and return the change |
| `timeline` | Play key and pointer events on a millisecond schedule (music, games, drags) |
| `script` | Run AppleScript (in process, compiled once) or JXA |
| `see` | A screenshot of one window or element, for what the outline can't show |
| `record` | Film a window with an animated agent cursor |

Steps: `click REF|X,Y`, `double REF`, `right REF`, `set REF VALUE`, `type [REF] TEXT`,
`key COMBO [xN]`, `menu A > B`, `pick REF OPTION`, `select REF`, `focus REF`,
`scroll REF up|down|left|right [N]`, `show REF`, `drag REF|X,Y to REF|X,Y`,
`wait gone REF | REF | text "T" | 500ms [within 10s]`, `lane`.

Timeline events, one per line: `MS down KEY`, `MS up KEY`, `MS tap KEY [HOLD]`, `MS down X,Y`,
`MS move X,Y`, `MS up X,Y`, `MS tap X,Y [HOLD]`. Hush plays them on a real-time thread, under a
millisecond off schedule at the median.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/OgnjenAdzic28/hush/main/install.sh | sh
```

That's it. The installer puts `hush` in `~/.local/bin` and runs `hush setup`, which asks macOS for
**Accessibility** and **Screen Recording** for your terminal and installs the Hush skill for
Claude Code and in `~/.agents/skills`, the shared folder Codex, Cursor, Amp, Cline and most other
agents read (plus a rule that lets Codex run `hush` outside its sandbox). Then ask your agent to do
something in an app.

Needs macOS 15 or later. Run `hush setup` again any time to check permissions or update the skill.

On Linux the same command installs a static binary for x86_64 or arm64, and `hush setup` checks the
accessibility bus, Xvfb and ffmpeg instead. On Debian or Ubuntu that's
`sudo apt install xvfb at-spi2-core ffmpeg`; on Fedora,
`sudo dnf install xorg-x11-server-Xvfb at-spi2-core ffmpeg-free`.

**Pick your agents**: install the skill with the [skills](https://skills.sh) CLI instead, which
lets you choose from 70+ agents:

```sh
npx skills add OgnjenAdzic28/hush --skill hush
```

**Another harness**: `hush setup --skills-dir <its skills folder>`, or use the MCP server:

```sh
claude mcp add hush -- hush mcp
```

```toml
# ~/.codex/config.toml
[mcp_servers.hush]
command = "hush"
args = ["mcp"]
```

**From source**: `cargo install --git https://github.com/OgnjenAdzic28/hush hush`, with Rust 1.100
or later (`rustup update`).

**Try it yourself**:

```sh
hush apps
hush observe Notes
hush act Notes 'click b2' 'type t1 "Groceries"' 'key enter'
hush see Notes --out notes.jpg
```

## Many agents at once

One Hush host serves every agent on your machine. It starts with the first command and goes away after
ten idle minutes. Each agent gets its own session with its own refs and diffs. Agents on different
apps run in parallel; two agents on the same app take turns, so their clicks and keys never
interleave.

Ten sessions hammering one host: 500 calls in 2.2 s, zero errors, observe in 48 ms and `see` in
28 ms at the median, on one core and 101 MB.

## The lane

Agents don't need your screen. The `lane` step moves an app's windows onto Hush's lane, a virtual
display nobody can see, parked off a corner of your main display where the pointer can't reach.
Windows there still render, take clicks and keys, and can be captured. When the host exits, apps in
the lane are hidden first, so nothing ever lands on your screen.

On Linux the lane is a hidden X server that Hush starts for itself. Every app an agent opens runs
there from the start, whether your desktop is X11 or Wayland.

## Recording demos

`record` films a window while Hush works in it, lane included, and draws an agent cursor into the
video: it glides to each click, leans into its motion, dips with a ripple on clicks and carries a
name tag. The cursor lives only in the video, and the pacing is real.

```sh
hush record Notes
hush act Notes 'click b2' 'type t1 "Groceries"'
hush record Notes --stop --label Claude --out demo.mp4
```

## Platforms

**macOS** 15 or later: everything above.

**Linux**: Hush reads and drives apps through AT-SPI, the accessibility bus GTK, Qt, Firefox,
Chromium and LibreOffice share. Apps an agent opens start on Hush's hidden display, where keys,
clicks, drags, timelines, `see` and `record` all work and none of it can reach your screen. Apps
you already had open take clicks, `set`, `type`, `select` and menus through accessibility, and
nothing else, so an agent can never type into your window by mistake. `cmd+s` means Ctrl+S there.

Every change runs the real binary against GNOME Calculator, Mousepad, GNOME Text Editor,
FeatherPad (Qt), LibreOffice Writer, Firefox and Google Chrome: start, read, type, read back,
picture. It also
runs against a real GNOME Shell Wayland session, with an app open on the desktop that Hush reads
and clicks without disturbing. Measured on GitHub's Linux runners: observe reads GNOME
Calculator's 98 elements in 24 ms, a click settles in 20 to 35 ms, and timeline events land
0.18 ms late at the median.

**Windows** is next.

## Contributing

See `CONTRIBUTING.md` and `AGENTS.md`. Security reports: `SECURITY.md`.

## License

MIT. See `LICENSE` and `NOTICE`.
