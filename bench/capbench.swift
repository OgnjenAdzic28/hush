// Read-only: time ScreenCaptureKit single-window capture and JPEG encode at 960 px wide.
import AppKit
import ScreenCaptureKit
import Foundation
func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1e6 }
func median(_ xs: [Double]) -> Double { let s = xs.sorted(); return s[s.count / 2] }
_ = NSApplication.shared
let sem = DispatchSemaphore(value: 0)
Task {
  do {
    let t0 = now()
    let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
    print(String(format: "shareable content query: %.1f ms", now() - t0))
    guard let win = content.windows.first(where: { $0.owningApplication?.applicationName == "Finder" && $0.frame.width > 300 }) ?? content.windows.first(where: { $0.frame.width > 400 && $0.windowLayer == 0 }) else { print("no window"); sem.signal(); return }
    print("window: \(win.owningApplication?.applicationName ?? "?") \(Int(win.frame.width))x\(Int(win.frame.height))")
    let filter = SCContentFilter(desktopIndependentWindow: win)
    let cfg = SCStreamConfiguration()
    let scale = 960.0 / win.frame.width
    cfg.width = 960; cfg.height = Int(win.frame.height * scale); cfg.showsCursor = false
    var cap: [Double] = [], enc: [Double] = []; var bytes = 0
    for _ in 0..<10 {
      let t = now(); let img = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg); cap.append(now() - t)
      let t2 = now()
      let rep = NSBitmapImageRep(cgImage: img)
      let data = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.6])!
      enc.append(now() - t2); bytes = data.count
    }
    print(String(format: "capture median: %.1f ms, jpeg encode median: %.1f ms, jpeg size: %d KB at %dx%d", median(cap), median(enc), bytes/1024, cfg.width, cfg.height))
  } catch { print("error: \(error)") }
  sem.signal()
}
sem.wait()
