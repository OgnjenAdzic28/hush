#!/usr/bin/env bash
# Builds Cua's BenchLab for the quiet runner, with launch changes only:
#
# 1. Its window opens behind the user's apps (orderBack) instead of in front of them
#    (makeKeyAndOrderFront). On a bench VM nobody sees the difference; on the owner's Mac a
#    window popping up over their work is the disturbance the run measures.
# 2. That window is made its app's main window, without activating anything. When the user is in
#    a full-screen app, the window opens on another Space, where Accessibility reaches a window
#    nobody asked about yet only as the app's main window.
# 3. When something hides it (Doorman hides every app an agent launches), it shows itself again
#    without activating and orders its window back in, as Hush's own fixture does: a window
#    ordered while hidden stays off screen after an unhide. Hush's lane can't show an app that
#    was hidden before its first window appeared (#62).
#
# Task logic, seeds, state, events and checkers are untouched.
#
# Usage: build_benchlab.sh <macos_bench dir> <out dir>
set -euo pipefail
bench="$1"
out="$2"
src="$(mktemp -d)/swift"
cp -R "$bench/swift" "$src"
python3 - "$src/BenchLab.swift" <<'EOF'
import sys
path = sys.argv[1]
s = open(path).read()
front = "        window.makeKeyAndOrderFront(nil)\n        rec.emit(\"app_start\""
assert s.count(front) == 1, "BenchLab's launch changed; update build_benchlab.sh"
s = s.replace(front, (
    "        window.orderBack(nil)\n"
    # Its app's main window, without activating anything: when the user is in a full-screen app
    # the window opens on another Space, where Accessibility reaches it only as the main window.
    "        window.makeMain()\n"
    # Doorman's hide can land before, during or after launch, and a window ordered in while the
    # app is hidden stays off screen after an unhide (which finishes asynchronously). So for the
    # first five seconds: show the app again without activating, then order the window back in.
    "        var ticks = 0\n"
    "        Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] timer in\n"
    "            ticks += 1\n"
    "            if ticks > 20 { timer.invalidate() }\n"
    "            if NSApp.isHidden { NSApp.unhideWithoutActivation() }\n"
    "            else if let w = self?.window, !w.isVisible { w.orderBack(nil) }\n"
    "        }\n"
    "        rec.emit(\"app_start\""))
delegate = "final class LabDelegate: NSObject, NSApplicationDelegate {\n"
assert s.count(delegate) == 1
s = s.replace(delegate, delegate + (
    "    func applicationDidHide(_ notification: Notification) { NSApp.unhideWithoutActivation() }\n"
    "    func applicationDidUnhide(_ notification: Notification) {\n"
    "        for w in NSApp.windows where w.title == \"BenchLab\" && !w.isVisible { w.orderBack(nil) }\n"
    "    }\n"))
open(path, "w").write(s)
EOF
bash "$src/build.sh" "$out"
