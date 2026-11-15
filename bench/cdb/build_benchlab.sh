#!/usr/bin/env bash
# Builds Cua's BenchLab for the quiet runner, with two launch changes and nothing else:
#
# 1. Its window opens behind the user's apps (orderBack) instead of in front of them
#    (makeKeyAndOrderFront). On a bench VM nobody sees the difference; on the owner's Mac a
#    window popping up over their work is the disturbance the run measures.
# 2. When something hides it (Doorman hides every app an agent launches), it shows itself again
#    without activating and orders its window back in (a window ordered while hidden stays off
#    screen after an unhide), as Hush's own test fixture does. Hush's lane can't show an app that was
#    hidden before its first window appeared without activating it (issue filed).
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
s = s.replace(front, "        if NSApp.isHidden { NSApp.unhideWithoutActivation() }\n        window.orderBack(nil)\n        rec.emit(\"app_start\"")
delegate = "final class LabDelegate: NSObject, NSApplicationDelegate {\n"
assert s.count(delegate) == 1
s = s.replace(delegate, delegate + "    func applicationDidHide(_ notification: Notification) {\n        NSApp.unhideWithoutActivation()\n        for w in NSApp.windows where w.title == \"BenchLab\" { w.orderBack(nil) }\n    }\n")
open(path, "w").write(s)
EOF
bash "$src/build.sh" "$out"
