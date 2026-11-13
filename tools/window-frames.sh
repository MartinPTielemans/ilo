#!/bin/sh
# Screenshots one process's main window as fast as it can, until <seconds> pass or the file <stop>
# appears, and says of each picture whether the window's rect <x> <y> <w> <h>, in points from its
# top-left, is one flat colour: as the page area is while no page shows in it (#1438). A recording
# compresses away what it would need to tell, and the hosted Mac's records nothing (#1108).
# Prints "capturing" once the first picture can be taken, then a line a picture:
#   <asked> <got> flat|page <luma range> <mean red> <mean green> <mean blue>
# with the seconds since 1970 as it was asked for and as it came back: it shows the window at some
# moment between, which on the Mac spans 25 ms or more. It keeps the first ten flat pictures in
# <flat-dir>, named for when they were asked for: on Linux of the whole window, to show what the UI
# around the page area drew with it.
# On the Mac it takes them as tools/window-screenshot.sh does. On Linux it reads check.sh's Xvfb or
# tools/headless.sh's sway session, each holding nothing but the app, and refuses outside them; on
# Windows it copies the screen where the window is, only on a CI runner, whose desktop holds nothing
# but the app.
# With <every-dir>, on Windows and Linux, it also keeps every picture of the rect there, named the
# same way, written once it stops so writing them doesn't slow the pictures: a motion's frames, to
# read and to show (check/peek-motion.sh).
# Usage: tools/window-frames.sh <pid> <seconds> <stop> <x> <y> <w> <h> <flat-dir> [every-dir]
set -e
[ $# -eq 8 ] || [ $# -eq 9 ] ||
  { echo "usage: $0 <pid> <seconds> <stop> <x> <y> <w> <h> <flat-dir> [every-dir]" >&2; exit 2; }
every=${9:-}
[ -z "$every" ] || mkdir -p "$every"
kill -0 "$1" 2> /dev/null || { echo "process $1 isn't running" >&2; exit 1; }
mkdir -p "$8"
# A rect flat to within this much luma shows no page: a page's text or edges reach far more.
flat=12
case "$(uname)" in
MINGW* | MSYS*)
  [ -n "${GITHUB_ACTIONS:-}" ] ||
    { echo "Copies the screen only on a CI runner, whose desktop holds nothing but the app." >&2; exit 1; }
  exec env PID="$(cat "/proc/$1/winpid")" SECONDS_LONG="$2" STOP="$(cygpath -w "$3")" X="$4" \
    Y="$5" W="$6" H="$7" FLAT_DIR="$(cygpath -w "$8")" FLAT="$flat" \
    EVERY_DIR="$([ -z "$every" ] || cygpath -w "$every")" \
    powershell.exe -NoProfile -NonInteractive -Command '
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;

public static class Frames {
  delegate bool EnumProc(IntPtr hwnd, IntPtr data);
  [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out RECT rect, int size);

