// Renders the speed race: one bar per tool, each filling in real time for as long as that tool took
// for one round of docs/comparison.md (read, click, type, read). Writes PNG frames at 60 fps.
//
// Usage: Race OUT_DIR

import AppKit

let out = CommandLine.arguments[1]
let (width, height, fps) = (1920, 1080, 60.0)
// Medians from docs/comparison.md (15 Nov 2026, one Mac, macOS 27.2).
let tools: [(String, Double, Bool, String)] = [
  ("Hush", 0.178, true, ""),
  ("agent-desktop", 0.653, false, "3.7x"),
  ("cua-driver", 1.134, false, "6.4x"),
  ("Peekaboo", 3.506, false, "19.7x"),
  ("open-computer-use", 4.297, false, "24.2x"),
]
let start = 0.8, slowest = tools.map(\.1).max()!, total = start + slowest + 2.2
let accent = NSColor(srgbRed: 0.486, green: 0.361, blue: 1, alpha: 1)
let muted = NSColor(srgbRed: 0.55, green: 0.55, blue: 0.6, alpha: 1)

func text(_ s: String, _ size: Double, _ weight: NSFont.Weight, _ color: NSColor, _ x: Double, _ y: Double,
          right: Bool = false, mono: Bool = false) {
  let font = mono ? NSFont.monospacedDigitSystemFont(ofSize: size, weight: weight) : NSFont.systemFont(ofSize: size, weight: weight)
  let a = NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: color])
  a.draw(at: NSPoint(x: right ? x - a.size().width : x, y: Double(height) - y - size))
}

try? FileManager.default.createDirectory(atPath: out, withIntermediateDirectories: true)
let frames = Int(total * fps)
for n in 0..<frames {
  let t = Double(n) / fps
  let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8,
                             samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                             bytesPerRow: 0, bitsPerPixel: 0)!
  NSGraphicsContext.saveGraphicsState()
  NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
  NSColor(srgbRed: 0.043, green: 0.043, blue: 0.059, alpha: 1).setFill()
  NSRect(x: 0, y: 0, width: width, height: height).fill()
  text("One round: read, click, type, read", 56, .bold, .white, 160, 120)
  text("Same window, same Mac, real time. Medians of 10 rounds.", 28, .regular, muted, 160, 200)
  let elapsed = max(0, t - start)
  for (i, (name, seconds, mine, ratio)) in tools.enumerated() {
    let y = 330.0 + Double(i) * 132
    let trackX = 520.0, trackW = 1080.0
    text(name, 34, mine ? .bold : .medium, mine ? .white : NSColor(white: 0.85, alpha: 1), 480, y + 6, right: true)
    let track = NSRect(x: trackX, y: Double(height) - y - 56, width: trackW, height: 56)
    NSColor(white: 1, alpha: 0.06).setFill()
    NSBezierPath(roundedRect: track, xRadius: 14, yRadius: 14).fill()
    let done = elapsed >= seconds
    let progress = min(1, elapsed / seconds)
    if progress > 0 {
      let bar = NSRect(x: trackX, y: track.minY, width: max(28, trackW * progress), height: 56)
      (mine ? accent : NSColor(white: 0.42, alpha: 1)).setFill()
      NSBezierPath(roundedRect: bar, xRadius: 14, yRadius: 14).fill()
    }
    let shown = done ? seconds : elapsed
    let label = shown < 1 ? String(format: "%.0f ms", shown * 1000) : String(format: "%.2f s", shown)
    text(label, 34, done ? .bold : .regular, done ? (mine ? accent : .white) : muted, 1760, y + 6, right: true, mono: true)
    if done && !mine {
      text("\(ratio) slower", 22, .semibold, NSColor(white: 1, alpha: 0.85), trackX + 20, y + 15)
    }
    if done && mine {
      text("done", 22, .semibold, .white, trackX + 44, y + 15)
    }
  }
  text("Hush 0.1 · docs/comparison.md", 22, .regular, NSColor(white: 1, alpha: 0.3), 160, 990)
  NSGraphicsContext.restoreGraphicsState()
  try! rep.representation(using: .png, properties: [:])!
    .write(to: URL(fileURLWithPath: String(format: "%@/%05d.png", out, n)))
}
print("\(frames) frames")
