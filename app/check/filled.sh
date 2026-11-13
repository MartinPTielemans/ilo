# check.sh runs this at 490.
# A page's full screen in a window that fills its screen, so that full screen keeps its size, as
# one zoomed on a notched Mac with the Dock hidden does, the menu bar beside the camera. The page
# fills the window full screen and comes back to the card after, and the Library opened then has
# the page beside its list, not over it, as it had stayed for good (#1633). The run hides the Dock
# and the menu bar for that and puts them back, so it runs only in a Mac check VM.

[ "$(uname)" = Darwin ] && [ "$(sysctl -n kern.hv_vmm_present)" = 1 ] || {
  echo "skip: filled. It hides the Dock and the menu bar, so it runs only in a Mac check VM."
  exit 0
}
serve
log="$out/filled.log"
step=0
printf '%s' "<title>loading</title>
<div style=\"font:24px system-ui\">A page that goes full screen on F</div>
<script>onkeydown = (e) => { if (e.key == 'f') document.documentElement.requestFullscreen(); };
requestAnimationFrame(() => requestAnimationFrame(() => document.title = 'ready'));</script>" \
  > "$out/site/filled.html"
# The menu bar hides only for the apps launched after.
hide_bars
BROWSER_DRIVE=1 BROWSER_URL="${site}filled.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  show_bars
  echo "skip: filled. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The first screen's frame and its visible area, x, y up from the bottom, width and height, as a new
# process's AppKit gives them.
screen_areas() {
  osascript -l JavaScript -e 'ObjC.import("AppKit");
const s = $.NSScreen.screens.objectAtIndex(0);
[s.frame, s.visibleFrame].map((r) => [r.origin.x, r.origin.y, r.size.width, r.size.height].join(" "))
  .join(" ")' 2>&1 | tr '\n' ' '
}
# The Dock started hidden still takes its space, even from a new process, until an app is active
# (#2633). The zoom fills what the app's AppKit leaves of the screen, so this waits, up to 20 s,
# for that to be the whole screen, within the 4 pt a hidden Dock may keep.
# Whether $visible is the whole of $at, the screen.
whole() {
  echo "$at $visible" | awk 'NF == 8 { ok = $5 - $1 <= 4 && $6 - $2 <= 4 &&
    $1 + $3 - $5 - $7 <= 4 && $2 + $4 - $6 - $8 <= 4 } END { exit !ok }'
}
shown=
for _ in $(seq 40); do
  send screen
  at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: screen at ' | cut -d' ' -f4-)
  visible=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: screen visible ' |
    cut -d' ' -f4-)
  if [ "$visible" != "$shown" ]; then
    echo "note: the app's visible area $visible of $at (x, y down from the top, w, h)"
    echo "note: a new process's screen and visible area $(screen_areas)(x, y up from the bottom)"
    shown=$visible
  fi
  whole && break
  sleep 0.5
done
whole || {
  echo "note: then a new process's screen and visible area $(screen_areas)"
  fail "the app's visible area stayed $visible of $at for 20 s: the Dock or the menu bar shows"
}
screen=$(echo "$at" | cut -d' ' -f3- | tr ' ' x)
# Zoomed by a double-click on its header, unless System Settings sets that to something else, and
# else placed over the screen.
if [ "$(defaults read -g AppleActionOnDoubleClick 2>/dev/null || echo Maximize)" = Maximize ]; then
  send doubleclick 115 24
  expect '^browser: window zoomed 1$' "a double-click on the header didn't zoom the window"
else
  send place 0 0 ${screen%x*} ${screen#*x}
fi
check_layout "filling its screen"
filled=$content
[ "$filled" = "$screen" ] ||
  fail "filling its screen, the window is $filled, not $screen: the Dock or the menu bar shows"
send click $page_point
send key f
expect '^browser: page entered full screen$' "the page didn't go full screen"
expect '^browser: window entered full screen$' "the page didn't take the window full screen"
# The page goes to its frame there with the shell's next frame, after AppKit's word.
expect '^browser: page laid in place, full screen 1$' "the page wasn't laid out in full screen"
check_layout "with the page full screen in a window that filled its screen" fills
[ "$content" = "$filled" ] || fail "full screen, the window is $content, not $filled as it was"
../tools/window-screenshot.sh "$pid" "$out/filled-full-screen.png" || fail "no window to capture"
send key escape
expect '^browser: page left full screen$' "escape didn't take the page out of full screen"
expect '^browser: window left full screen$' "the window stayed full screen"
expect '^browser: page laid in place, full screen 0$' "the page wasn't laid out back in its card"
check_layout "after the page's full screen in a window that fills its screen"
[ "$content" = "$filled" ] || fail "after full screen the window is $content, not $filled as it was"
docked=$view
send key l cmd shift
expect '^browser: library shown$' "⇧⌘L didn't open the Library after the page's full screen"
send layout
view=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed -n 's/.*WebContentsViewCocoa \([0-9,]* [0-9x]*\).*/\1/p')
../tools/window-screenshot.sh "$pid" "$out/filled-library.png" || fail "no window to capture"
[ "$view" = "417,10 ${docked#* }" ] ||
  fail "after the page's full screen the Library left the page at $view, over its list, not at 417"
send key escape
expect '^browser: library hidden$' "escape didn't close the Library"
exec 3>&-
quit
show_bars
echo "ok: a page's full screen fills a window filling its screen, and the Library pushes it after"
