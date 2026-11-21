# check.sh runs this at 990, only when named.
# A probe for #3621, never to land: on Windows, where the page view sits against the window's
# client area and the display's work area, at launch, maximized, restored and resized, with the
# sidebar shown and hidden, and what size the page itself takes. It runs the check's build, then
# the 0.2.2 release from its installer, as the report's is. It measures; it fails only when it
# can't.

if ! windows; then
  echo "skip: fit3621 is a Windows probe"
else
  # A white page with a red strip on its right edge and a blue one on its bottom edge, so a
  # capture shows whether its edges are on screen, and its sizes in its title on each resize.
  page=$(base64 << 'HTML' | tr -d '\n'
<!doctype html>
<title>loading</title>
<style>
html, body { margin: 0; height: 100%; background: #fff }
#r { position: fixed; top: 0; right: 0; width: 24px; height: 100%; background: rgb(224 0 0) }
#b { position: fixed; bottom: 0; left: 0; width: 100%; height: 24px; background: rgb(0 90 224) }
</style>
<div id="r"></div><div id="b"></div>
<script>
let n = 0;
function say() {
  const v = visualViewport;
  document.title = `fit ${++n} iw ${innerWidth} ih ${innerHeight} cw ` +
    `${document.documentElement.clientWidth} dpr ${devicePixelRatio} vv ${v.width}x${v.height} ` +
    `scale ${v.scale} ow ${outerWidth} oh ${outerHeight} sx ${screenX} sy ${screenY} ` +
    `screen ${screen.width}x${screen.height} avail ${screen.availWidth}x${screen.availHeight}`;
}
addEventListener("resize", say);
requestAnimationFrame(() => setTimeout(say));
</script>
HTML
)
  url="data:text/html;base64,$page"
  # The process's windows, as Windows has them, in pixels: each visible top-level window's rect,
  # DWM's frame, its client rect on screen, whether it's maximized, its DPI and its display's rect
  # and work area, and each visible window under it.
  cat > "$out/fit-rects.ps1" << 'PS'
param([int]$procId)
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
using System.Collections.Generic;
public static class FitRects {
  public delegate bool Proc(IntPtr h, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int l, t, r, b; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
  [StructLayout(LayoutKind.Sequential)] public struct MI { public int cb; public RECT m; public RECT w; public uint f; }
  [DllImport("user32.dll")] static extern bool EnumWindows(Proc p, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr h, Proc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr GetParent(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
  [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h, uint f);
  [DllImport("user32.dll")] static extern bool GetMonitorInfoW(IntPtr m, ref MI i);
  [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr h);
  static string Class(IntPtr h) { var s = new StringBuilder(256); GetClassNameW(h, s, 256); return s.ToString(); }
  static string R(RECT r) { return r.l + "," + r.t + " " + (r.r - r.l) + "x" + (r.b - r.t); }
  public static void Run(uint pid) {
    var tops = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) tops.Add(h); return true; }, IntPtr.Zero);
    foreach (var h in tops) {
      RECT w, c, d; GetWindowRect(h, out w); GetClientRect(h, out c);
      var o = new POINT(); ClientToScreen(h, ref o);
      DwmGetWindowAttribute(h, 9, out d, Marshal.SizeOf(typeof(RECT)));
      var mi = new MI(); mi.cb = Marshal.SizeOf(typeof(MI));
      GetMonitorInfoW(MonitorFromWindow(h, 2), ref mi);
      Console.WriteLine("fit top " + h.ToInt64() + " " + Class(h) + " window " + R(w) + " dwm " + R(d) +
        " client " + o.x + "," + o.y + " " + c.r + "x" + c.b + " zoomed " + (IsZoomed(h) ? 1 : 0) +
        " dpi " + GetDpiForWindow(h) + " monitor " + R(mi.m) + " work " + R(mi.w));
      EnumChildWindows(h, (k, l) => { if (IsWindowVisible(k)) { RECT r; GetWindowRect(k, out r);
        Console.WriteLine("fit child " + h.ToInt64() + " " + k.ToInt64() + " parent " +
          GetParent(k).ToInt64() + " " + Class(k) + " " + R(r)); } return true; }, IntPtr.Zero);
    }
  }
}
"@
[FitRects]::Run([uint32]$procId)
PS
  # fit_measure <name>: once the page's size and the layout are the same twice 0.5 s apart, as a
  # motion such as the sidebar's has ended, logs the layout, the page's last sizes, Windows' rects
  # and a capture under <name>.
  fit_measure() {
    last=
    for _ in $(seq 20); do
      send layout
      now="$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content')|$(
        grep '^browser: title [0-9]* fit ' "$log" | tail -1 | cut -d' ' -f5-)"
      [ "$now" = "$last" ] && break
      last=$now
      sleep 0.5
    done
    {
      echo "== $fit_app: $1"
      echo "$now" | tr '|' '\n'
      timeout 60 powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass \
        -File "$(cygpath -w "$out/fit-rects.ps1")" -procId "$(cat "/proc/$pid/winpid")" | tr -d '\r'
    } >> "$out/fit3621.txt"
    ../tools/window-screenshot.sh "$pid" "$out/fit-$fit_app-$2.png" || fail "no window to capture"
  }
  # fit_run <app>: the steps, on the app check.sh's windows_browser runs from $app.
  fit_run() {
    fit_app=$1
    log="$out/fit3621-$fit_app.log"
    step=0
    rm -f "$saved" "$launch_frame"
    BROWSER_DRIVE=1 BROWSER_URL="$url" "$browser" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await_load
    await '^browser: title [0-9]* fit ' || fail "$fit_app: the page didn't say its size"
    fit_measure "at launch" launch
    send maximize
    fit_measure "maximized" maximized
    send key s cmd
    fit_measure "maximized, sidebar hidden" maximized-hidden
    send key s cmd
    fit_measure "maximized, sidebar shown again" maximized-shown
    send maximize
    fit_measure "restored" restored
    send resize 1400 900
    fit_measure "resized to 1400x900" resized
    send key s cmd
    fit_measure "resized, sidebar hidden" resized-hidden
    send key s cmd
    send maximize
    fit_measure "maximized again" maximized-again
    exec 3>&-
    quit
  }
  : > "$out/fit3621.txt"
  fit_run build
  # The 0.2.2 release, from its installer, for this user alone, into the check's folder.
  curl -fsSL -o "$out/ilo-022-setup.exe" \
    https://github.com/MartinPTielemans/ilo/releases/download/v0.2.2/ilo-windows-x64-setup.exe ||
    fail "the 0.2.2 installer didn't download"
  "$out/ilo-022-setup.exe" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER \
    "/DIR=$(cygpath -w "$PWD/$out/ilo-022")" || fail "the 0.2.2 installer failed"
  for _ in $(seq 100); do
    [ -e "$out/ilo-022/chrome.exe" ] && break
    sleep 0.5
  done
  [ -e "$out/ilo-022/chrome.exe" ] || fail "the 0.2.2 installer left no chrome.exe"
  # The installer opens nothing when silent; anything it started from the folder ends first.
  for _ in $(seq 100); do
    pgrep -f "$(cygpath -m "$PWD/$out/ilo-022")" > /dev/null || break
    sleep 0.2
  done
  build_app=$app build_procs=$procs
  app="$PWD/$out/ilo-022"
  procs="$(cygpath -m "$app/chrome.exe")"
  rm -rf "$profile_dir"
  mkdir -p "$profile_dir/Default"
  fit_run release-022
  app=$build_app procs=$build_procs
  echo "fit3621: measured; see fit3621.txt"
fi
