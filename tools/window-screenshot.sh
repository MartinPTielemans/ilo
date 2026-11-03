#!/bin/sh
# Screenshots one process's main window with its child windows (menus, the command bar) drawn
# over it, as on screen, but nothing from other apps. A screen capture would include whatever
# else is open on the owner's Mac. With "all", it takes every window of the process on that display,
# such as two browser windows, in one picture.
# On Windows, under Git Bash, it draws each of the process's windows itself with PrintWindow, in
# their order on screen, so nothing of another app's can get into the picture. PrintWindow can't
# read what DirectComposition leaves see-through, as an overlay's round corners, or the invisible
# borders a window is resized by, and draws both black, so on a CI runner, whose desktop holds
# nothing but the app, it copies the screen where the windows show instead, as Windows composes it.
# On Linux it captures the private session tools/headless.sh --run starts, or check.sh's Xvfb,
# which hold nothing but the app, and refuses outside one rather than capture the owner's screen.
# Usage: tools/window-screenshot.sh <pid> <output.png> [all]
set -e
[ $# -eq 2 ] || [ $# -eq 3 ] && [ "${3:-all}" = all ] ||
  { echo "usage: $0 <pid> <output.png> [all]" >&2; exit 2; }
case "$(uname)" in
  MINGW* | MSYS*)
    [ -e "/proc/$1/winpid" ] || { echo "process $1 isn't running" >&2; exit 1; }
    exec env PID="$(cat "/proc/$1/winpid")" OUT="$(cygpath -w "$2")" ALL="${3:-}" \
      SCREEN="${GITHUB_ACTIONS:-}" \
      powershell.exe -NoProfile -NonInteractive -Command '
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Capture {
  delegate bool EnumProc(IntPtr hwnd, IntPtr data);
  [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hwnd);
  [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out RECT rect, int size);

  static Rectangle Bounds(IntPtr hwnd) {
    RECT r;
    GetWindowRect(hwnd, out r);
    return Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom);
  }

  // The window as it shows, without the invisible borders that resize it
  // (DWMWA_EXTENDED_FRAME_BOUNDS).
  static Rectangle Shown(IntPtr hwnd) {
    RECT r;
    if (DwmGetWindowAttribute(hwnd, 9, out r, Marshal.SizeOf(typeof(RECT))) != 0) return Bounds(hwnd);
    return Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom);
  }

  public static string Run(uint pid, string path, bool all, bool screen) {
    // Rects in pixels, as PrintWindow draws.
    SetProcessDPIAware();
    // Top-level windows come front to back. Each window owned by another, a menu or the command
    // bar, is drawn in the picture of the window that owns it.
    var windows = new List<IntPtr>();
    EnumWindows((hwnd, data) => {
      uint owner;
      GetWindowThreadProcessId(hwnd, out owner);
      if (owner == pid && IsWindowVisible(hwnd) && !IsIconic(hwnd) && !Bounds(hwnd).IsEmpty)
        windows.Add(hwnd);
      return true;
    }, IntPtr.Zero);
    const uint GW_OWNER = 4;
    var mains = windows.FindAll(w => GetWindow(w, GW_OWNER) == IntPtr.Zero);
    if (mains.Count == 0) return "process " + pid + " has no window on screen";
    // The largest window, as a process also has windows nobody sees.
    mains.Sort((a, b) => (Bounds(b).Width * Bounds(b).Height).CompareTo(
        Bounds(a).Width * Bounds(a).Height));
    // The window as it shows, without the clear borders it resizes by, in both ways of capturing.
    var area = Shown(mains[0]);
    if (all) mains.ForEach(w => area = Rectangle.Union(area, Shown(w)));
    if (screen) {
      using (var image = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb)) {
        using (var graphics = Graphics.FromImage(image))
          graphics.CopyFromScreen(area.Location, Point.Empty, area.Size);
        image.Save(path, ImageFormat.Png);
      }
      return null;
    }
    using (var image = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb))
    using (var graphics = Graphics.FromImage(image)) {
      for (int i = windows.Count - 1; i >= 0; i--) {
        var top = windows[i];
        while (GetWindow(top, GW_OWNER) != IntPtr.Zero) top = GetWindow(top, GW_OWNER);
        if (!(all ? mains.Contains(top) : top == mains[0])) continue;
        var bounds = Bounds(windows[i]);
        using (var window = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb)) {
          using (var g = Graphics.FromImage(window)) {
            var dc = g.GetHdc();
            // PW_RENDERFULLCONTENT, without which a window Chromium composites draws black.
            PrintWindow(windows[i], dc, 2);
            g.ReleaseHdc(dc);
          }
          graphics.DrawImageUnscaled(window, bounds.X - area.X, bounds.Y - area.Y);
        }
      }
      image.Save(path, ImageFormat.Png);
    }
    return null;
  }
}
"@
$failed = [Capture]::Run([uint32]$env:PID, $env:OUT, $env:ALL -eq "all", $env:SCREEN -eq "true")
if ($failed) { [Console]::Error.WriteLine($failed); exit 1 }'
    ;;
