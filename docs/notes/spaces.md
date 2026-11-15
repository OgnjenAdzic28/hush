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
