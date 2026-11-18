# Moonlight Sonata on a browser piano

The spec's hardest timing test: play movement 1 of Beethoven's Moonlight Sonata on an online
piano, in rhythm, through Hush's `timeline` tool, in the background, then record the audio and
compare note onsets with the score. Target: median onset error under 20 ms.

## Result (15 Nov 2026, macOS 27.2)

One `timeline` call: 2,288 events (1,142 notes) over 4 min 37 s, into virtualpiano.net in a
WebKit window on Hush's lane, while the owner worked in other apps.

| Measured | Result |
|---|---|
| Hush posting each event against its schedule | 0.13 ms late median, 0.51 ms worst |
| Notes the page received, right key, in order | 1,142 of 1,142 |
| Page key arrival against the score (constant delay removed) | 4 ms median, 17 ms at 95%, 41 ms worst |
| Note starts heard in the recording within 50 ms | 802 of 821 (chords count once); 5 ms median |
| The piano window came to the front | never (every front-app change followed the user's own click) |

The page-arrival times have 1 ms resolution (WebKit rounds `performance.now()`), and WebKit takes
about 4 ms per key, so the three notes of a chord arrive about 4 ms apart. The audio onset
detector is a simple energy-rise one with 5 ms steps; it finds some extra onsets in the sustained
notes and misses a few soft ones.

## More songs (15 Nov 2026)

`run.sh` plays any MIDI and writes a video of the window, synced with the page's audio:

| Piece | Notes | Largest chord | Reached the page | Arrival vs score |
|---|---|---|---|---|
| Scott Joplin, Maple Leaf Rag (public domain, Mutopia) | 2,566 in 2:24 | 7 | 2,566 | 9 ms median, 71 ms worst |
| Stevie Wonder, Ribbon in the Sky (piano and alto sax tracks of a free fan MIDI) | 1,487 in 3:44 | 5 | 1,487 | 1 ms median, 41 ms worst |

The piano window never came to the front in either. Fan MIDIs of copyrighted songs are fine for a
private test; keep their recordings private.

## The pieces

- `HushPiano.swift`: a minimal WebKit browser. It opens behind everything, never activates, shows
  itself again if Doorman hides it, and swallows keys the page leaves unhandled (otherwise AppKit
  beeps once per note). Whatever the page connects to the speakers is connected to a
  `MediaRecorder` instead (to the speakers too with `--audible`), one recording per audio context,
  saved on SIGUSR1. Every key the page receives is logged with its time.
  Chrome would be the obvious browser, but it takes focus when it launches.
- `compile.py`: MIDI to timeline lines for virtualpiano.net's keyboard (white keys C2 to C7 on
  `1` to `m`, black keys with Shift), with tempo changes, the sustain pedal and re-struck keys.
  `--track` picks tracks from a band arrangement; drums are always left out.
- `Rec.swift`: records one window with ScreenCaptureKit, wherever it is (the lane included),
  without touching the screen.
- `onsets.py`: note onsets heard in a recording against the schedule.
- `Front.swift`: logs every front-app change with how long ago the user last pressed a key or
  clicked on the real hardware, so a change can be told apart from the user's own.

## Run it

```sh
bench/piano/run.sh                                     # Moonlight Sonata, into ./hush-piano.mp4
bench/piano/run.sh song.mid out --track "piano,sax"    # any MIDI, chosen tracks
```

It downloads the MIDI from the Mutopia Project (moonlight1.mid, CC BY-SA 2.5, Winterberger's
1908 edition), builds the two Swift tools, opens the page, moves it to the lane, plays, saves the
recording and prints the timing. Nothing plays through the speakers.

An earlier attempt beeped once per note (the unhandled-key beep, fixed by `QuietWindow`) and
recorded silence (the recorder tapped the page's first audio context; the piano plays through
its second). In that attempt the piano window came to the front once, about 100 s in, while the
user was looking for the beeping. The run above, with the watcher on, had no such change.
