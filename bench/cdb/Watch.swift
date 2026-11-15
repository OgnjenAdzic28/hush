// Watches for disturbances to the user while an agent works, for the quiet
// Bench v2 runner (run.py). Every 20 ms it samples the front app, whether
// any window of the target app is on a screen the user can see, the
// pointer position and the pasteboard, and writes a JSON line whenever one
// changes. It only reads; it never takes focus.
//
// Build: swiftc -O Watch.swift -o watch
// Usage: watch <target bundle name> <out.jsonl>   (stops on SIGTERM)

import AppKit

let target = CommandLine.arguments[1]
let out = FileHandle(forWritingAtPath: {
  FileManager.default.createFile(atPath: CommandLine.arguments[2], contents: nil)
  return CommandLine.arguments[2]
}())!

func write(_ record: [String: Any]) {
  var record = record
  record["t"] = Date().timeIntervalSince1970
  if let data = try? JSONSerialization.data(withJSONObject: record) {
    out.write(data + Data("\n".utf8))
  }
}

/// Bounds of the displays a person can see: every display except Hush's
/// lane (vendor 0x4855, product 0x5348).
func visibleDisplays() -> [CGRect] {
  var ids = [CGDirectDisplayID](repeating: 0, count: 16)
  var count: UInt32 = 0
  CGGetOnlineDisplayList(16, &ids, &count)
  return ids.prefix(Int(count))
    .filter { !(CGDisplayVendorNumber($0) == 0x4855 && CGDisplayModelNumber($0) == 0x5348) }
    .map { CGDisplayBounds($0) }
}

/// Titles of the target's windows that are on screen where the user can see them.
func targetWindowsInSight() -> [String] {
  let displays = visibleDisplays()
  let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
  return list.compactMap { w in
    guard (w[kCGWindowOwnerName as String] as? String) == target,
          (w[kCGWindowLayer as String] as? Int) == 0,
          let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { return nil }
    let rect = CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
    guard displays.contains(where: { $0.intersects(rect) }) else { return nil }
    return (w[kCGWindowName as String] as? String) ?? "?"
  }
}

var lastFront = ""
var lastInSight: [String] = []
var lastPointer = CGPoint(x: -1, y: -1)
var lastPasteboard = NSPasteboard.general.changeCount
write(["event": "start", "pasteboard": lastPasteboard])

signal(SIGTERM) { _ in exit(0) }
Timer.scheduledTimer(withTimeInterval: 0.02, repeats: true) { _ in
  let front = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""
  if front != lastFront {
    write(["event": "front", "app": front])
    lastFront = front
  }
  let inSight = targetWindowsInSight()
  if inSight != lastInSight {
    write(["event": "in_sight", "windows": inSight])
    lastInSight = inSight
  }
  let pointer = CGEvent(source: nil)?.location ?? .zero
  if pointer != lastPointer {
    write(["event": "pointer", "x": pointer.x, "y": pointer.y])
    lastPointer = pointer
  }
  let pasteboard = NSPasteboard.general.changeCount
  if pasteboard != lastPasteboard {
    write(["event": "pasteboard", "count": pasteboard])
    lastPasteboard = pasteboard
  }
}
RunLoop.main.run()
