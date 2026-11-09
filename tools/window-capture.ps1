# Windows' counterpart of tools/headless.sh, for a CI runner, whose desktop is its own: runs a
# program, and once it has logged a line matching <ready> and the picture has stopped changing,
# captures the screen where its largest window is, overlays over it included. Its log (stderr) goes
# beside the capture, and the program goes when it ends. -Moving lets that many pixels keep
# changing, such as a blinking caret. On a desktop someone uses, the capture holds whatever covers
# the window, so it's for CI only. With -Point, the capture is taken again with the pointer at the
# window's point that the first log line matching it ends in, as parity.sh's --then does on Linux.
# Usage: window-capture.ps1 <output.png> <ready> <program> [-Moving <n>] [-Point <pattern>]
param(
  [Parameter(Mandatory)] [string] $Out,
  [Parameter(Mandatory)] [string] $Ready,
  [Parameter(Mandatory)] [string] $Program,
  [int] $Moving = 0,
  [string] $Point = ''
)
$ErrorActionPreference = 'Stop'
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Capture {
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr window, int attribute, out Rect rect, int size);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc each, IntPtr data);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  delegate bool EnumProc(IntPtr window, IntPtr data);
  [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }

  // Screen pixels, not the scaled points a DPI-unaware process gets.
  public static void Aware() { SetProcessDPIAware(); }

  // The process's largest visible window as drawn, without the invisible borders it resizes by:
  // the preview's own, not the overlay in front of it that Process.MainWindowHandle finds first.
  // Empty while it has none.
  public static Rectangle Largest(int process) {
    var largest = Rectangle.Empty;
    EnumWindows((window, _) => {
      uint owner;
      GetWindowThreadProcessId(window, out owner);
      Rect rect;
      if (owner == process && IsWindowVisible(window) &&
          DwmGetWindowAttribute(window, 9, out rect, Marshal.SizeOf(typeof(Rect))) == 0) {
        var bounds = Rectangle.FromLTRB(rect.Left, rect.Top, rect.Right, rect.Bottom);
        if (bounds.Width * bounds.Height > largest.Width * largest.Height) largest = bounds;
      }
      return true;
    }, IntPtr.Zero);
    return largest;
  }

  public static Bitmap Screen(Rectangle area) {
    var picture = new Bitmap(area.Width, area.Height, PixelFormat.Format32bppArgb);
    using (var graphics = Graphics.FromImage(picture)) graphics.CopyFromScreen(area.Location, Point.Empty, area.Size);
    return picture;
  }

  static int[] Pixels(Bitmap picture) {
    var data = picture.LockBits(new Rectangle(Point.Empty, picture.Size), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    var pixels = new int[picture.Width * picture.Height];
    Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
    picture.UnlockBits(data);
    return pixels;
  }

  // How many pixels differ between two pictures of one size.
  public static int Differ(Bitmap a, Bitmap b) {
    int[] x = Pixels(a), y = Pixels(b);
    int count = 0;
    for (int i = 0; i < x.Length; i++) if (x[i] != y[i]) count++;
    return count;
  }

  // How many pixels differ from the first, which a window that hasn't drawn yet has everywhere.
  public static int Varied(Bitmap picture) {
    int[] pixels = Pixels(picture);
    int count = 0;
    foreach (int pixel in pixels) if (pixel != pixels[0]) count++;
    return count;
  }
}
'@
[Capture]::Aware()

$Out = [IO.Path]::GetFullPath($Out)
$log = [IO.Path]::ChangeExtension($Out, 'log')
New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
# Not $program: PowerShell's names ignore case, and that one is the path.
$process = Start-Process -FilePath $Program -RedirectStandardError $log -PassThru -WindowStyle Normal
try {
  $deadline = (Get-Date).AddSeconds(30)
  while (-not (Select-String -Quiet -Path $log -Pattern $Ready -ErrorAction SilentlyContinue)) {
    if ($process.HasExited -or (Get-Date) -gt $deadline) {
      Get-Content $log -Tail 30 -ErrorAction SilentlyContinue | Write-Error -ErrorAction Continue
      throw "No `"$Ready`" in the program's log."
    }
    Start-Sleep -Milliseconds 100
  }
  $last = $null
  $taken = $false
  for ($try = 0; $try -lt 100 -and -not $taken; $try++) {
    $area = [Capture]::Largest($process.Id)
    if (-not $area.IsEmpty) {
      $now = [Capture]::Screen($area)
      if ($last -and $last.Size -eq $now.Size -and [Capture]::Differ($last, $now) -le $Moving -and
          [Capture]::Varied($now) -gt $Moving) {
        $now.Save($Out, [Drawing.Imaging.ImageFormat]::Png)
        $taken = $true
      }
      if ($last) { $last.Dispose() }
      $last = $now
    }
    Start-Sleep -Milliseconds 200
  }
  if (-not $taken) { throw 'The picture never stood still.' }
  if ($Point) {
    # The window's points, in pixels at the scale it's drawn at, as it's 1280 points wide.
    $words = -split (Select-String -Path $log -Pattern $Point | Select-Object -First 1).Line
    $scale = $area.Width / 1280
    $x = $area.X + [int]([double]$words[-2] * $scale)
    $y = $area.Y + [int]([double]$words[-1] * $scale)
    [Capture]::SetCursorPos($x - 1, $y) | Out-Null
    Start-Sleep -Milliseconds 300
    [Capture]::SetCursorPos($x, $y) | Out-Null
    Start-Sleep -Milliseconds 1500
    $now = [Capture]::Screen($area)
    $now.Save($Out, [Drawing.Imaging.ImageFormat]::Png)
  }
  $Out
} finally {
  if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
}
