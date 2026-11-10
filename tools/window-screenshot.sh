#!/bin/sh
# Screenshots one process's main window with its child windows (menus, the command bar) drawn
# over it, as on screen, but nothing from other apps. A screen capture would include whatever
# else is open on the owner's Mac. With "all", it takes every window of the process on that display,
# such as two browser windows, in one picture. With "screen", the Mac takes the main window's whole
# display with the process's windows alone drawn on it, such as a Mini Player at the display's
# corner, and the others take it as "all".
# On Windows, under Git Bash, it copies the screen where the windows show, as Windows composes it,
# cut to their shapes, while no other app's window is in front of them there. Otherwise it takes
# each of the process's windows with Windows.Graphics.Capture, as Windows composes them, in their
# order on screen, so nothing of another app's can get into the picture. Where that can't capture,
# it draws them with PrintWindow, and says so: PrintWindow draws what DirectComposition leaves
# see-through, as an overlay's margin, black (#1257), and has the browser window repaint its
# frame, where Windows' theme paints the caption under the UI, on screen too (#1262).
# On Linux it captures the private session tools/headless.sh --run starts, or check.sh's Xvfb,
# which hold nothing but the app, and refuses outside one rather than capture the owner's screen.
# There POINTER=1 draws the display's pointer in, where the driver's warp put it, with the cursor
# the window under it shows.
# Usage: tools/window-screenshot.sh <pid> <output.png> [all|screen]
set -e
[ $# -eq 2 ] || [ $# -eq 3 ] && { [ "${3:-all}" = all ] || [ "$3" = screen ]; } ||
  { echo "usage: $0 <pid> <output.png> [all|screen]" >&2; exit 2; }
case "$(uname)" in
  MINGW* | MSYS*)
    [ -e "/proc/$1/winpid" ] || { echo "process $1 isn't running" >&2; exit 1; }
    exec env PID="$(cat "/proc/$1/winpid")" OUT="$(cygpath -w "$2")" ALL="${3:+all}" \
      powershell.exe -NoProfile -NonInteractive -Command '
