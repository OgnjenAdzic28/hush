---
name: hush
description: Use for any task that needs a desktop app, such as opening one, reading its window, clicking, typing, picking menus or filling in forms in any app (Calculator, an editor, a file manager, settings, a chat app, an IDE, a game), checking a page in a real browser window, or testing and demoing a desktop app. Also use it when the user says "computer use" or asks for something done on their computer. Load it before deciding an app task would take over the screen, since Hush works in the background while the user keeps working.
---

# Hush: computer use for agents

Hush reads an app's window as text and acts on it in the background. The user keeps working in
front; nothing comes to the front, the pointer doesn't move, the clipboard isn't touched. Every
command answers as soon as the app has settled, so there is nothing to sleep for. Hush runs on
macOS and Linux, with Windows next.

## Check once

```sh
hush apps            # running apps; fails if hush is missing or lacks permission
```

If `hush` isn't on PATH, ask the user to install it:

```sh
curl -fsSL https://raw.githubusercontent.com/OgnjenAdzic28/hush/main/install.sh | sh
```

If a command says it lacks Accessibility or Screen Recording, ask the user to grant it to the app
running you, in System Settings > Privacy & Security. On Linux, if it says Xvfb, ffmpeg or the
accessibility bus is missing, ask the user to run `hush setup`, which says what to install. If it says it is inside a sandbox, ask the
user to run `hush setup` or let `hush` run outside your sandbox. Don't try to work around either.

## The loop

1. **Find the app.** `hush apps notes`. Use the name it prints.
2. **Read it.** `hush observe Notes`. One line per element, each with a ref (`b3` button, `t1`
   text field, `r4` row...). If the app isn't running, this starts it in the background first;
   don't use `open`, which can bring it to the front.

   ```text
   # Notes · window "Notes" 1100x700 · focus t2
   b1 button "New Note"
   t1 search field "Search"
   r4 row "Groceries"
   t2 textarea = "milk, eggs" (10 chars)
   ```

3. **Act, in one batch.** Put every step you already know into one call. Steps run in order, each
   waits for the app to settle, and the batch stops at the first failure:

   ```sh
   hush act Notes 'click b1' 'type t2 "Call the dentist"' 'key cmd+s'
   ```

   Put each step in single quotes and text inside it in double quotes, as above. Escaped quotes
   (`"type t2 \"Hi\""`) make some harnesses, Codex among them, run the command in their sandbox,
   where Hush can't work.

   The reply is only what changed (`+` added, `-` removed, `~` changed), so read it instead of
   observing again. When a step opens a new window (`key cmd+n`, a dialog), the steps after it
   run there and the reply says so, unless you named a window with `--window`.
4. **Repeat** from the reply. `hush observe Notes` again returns only changes since your last
   read; `--full` gives the whole outline.

Refs stay valid across calls while the element exists. All `hush` shell commands share one
session, so refs from one command work in the next.

Anywhere a step takes a ref, it also takes the element as its outline line shows it:
`click button "Save"`, `type field "Name" "Ada"`, `wait sheet "Saved"`. Hush finds it in the
window when the step runs, so a batch can act on a window you haven't observed, or on what an
earlier step opened. The name matches exactly, then ignoring case, then as part of the name.
When several match, add which one: `button[2] "OK"`, or `field[1]` for an unnamed field.

## Steps

| Step | Use |
|---|---|
| `click REF` / `click X,Y` | press a button, row, link; X,Y are window points |
| `double REF`, `right REF` | double click, right click |
| `type [REF] TEXT` | type into a field (or the focused one); quote text with spaces |
| `set REF VALUE` | set a field, slider or checkbox directly; faster than typing |
| `key COMBO [xN]` | `enter`, `cmd+s`, `shift+tab`, `down x3` |
| `menu A > B` | a menu bar command: `menu File > Export…` |
| `pick REF OPTION` | choose from a popup or combo box |
| `select REF`, `focus REF`, `show REF` | select a row, focus a field, scroll it into view |
| `scroll REF up\|down\|left\|right [N]` | scroll a list or page |
| `drag A to B` | refs or X,Y; dragging onto a ref takes its place in a list |
| `wait REF`, `wait gone REF`, `wait text "T"` | wait for something to appear or go (default 5 s; add `within 30s` for longer) |
| `lane`, `unlane` | move the window off screen, where it still renders and takes input; put it back |

