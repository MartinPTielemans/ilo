#!/bin/sh
# Prints the accessibility tree of one process's windows as VoiceOver reads it, a line an element:
# its depth, role/subrole, "title" (or description), and its value, selected and expanded states
# and identifier where it has them. Pages' own trees are left out. Asking for the tree is what
# turns it on in a GPUI window, so with a `text` it asks again for up to 10 s until a line has it.
# It needs this terminal to have the Accessibility permission and prints "accessibility: skipped"
# if it hasn't; it never asks for it. Run it in the check VM, not on the owner's Mac.
# Usage: tools/ax-tree.sh <pid> [text]
set -e
[ $# -ge 1 ] && [ $# -le 2 ] || { echo "usage: $0 <pid> [text]" >&2; exit 2; }
PID="$1" UNTIL="${2:-}" swift - <<'SWIFT'
import ApplicationServices
import Foundation

let environment = ProcessInfo.processInfo.environment
let pid = Int32(environment["PID"]!)!
let until = environment["UNTIL"] ?? ""

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
func text(_ object: AnyObject?) -> String? {
  switch object {
  case let string as String: return string
  case let number as NSNumber: return number.stringValue
  default: return nil
  }
}
func line(_ element: AXUIElement, _ depth: Int) -> String {
  let role = text(value(element, kAXRoleAttribute)) ?? "?"
  // AppKit answers AXUnknown for an element with no subrole.
  let subrole = text(value(element, kAXSubroleAttribute)).flatMap { $0 == "AXUnknown" ? nil : $0 }
    ?? "-"
  let title = text(value(element, kAXTitleAttribute)).flatMap { $0.isEmpty ? nil : $0 }
    ?? text(value(element, kAXDescriptionAttribute)) ?? ""
  var words = ["\(depth) \(role)/\(subrole) \"\(title)\""]
  if let shown = text(value(element, kAXValueAttribute)), !shown.isEmpty {
    words.append("value=\(shown)")
  }
  if (value(element, kAXSelectedAttribute) as? Bool) == true { words.append("selected") }
  if let expanded = value(element, kAXExpandedAttribute) as? Bool {
    words.append(expanded ? "expanded" : "collapsed")
  }
  if let id = text(value(element, kAXIdentifierAttribute)), !id.isEmpty {
    words.append("id=\(id)")
  }
  return words.joined(separator: " ")
}
func dump() -> [String] {
  var lines: [String] = []
  var seen: [AXUIElement] = []
  func walk(_ element: AXUIElement, _ depth: Int) {
    guard depth < 16, !seen.contains(where: { CFEqual($0, element) }) else { return }
    seen.append(element)
    lines.append(line(element, depth))
    let role = text(value(element, kAXRoleAttribute))
    // A page's tree is Chromium's, and long.
    if role == "AXWebArea" || role == "AXMenuBar" { return }
    for child in value(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
      walk(child, depth + 1)
    }
  }
  for window in value(app, kAXWindowsAttribute) as? [AXUIElement] ?? [] { walk(window, 0) }
  return lines
}
var lines = dump()
let deadline = Date().addingTimeInterval(10)
while !until.isEmpty, !lines.contains(where: { $0.contains(until) }), Date() < deadline {
  Thread.sleep(forTimeInterval: 0.2)
  lines = dump()
}
print(lines.joined(separator: "\n"))
SWIFT
