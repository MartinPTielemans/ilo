// `swift display-mode.swift <width> <height>` sets the main display to that mode, as System
// Settings › Displays does, for good. check-mac.yml sets the hosted runner's 1024 by 768 screen to
// 1920 by 1080 with it, as the checks' windows and the points they click are laid out for a screen
// as big as the VMs' (#1554). Fails, naming the modes there are, when there's no such mode.
import CoreGraphics
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 3, let width = Int(arguments[1]), let height = Int(arguments[2]) else {
  print("usage: swift display-mode.swift <width> <height>")
  exit(2)
}
let display = CGMainDisplayID()
// With the 1x modes a scaled one hides, as the runner's display has only those.
let options = [kCGDisplayShowDuplicateLowResolutionModes: kCFBooleanTrue] as CFDictionary
let modes = (CGDisplayCopyAllDisplayModes(display, options) as? [CGDisplayMode]) ?? []
guard
  let mode = modes.first(where: {
    $0.width == width && $0.height == height && $0.isUsableForDesktopGUI()
  })
else {
  let sizes = modes.filter { $0.isUsableForDesktopGUI() }.map { "\($0.width)x\($0.height)" }
  print("::error::The main display has no \(width) by \(height) mode, only \(sizes.joined(separator: " "))")
  exit(1)
}
var config: CGDisplayConfigRef?
guard CGBeginDisplayConfiguration(&config) == .success,
  CGConfigureDisplayWithDisplayMode(config, display, mode, nil) == .success,
  CGCompleteDisplayConfiguration(config, .permanently) == .success
else {
  print("::error::CoreGraphics didn't set the main display to \(width) by \(height)")
  exit(1)
}
let bounds = CGDisplayBounds(display)
guard Int(bounds.width) == width && Int(bounds.height) == height else {
  print("::error::The main display is \(Int(bounds.width)) by \(Int(bounds.height)) after setting \(width) by \(height)")
  exit(1)
}
print("The main display is \(width) by \(height).")
