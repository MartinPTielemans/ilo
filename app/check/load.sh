# A page loads in the browser window, over the GPUI sidebar drawn in it, and quitting through
# AppKit closes the page and ends every Chromium process. The first run shows the welcome and no
# page, so the page is given. On Windows the window's icons, which the taskbar's thumbnails and
# Alt+Tab show, are ilo's, not Chromium's (#1350).

log="$out/load.log"
rm -f "$saved" "$launch_frame"
BROWSER_URL='https://en.wikipedia.org/wiki/Arc_(web_browser)' "$browser" \
  < /dev/null > "$log" 2>&1 &
pid=$!
await_load
../tools/window-screenshot.sh "$pid" "$out/load.png" || fail "no window to capture"
icons=
if windows; then
  # The window takes chrome.dll's icon. An engine from before #1350 has Chromium's, which its
  # package step now refuses, so only such an engine skips this.
  ico=platform/windows/package/Assets/ilo.ico
  if python3 -I ../engine/windows/icons.py "$app/chrome.dll" '#101' "$ico" > /dev/null 2>&1; then
    python3 -I check/app-windows.py icon "$(cat "/proc/$pid/winpid")" "$ico" > "$out/icons.txt" ||
      fail "the window's icons aren't ilo's: $(tr '\n' ';' < "$out/icons.txt")"
    icons=" The window's icons are ilo's."
  else
    echo "skip: load's window icons. This engine's chrome.dll has Chromium's icon, from before" \
      "#1350."
  fi
fi
quit
echo "ok: $(grep '^browser: loaded' "$log" | cut -d' ' -f4-) loaded, quit cleanly.$icons" \
  "Window: $out/load.png"