Add-Type -ReferencedAssemblies System.Drawing, System.Windows.Forms -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
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
  [DllImport("d3d11.dll")] static extern int D3D11CreateDevice(IntPtr adapter, int driver, IntPtr software, uint flags, IntPtr levels, uint count, uint sdk, out IntPtr device, out int level, out IntPtr context);
  [DllImport("d3d11.dll")] static extern int CreateDirect3D11DeviceFromDXGIDevice(IntPtr dxgi, out IntPtr device);
  [DllImport("combase.dll", CharSet = CharSet.Unicode)] static extern int WindowsCreateString(string text, int length, out IntPtr name);
  [DllImport("combase.dll")] static extern int RoGetActivationFactory(IntPtr name, ref Guid iid, out IntPtr factory);

  [ComImport, Guid("3628E81B-3CAC-4C60-B7F4-23CE0E0C3356"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IGraphicsCaptureItemInterop {
    IntPtr CreateForWindow(IntPtr window, ref Guid iid);
    IntPtr CreateForMonitor(IntPtr monitor, ref Guid iid);
  }

  // Windows.Graphics.Capture, which PowerShell drives: a Direct3D device for it, on the GPU or
  // else WARP, as a CI runner has none.
  public static object Device() {
    IntPtr device, context, dxgi, inspectable;
    int level;
    // D3D_DRIVER_TYPE_HARDWARE, then _WARP; D3D11_CREATE_DEVICE_BGRA_SUPPORT.
    int result = D3D11CreateDevice(IntPtr.Zero, 1, IntPtr.Zero, 0x20, IntPtr.Zero, 0, 7, out device, out level, out context);
    if (result < 0)
      result = D3D11CreateDevice(IntPtr.Zero, 5, IntPtr.Zero, 0x20, IntPtr.Zero, 0, 7, out device, out level, out context);
    Marshal.ThrowExceptionForHR(result);
    var iid = new Guid("54ec77fa-1377-44e6-8c32-88fd5f44c84c");
    Marshal.ThrowExceptionForHR(Marshal.QueryInterface(device, ref iid, out dxgi));
    Marshal.ThrowExceptionForHR(CreateDirect3D11DeviceFromDXGIDevice(dxgi, out inspectable));
    Marshal.Release(dxgi);
    Marshal.Release(context);
    Marshal.Release(device);
    return Marshal.GetObjectForIUnknown(inspectable);
  }

  // The GraphicsCaptureItem of |hwnd|.
  public static object Item(IntPtr hwnd) {
    const string name = "Windows.Graphics.Capture.GraphicsCaptureItem";
    IntPtr hstring, factory;
    Marshal.ThrowExceptionForHR(WindowsCreateString(name, name.Length, out hstring));
    var interop = typeof(IGraphicsCaptureItemInterop).GUID;
    Marshal.ThrowExceptionForHR(RoGetActivationFactory(hstring, ref interop, out factory));
    var items = (IGraphicsCaptureItemInterop)Marshal.GetObjectForIUnknown(factory);
    var iid = new Guid("79C3F95B-31F7-4EC2-A464-632EF5D30760");
    return Marshal.GetObjectForIUnknown(items.CreateForWindow(hwnd, ref iid));
  }

  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, System.Text.StringBuilder name, int size);
  public static string ClassOf(IntPtr hwnd) {
    var name = new System.Text.StringBuilder(256);
    GetClassName(hwnd, name, name.Capacity);
    return name.ToString();
  }

  // The |width| by |height| at the top left of |pixels|, premultiplied BGRA |across| wide.
  public static Bitmap FromBgra(byte[] pixels, int across, int width, int height) {
    var image = new Bitmap(width, height, PixelFormat.Format32bppPArgb);
    var data = image.LockBits(new Rectangle(0, 0, width, height), ImageLockMode.WriteOnly, PixelFormat.Format32bppPArgb);
    for (int y = 0; y < height; y++)
      Marshal.Copy(pixels, y * across * 4, data.Scan0 + y * data.Stride, width * 4);
    image.UnlockBits(data);
    return image;
  }
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out RECT rect, int size);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out int value, int size);
  [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr hwnd);

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

  // Whether a window of another app shows in front of one of |shown| where it would be in a copy
  // of the screen. A cloaked window (DWMWA_CLOAKED), as a Store app kept out of sight has, never
  // shows.
  static bool Covered(uint pid, List<IntPtr> shown) {
    var behind = new List<IntPtr>(shown);
    var covered = false;
    EnumWindows((hwnd, data) => {
      if (behind.Remove(hwnd)) return behind.Count > 0;
      uint owner;
      int cloaked;
      GetWindowThreadProcessId(hwnd, out owner);
      if (owner != pid && IsWindowVisible(hwnd) && !IsIconic(hwnd) &&
          !(DwmGetWindowAttribute(hwnd, 14, out cloaked, sizeof(int)) == 0 && cloaked != 0) &&
          behind.Exists(w => Shown(w).IntersectsWith(Shown(hwnd))))
        covered = true;
      return !covered;
    }, IntPtr.Zero);
    return covered;
  }

  // Windows 11 rounds the corners of a window, 8 points across, unless it fills its screen.
  static readonly bool Rounds = int.Parse((string)Microsoft.Win32.Registry.GetValue(
      "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
      "CurrentBuildNumber", "0")) >= 22000;

  // The shape of the window on screen, from |origin|: what a copy of the screen holds of it.
  static GraphicsPath Shape(IntPtr hwnd, Point origin) {
    var r = Shown(hwnd);
    r.Offset(-origin.X, -origin.Y);
    var path = new GraphicsPath();
    var fills = IsZoomed(hwnd) ||
        System.Windows.Forms.Screen.FromHandle(hwnd).Bounds == Shown(hwnd);
    if (!Rounds || fills) {
      path.AddRectangle(r);
      return path;
    }
    var d = 2 * 8 * (int)GetDpiForWindow(hwnd) / 96;
    path.AddArc(r.Left, r.Top, d, d, 180, 90);
    path.AddArc(r.Right - d, r.Top, d, d, 270, 90);
    path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
    path.AddArc(r.Left, r.Bottom - d, d, d, 90, 90);
    path.CloseFigure();
    return path;
  }

  // Draws |hwnd| into |graphics|, whose top left is |origin| on screen, as |grab| takes it, or else
  // with PrintWindow, and says whether |grab| took it.
  static bool Draw(IntPtr hwnd, Graphics graphics, Point origin, Func<IntPtr, Bitmap> grab) {
    var bounds = Bounds(hwnd);
    if (bounds.Width <= 0 || bounds.Height <= 0) return true;
    using (var taken = grab(hwnd)) {
      if (taken != null) {
        // The window as Windows composes it, with the clear borders it resizes by or without.
        var at = taken.Size == Shown(hwnd).Size ? Shown(hwnd).Location : bounds.Location;
        graphics.DrawImageUnscaled(taken, at.X - origin.X, at.Y - origin.Y);
        return true;
      }
    }
    using (var window = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb)) {
      using (var g = Graphics.FromImage(window)) {
        var dc = g.GetHdc();
        // PW_RENDERFULLCONTENT, without which a window Chromium or GPUI composites draws black.
        PrintWindow(hwnd, dc, 2);
        g.ReleaseHdc(dc);
      }
      graphics.DrawImageUnscaled(window, bounds.X - origin.X, bounds.Y - origin.Y);
    }
    return false;
  }

  public static string Run(uint pid, string path, bool all, Func<IntPtr, Bitmap> grab) {
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
    var shown = all ? mains : mains.GetRange(0, 1);
    if (!Covered(pid, shown)) {
      using (var screen = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb))
      using (var image = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb)) {
        using (var graphics = Graphics.FromImage(screen))
          graphics.CopyFromScreen(area.Location, Point.Empty, area.Size);
        // Clear outside the windows, where what is behind them shows: past their round corners,
        // and between the windows "all" takes.
        using (var graphics = Graphics.FromImage(image))
        using (var shape = new Region()) {
          shape.MakeEmpty();
          foreach (var w in shown)
            using (var outline = Shape(w, area.Location)) shape.Union(outline);
          graphics.Clip = shape;
          graphics.DrawImageUnscaled(screen, 0, 0);
        }
        image.Save(path, ImageFormat.Png);
      }
      return null;
    }
    var printed = new List<string>();
    using (var image = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb))
    using (var graphics = Graphics.FromImage(image)) {
      for (int i = windows.Count - 1; i >= 0; i--) {
        var top = windows[i];
        while (GetWindow(top, GW_OWNER) != IntPtr.Zero) top = GetWindow(top, GW_OWNER);
        if (!(all ? mains.Contains(top) : top == mains[0])) continue;
        if (!Draw(windows[i], graphics, area.Location, grab)) printed.Add(ClassOf(windows[i]));
      }
      image.Save(path, ImageFormat.Png);
    }
    if (printed.Count > 0)
      Console.Error.WriteLine("A window of another app is in front of the app, and " +
          "Windows.Graphics.Capture could not take " + string.Join(", ", printed) + ", so the " +
          "capture drew them with PrintWindow, which draws the clear margins of overlays black.");
    return null;
  }
}
"@
# One frame of a window as Windows composes it, by Windows.Graphics.Capture, or $null where that
# fails: PowerShell knows the WinRT types, and Capture makes what needs COM.
$grab = [Func[IntPtr, System.Drawing.Bitmap]] {
  param($hwnd)
  try {
    $null = [Windows.Graphics.Capture.GraphicsCaptureSession, Windows.Graphics, ContentType = WindowsRuntime]
    $null = [Windows.Graphics.Capture.Direct3D11CaptureFramePool, Windows.Graphics, ContentType = WindowsRuntime]
    $null = [Windows.Graphics.DirectX.DirectXPixelFormat, Windows.Graphics, ContentType = WindowsRuntime]
    $null = [Windows.Graphics.Imaging.SoftwareBitmap, Windows.Graphics, ContentType = WindowsRuntime]
    $null = [Windows.Storage.Streams.Buffer, Windows.Storage.Streams, ContentType = WindowsRuntime]
    Add-Type -AssemblyName System.Runtime.WindowsRuntime
    if (-not [Windows.Graphics.Capture.GraphicsCaptureSession]::IsSupported()) {
      [Console]::Error.WriteLine("Windows.Graphics.Capture is not supported here.")
      return $null
    }
    $item = [Capture]::Item($hwnd)
    # By reflection, which has the runtime ask the device for IDirect3DDevice, as PowerShell does not.
    $pool = [Windows.Graphics.Capture.Direct3D11CaptureFramePool].GetMethod("CreateFreeThreaded").Invoke(
      $null, @([Capture]::Device(), [Windows.Graphics.DirectX.DirectXPixelFormat]::B8G8R8A8UIntNormalized,
      1, $item.Size))
    $session = $pool.CreateCaptureSession($item)
    try { $session.IsCursorCaptureEnabled = $false } catch {}
    $session.StartCapture()
    $frame = $null
    for ($i = 0; $i -lt 150 -and $null -eq $frame; $i++) {
      $frame = $pool.TryGetNextFrame()
      if ($null -eq $frame) { Start-Sleep -Milliseconds 20 }
    }
    if ($null -eq $frame) {
      [Console]::Error.WriteLine("Windows.Graphics.Capture gave no frame.")
      $session.Dispose(); $pool.Dispose()
      return $null
    }
    $asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
      $_.Name -eq "AsTask" -and $_.GetParameters().Count -eq 1 -and
        $_.GetParameters()[0].ParameterType.Name -eq "IAsyncOperation``1" } | Select-Object -First 1
    $surface = [Windows.Graphics.DirectX.Direct3D11.IDirect3DSurface, Windows.Graphics, ContentType = WindowsRuntime]
    $alpha = [Windows.Graphics.Imaging.BitmapAlphaMode]
    $copy = [Windows.Graphics.Imaging.SoftwareBitmap].GetMethod("CreateCopyFromSurfaceAsync",
      [Type[]] @($surface, $alpha)).Invoke($null, @($frame.Surface, $alpha::Premultiplied))
    $task = $asTask.MakeGenericMethod([Windows.Graphics.Imaging.SoftwareBitmap]).Invoke($null, @($copy))
    $bitmap = $task.Result
    $buffer = New-Object Windows.Storage.Streams.Buffer ([uint32]($bitmap.PixelWidth * $bitmap.PixelHeight * 4))
    $bitmap.CopyToBuffer($buffer)
    $bytes = [System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions]::ToArray($buffer)
    $size = $frame.ContentSize
    $image = [Capture]::FromBgra($bytes, $bitmap.PixelWidth, [Math]::Min($size.Width, $bitmap.PixelWidth),
      [Math]::Min($size.Height, $bitmap.PixelHeight))
    $bitmap.Dispose(); $frame.Dispose(); $session.Dispose(); $pool.Dispose()
    return $image
  } catch {
    [Console]::Error.WriteLine("Windows.Graphics.Capture failed for " + [Capture]::ClassOf($hwnd) +
      ": " + $_.Exception.Message)
    return $null
  }
}
$failed = [Capture]::Run([uint32]$env:PID, $env:OUT, $env:ALL -eq "all", $grab)
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
      # Its windows carry the number it has in its own namespace, a Flatpak's sandbox's.
      for window in $(xdotool search --onlyvisible --pid "$(awk '/^NSpid:/ { print $NF }' "/proc/$1/status")" 2> /dev/null); do
        # WINDOW, X, Y, WIDTH, HEIGHT and SCREEN, X and Y from the root's corner.
        eval "$(xdotool getwindowgeometry --shell "$window")"
        [ $((WIDTH * HEIGHT)) -gt "$best" ] || continue
        best=$((WIDTH * HEIGHT))
        echo "${WIDTH}x$HEIGHT+$((X < 0 ? 0 : X))+$((Y < 0 ? 0 : Y))"
      done | tail -1
    )
    if [ -n "${POINTER:-}" ]; then
      # import leaves the pointer out, and x11grab draws it.
      size=${area%%+*}
      offset=${area#"$size"}
      exec ffmpeg -loglevel error -y -f x11grab -draw_mouse 1 ${area:+-video_size "$size"} \
        -i "$DISPLAY$(echo "$offset" | sed 's/^+\([0-9]*\)+\([0-9]*\)$/+\1,\2/')" \
        -frames:v 1 -update 1 "$2"
    fi
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
  [ -z "$area" ] || exec env WAYLAND_DISPLAY="$BROWSER_HEADLESS" grim ${POINTER:+-c} -g "$area" "$2"
  exec env WAYLAND_DISPLAY="$BROWSER_HEADLESS" grim ${POINTER:+-c} "$2"
fi
PID="$1" OUT="$2" ALL="${3:-}" swift - <<'SWIFT'
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

let pid = Int32(ProcessInfo.processInfo.environment["PID"]!)!
let out = URL(fileURLWithPath: ProcessInfo.processInfo.environment["OUT"]!)
let all = ProcessInfo.processInfo.environment["ALL"] == "all"
let screen = ProcessInfo.processInfo.environment["ALL"] == "screen"

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

let frame = screen ? display.frame : all
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
