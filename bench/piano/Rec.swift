// Records one window to an H.264 movie with ScreenCaptureKit, wherever the window is (Hush's lane
// included), without touching the screen. Stops and finishes the file on SIGINT or SIGTERM.
//
// Usage: rec APP_NAME WINDOW_TITLE OUT.mp4 [FPS]

import AppKit
import AVFoundation
import CoreMedia
import ScreenCaptureKit

let appName = CommandLine.arguments[1]
let title = CommandLine.arguments[2]
let outURL = URL(fileURLWithPath: CommandLine.arguments[3])
let fps = Int32(CommandLine.arguments.count > 4 ? Int(CommandLine.arguments[4]) ?? 30 : 30)
try? FileManager.default.removeItem(at: outURL)
// ScreenCaptureKit needs this process connected to the window server.
_ = NSApplication.shared

final class Recorder: NSObject, SCStreamOutput {
  let writer: AVAssetWriter
  let input: AVAssetWriterInput
  var started = false
  var frames = 0
  init(width: Int, height: Int) throws {
    writer = try AVAssetWriter(outputURL: outURL, fileType: .mp4)
    input = AVAssetWriterInput(mediaType: .video, outputSettings: [
      AVVideoCodecKey: AVVideoCodecType.h264,
      AVVideoWidthKey: width,
      AVVideoHeightKey: height,
      AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 12_000_000],
    ])
    input.expectsMediaDataInRealTime = true
    writer.add(input)
  }
  func stream(_ s: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer, of type: SCStreamOutputType) {
    guard type == .screen, buffer.isValid,
          let attachments = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
          let status = attachments.first?[.status] as? Int, status == SCFrameStatus.complete.rawValue
    else { return }
    if !started {
      writer.startWriting()
      writer.startSession(atSourceTime: buffer.presentationTimeStamp)
      started = true
      print("first frame at \(Date().timeIntervalSince1970)")
      fflush(stdout)
    }
    if input.isReadyForMoreMediaData { input.append(buffer); frames += 1 }
  }
}

var keep: (SCStream, Recorder)?
let done = DispatchSemaphore(value: 0)

Task {
  do {
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
    guard let window = content.windows.first(where: { $0.owningApplication?.applicationName == appName && $0.title == title }) else {
      print("no window for \(appName)"); exit(1)
    }
    let filter = SCContentFilter(desktopIndependentWindow: window)
    let scale = Int(filter.pointPixelScale.rounded())
    let width = Int(window.frame.width) * scale, height = Int(window.frame.height) * scale
    let config = SCStreamConfiguration()
    config.width = width
    config.height = height
    config.minimumFrameInterval = CMTime(value: 1, timescale: fps)
    config.showsCursor = false
    config.pixelFormat = kCVPixelFormatType_32BGRA
    let recorder = try Recorder(width: width, height: height)
    let stream = SCStream(filter: filter, configuration: config, delegate: nil)
    try stream.addStreamOutput(recorder, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames"))
    try await stream.startCapture()
    keep = (stream, recorder)
    print("recording \(width)x\(height) at \(fps) fps")
    fflush(stdout)
  } catch {
    print("failed: \(error)"); exit(1)
  }
}

func finish() {
  guard let (stream, recorder) = keep, recorder.started else { print("no frames captured"); exit(1) }
  Task {
    try? await stream.stopCapture()
    recorder.input.markAsFinished()
    await recorder.writer.finishWriting()
    print("saved \(recorder.frames) frames")
    exit(0)
  }
}
signal(SIGINT, SIG_IGN); signal(SIGTERM, SIG_IGN)
let sources = [SIGINT, SIGTERM].map { sig -> DispatchSourceSignal in
  let s = DispatchSource.makeSignalSource(signal: sig, queue: .main)
  s.setEventHandler { finish() }
  s.resume()
  return s
}
dispatchMain()