  public static void Run(uint pid, double seconds, string stop, int x, int y, int w, int h, string flatDir, int flat, string everyDir) {
    SetProcessDPIAware();
    // The biggest visible window of the process that no other owns: the browser window.
    Rectangle main = Rectangle.Empty;
    EnumWindows((hwnd, data) => {
      uint owner;
      GetWindowThreadProcessId(hwnd, out owner);
      if (owner != pid || !IsWindowVisible(hwnd) || GetWindow(hwnd, 4) != IntPtr.Zero) return true;
      RECT r;
      // As it shows, without the invisible borders that resize it (DWMWA_EXTENDED_FRAME_BOUNDS).
      if (DwmGetWindowAttribute(hwnd, 9, out r, Marshal.SizeOf(typeof(RECT))) != 0) return true;
      Rectangle shown = Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom);
      if (shown.Width * shown.Height > main.Width * main.Height) main = shown;
      return true;
    }, IntPtr.Zero);
    if (main.IsEmpty) throw new Exception("no window of process " + pid);
    DateTime epoch = new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc);
    DateTime end = DateTime.UtcNow.AddSeconds(seconds);
    int saved = 0;
    var every = new List<KeyValuePair<double, Bitmap>>();
    using (Bitmap bitmap = new Bitmap(w, h, PixelFormat.Format32bppRgb))
    using (Graphics graphics = Graphics.FromImage(bitmap)) {
      Console.WriteLine("capturing " + w + "x" + h);
      Console.Out.Flush();
      byte[] bytes = null;
      while (DateTime.UtcNow < end && !File.Exists(stop)) {
        double at = (DateTime.UtcNow - epoch).TotalSeconds;
        graphics.CopyFromScreen(main.X + x, main.Y + y, 0, 0, new Size(w, h));
        double got = (DateTime.UtcNow - epoch).TotalSeconds;
        BitmapData data = bitmap.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppRgb);
        if (bytes == null) bytes = new byte[data.Stride * h];
        Marshal.Copy(data.Scan0, bytes, 0, bytes.Length);
        int stride = data.Stride;
        bitmap.UnlockBits(data);
        int low = 255, high = 0;
        long red = 0, green = 0, blue = 0, n = 0;
        for (int row = 0; row < h; row += 6) {
          for (int column = 0; column < w; column += 6) {
            int i = row * stride + column * 4;
            int b = bytes[i], g = bytes[i + 1], r = bytes[i + 2];
            int luma = (r * 299 + g * 587 + b * 114) / 1000;
            low = Math.Min(low, luma);
            high = Math.Max(high, luma);
            red += r; green += g; blue += b; n++;
          }
        }
        bool isFlat = high - low < flat;
        Console.WriteLine(string.Format(CultureInfo.InvariantCulture, "{0:F3} {1:F3} {2} {3} {4} {5} {6}",
          at, got, isFlat ? "flat" : "page", high - low, red / n, green / n, blue / n));
        Console.Out.Flush();
        if (isFlat && saved < 10) {
          saved++;
          bitmap.Save(Path.Combine(flatDir, at.ToString("F3", CultureInfo.InvariantCulture) + ".png"), ImageFormat.Png);
        }
        if (everyDir != "") every.Add(new KeyValuePair<double, Bitmap>(at, (Bitmap)bitmap.Clone()));
      }
    }
    foreach (var picture in every) {
      picture.Value.Save(Path.Combine(everyDir, picture.Key.ToString("F3", CultureInfo.InvariantCulture) + ".png"), ImageFormat.Png);
      picture.Value.Dispose();
    }
  }
}
"@
[Frames]::Run([uint32]$env:PID, [double]$env:SECONDS_LONG, $env:STOP, [int]$env:X, [int]$env:Y,
  [int]$env:W, [int]$env:H, $env:FLAT_DIR, [int]$env:FLAT, [string]$env:EVERY_DIR)
'
  ;;
