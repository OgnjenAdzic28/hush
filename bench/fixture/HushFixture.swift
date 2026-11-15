// A small AppKit app for the live act test: a button, a text field, a
// checkbox, a popup, a menu item with a shortcut and a canvas that only
// takes pointer input. It opens its window behind everything without
// activating and logs every input it receives, including activation, so the
// test can prove Hush never took focus. Build: swiftc -O HushFixture.swift

import AppKit
let logPath = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/hush-fixture.log"
FileManager.default.createFile(atPath: logPath, contents: nil)
let logHandle = FileHandle(forWritingAtPath: logPath)!
func log(_ s: String) { logHandle.write((s + "\n").data(using: .utf8)!) }

final class Canvas: NSView {
  var clicks = 0
  override func mouseDown(with e: NSEvent) { clicks += 1; let p = convert(e.locationInWindow, from: nil); log("canvas click \(clicks) at \(Int(p.x)),\(Int(p.y)) count \(e.clickCount)") }
  override func rightMouseDown(with e: NSEvent) { log("canvas right click") }
  override func scrollWheel(with e: NSEvent) { log("canvas scroll dy \(e.scrollingDeltaY)") }
  override func draw(_ r: NSRect) { NSColor.systemTeal.setFill(); r.fill() }
}

final class Delegate: NSObject, NSApplicationDelegate, NSTextFieldDelegate {
  var window: NSWindow!
  var later: NSWindow?
  var count = 0
  let label = NSTextField(labelWithString: "Count 0")
  let field = NSTextField(frame: NSRect(x: 20, y: 300, width: 300, height: 24))
  let check = NSButton(checkboxWithTitle: "Agree", target: nil, action: nil)
  let popup = NSPopUpButton(frame: NSRect(x: 20, y: 220, width: 160, height: 26))
  func applicationDidFinishLaunching(_ n: Notification) {
    let menu = NSMenu(); let appItem = NSMenuItem(); menu.addItem(appItem)
    let appMenu = NSMenu(); appMenu.addItem(withTitle: "Quit", action: #selector(NSApp.terminate), keyEquivalent: "q"); appItem.submenu = appMenu
    let testItem = NSMenuItem(); menu.addItem(testItem)
    let testMenu = NSMenu(title: "Test"); testItem.submenu = testMenu
    testMenu.addItem(withTitle: "Mark", action: #selector(mark), keyEquivalent: "k").target = self
    testMenu.addItem(withTitle: "Reset", action: #selector(reset), keyEquivalent: "").target = self
    NSApp.mainMenu = menu
    window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 480, height: 420), styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = "Hush Fixture"
    let v = window.contentView!
    let button = NSButton(title: "Increment", target: self, action: #selector(increment)); button.frame = NSRect(x: 20, y: 360, width: 120, height: 30); v.addSubview(button)
    label.frame = NSRect(x: 160, y: 365, width: 200, height: 20); v.addSubview(label)
    field.placeholderString = "Name"; field.delegate = self; v.addSubview(field)
    check.frame = NSRect(x: 20, y: 260, width: 120, height: 24); check.target = self; check.action = #selector(toggled); v.addSubview(check)
    popup.addItems(withTitles: ["Courier", "Helvetica", "Menlo"]); popup.target = self; popup.action = #selector(picked); v.addSubview(popup)
    // Every pointer event that reaches the app, for diagnosing routing.
    NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp, .rightMouseDown, .mouseMoved, .scrollWheel]) { e in
      log("event \(e.type.rawValue) window \(e.windowNumber) at \(Int(e.locationInWindow.x)),\(Int(e.locationInWindow.y))")
      return e
    }
    let canvas = Canvas(frame: NSRect(x: 20, y: 20, width: 440, height: 180)); v.addSubview(canvas)
    // An ordinary window: behind everything on the user's Desktop, or on the
    // Desktop's Space while the user is in a full-screen app, where only
    // Hush's off-Space routes can reach it.
    window.orderBack(nil)
    // A second window a moment later, after Hush has started watching: in a
    // full-screen app's Space it is only reachable through its creation
    // notification.
    Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { _ in
      let later = NSWindow(contentRect: NSRect(x: 260, y: 160, width: 320, height: 160), styleMask: [.titled], backing: .buffered, defer: false)
      later.title = "Later"; later.isReleasedWhenClosed = false
      let button = NSButton(title: "Later button", target: self, action: #selector(self.laterPressed)); button.frame = NSRect(x: 20, y: 60, width: 160, height: 30)
      later.contentView!.addSubview(button)
      later.orderBack(nil); self.later = later
      log("later window")
    }
    log("ready")
  }
  @objc func increment() { count += 1; label.stringValue = "Count \(count)"; log("increment \(count)") }
  @objc func toggled() { log("checkbox \(check.state == .on ? "on" : "off")") }
  @objc func picked() { log("picked \(popup.titleOfSelectedItem ?? "")") }
  @objc func mark() { log("menu mark") }
  @objc func laterPressed() { log("later pressed") }
  @objc func reset() { log("menu reset") }
  func controlTextDidChange(_ n: Notification) { log("field \(field.stringValue)") }
  func controlTextDidEndEditing(_ n: Notification) { log("field end \(field.stringValue)") }
  func applicationDidBecomeActive(_ n: Notification) { log("ACTIVATED") }
  func applicationDidResignActive(_ n: Notification) { log("resigned active") }
  func applicationDidHide(_ n: Notification) { log("hidden") }
  func applicationDidUnhide(_ n: Notification) { log("UNHIDDEN") }
}
let app = NSApplication.shared
let d = Delegate(); app.delegate = d
app.run()