## The other commands

- **Scriptable apps** (Finder, Mail, Music, Safari, most Apple apps): write the whole script,
  `tell` included: `hush script Finder 'tell application "Finder" to get name of startup disk'`.
  One AppleScript line often replaces many clicks. `--js` for JXA.
- **Linux**: open apps with `hush observe APP`, which starts them on Hush's hidden display. Keys,
  pointer clicks, `see`, `timeline` and `record` only work on apps started that way. Apps the user
  already had open take `click`, `set`, `type`, `select` and `menu` on refs. `script` is macOS only.
- **Menus**: `hush observe Xcode --menus` lists every command with its shortcut.
- **Big windows**: long parts are folded. Open one with `hush observe Mail --focus e7`.
- **Pixels**: `hush see Preview --out shot.jpg` (or `--ref v2` for one element). Only for what the
  outline can't show: canvases, images, video, charts. Apps that draw their own interface (games,
  some Electron apps) also show labels read from pixels as `v` refs; click or drag them like any
  ref.
- **Timing**: `hush timeline APP` plays keys and pointer events on a millisecond schedule (music,
  games, holds). Plan the whole sequence, then send it once, one event per line, `-` to read
  stdin:

  ```sh
  printf '0 tap a\n120 tap s\n240 down 400,300\n300 move 420,300\n360 up 420,300\n' \
    | hush timeline Piano -
  ```

- **Recording a demo**: `hush record APP`, act, then `hush record APP --stop --out demo.mp4`.

## Tests

To test a desktop app, write a `.hush` file: an `app NAME` line, steps as in `act`, and
`expect` lines that check the window. Name elements as the outline shows them, never by ref
(refs only work in your session):

```text
app Notes
click button "New Note"
type textarea[1] "Groceries"
expect text "Groceries"
expect field "Title" = "Groceries"
expect gone sheet "Untitled"
```

Run it with `hush test notes.hush` (or a folder of them). Each file runs in a fresh session and
stops at its first failure with what the window shows instead; the outline and a picture of the
failure are in `hush-results/`, plus a movie with `--record`. `window "Title"` switches to another
window of the app. `expect` takes what `wait` takes, plus `ELEMENT = VALUE`.

To write a test from a run, do the task once with `hush act`, then `hush save APP FILE`: it writes
every step since the last save with refs turned into locators. Read the file, add `expect` lines
for what should be true, and run it.

## Good habits

- Read before acting, then trust the diffs. A second `observe` right after an `act` is wasted.
- Batch. Five steps in one `act` is one round trip; five calls is five.
- Prefer `set` over `type` for fields, `menu` over hunting for buttons, `script` for apps that
  have a dictionary.
- Use `wait` for slow things (a sheet, a download, a page load) instead of sleeping.
- Some commands only work on the app in front, so in the background they say "disabled right now"
  (TextEdit's Format > Bold, say). Use the app's AppleScript for those when it has one:
  `set font of paragraph 1 of text of front document to "Helvetica-Bold"`.
- Never activate or raise an app to make something work. If Hush can't do it in the background, it
  says so; report that.
- If Hush says an app is off limits or read-only, the user set that in Hush's limits. Tell them
  what you needed; don't reach the app another way (`osascript`, `open`, keystrokes). While the
  user has limits, `script` is off; use `observe` and `act`.
- If an app "isn't responding", it is stuck, not slow: tell the user rather than retrying in a
  loop.

## From MCP instead

`hush mcp` serves the same commands as MCP tools (`apps`, `observe`, `act`, `timeline`, `script`,
`see`, `record`) for harnesses that prefer it. The habits above apply unchanged.