Linux)
  # The process's biggest window, at its place on the screen.
  where=
  if [ "${BROWSER_HEADLESS:-}" = xvfb ]; then
    for window in $(xdotool search --onlyvisible --pid "$1" 2> /dev/null); do
      eval "$(xdotool getwindowgeometry --shell "$window")"
      if [ -z "$where" ] || [ $((WIDTH * HEIGHT)) -gt "$area" ]; then
        area=$((WIDTH * HEIGHT))
        where="$X $Y $WIDTH $HEIGHT"
      fi
    done
  elif [ -n "${BROWSER_HEADLESS:-}" ] && [ -n "${SWAYSOCK:-}" ]; then
    # tools/headless.sh's sway session, whose tree names each window's process.
    where=$(swaymsg -t get_tree | python3 -I -c '
import json, sys
def nodes(node):
    yield node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from nodes(child)
ours = [node["rect"] for node in nodes(json.load(sys.stdin))
        if node.get("pid") == int(sys.argv[1]) and node.get("visible")]
if ours:
    rect = max(ours, key=lambda rect: rect["width"] * rect["height"])
    print("%(x)d %(y)d %(width)d %(height)d" % rect)
' "$1")
  else
    echo "Reads only check.sh's Xvfb (BROWSER_XVFB) or tools/headless.sh's sway session." >&2
    exit 1
  fi
  [ -n "$where" ] || { echo "no window of process $1" >&2; exit 1; }
  # shellcheck disable=SC2086 # four numbers
  exec python3 -I - "$2" "$3" $where "$4" "$5" "$6" "$7" "$8" "$flat" "$every" << 'PY'
import ctypes, ctypes.util, os, struct, subprocess, sys, time, zlib

seconds, stop = float(sys.argv[1]), sys.argv[2]
left, top, width, height, x, y, w, h = (int(each) for each in sys.argv[3:11])
flat_dir, flat, every_dir = sys.argv[11], int(sys.argv[12]), sys.argv[13]


class XImage(ctypes.Structure):
    _fields_ = [(name, ctypes.c_int) for name in ("width", "height", "xoffset", "format")] + [
        ("data", ctypes.c_void_p)] + [(name, ctypes.c_int) for name in (
            "byte_order", "bitmap_unit", "bitmap_bit_order", "bitmap_pad", "depth",
            "bytes_per_line", "bits_per_pixel")]


# grab() gives the whole window's pixels, the bytes a row, a pixel, and where red, green and blue
# are in one, or None. The whole window, so a flat picture shows what the UI around the page area
# drew with it.
if os.environ.get("BROWSER_HEADLESS") == "xvfb":
    x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11") or "libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XDefaultRootWindow.restype = ctypes.c_ulong
    x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XGetImage.restype = ctypes.POINTER(XImage)
    x11.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int,
                              ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
    x11.XFree.argtypes = [ctypes.c_void_p]
    display = x11.XOpenDisplay(None)
    if not display:
        sys.exit("no X display")
    root = x11.XDefaultRootWindow(display)

    def grab():
        image = x11.XGetImage(display, root, left, top, width, height, 0xffffffff, 2)  # ZPixmap
        if not image:
            return None
        line, depth = image.contents.bytes_per_line, image.contents.bits_per_pixel
        data = ctypes.string_at(image.contents.data, line * height)
        x11.XFree(image.contents.data)
        x11.XFree(image)
        if depth != 32:
            sys.exit("the screen has %d bits a pixel, not 32" % depth)
        return data, line, 4, (2, 1, 0)
else:
    # Wayland lets no client read another's window, but sway's screencopy gives grim its output.
    # A grim a picture, each a process of its own.
    where = ["grim", "-t", "ppm", "-g", "%d,%d %dx%d" % (left, top, width, height), "-"]
    env = dict(os.environ, WAYLAND_DISPLAY=os.environ["BROWSER_HEADLESS"])

    def grab():
        ppm = subprocess.run(where, env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout
        # grim's header: P6, the width and height, and 255, each on a line of its own.
        head = ppm.split(b"\n", 3)
        if len(head) < 4 or head[0] != b"P6" or head[1].split() != [b"%d" % width, b"%d" % height]:
            return None
        return head[3], width * 3, 3, (0, 1, 2)


def png(path, rows, width=width, height=height):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(
            ">I", zlib.crc32(kind + body) & 0xffffffff)
    raw = b"".join(b"\0" + row for row in rows)
    with open(path, "wb") as file:
        file.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                   + chunk(b"IDAT", zlib.compress(raw, 1)) + chunk(b"IEND", b""))


def rgb_rows(picture, left, top, columns, lines):
    data, line, size, (ri, gi, bi) = picture
    rows = []
    for row in range(top, top + lines):
        pixels = data[row * line + left * size:row * line + (left + columns) * size]
        rgb = bytearray(columns * 3)
        rgb[0::3], rgb[1::3], rgb[2::3] = pixels[ri::size], pixels[gi::size], pixels[bi::size]
        rows.append(bytes(rgb))
    return rows


print("capturing %dx%d" % (w, h), flush=True)
end, saved, every = time.time() + seconds, 0, []
while time.time() < end and not os.path.exists(stop):
    at = time.time()
    picture = grab()
    got = time.time()
    if not picture:
        continue
    data, line, size, (ri, gi, bi) = picture
    low, high, red, green, blue, n = 255, 0, 0, 0, 0, 0
    for row in range(y, y + h, 6):
        for i in range(row * line + x * size, row * line + (x + w) * size, 6 * size):
            r, g, b = data[i + ri], data[i + gi], data[i + bi]
            luma = (r * 299 + g * 587 + b * 114) // 1000
            low, high = min(low, luma), max(high, luma)
            red, green, blue, n = red + r, green + g, blue + b, n + 1
    is_flat = high - low < flat
    print("%.3f %.3f %s %d %d %d %d" % (at, got, "flat" if is_flat else "page", high - low,
                                         red // n, green // n, blue // n), flush=True)
    if is_flat and saved < 10:
        saved += 1
        png(os.path.join(flat_dir, "%.3f.png" % at), rgb_rows(picture, 0, 0, width, height))
    if every_dir:
        every.append((at, picture))
for at, picture in every:
    png(os.path.join(every_dir, "%.3f.png" % at), rgb_rows(picture, x, y, w, h), w, h)
PY
  ;;
esac
[ -z "$every" ] || { echo "Keeps every picture only on Windows and Linux." >&2; exit 1; }
PID="$1" SECONDS_LONG="$2" STOP="$3" X="$4" Y="$5" W="$6" H="$7" FLAT_DIR="$8" FLAT="$flat" \
  exec swift - << 'SWIFT'
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

let environment = ProcessInfo.processInfo.environment
let pid = Int32(environment["PID"]!)!
let seconds = Double(environment["SECONDS_LONG"]!)!
let stop = environment["STOP"]!
let (x, y, w, h) = (Int(environment["X"]!)!, Int(environment["Y"]!)!, Int(environment["W"]!)!,
  Int(environment["H"]!)!)
let flatDir = environment["FLAT_DIR"]!
let flatRange = Int(environment["FLAT"]!)!
let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
let windows = content.windows.filter { $0.owningApplication?.processID == pid }
guard let main = windows.filter({ $0.windowLayer == 0 })
  .max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }) else {
  FileHandle.standardError.write("no window of process \(pid)\n".data(using: .utf8)!)
  exit(1)
}
let display = content.displays.first(where: { $0.frame.intersects(main.frame) })!
// Its windows over it too, as the command bar, as on screen.
let filter = SCContentFilter(display: display, including: windows)
let config = SCStreamConfiguration()
// Only the rect, in points, so a picture's pixels are its points whatever the screen's scale.
config.sourceRect = CGRect(x: main.frame.minX - display.frame.minX + CGFloat(x),
  y: main.frame.minY - display.frame.minY + CGFloat(y), width: CGFloat(w), height: CGFloat(h))
config.width = w
config.height = h
config.showsCursor = false
var pixels = [UInt8](repeating: 0, count: w * h * 4)
let space = CGColorSpaceCreateDeviceRGB()
let end = Date().addingTimeInterval(seconds)
var saved = 0
print("capturing \(w)x\(h)")
fflush(stdout)
while Date() < end && !FileManager.default.fileExists(atPath: stop) {
  let at = Date().timeIntervalSince1970
  let shot: CGImage? = try? await SCScreenshotManager.captureImage(contentFilter: filter,
    configuration: config)
  let got = Date().timeIntervalSince1970
  guard let image = shot else { continue }
  let context = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8,
    bytesPerRow: w * 4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
  context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
  var (low, high, r, g, b, n) = (255, 0, 0, 0, 0, 0)
  for row in stride(from: 0, to: h, by: 6) {
    for column in stride(from: 0, to: w, by: 6) {
      let i = (row * w + column) * 4
      let (pr, pg, pb) = (Int(pixels[i]), Int(pixels[i + 1]), Int(pixels[i + 2]))
      let luma = (pr * 299 + pg * 587 + pb * 114) / 1000
      low = min(low, luma)
      high = max(high, luma)
      r += pr; g += pg; b += pb; n += 1
    }
  }
  let flat = high - low < flatRange
  print(String(format: "%.3f %.3f %@ %ld %ld %ld %ld", at, got, flat ? "flat" : "page",
    high - low, r / n, g / n, b / n))
  fflush(stdout)
  if flat && saved < 10 {
    saved += 1
    let url = URL(fileURLWithPath: String(format: "%@/%.3f.png", flatDir, at))
    if let file = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1,
      nil) {
      CGImageDestinationAddImage(file, image, nil)
      CGImageDestinationFinalize(file)
    }
  }
}
SWIFT
