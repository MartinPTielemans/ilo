#!/bin/sh
# Records one process's main window for a few seconds, with the windows it opens over it while
# recording (menus, the command bar), as on screen, but nothing from other apps. For PRs whose
# change moves: a screen recording would include whatever else is open on the owner's Mac.
# Prints "recording" once frames are being written, so a caller can start the motion then.
# On Linux it records check.sh's Xvfb, which holds nothing but the app, and refuses outside it.
# Usage: tools/window-record.sh <pid> <seconds> <output.mp4>
set -e
[ $# -eq 3 ] || { echo "usage: $0 <pid> <seconds> <output.mp4>" >&2; exit 2; }
if [ "$(uname)" = Linux ]; then
  [ "${BROWSER_HEADLESS:-}" = xvfb ] ||
    { echo "Records only check.sh's Xvfb (BROWSER_XVFB)." >&2; exit 1; }
  kill -0 "$1" 2>/dev/null || { echo "process $1 isn't running" >&2; exit 1; }
  progress=$(mktemp)
  # The fastest preset, as the app draws on the same CPU, with lavapipe, while it records.
  ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -framerate 60 -i "$DISPLAY" \
    -t "$2" -c:v libx264 -preset ultrafast -pix_fmt yuv420p -progress "$progress" "$3" &
  recorder=$!
  # Ended, it ends its recording: a check that fails ends it before its own picture (#3191).
  trap 'kill "$recorder" 2>/dev/null; rm -f "$progress"; exit 143' TERM INT
  # ffmpeg reports its frames each half second, the first report once it has written some.
  until grep -q '^frame=[1-9]' "$progress"; do
    kill -0 "$recorder" 2>/dev/null || break
    sleep 0.05
  done
  status=0
  grep -q '^frame=[1-9]' "$progress" && echo recording
  wait "$recorder" || status=$?
  rm -f "$progress"
  exit "$status"
fi
# The recording is this process, so ending it ends the recording (#3191).
PID="$1" SECONDS_LONG="$2" OUT="$3" exec swift - <<'SWIFT'
import AVFoundation
import Foundation
import ScreenCaptureKit

let environment = ProcessInfo.processInfo.environment
let pid = Int32(environment["PID"]!)!
let seconds = Double(environment["SECONDS_LONG"]!)!
let out = URL(fileURLWithPath: environment["OUT"]!)

func fail(_ message: String) -> Never {
  FileHandle.standardError.write(Data((message + "\n").utf8))
  exit(1)
}

// The stream's frames, written as they come. Not SCRecordingOutput, which needs macOS 15, and the
// app runs on 14 (#4354). ScreenCaptureKit also sends frames with no new picture, while nothing on
// screen moves; only complete ones are written, and the last holds to the recording's end.
final class Writer: NSObject, SCStreamOutput {
  let queue = DispatchQueue(label: "window-record")
  let writer: AVAssetWriter
  let input: AVAssetWriterInput
  var first: CMTime?
  var began: CheckedContinuation<Void, Never>?
  init(width: Int, height: Int) throws {
    writer = try AVAssetWriter(outputURL: out, fileType: .mp4)
    input = AVAssetWriterInput(mediaType: .video, outputSettings: [
      AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: width, AVVideoHeightKey: height,
    ])
    input.expectsMediaDataInRealTime = true
    writer.add(input)
  }
  func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer,
              of type: SCStreamOutputType) {
    guard type == .screen, buffer.isValid,
      let attachments = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: false)
        as? [[SCStreamFrameInfo: Any]],
      let status = attachments.first?[.status] as? Int, SCFrameStatus(rawValue: status) == .complete
    else { return }
    if first == nil {
      guard writer.startWriting() else { fail("writing failed: \(String(describing: writer.error))") }
      first = buffer.presentationTimeStamp
      writer.startSession(atSourceTime: buffer.presentationTimeStamp)
      began?.resume()
      began = nil
    }
    if input.isReadyForMoreMediaData { input.append(buffer) }
  }
}

// A stream can end by itself before it's stopped, and then stopping it says only that it had
// (-3808, #2280), so this says why it ended and when, a line check.sh's FAIL or skip carries. The
// recording ends with it, as nothing else would end the wait for its frames (#3417).
final class Ended: NSObject, SCStreamDelegate {
  var began: Date?
  var planned = 0.0
  func stream(_ stream: SCStream, didStopWithError error: Error) {
    let when = began.map {
      String(format: "%.1f s into %.1f s", Date().timeIntervalSince($0), planned)
    } ?? "before its first frame"
    fail("the stream ended by itself \(when): \(error)")
  }
}

let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
let windows = content.windows.filter { $0.owningApplication?.processID == pid }
// The largest normal window; a process can also own invisible helper windows.
guard let main = windows.filter({ $0.windowLayer == 0 })
  .max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height })
else { fail("process \(pid) has no window on screen") }
guard let app = main.owningApplication else { fail("no app owns the window") }
guard let display = content.displays.first(where: { $0.frame.intersects(main.frame) })
else { fail("no display shows the window") }

// The app's windows, so ones it opens while recording are in it too.
let filter = SCContentFilter(display: display, including: [app], exceptingWindows: [])
let config = SCStreamConfiguration()
config.sourceRect = main.frame.offsetBy(dx: -display.frame.minX, dy: -display.frame.minY)
// Even, as H.264 wants.
config.width = Int(main.frame.width * CGFloat(filter.pointPixelScale)) / 2 * 2
config.height = Int(main.frame.height * CGFloat(filter.pointPixelScale)) / 2 * 2
config.minimumFrameInterval = CMTime(value: 1, timescale: 60)
config.showsCursor = false

try? FileManager.default.removeItem(at: out)
let writer = try Writer(width: config.width, height: config.height)
let ended = Ended()
ended.planned = seconds
let stream = SCStream(filter: filter, configuration: config, delegate: ended)
try stream.addStreamOutput(writer, type: .screen, sampleHandlerQueue: writer.queue)
// The handler forms, since Swift can pick a startCapture() that returns before capture starts.
try await withCheckedThrowingContinuation { (started: CheckedContinuation<Void, Error>) in
  stream.startCapture { error in
    if let error { started.resume(throwing: error) } else { started.resume() }
  }
}
// Frames are being written once the first is.
await withCheckedContinuation { continuation in
  writer.queue.async {
    if writer.first == nil { writer.began = continuation } else { continuation.resume() }
  }
}
ended.began = Date()
print("recording")
fflush(stdout)
try await Task.sleep(for: .seconds(seconds))
// The file is complete only once the writer says so, after the stream stops. That takes well under
// a second, so one that hasn't come in 5 s isn't coming, and the caller hears why rather than
// waiting with the helper (#3417).
do {
  let late = String(format: "the recording wasn't finished 5 s after stopCapture, "
    + "at the end of its %.1f s recording", seconds)
  DispatchQueue.global().asyncAfter(deadline: .now() + 5) { fail(late) }
}
await withCheckedContinuation { (stopped: CheckedContinuation<Void, Never>) in
  stream.stopCapture { error in
    if let error { fail("stopping failed: \(error)") }
    stopped.resume()
  }
}
await withCheckedContinuation { (finished: CheckedContinuation<Void, Never>) in
  writer.queue.async {
    writer.input.markAsFinished()
    writer.writer.endSession(atSourceTime: writer.first! + CMTime(seconds: seconds, preferredTimescale: 600))
    writer.writer.finishWriting {
      if writer.writer.status != .completed {
        fail("recording failed: \(String(describing: writer.writer.error))")
      }
      finished.resume()
    }
  }
}
SWIFT
