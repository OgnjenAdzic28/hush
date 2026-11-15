# Windows on other Spaces: what AppKit does

Traced on 15 Nov 2026, macOS 27.2, by attaching lldb to a throwaway AppKit app (our own process)
while another process sent it Accessibility requests.

## Why `AXWindows` misses them

`-[NSApplication(NSApplicationAccessibility) accessibilityWindowsAttribute]`:

1. asks `+[NSCGSSpace currentManagedSpaces]` and `allUnmanagedSpaces`;
2. lists the app's windows on those Spaces with `+[NSCGSWindow windowsWithOptions:onSpaces:forConnectionID:]`;
3. maps each to its element with `_accessibilityWindowForWindowNumber:` and keeps the visible ones;
4. adds minimized windows; if the app is hidden, adds every window it would show again.

So a window on another Desktop, or on the Desktop while the user is in a full-screen app, isn't
listed. It isn't a timing or lazy-loading issue: it is a Space filter.

## Why remote tokens only sometimes find them

`_AXUIElementCreateWithRemoteToken` builds an element from (pid, "coco", element id) without asking
the app. The app resolves the id in `_NSAccessibilityUIElementForUniqueId`: 1 is the app, 2 the
menu bar, anything else is looked up in a table that `UniqueIdForUIElement` fills the first time
an element is handed to any assistive app. A window nobody has asked about has no id yet.

## What hands a window out regardless of Space

- **Window-created notifications.** An AXObserver registered for `AXWindowCreated` gets the new
  window's element, and from then on its id resolves for every process. Hush registers one on
  every app it lists.
- **`AXMainWindow` and `AXFocusedWindow`** return AppKit's `_mainWindow` / key window ivars, with
  no Space filter.
- **Hiding the app** lists everything (step 4 above), in under 10 ms. Hush doesn't use it: setting
  `AXHidden` back to false calls `-[NSApplication unhide:]`, which activates the app when macOS
  allows it, and `NSRunningApplication.unhide()` from another process did nothing.

## Tried and rejected

- Hit-testing (`AXUIElementCopyElementAtPosition`) only sees on-screen windows.
- Setting `AXManualAccessibility` or `AXEnhancedUserInterface` on an AppKit app changes nothing.
- An app can't move its own windows to another Space (`SLSMoveWindowsToManagedSpace` is ignored).
- Parking a hidden window off every display: AppKit pulls it back on screen when it is shown.

## Hidden apps (Doorman's lane)

Doorman hides every app an agent launches. For those, `AXWindows` already lists every window on
every Space, and every Accessibility step works while the app stays hidden. Pointer events reach
the app at the right window-local point but AppKit doesn't deliver them to views, and a window that
was never shown has no rendered contents to capture.

## What other projects do (read 15 Nov 2026)

- **alt-tab-macos** reaches the same conclusion from AppKit (`PublishedWindows.swift`): `AXWindows`
  is one `windowsWithOptions:onSpaces:` query on `currentManagedSpaces`, and `AXFocusedWindow` /
  `AXMainWindow` skip it. It brute-forces remote tokens only for window ids it has no element for,
  in 250 ms slices that resume where they stopped, because a long-running Finder keeps its windows
  at ids the first slice never reaches. It also checks the role, since children report their
  window's id.
- **yabai** scans ids `0..0x7fff` for windows its list says are missing and retries on Space
  changes. Its author found the scan can fail right after a clean reboot, so callers must not rely
  on it.
- **AeroSpace** avoids the problem: one native Space, and inactive workspaces parked in a screen
  corner. That means owning the user's window layout, which Hush doesn't.
- Before macOS 12.3, alt-tab briefly moved other-Space windows to the current Space with
  `CGSAddWindowsToSpaces` to read them; that stopped working, and it flashed the screen.
- Nobody hides and unhides apps to list windows.

What Hush took from this: the scan always tries the first 1,000 ids, then continues up to `0x8000`
for 100 ms per call from where the last call stopped; only titled windows are searched for (the
window server also lists untitled helper surfaces, such as Electron's 500x500 placeholders, which
cost every call a full scan); and an app's windows are read when it activates, while they are on
the user's Space, so they stay reachable after the user moves on.
