// A minimal, quiet web window for demos: opens behind everything, never activates, shows itself
// again (without activating) if a lane tool hides it, and swallows keys the page leaves unhandled
// so AppKit never beeps. Hush drives the page through Accessibility like any app.
//
// Usage: DemoShell URL TITLE [WIDTH HEIGHT]

import AppKit
import WebKit

let args = CommandLine.arguments
let url = URL(string: args[1])!
let title = args[2]
let width = args.count > 3 ? Double(args[3]) ?? 640 : 640
let height = args.count > 4 ? Double(args[4]) ?? 400 : 400

final class QuietWindow: NSWindow {
  override func noResponder(for eventSelector: Selector) {}
}

final class Delegate: NSObject, NSApplicationDelegate {
  var window: NSWindow!
  var web: WKWebView!
  func applicationDidFinishLaunching(_ n: Notification) {
    web = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: height))
    window = QuietWindow(contentRect: web.frame, styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = title
    window.contentView = web
    window.orderBack(nil)
    window.makeMain()
    window.makeFirstResponder(web)
    web.load(URLRequest(url: url))
    var ticks = 0
    Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] timer in
      ticks += 1
      if ticks > 40 { timer.invalidate() }
      if NSApp.isHidden { NSApp.unhideWithoutActivation() }
      else if let w = self?.window, !w.isVisible { w.orderBack(nil); w.makeMain() }
    }
  }
  func applicationDidHide(_ n: Notification) { NSApp.unhideWithoutActivation() }
}

let app = NSApplication.shared
let delegate = Delegate()
app.delegate = delegate
app.run()