esac
if [ "$(uname)" = Linux ]; then
  [ -n "${BROWSER_HEADLESS:-}" ] ||
    { echo "Captures only a session of tools/headless.sh --run or check.sh's Xvfb." >&2; exit 1; }
  kill -0 "$1" 2>/dev/null || { echo "process $1 isn't running" >&2; exit 1; }
  # The largest window of the process on screen is its main one, captured at its own size and
  # place with what's over it, as on the Mac, so a check reads a resized window's pixels where
  # they are. With "all", or with no window found, the whole screen.
  area=
  if [ "$BROWSER_HEADLESS" = xvfb ]; then
    # check.sh's Xvfb (BROWSER_XVFB), whose root window is its whole screen.
    [ -n "${3:-}" ] || area=$(
      best=0
      for window in $(xdotool search --onlyvisible --pid "$1" 2> /dev/null); do
        # WINDOW, X, Y, WIDTH, HEIGHT and SCREEN, X and Y from the root's corner.
        eval "$(xdotool getwindowgeometry --shell "$window")"
        [ $((WIDTH * HEIGHT)) -gt "$best" ] || continue
        best=$((WIDTH * HEIGHT))
        echo "${WIDTH}x$HEIGHT+$((X < 0 ? 0 : X))+$((Y < 0 ? 0 : Y))"
      done | tail -1
    )
    [ -z "$area" ] || exec import -window root -crop "$area" +repage "$2"
    exec import -window root "$2"
  fi
  # tools/headless.sh's sway session, which names the app's windows' process in its tree.
  [ -n "${3:-}" ] || area=$(swaymsg -t get_tree 2> /dev/null | python3 -I -c '
import json, sys
def nodes(node):
    yield node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from nodes(child)
ours = [node["rect"] for node in nodes(json.load(sys.stdin))
        if node.get("pid") == int(sys.argv[1]) and node.get("visible")]
if ours:
    rect = max(ours, key=lambda rect: rect["width"] * rect["height"])
    print("%(x)d,%(y)d %(width)dx%(height)d" % rect)
' "$1" 2> /dev/null) || area=
  [ -z "$area" ] || exec env WAYLAND_DISPLAY="$BROWSER_HEADLESS" grim -g "$area" "$2"
  exec env WAYLAND_DISPLAY="$BROWSER_HEADLESS" grim "$2"
fi
PID="$1" OUT="$2" ALL="${3:-}" swift - <<'SWIFT'
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

let pid = Int32(ProcessInfo.processInfo.environment["PID"]!)!
let out = URL(fileURLWithPath: ProcessInfo.processInfo.environment["OUT"]!)
let all = ProcessInfo.processInfo.environment["ALL"] == "all"

func fail(_ message: String) -> Never {
  FileHandle.standardError.write(Data((message + "\n").utf8))
  exit(1)
}

let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
let windows = content.windows.filter { $0.owningApplication?.processID == pid }
// The largest normal window; a process can also own invisible helper windows.
guard let main = windows.filter({ $0.windowLayer == 0 })
  .max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height })
else { fail("process \(pid) has no window on screen") }
guard let display = content.displays.first(where: { $0.frame.intersects(main.frame) })
else { fail("no display shows the window") }

let frame = all
  ? windows.filter { $0.windowLayer == 0 && $0.frame.intersects(display.frame) }
    .reduce(main.frame) { $0.union($1.frame) }.intersection(display.frame)
  : main.frame
let filter = SCContentFilter(display: display, including: windows)
let config = SCStreamConfiguration()
config.sourceRect = frame.offsetBy(dx: -display.frame.minX, dy: -display.frame.minY)
config.width = Int(frame.width * CGFloat(filter.pointPixelScale))
config.height = Int(frame.height * CGFloat(filter.pointPixelScale))
config.showsCursor = false
let image: CGImage = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)

guard let file = CGImageDestinationCreateWithURL(out as CFURL, UTType.png.identifier as CFString, 1, nil)
else { fail("can't write \(out.path)") }
CGImageDestinationAddImage(file, image, nil)
guard CGImageDestinationFinalize(file) else { fail("can't write \(out.path)") }
SWIFT
