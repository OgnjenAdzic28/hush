# Background input: what the reference repos do

Read on 15 Nov 2026 from the shallow clones in `~/projects/reference`. These are notes for building
our own input layer, not code to copy. open-computer-use and Cua core are MIT, so the private ABI
facts below (record layouts, event fields) can be reused with attribution in the one file that
holds private API.

## The short version

1. **Accessibility actions first.** `AXPress`, `AXValue` writes, `AXFocused`, menu items by path.
   They work on background windows and never touch the user's input. Peekaboo and Cua both prefer
   them for single left clicks.
2. **Keys to a native AppKit app:** the public `CGEvent.postToPid` works even when the app is
   inactive and its window is not key. No private API needed.
3. **Keys to Chromium or Electron:** they arrive, but the text is not inserted because the page
   is only focused while its `NSWindow` is the AppKit key window. The fix ("sky_key"):
   - post a synthetic-activate record to the target's PSN only (`SLPSPostEventRecordTo`,
     record `[0x08]=0x0D`, `[0x8A]=0x01`, window id at `0x3C..0x3F`);
   - then post yabai's make-key-window record pair (`[0x3A]=0x10`, `[0x20..0x30)=0xFF`,
     `[0x08]` = `0x01` then `0x02`). The order matters; the other way round it is ignored;
   - post keys with `postToPid`;
   - post the deactivate record (`[0x8A]=0x02`).
   The real front app never resigns key or active, the pointer and z-order don't change, and it
   works across Spaces. Hidden (`cmd+h`) apps reject it: fail closed, never unhide.
4. **Cmd shortcuts to a background app don't work as key events** (AppKit only routes key
   equivalents for the active app). Find the menu item by `AXMenuItemCmdChar` +
   `AXMenuItemCmdModifiers` and `AXPress` it instead.
5. **Pixel clicks ("sky_click")** for things with no AX element: a mouse event recipe of
   move to target, primer down/up at (-1, -1), then down/up at the target. Each event gets raw
   fields stamped (40 target pid, 51 window number, 58 click group id, 91/92 window under
   pointer, 1 click state, 0 phase) and `CGEventSetWindowLocation`. On macOS 27 the location
   is the **window-local** point; before 27 it was the screen point (Cua tests this). Every event
   goes out through both `SLEventPostToPid` and public `postToPid`. Wrap it in the synthetic
   activate/deactivate records when the target isn't frontmost.
6. **Proof before keys (from Cua):** after the key-window records, re-read the app's focused
   window through AX and only post keys when it really is the target. Refusing is safe; a key
   that lands in a Find bar or another document isn't.

## Timing measured by open-computer-use (macOS 27.0, M5, covered Chrome window)

| Path | Returns p50 | Page reflects it p50 |
| - | - | - |
| sky_click | 82 ms | 260 ms |
| sky_key type_text | 30 ms | 237 ms |

- The gaps inside the click recipe can't be zero: they failed at 0, passed at 0.05x and
  default to 0.2x (20 ms after the primer, 20 ms renderer settle).
- The gap after a focus record can be ~0 on 27 but use 10 ms; macOS 26.6 dropped 1 in 50 keys at 0.
- These are exactly the fixed sleeps our settle detector should replace where it can observe the
  result (AX value change, focused element change).

## Other useful findings

- `SLSHWCaptureWindowList` captures a window's backing store in 15 to 33 ms, works for covered
  windows and inactive full-screen Spaces where ScreenCaptureKit fails (-3811). SCK is the fallback.
- `SLSPackagesEnableWindowOcclusionNotifications(cid, wid, 0, &prev)` keeps a covered Chromium
  window thinking it is visible, so its page keeps rendering and its AX tree stays full.
- `_AXUIElementGetWindow` maps an AX window to its `CGWindowID`.
- Literal text must not inherit modifiers the user is holding (Peekaboo): build events with
  explicit flags, never from the combined session state.
- Physical keystrokes while the target holds synthetic key state still go to the real front app.

## What Cua Driver Bench v2 counts as "disturbed"

All eight must hold for a background pass (Amendment 22): no loss of key focus, no window raised
over the user window, typing never lost or redirected, pointer never moved more than 1 px,
clipboard never changed, no Space or full-screen switch, no sheet or dialog on the user's app, and
the user's app never deactivated or activated. A criterion that wasn't measured counts as disturbed.

So: never use the clipboard for paste, never `SLPSSetFrontProcessWithOptions` (it makes the front
app resign active), never move the real cursor.

## Rejected by the references

- `SLPSSetFrontProcessWithOptions` (Cua's foreground assist): front app resigns active.
- `CGSSetConnectionProperty(..., "SetFrontmost")`: returns 1000 on macOS 27.
- `SLSEventAuthenticationMessage` alone, window fields on key events, AX `AXFocused`/`AXMain` on
  the window: none of these focus a Chromium page.

## Files worth rereading

- `open-computer-use/packages/OpenComputerUseKit/Sources/OpenComputerUseKit/SkyLightSPI.swift`,
  `SkyClickSimulation.swift`, `SkyKeyboardSimulation.swift`
- `open-computer-use/docs/references/macos-skylight-background-keyboard.md` (Chinese),
  `background-input-benchmarks.md`
- `cua/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs` (key-window proof),
  `input/mouse.rs` (window-local stamp on 27)
- `Peekaboo/.../Services/UI/BackgroundInputDriver.swift` (AX-first click target resolution)
