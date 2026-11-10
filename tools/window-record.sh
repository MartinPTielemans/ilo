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
PID="$1" SECONDS_LONG="$2" OUT="$3" swift - <<'SWIFT'
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

final class Finished: NSObject, SCRecordingOutputDelegate {
  var done: CheckedContinuation<Void, Never>?
  func recordingOutputDidFinishRecording(_ recordingOutput: SCRecordingOutput) {
    done?.resume()
  }
  func recordingOutput(_ recordingOutput: SCRecordingOutput, didFailWithError error: Error) {
    fail("recording failed: \(error)")
  }
}

// A stream can end by itself before it's stopped, and then stopping it says only that it had
// (-3808, #2280), so this says why it ended and when, a line check.sh's FAIL carries.
final class Ended: NSObject, SCStreamDelegate {
  var began: Date?
  var planned = 0.0
  func stream(_ stream: SCStream, didStopWithError error: Error) {
    let when = began.map {
      String(format: "%.1f s into %.1f s", Date().timeIntervalSince($0), planned)
    } ?? "before its first frame"
    let line = "the stream ended by itself \(when): \(error)\n"
    FileHandle.standardError.write(Data(line.utf8))
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
config.width = Int(main.frame.width * CGFloat(filter.pointPixelScale))
config.height = Int(main.frame.height * CGFloat(filter.pointPixelScale))
config.minimumFrameInterval = CMTime(value: 1, timescale: 60)
config.showsCursor = false

let recording = SCRecordingOutputConfiguration()
recording.outputURL = out
recording.outputFileType = .mp4
recording.videoCodecType = .h264
let finished = Finished()
let output = SCRecordingOutput(configuration: recording, delegate: finished)
let ended = Ended()
ended.planned = seconds
let stream = SCStream(filter: filter, configuration: config, delegate: ended)
try? FileManager.default.removeItem(at: out)
try stream.addRecordingOutput(output)
// The handler forms, since Swift can pick a startCapture() that returns before capture starts.
try await withCheckedThrowingContinuation { (started: CheckedContinuation<Void, Error>) in
  stream.startCapture { error in
    if let error { started.resume(throwing: error) } else { started.resume() }
  }
}
ended.began = Date()
print("recording")
fflush(stdout)
try await Task.sleep(for: .seconds(seconds))
// The file is complete only once the recording output says so, after the stream stops.
await withCheckedContinuation { continuation in
  finished.done = continuation
  stream.stopCapture { error in
    if let error { fail("stopping failed: \(error)") }
  }
}
SWIFT
