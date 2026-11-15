# Reading only what changed (issue #28): measured, not adopted

After a step Hush reads the whole window again. The idea was to read only the elements the app's
accessibility events named, and splice them into the last snapshot. I built it and compared every
patched read with a full read taken right after it (15 Nov 2026, macOS 27.2):

- **Electron (Discord), read-only for a minute:** 3 of 3 patched reads missed elements a full read
  found, one of them a new chat message. Chromium doesn't post a notification for every node it
  adds or replaces.
- **AppKit (the fixture):** value, title, focus and checkbox changes patched exactly (0.2 to 0.4 ms
  against 1.1 ms). But `addSubview` posts no notification at all, so a step that changes a value
  and also shows a new view would be reported without the view.
- **Title bar buttons:** AppKit rebuilds the glyphs inside close, minimize and zoom whenever the
  window's key state changes, with no notification. Hush's synthetic focus changes it on every step.
  Hush now skips those children in every read, so they no longer come and go between reads.

A patched read that can miss a change is worse than a slower full read: the agent would act on a
window that isn't there. So full reads stay.

Reading in parallel was measured too: 66 ms serial against 50 ms with 2 to 8 threads for Discord's
1,400 elements. The app answers accessibility requests one at a time on its main thread, so threads
only overlap Hush's side of each round trip. Not worth the extra machinery for 1.3x.

Native windows are already fast to read in full (about 1 ms for the fixture, 14 to 84 ms for big
windows measured on 13 Nov); Electron and Chromium windows are the slow ones, and those are the ones
whose events can't be trusted.
