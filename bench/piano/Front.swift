// Logs every change of the front app, with how long ago the user last pressed a key or clicked
// on the real hardware (events posted to one process, as Hush does, don't count there), so a
// front-app change can be told apart: the user's own switch, or something else's.
import AppKit
let out = FileHandle(forWritingAtPath: { FileManager.default.createFile(atPath: CommandLine.arguments[1], contents: nil); return CommandLine.arguments[1] }())!
var last = ""
signal(SIGTERM) { _ in exit(0) }
Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true) { _ in
  let front = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""
  guard front != last else { return }
  last = front
  let key = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .keyDown)
  let click = min(CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .leftMouseDown),
                  CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .rightMouseDown))
  let flags = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .flagsChanged)
  out.write(String(format: "{\"t\":%.3f,\"front\":\"%@\",\"user_key_s\":%.2f,\"user_click_s\":%.2f,\"user_modifier_s\":%.2f}\n", Date().timeIntervalSince1970, front, key, click, flags).data(using: .utf8)!)
}
RunLoop.main.run()
