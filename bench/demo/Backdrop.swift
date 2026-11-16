// Draws a demo video's backdrop: a dark canvas with a title and a subtitle near the top.
//
// Usage: Backdrop OUT.png WIDTH HEIGHT TITLE SUBTITLE [TITLE_Y]

import AppKit

let a = CommandLine.arguments
let (out, width, height, title, subtitle) = (a[1], Int(a[2])!, Int(a[3])!, a[4], a[5])
let titleY = a.count > 6 ? Double(a[6])! : Double(height) * 0.16

let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
let bounds = NSRect(x: 0, y: 0, width: width, height: height)
NSColor(srgbRed: 0.043, green: 0.043, blue: 0.059, alpha: 1).setFill()
bounds.fill()
// A soft violet glow behind the title.
let glow = NSGradient(colors: [NSColor(srgbRed: 0.49, green: 0.36, blue: 1, alpha: 0.22), .clear])!
glow.draw(fromCenter: NSPoint(x: Double(width) / 2, y: Double(height) - titleY), radius: 0,
          toCenter: NSPoint(x: Double(width) / 2, y: Double(height) - titleY), radius: Double(width) * 0.35, options: [])

func centered(_ text: String, size: Double, weight: NSFont.Weight, color: NSColor, y: Double) {
  let attributes: [NSAttributedString.Key: Any] = [
    .font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color, .kern: -size * 0.02,
  ]
  let s = NSAttributedString(string: text, attributes: attributes)
  let w = s.size().width
  s.draw(at: NSPoint(x: (Double(width) - w) / 2, y: Double(height) - y - size))
}
centered(title, size: Double(width) * 0.025, weight: .bold, color: .white, y: titleY - Double(width) * 0.0125)
centered(subtitle, size: Double(width) * 0.0115, weight: .regular,
         color: NSColor(srgbRed: 0.55, green: 0.55, blue: 0.6, alpha: 1), y: titleY + Double(width) * 0.022)
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
