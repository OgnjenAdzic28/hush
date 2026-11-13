// Read-only latency probe: AX tree walks, single attribute reads, osascript overhead.
// Checks permissions without prompting; never sends input or Apple Events to other apps.
import AppKit
import ApplicationServices
import Foundation

func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1e6 }
func median(_ xs: [Double]) -> Double { let s = xs.sorted(); return s[s.count / 2] }

let trusted = AXIsProcessTrusted()
print("AX trusted: \(trusted)")
print("Screen capture preflight: \(CGPreflightScreenCaptureAccess())")

func children(_ e: AXUIElement) -> [AXUIElement] {
    var v: CFTypeRef?
    guard AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &v) == .success,
          let arr = v as? [AXUIElement] else { return [] }
    return arr
}

// Walk like a naive snapshot tool: read role, title, value for every node.
func walk(_ e: AXUIElement, depth: Int, count: inout Int) {
    count += 1
    if depth > 60 { return }
    var v: CFTypeRef?
    _ = AXUIElementCopyAttributeValue(e, kAXRoleAttribute as CFString, &v)
    _ = AXUIElementCopyAttributeValue(e, kAXTitleAttribute as CFString, &v)
    _ = AXUIElementCopyAttributeValue(e, kAXValueAttribute as CFString, &v)
    for c in children(e) { walk(c, depth: depth + 1, count: &count) }
}

// Same walk with one batched call per node (AXUIElementCopyMultipleAttributeValues).
func walkBatched(_ e: AXUIElement, depth: Int, count: inout Int) {
    count += 1
    if depth > 60 { return }
    let attrs = [kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute, kAXChildrenAttribute] as CFArray
    var vals: CFArray?
    guard AXUIElementCopyMultipleAttributeValues(e, attrs, AXCopyMultipleAttributeOptions(rawValue: 0), &vals) == .success,
          let arr = vals as? [AnyObject], arr.count == 4, let kids = arr[3] as? [AXUIElement] else { return }
    for c in kids { walkBatched(c, depth: depth + 1, count: &count) }
}

if trusted {
    let apps = NSWorkspace.shared.runningApplications.filter { $0.activationPolicy == .regular }
    print("\napp | elements | naive walk ms (median of 3) | batched walk ms | one attribute read ms")
    for app in apps.prefix(12) {
        let root = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(root, 2.0)
        var naive: [Double] = [], batched: [Double] = [], single: [Double] = []
        var n = 0
        for _ in 0..<3 {
            var c = 0; let t0 = now(); walk(root, depth: 0, count: &c); naive.append(now() - t0); n = c
            var c2 = 0; let t1 = now(); walkBatched(root, depth: 0, count: &c2); batched.append(now() - t1)
        }
        var v: CFTypeRef?
        for _ in 0..<20 {
            let t = now(); _ = AXUIElementCopyAttributeValue(root, kAXFocusedWindowAttribute as CFString, &v); single.append(now() - t)
        }
        print(String(format: "%@ | %d | %.0f | %.0f | %.2f", app.localizedName ?? "?", n, median(naive), median(batched), median(single)))
    }
}

// osascript: cold process spawn vs in-process NSAppleScript, trivial script, no Apple Events.
var cold: [Double] = []
for _ in 0..<10 {
    let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/bin/osascript"); p.arguments = ["-e", "return 1"]
    p.standardOutput = FileHandle.nullDevice
    let t = now(); try? p.run(); p.waitUntilExit(); cold.append(now() - t)
}
let script = NSAppleScript(source: "return 1")!
var err: NSDictionary?
script.compileAndReturnError(&err)
var warm: [Double] = []
for _ in 0..<50 { let t = now(); _ = script.executeAndReturnError(&err); warm.append(now() - t) }
print(String(format: "\nosascript cold spawn median: %.1f ms", median(cold)))
print(String(format: "NSAppleScript in-process, compiled, median: %.3f ms", median(warm)))
