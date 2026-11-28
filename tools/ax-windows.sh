#!/bin/sh
# Prints what other apps see of one process's windows: its windows on screen in the window
# server's order, front first, with their level, alpha and frame, as T3 Code's window capture reads
# them (it takes the first at level 0), and its AXFocusedWindow, AXMainWindow and AXWindows with
# their roles, subroles and frames. The accessibility half needs this terminal to have the
# Accessibility permission and says so if it hasn't; it never asks for it. Run it in the check VM,
# not on the owner's Mac.
# With a point, in points from the main display's top left, it also prints what accessibility's hit
# test finds there, as VoiceOver's does under the pointer: "hit at X Y: <role>/<subrole> "<name>"".
# It asks every 0.1 s for 2 s, as VoiceOver asks again while the pointer rests, and prints each
# answer that differs from the last: a GPUI window builds its tree once first asked.
# Usage: tools/ax-windows.sh <pid> [<x> <y>]
set -e
[ $# -eq 1 ] || [ $# -eq 3 ] || { echo "usage: $0 <pid> [<x> <y>]" >&2; exit 2; }
PID="$1" AT="${2:-} ${3:-}" swift - <<'SWIFT'
import ApplicationServices
import CoreGraphics
import Foundation

let pid = Int32(ProcessInfo.processInfo.environment["PID"]!)!

print("window server, front first:")
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
  as? [[String: Any]] ?? []
for info in list where info[kCGWindowOwnerPID as String] as? Int32 == pid {
  let bounds = info[kCGWindowBounds as String] as? [String: Double] ?? [:]
  print(String(format: "  %@ level %@ alpha %@ at %.0f,%.0f %.0fx%.0f",
               "\(info[kCGWindowNumber as String] ?? "?")", "\(info[kCGWindowLayer as String] ?? "?")",
               "\(info[kCGWindowAlpha as String] ?? "?")", bounds["X"] ?? 0, bounds["Y"] ?? 0,
               bounds["Width"] ?? 0, bounds["Height"] ?? 0))
}

guard AXIsProcessTrusted() else {
  print("accessibility: skipped, this terminal doesn't have the Accessibility permission")
  exit(0)
}
let app = AXUIElementCreateApplication(pid)
func value(_ element: AXUIElement, _ attribute: String) -> AnyObject? {
  var result: AnyObject?
  return AXUIElementCopyAttributeValue(element, attribute as CFString, &result) == .success
    ? result : nil
}
func describe(_ element: AnyObject?) -> String {
  guard let element, CFGetTypeID(element) == AXUIElementGetTypeID() else { return "none" }
  let window = element as! AXUIElement
  var origin = CGPoint.zero, size = CGSize.zero
  if let position = value(window, kAXPositionAttribute) {
    AXValueGetValue(position as! AXValue, .cgPoint, &origin)
  }
  if let extent = value(window, kAXSizeAttribute) {
    AXValueGetValue(extent as! AXValue, .cgSize, &size)
  }
  return String(format: "%@/%@ \"%@\" at %.0f,%.0f %.0fx%.0f",
                value(window, kAXRoleAttribute) as? String ?? "?",
                value(window, kAXSubroleAttribute) as? String ?? "-",
                value(window, kAXTitleAttribute) as? String ?? "", origin.x, origin.y,
                size.width, size.height)
}
print("AXFocusedWindow: \(describe(value(app, kAXFocusedWindowAttribute)))")
print("AXMainWindow: \(describe(value(app, kAXMainWindowAttribute)))")
print("AXWindows:")
for window in value(app, kAXWindowsAttribute) as? [AXUIElement] ?? [] {
  print("  \(describe(window))")
}
let at = ProcessInfo.processInfo.environment["AT"]!.split(separator: " ").compactMap { Float($0) }
if at.count == 2 {
  var last = ""
  for _ in 0..<20 {
    var hit: AXUIElement?
    var found = "none"
    if AXUIElementCopyElementAtPosition(app, at[0], at[1], &hit) == .success, let hit {
      let title = value(hit, kAXTitleAttribute) as? String ?? ""
      let name = title.isEmpty ? value(hit, kAXDescriptionAttribute) as? String ?? "" : title
      let role = value(hit, kAXRoleAttribute) as? String ?? "?"
      found = "\(role)/\(value(hit, kAXSubroleAttribute) as? String ?? "-") \"\(name)\""
    }
    if found != last {
      print("hit at \(Int(at[0])) \(Int(at[1])): \(found)")
      last = found
    }
    Thread.sleep(forTimeInterval: 0.1)
  }
}
SWIFT
