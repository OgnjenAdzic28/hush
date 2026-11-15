// A minimal WebKit browser window for the piano demo, built to stay out of the user's way:
// it opens behind everything, never activates, shows itself again if something hides it, and
// keeps its page's audio off the speakers. The page's Web Audio output goes to a recorder
// instead (sent to disk on SIGUSR1), and every key the page receives is logged with its time.
//
// Usage: HushPiano URL LOG AUDIO_OUT [--audible]

import AppKit
import WebKit

let args = CommandLine.arguments
let url = URL(string: args[1])!
let logPath = args[2]
let audioPath = args[3]
let audible = args.contains("--audible")
FileManager.default.createFile(atPath: logPath, contents: nil)
let logHandle = FileHandle(forWritingAtPath: logPath)!
func log(_ s: String) { logHandle.write((s + "\n").data(using: .utf8)!) }

// Runs before any page script. The page's audio graph stays native: whatever the page connects to
// the speakers is also connected to a MediaRecorder, and (unless --audible) not to the speakers.
// Keys are timestamped as the page receives them.
let hooks = """
(() => {
  const audible = \(audible);
  const realConnect = AudioNode.prototype.connect;
  const taps = new Map();
  function tap(ctx) {
    let entry = taps.get(ctx);
    if (!entry) {
      const sink = ctx.createMediaStreamDestination();
      const recorder = new MediaRecorder(sink.stream, { mimeType: 'audio/mp4' });
      entry = { sink, recorder, chunks: [], connections: 0, index: taps.size };
      recorder.ondataavailable = e => entry.chunks.push(e.data);
      recorder.start(1000);
      taps.set(ctx, entry);
      webkit.messageHandlers.hush.postMessage({ kind: 'recording', context: entry.index, state: ctx.state, rate: ctx.sampleRate });
    }
    return entry;
  }
  AudioNode.prototype.connect = function (dest, ...rest) {
    if (dest instanceof AudioDestinationNode) {
      const entry = tap(this.context);
      entry.connections += 1;
      realConnect.call(this, entry.sink, ...rest);
      if (audible) realConnect.call(this, dest, ...rest);
      return dest;
    }
    return realConnect.call(this, dest, ...rest);
  };
  window.__hushStop = () => {
    for (const [ctx, entry] of taps) {
      webkit.messageHandlers.hush.postMessage({ kind: 'context', context: entry.index, state: ctx.state, time: ctx.currentTime, connections: entry.connections });
      entry.recorder.onstop = async () => {
        const buf = await new Blob(entry.chunks, { type: 'audio/mp4' }).arrayBuffer();
        let s = ''; const b = new Uint8Array(buf);
        for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
        webkit.messageHandlers.hush.postMessage({ kind: 'audio', context: entry.index, data: btoa(s) });
      };
      entry.recorder.stop();
    }
  };
  for (const type of ['keydown', 'keyup']) {
    window.addEventListener(type, e => {
      if (e.repeat) return;
      webkit.messageHandlers.hush.postMessage({ kind: type, key: e.key, t: performance.timeOrigin + performance.now() });
    }, true);
  }
})();
"""

/// Keys the page leaves unhandled come back up the responder chain; without this, AppKit plays
/// the "can't do that" beep for each one.
final class QuietWindow: NSWindow {
  var swallowed = 0
  override func noResponder(for eventSelector: Selector) {
    swallowed += 1
    if swallowed == 1 || swallowed % 200 == 0 { log("{\"kind\":\"swallowed\",\"count\":\(swallowed)}") }
  }
}

final class Handler: NSObject, WKScriptMessageHandler {
  func userContentController(_ c: WKUserContentController, didReceive m: WKScriptMessage) {
    guard let body = m.body as? [String: Any], let kind = body["kind"] as? String else { return }
    if kind == "audio" {
      let data = Data(base64Encoded: body["data"] as? String ?? "") ?? Data()
      let index = body["context"] as? Int ?? 0
      let path = index == 0 ? audioPath : audioPath.replacingOccurrences(of: ".m4a", with: "-\(index).m4a")
      FileManager.default.createFile(atPath: path, contents: data)
      log("{\"kind\":\"audio_saved\",\"context\":\(index),\"bytes\":\(data.count)}")
      return
    }
    if let json = try? JSONSerialization.data(withJSONObject: body), let s = String(data: json, encoding: .utf8) { log(s) }
  }
}

final class Delegate: NSObject, NSApplicationDelegate, WKNavigationDelegate {
  var window: NSWindow!
  var web: WKWebView!
  let handler = Handler()
  func applicationDidFinishLaunching(_ n: Notification) {
    let config = WKWebViewConfiguration()
    config.userContentController.add(handler, name: "hush")
    config.userContentController.addUserScript(WKUserScript(source: hooks, injectionTime: .atDocumentStart, forMainFrameOnly: false))
    config.mediaTypesRequiringUserActionForPlayback = []
    web = WKWebView(frame: NSRect(x: 0, y: 0, width: 1400, height: 820), configuration: config)
    web.navigationDelegate = self
    window = QuietWindow(contentRect: web.frame, styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
    window.title = "Hush Piano"
    window.contentView = web
    window.orderBack(nil)
    window.makeMain()
    window.makeFirstResponder(web)
    web.load(URLRequest(url: url))
    // Something may hide an app an agent launched (Doorman does); show again without activating.
    var ticks = 0
    Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] timer in
      ticks += 1
      if ticks > 40 { timer.invalidate() }
      if NSApp.isHidden { NSApp.unhideWithoutActivation() }
      else if let w = self?.window, !w.isVisible { w.orderBack(nil); w.makeMain() }
    }
    signal(SIGUSR1, SIG_IGN)
    let stop = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
    stop.setEventHandler { [weak self] in self?.web.evaluateJavaScript("__hushStop()") { _, _ in } }
    stop.resume()
    self.stopSource = stop
    log("{\"kind\":\"ready\"}")
  }
  var stopSource: DispatchSourceSignal?
  func webView(_ w: WKWebView, didFinish n: WKNavigation!) { log("{\"kind\":\"loaded\"}") }
  func applicationDidHide(_ n: Notification) { NSApp.unhideWithoutActivation() }
  func applicationDidBecomeActive(_ n: Notification) {
    let front = NSWorkspace.shared.frontmostApplication?.processIdentifier == getpid()
    log("{\"kind\":\"\(front ? "ACTIVATED" : "synthetic_active")\"}")
  }
}

let app = NSApplication.shared
let delegate = Delegate()
app.delegate = delegate
app.run()
