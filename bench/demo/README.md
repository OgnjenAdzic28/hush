# Demo videos

Recorded with Hush's own `record` tool, which draws the agent cursor into the video only. Everything
runs in Hush's lane, in the background: nothing appears on the screen of the person using the Mac.

| Video | Script | What it shows |
|---|---|---|
| Ten agents at once | `ten.py` | Ten sessions on one Hush host, each working its own task board (add a task, drag it across, complete it, add another), each with its own cursor. All ten finished in about 15 s; the tiles play in real time. |
| FilmCraft at full speed | `filmcraft.py` | The FilmCraft edit from the agent test as fixed steps, with no model in between: playhead to 10 s, cut, a "Made with Hush" title, a 1080p export. The steps take about 7 s. FilmCraft's own encoder takes about 25 s more to render; the published cut skips that wait. |
| Speed against other tools | `Race.swift` | One round of `docs/comparison.md` (read, click, type, read) for each tool, each bar filling in real time for as long as that tool took. |

## The pieces

- `board.html`: the task board. Title, accent color and starting cards come from the query string.
  Cards move with plain mouse events, so any pointer, real or synthetic, can drag them.
- `DemoShell.swift`: a minimal quiet web window: opens behind everything, never activates, shows
  itself again without activating if a lane tool hides it, and swallows unhandled keys so AppKit
  never beeps. `ten.py` builds ten copies with different names.
- `Backdrop.swift`: the titled backdrop the ten recordings are tiled onto (this ffmpeg build has no
  text filter).
- `Race.swift`: renders the comparison as PNG frames for ffmpeg.

## Run them

```sh
swiftc -O -o /tmp/demo/DemoShell bench/demo/DemoShell.swift
python3 bench/demo/ten.py target/release/hush /tmp/demo /tmp/demo/ten-agents.mp4
python3 bench/demo/filmcraft.py target/release/hush /tmp/demo/fc        # FilmCraft running, muted
swiftc -O -o /tmp/demo/race bench/demo/Race.swift && /tmp/demo/race /tmp/demo/race
ffmpeg -framerate 60 -i /tmp/demo/race/%05d.png -pix_fmt yuv420p /tmp/demo/race.mp4
```

Learned while making them:

- WebKit reports a text input as a search field when the page is titled "Research"; scripts
  should accept either role.
- FilmCraft draws its own text fields: setting their values through Accessibility doesn't reach its
  model, so the scripts type into them. It takes one key per screen refresh, so a burst of arrow
  keys moves the playhead one frame; `timeline` spaces them 20 ms apart. Its window grows a little
  each time it is moved into the lane.
