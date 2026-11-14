# check.sh runs this at 40.
# The page view keeps its margins and GPUI's view fills the window through a resize and full screen,
# and the window's buttons stay in the header row. A page that goes full screen takes the window
# with it, and escape or the page leaving brings both back, the page resized once each way, straight
# to its new size, and its leaving reported once, timed from the page's request to the window's and
# the UI's steps, going in and out. After the window leaves full screen its page draws again at its
# new size, and on the Mac a display plugged in while it's full screen keeps it at its size with its
# page drawn. The window leaving full screen by its green button takes the page
# out with it and brings back the sidebar and the card, and the page's full screen takes the window
# again after. A full-screen window's display going away keeps the window and the page together,
# both full screen or both out, and escape and the page's full screen work after; going away while
# the page takes the window full screen leaves both windowed. The footer's menu
# flips up at the screen's bottom in full screen.
# Esc in full screen goes to a page that keeps it, and twice or held leaves anyway, and ⌘N in full
# screen, with the page's keyboard, opens a window that goes full screen too and then opens ⌘T's
# command bar, whose URL loads there. The command bar opens in full screen and keeps the keyboard
# when a page commits under it, then
# gives the keyboard to the new page, and scrolling a long page doesn't stutter. Dragging the
# window's background moves the window, and the buttons, URL field and rows keep their presses. A
# double-click there zooms it, and one in the
# sidebar below the rows opens a new tab. ⌘S, with the page or the UI focused, and the sidebar
# button hide the sidebar and the window's buttons, and the card takes its place, through a resize;
# ⌘S brings them back, and the core hears the page moves for each motion and is still after it. The
# pointer kept at the window's edge brings the hidden sidebar out, and one passing it doesn't. A tab
# opened with the sidebar hidden says so in a toast, a second one bumps it, and a click on it
# brings the sidebar back. The sidebar's edge drags its width between 164 and 500, stopping at
# them, and dragged on past half the narrowest it hides the sidebar, keeping its width; a
# double-click on it puts the width back. A page that hasn't drawn yet shows black, or white with
# View › Appearance on Light, and with the window widened, the space a page hasn't drawn at its new
# size shows the page's own background. In a check VM, the sidebar over grey backdrops is Dia's composite,
# dark and then light, with the Mac's appearance, and View › Appearance turns the light window dark
# and back. On the Mac, the Dock icon's dot takes the Space's color, and stays green with its switch
# in Settings › General off; on Windows, the taskbar button's switch turns off and on.

# A long page that times its frames while it scrolls and puts the numbers in its title.
page=$(base64 << 'HTML' | tr -d '\n'
<!doctype html>
<title>loading</title>
<style>
  body { font: 16px/1.5 system-ui; margin: 0 auto; max-width: 720px; padding: 0 16px }
  .card { margin: 16px 0; padding: 16px; border-radius: 12px; box-shadow: 0 2px 8px rgb(0 0 0 / 15%) }
</style>
<script>
  const text = "The quick brown fox jumps over the lazy dog. ".repeat(12);
  for (let i = 0; i < 300; i++) {
    document.write(`<div class=card style="background: hsl(${(i * 37) % 360} 70% 92%)"><h2>Section ${i}</h2><p>${text}</p></div>`);
  }
  // requestAnimationFrame runs once per frame the page draws, so the gaps between its calls are
  // frame times. Frames count while scroll events arrive. A frame slower than 60 Hz shows as a
  // stutter on any display.
  const intervals = [];
  let last = 0, lastScroll = 0, measuring = false;
  addEventListener("scroll", () => {
    lastScroll = performance.now();
    if (!measuring) {
      measuring = true;
      requestAnimationFrame(frame);
    }
  }, { passive: true });
  function frame(time) {
    if (last && time - lastScroll < 20) intervals.push(time - last);
    last = time;
    if (performance.now() - lastScroll < 300) return requestAnimationFrame(frame);
    // A scroll after the pause, as when the session stalls mid-scroll, counts on from here.
    measuring = false;
    const sorted = [...intervals].sort((a, b) => a - b);
    const at = q => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))].toFixed(1);
    const fps = Math.round(1000 * intervals.length / intervals.reduce((a, b) => a + b, 0));
    const slow = intervals.filter(i => i > 1000 / 60).length;
    document.title = `frames ${intervals.length} fps ${fps} median ${at(0.5)} p99 ${at(0.99)} max ${at(1)} slow ${slow} scrolled ${Math.round(scrollY)}`;
  }
  // An f takes the page full screen, as the full-screen button of a video does, and an x leaves it,
  // as the same button does in full screen.
  addEventListener("keydown", e => {
    if (e.metaKey) return;
    if (e.key == "f") document.documentElement.requestFullscreen();
    if (e.key == "x" && document.fullscreenElement) document.exitFullscreen();
  });
  // The title says whether the document is full screen, each time that changes.
  document.addEventListener("fullscreenchange", () => {
    document.title = `fullscreen ${document.fullscreenElement ? 1 : 0}`;
  });
  // Each resize counts in the title, with the size it left the page at.
  let resizes = 0;
  addEventListener("resize", () => {
    document.title = `resized ${++resizes} times, to ${innerWidth}x${innerHeight}`;
  });
  // After an n, the page goes to another site when its window loses the keyboard, so a new page
  // commits while the command bar is open.
  let leave = false;
  addEventListener("keydown", e => leave ||= e.key == "n");
  addEventListener("blur", () => leave && (location = "https://example.com/"));
  // A w opens the page the fragment names in a tab.
  addEventListener("keydown", e => e.key == "w" && window.open(location.hash.slice(1)));
  // A g opens gap.html from the check server in a tab.
  addEventListener("keydown", e => e.key == "g" && window.open(location.hash.slice(1).replace(/wait$/, "gap.html")));
  requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
</script>
HTML
)
log="$out/window.log"
step=0
rm -f "$saved" "$launch_frame"
serve
# An orange page with a blue strip down its right edge that, once it has drawn, stalls for 6 s on
# its first resize, so the frame it drew before stays up while the window is wider.
printf '%s' '<!doctype html><title>loading</title>
<style>html { background: rgb(224 90 0) } div { position: fixed; inset: 0 0 0 auto; width: 60px; background: rgb(0 90 224) }</style>
<div></div>
<script>
let armed = false;
addEventListener("resize", () => {
  if (!armed) return;
  armed = false;
  const start = performance.now();
  while (performance.now() - start < 6000);
  requestAnimationFrame(() => setTimeout(() => document.title = "gap drew"));
});
requestAnimationFrame(() => setTimeout(() => (armed = true, document.title = "gap ready")));
</script>' > "$out/site/gap.html"
url="data:text/html;base64,$page#${site}wait"
# With the motion GitHub's hosted Mac and Windows reduce, Windows' as "Show animations in Windows"
# is off on its runner: the sidebar's steps below and the toast recorded below move.
motion=
if [ "$(uname)" = Darwin ] || windows; then
  motion=full
fi
BROWSER_DRIVE=1 BROWSER_MOTION=$motion BROWSER_URL="$url" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the scrolling page didn't draw"

# The window's buttons sit in the header row, where Arc's do. In full screen they come down with
# the menu bar instead. They go with the sidebar ("hidden").
check_buttons() {
  buttons=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout buttons' |
    cut -d' ' -f4-)
  expected="20.0,24.0 43.0,24.0 66.0,24.0"
  # On Windows they're the system's, in its own title bar (platform/windows/drive.cc).
  { [ "${2:-}" = hidden ] || windows; } && expected="hidden hidden hidden"
  [ "$buttons" = "$expected" ] || fail "$1: the window's buttons are at $buttons, not $expected"
}
# capture <png> <shown>: captures the window into png. Under Wayland, sway keeps showing the
# window's last frame, or its own grey where there was none, until the app's next frame reaches it,
# so there it captures again until the function shown passes on the capture, 100 captures at most.
# $captures counts them and $redraw says how long it took (#2336).
capture() {
  wayland=
  [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ] && wayland=1 && start=$(date +%s%N)
  ../tools/window-screenshot.sh "$pid" "$1" || fail "no window to capture"
  captures=1
  redraw=
  [ -n "$wayland" ] || return 0
  while ! "$2" "$1" && [ $captures -lt 100 ]; do
    ../tools/window-screenshot.sh "$pid" "$1" || fail "no window to capture"
    captures=$((captures + 1))
  done
  redraw="$(( ($(date +%s%N) - start) / 1000000 )) ms, capture $captures"
}
# Waits for a page's full screen $1 (enter or exit) to end in the window and the UI to have placed
# the page and drawn at the window's new size. Their lines carry the clock the core's and the host's
# do (drive.mm, tab.cc).
full_screen_done() {
  # Off the Mac the window manager has no transition to end, so no "did": laid and drawn end it.
  steps="did laid drawn"
  [ "$(uname)" = Darwin ] || steps="laid drawn"
  for each in $steps; do
    expect "^browser: full screen $1 $each " "a page's full screen $1 had no \"$each\" step"
  done
}
# The medians, in milliseconds, of the steps of a page's full screen $3 (enter or exit) in the log's
# lines $1 to $2: the core's request to the app's report (report), to the window's start
# (toggle), the window's start to the page placed (placed), to the UI's first frame at the new size
# (drawn) and to the transition's end (transition), and the request to the first frame and the end.
full_screen_times() {
  steps=$(sed -n "$1,${2}p" "$log" | awk -v way="$3" '
    function between(name, from, to) {
      if ((from in t) && (to in t)) print name, t[to] - t[from]
    }
    function flush() {
      between("report", "asked", "told")
      between("toggle", "told", "will")
      between("placed", "will", "laid")
      between("drawn", "will", "drawn")
      between("transition", "will", "did")
      between("to-drawn", "asked", "drawn")
      between("total", "asked", "did")
      delete t
    }
    $1 == "browser:" && $2 == "full" && $3 == "screen" && $4 == way {
      if ($5 == "asked") flush()
      if (!($5 in t)) t[$5] = $(NF - 1)
    }
    END { flush() }')
  for each in report toggle placed drawn transition to-drawn total; do
    echo "$steps" | awk -v name="$each" '$1 == name { print $2 }' | sort -n |
      awk -v name="$each" '{ v[NR] = $1 } END { if (NR) printf "%s %.1f, ", name, v[int((NR + 1) / 2)] }'
  done
}
# The page's resizes so far, from its title.
page_resizes() {
  count=$(grep '^browser: title [0-9]* resized ' "$log" | tail -1 | cut -d' ' -f5)
  echo "${count:-0}"
}
# The page was resized once since it had been $resizes times, straight to $2 (WxH). Called once the
# window's transition has ended, so it waits only for the title the page's last resize gave it.
resized_once() {
  line="resized $((resizes + 1)) times, to $2"
  await "^browser: title [0-9]* $line\$" || true
  last=$(grep '^browser: title [0-9]* resized ' "$log" | tail -1 | cut -d' ' -f4-)
  [ "$last" = "$line" ] || fail "$1: after $resizes resizes the page was $last, not once to $2"
}
check_layout "at launch"
check_buttons "at launch"

# On Windows, moved to a display of another scale, as Windows tells it (WM_DPICHANGED) with its
# size in pixels kept, the page fills the content area at that scale: its insets, in DIPs, scaled
# by the new display's, not the old one's (#1823). 100% to 150% and back, then to 125% and back.
if windows; then
  # The content's size and the page's rect, in pixels, in $rect.
  page_rect() {
    send layout
    rect=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' | sed -n \
      's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*WebContentsViewCocoa \(-*[0-9]*\),\(-*[0-9]*\) \([0-9]*\)x\([0-9]*\).*/\1 \2 \3 \4 \5 \6/p')
  }
  page_rect
  at_100=$rect
  [ -n "$at_100" ] || fail "the layout named no page at launch"
  for percent in 150 100 125 100; do
    send dpi "$percent"
    page_rect
    python3 -I - "$percent" $at_100 $rect << 'PY' ||
import math, sys
s = int(sys.argv[1]) / 100
cw, ch, x, y, w, h = map(int, sys.argv[2:8])
cw2, ch2, x2, y2, w2, h2 = map(int, sys.argv[8:14])
# The insets at 100%, in DIPs: the layout's y counts from the content's bottom.
left, bottom, right, top = x, y, cw - x - w, ch - y - h
# The core's layout: the content in DIPs at the scale, floored, inset, and scaled to pixels.
fw = math.floor(cw2 / s) - left - right
fh = math.floor(ch2 / s) - top - bottom
px_left, px_top = math.floor(left * s), math.floor(top * s)
px_right, px_bottom = math.ceil((left + fw) * s), math.ceil((top + fh) * s)
want = (px_left, ch2 - px_bottom, px_right - px_left, px_bottom - px_top)
got = (x2, y2, w2, h2)
if any(abs(a - b) > 1 for a, b in zip(want, got)):
    sys.exit(f"at {sys.argv[1]}% the page is {got[0]},{got[1]} {got[2]}x{got[3]} in {cw2}x{ch2}, "
             f"not {want[0]},{want[1]} {want[2]}x{want[3]}")
PY
      fail "the page didn't fill the content area at the new display's scale"
  done
fi

# On Windows, the display's own scale goes up a step and back while the app runs, as Settings'
# Scale changes it. GPUI's window, a child of the browser window, hears no WM_DPICHANGED of its own,
# and drew on at the old scale, giving the core a frame for the page in the old scale's points, so
# the page ran off the window (#1358). Now it lays out at the new scale with its parent, and the
# page keeps its insets in DIPs. The command bar's window keeps its size in points. The display goes
# back to its scale even if the run fails.
if windows; then
  read -r scale_at scale_least scale_most << EOF
$(python3 -I check/scale.py get)
EOF
  if [ -z "$scale_most" ] || [ "$scale_most" -le "$scale_at" ]; then
    echo "SKIP: the display's scale is at its largest ($scale_at of $scale_least to $scale_most)"
  else
    run_undo="python3 -I check/scale.py set $scale_at"
    # The page's insets from $rect, in DIPs: left, bottom, right and top.
    insets() {
      set -- $rect
      echo "$3 $4 $(($1 - $3 - $5)) $(($2 - $4 - $6))"
    }
    # Whether the insets $1 and $2 are the same, to a DIP, as the core floors the content's size.
    same_insets() {
      echo "$1 $2" | awk '{ for (i = 1; i <= 4; i++) if ((d = $i - $(i + 4)) > 1 || d < -1) exit 1 }'
    }
    # The overlays' sizes in points, in $bar.
    bar_now() {
      send overlays
      bar=$(grep '^browser: overlays shown ' "$log" | tail -1 | cut -d' ' -f4- |
        sed 's/ at [0-9-]*,[0-9-]*//g')
    }
    # Lays out until the page is inset by $1 and the content isn't $2 in DIPs, for up to 10 s, as
    # the UI lays out once it has heard the scale.
    page_inset() {
      for _ in $(seq 20); do
        page_rect
        set -- "$1" "$2" $rect
        [ "$3x$4" != "$2" ] && same_insets "$1" "$(insets)" && return 0
        sleep 0.5
      done
      return 1
    }
    send key l cmd
    expect '^browser: command bar shown$' "⌘L didn't open the command bar before the scale changed"
    page_rect
    rect_before=$rect insets_before=$(insets)
    set -- $rect
    content_before="$1x$2"
    bar_now
    bar_before=$bar
    python3 -I check/scale.py set $((scale_at + 1)) > /dev/null
    page_inset "$insets_before" "$content_before" ||
      fail "a step up the display's scale left the page at $rect in the content, not inset by $insets_before"
    ../tools/window-screenshot.sh "$pid" "$out/display-scaled.png" || fail "no window to capture"
    bar_now
    [ "$bar" = "$bar_before" ] ||
      fail "the command bar's window was $bar at the display's new scale, not $bar_before"
    python3 -I check/scale.py set "$scale_at" > /dev/null
    run_undo=
    for _ in $(seq 20); do
      page_rect
      [ "$rect" = "$rect_before" ] && break
      sleep 0.5
    done
    [ "$rect" = "$rect_before" ] ||
      fail "back at the display's scale the page was at $rect in the content, not $rect_before"
    bar_now
    [ "$bar" = "$bar_before" ] ||
      fail "the command bar's window was $bar back at the display's scale, not $bar_before"
    send key escape
    expect '^browser: command bar hidden$' "escape didn't close the command bar"
  fi
fi

# A page shows black until its first frame while websites are told the appearance is dark, and
# white while it's light, as Arc's does, with View › Appearance on each in turn; its tab is loading
# from the start. The page's w opens one whose server never answers, so it never commits. Its URL
# shows, as Chrome's does for a tab a page opened, until the tab's first, empty document is touched,
# when Chrome hides it. That's at once in most runs and not for seconds in some (#2215).
# A page already waiting as the appearance changes keeps its colour on the Mac until it draws, so
# each colour gets a page opened after it.
first=$(today)
# Whether the colour "r g b" is black, or white with $2 light, as near as the capture keeps them.
unpainted() {
  set -- $1 $2
  case $4 in
  light) [ "$1" -ge 251 ] && [ "$2" -ge 251 ] && [ "$3" -ge 251 ] ;;
  *) [ "$1" -le 4 ] && [ "$2" -le 4 ] && [ "$3" -le 4 ] ;;
  esac
}
unpainted_seen=
for scheme in light dark; do
  item=Light hex=ffffff
  [ $scheme = light ] || item=Dark hex=000000
  send menu "Appearance › $item"
  expect "^browser: appearance $scheme\$" "View › Appearance › $item didn't change the appearance"
  send click $page_point
  send key w
  expect "^browser: tab [0-9]* opened by $first in the foreground\$" "the page didn't open a tab"
  tab=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
  expect "^browser: page $tab back 0 forward 0 loading 1 \(${site}wait\)*\$" \
    "a tab opened to a page that hasn't come doesn't say it's loading"
  for _ in $(seq 20); do
    ../tools/window-screenshot.sh "$pid" "$out/before-paint-$scheme.png" ||
      fail "no window to capture"
    seen=$(pixel "$out/before-paint-$scheme.png" 0.6 0.5)
    unpainted "$seen" $scheme && break
  done
  unpainted "$seen" $scheme ||
    fail "with View › Appearance › $item, a page that hasn't drawn shows $seen, not #$hex"
  [ "$(grep '^browser: page color ' "$log" | tail -1)" = "browser: page color $hex" ] ||
    fail "with View › Appearance › $item, the core wasn't given #$hex for a page not yet drawn"
  unpainted_seen="$unpainted_seen $scheme $seen,"
  send key w cmd
  expect_tabs "$first" "$first" "after closing the tab of the page that hasn't come"
done
send menu "Appearance › Automatic"
expect '^browser: appearance automatic$' \
  "View › Appearance › Automatic didn't give back the system's appearance"
send click $page_point

# 300 trackpad events of 10 points, 8 ms apart. X11's events carry no distance, so the Linux driver
# turns the wheel, which Chromium scrolls 120 points a notch there: 25 notches, five at a time.
# Chromium hands a page one wheel event at a time and merges those that queue meanwhile, capping a
# merged one at the page's height (LimitScrollDeltaToScrollerSize), as Chrome does. In a slow
# session the 25 at once came as one notch and 24 merged, and the page stopped at its height, 779
# points (#2256). Five, 600 points, stay under it.
if [ "$(uname)" = Linux ]; then
  for to in 600 1200 1800 2400 3000; do
    send scroll $page_point 1 5
    await "^browser: title [0-9]* frames .* scrolled $to\$" || break
  done
else
  send scroll $page_point 10 300
fi
expect '^browser: title [0-9]* frames' "scrolling didn't move the page"
# The page says its count after each pause of 300 ms, so the last is the whole scroll's.
await '^browser: title [0-9]* frames .* scrolled 3000$' || true
timing=$(grep '^browser: title [0-9]* frames' "$log" | tail -1 | sed 's/^browser: title [0-9]* //')
echo "$timing" | grep -q ' scrolled 3000$' || fail "the page didn't scroll by exactly 3000 points: $timing"
frames=$(echo "$timing" | sed 's/^frames \([0-9]*\) .*/\1/')
slow=$(echo "$timing" | sed 's/.* slow \([0-9]*\) .*/\1/')
# A VM's display runs at 60 Hz and its frames come when the host gives it time, so only a Mac's own
# screen can judge the scroll, and a VM reports it, as Linux's headless session does.
if [ "$(uname)" = Linux ]; then
  timing="$timing, in a headless session, not judged"
elif windows; then
  timing="$timing, on Windows, which runs the check in a VM, not judged"
elif [ "$(sysctl -n kern.hv_vmm_present)" = 1 ]; then
  timing="$timing, in a VM, not judged"
elif [ $((slow * 100)) -gt "$frames" ]; then
  fail "more than 1% of frames took longer than 1/60 s: $timing"
fi

send resize 1000 700
check_layout "after a resize"
check_buttons "after a resize"
# Widened, the space a page hasn't drawn yet shows the page's own background, as Arc's does, not
# the colour of a page before its first paint: the frame the page drew before stays at its old
# size, its blue strip 200 to the left, until the page draws at the new size. Blue at the new right
# edge means the page drew, or its frame was stretched to fill, either way leaving no gap.
send key g
expect "^browser: tab [0-9]* opened by $first in the foreground\$" "the page didn't open gap.html"
tab=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
await "^browser: title $tab gap ready\$" || fail "gap.html didn't draw"
# Its layout was checked after the resize above: the Mac keeps the page it left at its old size
# in the layout's list, which check_layout would read.
send resize 1200 700
expect '^browser: drawn at 1200x700$' "the window didn't draw widened over gap.html"
# Under Wayland the UI lays out the new size before the window shows a frame at it, and until it
# does, sway's own grey, 63 63 63, fills the rest of the width sway gave it.
widened() { [ "$(pixel "$1" 0.97 0.5)" != "63 63 63" ]; }
capture "$out/resize-gap.png" widened
grep -q "^browser: title $tab gap drew\$" "$log" && fail "gap.html drew at the new size before the capture"
set -- $(pixel "$out/resize-gap.png" 0.97 0.5)
[ "$captures" = 1 ] || set -- "$@" "in capture $captures"
if [ "$1" -ge 170 ] && [ "$2" -ge 50 ] && [ "$2" -le 130 ] && [ "$3" -le 60 ]; then
  gap="the space gap.html hadn't drawn yet showed its orange, $*"
elif [ "$3" -ge 170 ] && [ "$1" -le 60 ]; then
  gap="gap.html's frame filled the wider page view, $*, leaving no gap"
else
  fail "widened, the space gap.html hadn't drawn yet showed $*, not the page's orange"
fi
await "^browser: title $tab gap drew\$" || fail "gap.html didn't draw at the new size"
send key w cmd
expect_tabs "$first" "$first" "after closing gap.html"
send resize 1000 700
check_layout "after narrowing the window back"
send click $page_point
# The window's resize border, about 3 outside its edge (Dia), is AppKit's to track, as for any
# resizable window. Whether AppKit takes a drag the driver posts there only a Mac's run tells: a
# drag from 2 outside the right edge either widens the window by 100 or leaves it, and the run
# says which. On Wayland the border is the compositor's, and the driver puts the pointer only over
# the app's own surfaces, so there's no drag to post (#1572).
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  edge="on Wayland the window's border is the compositor's, which the driver can't reach"
else
  send drag 1002 350 1102 350
  check_layout "after a drag at the window's edge"
  case "$width" in
    1100)
      edge="a drag 2 outside the window's right edge widened it by 100"
      check_buttons "after a drag at the window's edge"
      ../tools/window-screenshot.sh "$pid" "$out/edge-resized.png" || fail "no window to capture"
      send resize 1000 700
      check_layout "after the edge's resize"
      ;;
    1000) edge="the window didn't take a posted drag 2 outside its right edge, so no run can drag it" ;;
    *) fail "a drag at the window's edge left it $width wide, neither 1000 nor 1100" ;;
  esac
fi
send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen"
# Once there, it says how to leave, at the card's top right, as Arc's does, in Chrome's words off the
# Mac (#772).
note='To exit full screen, press ⎋\.'
{ windows || [ "$(uname)" = Linux ]; } && note='Press Esc to exit full screen'
expect "^browser: toast shown $note\$" "full screen didn't say how to leave"
expect '^browser: toast settled$' "the note on leaving full screen didn't settle"
../tools/window-screenshot.sh "$pid" "$out/fullscreen-note.png" || fail "no window to capture"
check_layout "in full screen"
send key l cmd
expect '^browser: command bar shown$' "⌘L in full screen didn't open the command bar"
send state
[ "$key" = GPUIPanel ] || fail "the command bar in full screen didn't get the keyboard, $key did"
../tools/window-screenshot.sh "$pid" "$out/fullscreen.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar in full screen"
# The footer is at the screen's bottom in full screen, so the +'s menu flips up over the pointer.
post click 204 $((height - 25))
plus="New Space…, New Folder…, New Split View, New Note, $(plus_adds)"
expect "^browser: menu shown $(cased "$plus")\$" \
  "the footer's + in full screen showed no menu"
expect '^browser: menu window at ' "the +'s menu in full screen didn't open its window"
top=$(grep '^browser: menu window at' "$log" | tail -1 | cut -d' ' -f5 | cut -d, -f2)
[ "$top" -lt $((height - 65)) ] ||
  fail "the +'s menu at the screen's bottom opened down, its window's top at $top, not up over the pointer"
../tools/window-screenshot.sh "$pid" "$out/fullscreen-menu.png" || fail "no window to capture"
send key escape
expect '^browser: menu closed$' "escape didn't close the +'s menu in full screen"
# The + took the keyboard for the UI, and the page's keys below need it back.
send click $page_point
send state
in_browser RenderWidgetHostViewCocoa "after a click on the page under the closed +'s menu"
send fullscreen
expect '^browser: window left full screen$' "the window didn't leave full screen"
check_layout "after full screen"
check_buttons "after full screen"

# A page that goes full screen takes the window with it, and escape brings both back. The app
# knows before the window resizes, so the page is laid out once each way, at the window's new size,
# and the transition shows no frame of the other layout.
resizes=$(page_resizes)
send key f
asked=$step
expect '^browser: page entered full screen$' "the page didn't go full screen"
expect '^browser: window entered full screen$' "the window didn't go full screen with the page"
check_layout "with the page full screen" fills
# From the key on, as check_layout sends a command of its own.
logged "^browser: toast shown $note" "$asked" &&
  fail "a page's full screen showed the window's note on leaving it"
resized_once "going full screen" "$content"
../tools/window-screenshot.sh "$pid" "$out/page-fullscreen.png" || fail "no window to capture"
resizes=$(page_resizes)
send key escape
expect '^browser: page left full screen$' "escape didn't take the page out of full screen"
expect '^browser: window left full screen$' "the window didn't leave full screen with the page"
check_layout "after the page's full screen"
check_buttons "after the page's full screen"
resized_once "leaving full screen by escape" "${view#* }"
# So does the page leaving full screen itself, which Chrome tells of twice and the app hears once.
resizes=$(page_resizes)
send key f
expect '^browser: page entered full screen$' "the page didn't go full screen again"
expect '^browser: window entered full screen$' "the page didn't take the window full screen again"
check_layout "with the page full screen again" fills
resized_once "going full screen again" "$content"
resizes=$(page_resizes)
send key x
expect '^browser: page left full screen$' "exitFullscreen didn't take the page out of full screen"
expect '^browser: window left full screen$' "the window stayed full screen after the page left it"
left=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: page left full screen$')
[ "$left" = 1 ] || fail "the page leaving full screen itself was reported $left times"
check_layout "after the page left full screen itself"
resized_once "the page leaving full screen itself" "${view#* }"

# The window leaving full screen by its green button, as ⌃⌘F and its display going away do, takes
# the page out with it, as Chrome's does, and the sidebar and the card come back (#415). The page's
# full screen then takes the window again, and escape brings both back. The empty sidebar's colour,
# where the page would be if it still filled the window, is the same as before each time.
windowed=$content
sidebar_at="$(awk -v w="$width" 'BEGIN { print 110 / w }') 0.6"
# Just inside the page view's left edge, the page's own margin.
page_at="$(awk -v x="${view%%,*}" -v w="$width" 'BEGIN { print (x + 4) / w }') 0.6"
# Colours $1 and $2 differ by more than 8 in a channel.
differ() {
  echo "$1 $2" | awk '{ for (i = 1; i <= 3; i++) if ($i - $(i + 3) > 8 || $(i + 3) - $i > 8) exit 0; exit 1 }'
}
# The windowed frame is on screen in capture $1: the sidebar isn't the page beside it. Until then
# the page fills the window where the sidebar is.
windowed_shown() { differ "$(pixel "$1" $sidebar_at)" "$(pixel "$1" $page_at)"; }
capture "$out/before-green-button.png" windowed_shown
[ -z "$wayland" ] || windowed_shown "$out/before-green-button.png" ||
  fail "before the green button, with the window laid out windowed, the page still covers the sidebar after $redraw"
redraws=${redraw:+"before the green button $redraw"}
sidebar=$(pixel "$out/before-green-button.png" $sidebar_at)
# The sidebar in capture $1 has its colour from before, within 8 in each channel.
sidebar_back() {
  sidebar_now=$(pixel "$1" $sidebar_at)
  ! differ "$sidebar_now" "$sidebar"
}
send key f
expect '^browser: page entered full screen$' "the page didn't go full screen before the green button"
expect '^browser: window entered full screen$' "the page didn't take the window full screen before the green button"
expect '^browser: title [0-9]* fullscreen 1$' "the page didn't say it's full screen before the green button"
check_layout "with the page full screen before the green button" fills
send fullscreen
expect '^browser: page left full screen$' "the green button took the window out and left the page full screen"
expect '^browser: title [0-9]* fullscreen 0$' "the page's document stayed full screen after the green button"
expect '^browser: window left full screen$' "the green button didn't take the window out of full screen"
check_layout "after the green button took the page's window out of full screen"
check_buttons "after the green button took the page's window out of full screen"
[ "$content" = "$windowed" ] ||
  fail "after the green button the window's content is $content, not $windowed as before"
capture "$out/green-button.png" sidebar_back
redraws=${redraws:+"$redraws, "}${redraw:+"after the green button $redraw"}
sidebar_back "$out/green-button.png" ||
  fail "after the green button the sidebar is $sidebar_now, not $sidebar: the page still covers it"
send key f
expect '^browser: page entered full screen$' "the page didn't go full screen after the green button"
expect '^browser: window entered full screen$' "the page didn't take the window full screen after the green button"
check_layout "with the page full screen after the green button" fills
send key escape
expect '^browser: page left full screen$' "escape didn't take the page out of full screen after the green button"
expect '^browser: window left full screen$' "the window didn't leave full screen with the page after the green button"
check_layout "after the page's full screen after the green button"
check_buttons "after the page's full screen after the green button"
capture "$out/green-button-again.png" sidebar_back
redraws=${redraws:+"$redraws, "}${redraw:+"after the page's next full screen $redraw"}
sidebar_back "$out/green-button-again.png" ||
  fail "after the page's next full screen the sidebar is $sidebar_now, not $sidebar: the page still covers it"
[ -z "${redraws:-}" ] || echo "window: under Wayland the windowed frame reached the capture $redraws"

# The page's window full screen on a second display, the main one, as for a video on an external
# monitor, and the display going away, then and again as the window leaves full screen: the window
# and the page stay together. AppKit keeps the window full screen on the other display, with the
# page, or takes it out, and the page with it, and the sidebar and the card come back. From there
# escape and the page's full screen work. The check VM's AppKit keeps the window full screen, and
# finishes a way out it started, but on the owner's Mac it took the window out without finishing,
# and the page stayed laid out full screen, for good (#415).
# The Mac's is the main display, at 0 0; sway's an output beside the session's.
second_display() {
  send display add 1280 800 main
  added=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: display ')
  case "$added" in
  "browser: display added at "*" 1280 800") set -- ${added#browser: display added at } ;;
  *) fail "no second display for the page's full screen: $added" ;;
  esac
  [ "$(uname)" != Darwin ] || [ "$1 $2" = "0 0" ] || fail "the second display isn't the main one: $added"
  send place $(($1 + 40)) $(($2 + 40)) 1000 700
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen on the second display"
  expect '^browser: window entered full screen$' "the page didn't take the window full screen on the second display"
  check_layout "with the page full screen on the second display" fills
  [ "$content" = 1280x800 ] || fail "the page took the window full screen at $content, not on the second display"
}
# Waits up to $3 tenths of a second (100) for a line matching $1 logged since command $from, else
# fails with $2.
expect_from() {
  for _ in $(seq ${3:-100}); do
    sed -n "/^browser: drive $from: /,\$p" "$log" | grep -q "$1" && return 0
    sleep 0.1
  done
  fail "$2"
}
# The display went away $1, and the window and the page are together, and leave and take full
# screen together. $2 names the captures. AppKit's lines count from command $from.
together() {
  expect '^browser: display removed$' "the second display didn't go away $1"
  send layout
  if sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout full screen 1$'; then
    check_layout "full screen on the other display, after the second went away $1" fills
    ! sed -n "/^browser: drive $from: /,\$p" "$log" | grep -q '^browser: title [0-9]* fullscreen 0$' ||
      fail "the window stayed full screen after the second display went away $1, and the page left"
    send key escape
    expect '^browser: page left full screen$' "escape didn't take the page out of full screen after its display went away $1"
    expect '^browser: window left full screen$' "the window didn't leave full screen with the page after its display went away $1"
  else
    expect_from '^browser: window left full screen$' "AppKit took the window out of full screen as its display went away $1, and the app never heard it was done"
    expect_from '^browser: page left full screen$' "the page stayed full screen as its window left it, its display gone $1"
    expect_from '^browser: title [0-9]* fullscreen 0$' "the page's document stayed full screen as its window left it, its display gone $1"
  fi
  # sway puts a window that leaves full screen back where it was, here on the display that went and
  # off the one left, and under Wayland ilo can't place its windows: a user moves it back.
  [ -z "${WAYLAND_DISPLAY:-}" ] || send place $frame
  windowed_again "$1" "$2"
}
# After the page's display went away $1, the window is windowed with its sidebar and card back, and
# the page takes it full screen and escape brings both back. $2 names the captures.
windowed_again() {
  check_layout "after the page's display went away $1"
  check_buttons "after the page's display went away $1"
  # As the window's way out ends, macOS can make another app active, and the app takes it back
  # (host.mm). The sidebar's material is the active window's only once it has.
  for _ in $(seq 30); do
    [ "$active" = 1 ] && break
    sleep 0.1
    send state
  done
  [ "$active" = 1 ] ||
    fail "Browser wasn't the active app again after the page's display went away $1, so its window isn't key"
  ../tools/window-screenshot.sh "$pid" "$out/$2.png" || fail "no window to capture"
  sidebar_back "$out/$2.png" ||
    fail "after the page's display went away $1 the sidebar is $sidebar_now, not $sidebar: the page still covers it"
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen after its display went away $1"
  expect '^browser: window entered full screen$' "the page didn't take the window full screen after its display went away $1"
  check_layout "with the page full screen after its display went away $1" fills
  send key escape
  expect '^browser: page left full screen$' "escape didn't take the page out of full screen after its display went away $1"
  expect '^browser: window left full screen$' "the window didn't leave full screen with the page after its display went away $1"
  check_layout "after the page's full screen after its display went away $1"
  check_buttons "after the page's full screen after its display went away $1"
  ../tools/window-screenshot.sh "$pid" "$out/$2-again.png" || fail "no window to capture"
  sidebar_back "$out/$2-again.png" ||
    fail "after the page's next full screen the sidebar is $sidebar_now, not $sidebar: the page still covers it"
}
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  # sway takes a window full screen and out in one step, with no way in or out for the display to
  # go during, so those steps are the Mac's.
  send order
  frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
  second_display
  send display remove
  from=$step
  together "in full screen" display-gone
  echo "skip: the second display going as the window leaves or enters full screen: sway does either in one step."
  displays="a full-screen window's display going away in full screen keeps the window and the page together"
  display_shots="$out/display-gone.png, $out/display-gone-again.png, "
elif [ "$(uname)" = Linux ]; then
  echo "skip: second display: the X11 driver can't add one yet."
elif windows; then
  echo "skip: second display: the Windows driver can't add one yet."
else
  displays="a full-screen window's display going away, in full screen and as it leaves, keeps the window and the page together, and going away as the page takes the window full screen leaves both windowed"
  display_shots="$out/display-gone.png, $out/display-gone-again.png, $out/display-gone-leaving.png, $out/display-gone-leaving-again.png, $out/display-gone-entering.png, $out/display-gone-entering-again.png, "
  send order
  frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
  second_display
  send display remove
  from=$step
  together "in full screen" display-gone
  second_display
  post key escape
  from=$step
  expect '^browser: full screen exit will ' "escape didn't start taking the window out of full screen on the second display"
  send display remove
  together "as the window left full screen" display-gone-leaving
  # The display going away while the page is still taking the window there. AppKit finishes the way
  # in some seconds later on the other display, where the page didn't ask to be, and the page's full
  # screen ends with the window's there, rather than aborting the Dev engine in Chrome's controller
  # (#420). An AppKit that gives up on the way in instead leaves both windowed too.
  send display add 1280 800 main
  added=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: display ')
  [ "$added" = "browser: display added at 0 0 1280 800" ] ||
    fail "no second display for the page's full screen to start on: $added"
  send place 40 40 1000 700
  post key f
  from=$step
  expect '^browser: full screen enter will ' "the page didn't start taking the window full screen on the second display"
  send display remove
  expect '^browser: display removed$' "the second display didn't go away as the window entered full screen"
  expect_from '^browser: title [0-9]* fullscreen 0$' "the page stayed full screen after its display went away as the window entered full screen" 300
  expect_from '^browser: page left full screen$' "the app kept the page full screen after its display went away as the window entered full screen" 100
  expect_from '^browser: window left full screen$' "the window stayed full screen after the page's display went away on the way in" 100
  windowed_again "as the window entered full screen" display-gone-entering
  send place $frame
fi

# Full screen holds up (Arc's release notes): once the window leaves full screen its page draws
# again at its new size, not a frame from before or nothing, and on the Mac a display plugged in
# while it's full screen, and taken out again, leaves it full screen at its size with its page
# drawn. The page is blue in a card narrower than 900 and orange in a wider one, so a capture says
# which size it last drew at.
printf '%s' '<title>sized</title><style>body{margin:0;background:#3366cc}
@media (min-width:900px){body{background:#cc6633}}</style>' > "$out/site/sized.html"
# Whether capture $1, at the page's middle, is "r g b" $2 within 16 a channel, as a display's colour
# profile moves it.
drawn_as() {
  seen=$(pixel "$1" 0.6 0.5)
  set -- $seen $2
  [ $(($1 > $4 ? $1 - $4 : $4 - $1)) -le 16 ] && [ $(($2 > $5 ? $2 - $5 : $5 - $2)) -le 16 ] &&
    [ $(($3 > $6 ? $3 - $6 : $6 - $3)) -le 16 ]
}
# Captures the window as $1 until the page shows "r g b" $2, for up to 2 s, else fails saying $3.
await_drawn() {
  for _ in $(seq 20); do
    ../tools/window-screenshot.sh "$pid" "$out/$1.png" || fail "no window to capture"
    drawn_as "$out/$1.png" "$2" && return 0
    sleep 0.1
  done
  fail "$3: the page shows $seen, not $2"
}
blue="51 102 204" orange="204 102 51"
shown_before=$(grep '^browser: selected [0-9]' "$log" | tail -1 | cut -d' ' -f3)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar for the sized page"
send insert "${site}sized.html"
send key return
expect "^browser: loaded [0-9]* ${site}sized.html\$" "the sized page didn't load"
send click $page_point
await_drawn sized-windowed "$blue" "the sized page in the window"
send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen with the sized page"
check_layout "full screen with the sized page"
full=$content
await_drawn sized-full-screen "$orange" "the sized page in full screen"
if [ "$(uname)" = Darwin ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
  send display add 1280 800
  added=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: display ')
  case "$added" in "browser: display added at "*) ;; *) fail "no display to plug in: $added" ;; esac
  check_layout "full screen as a display was plugged in"
  [ "$content" = "$full" ] || fail "a display plugged in took the full-screen window from $full to $content"
  await_drawn display-plugged "$orange" "full screen as a display was plugged in"
  send display remove
  expect '^browser: display removed$' "the plugged-in display didn't go away"
  check_layout "full screen as the plugged-in display went away"
  [ "$content" = "$full" ] || fail "the plugged-in display going took the full-screen window from $full to $content"
  await_drawn display-unplugged "$orange" "full screen as the plugged-in display went away"
else
  driver=X11
  ! windows || driver=Windows
  echo "skip: a display plugged in while full screen: the $driver driver can't add one yet."
fi
send fullscreen
expect '^browser: window left full screen$' "the window didn't leave full screen with the sized page"
check_layout "after full screen with the sized page"
await_drawn sized-left "$blue" "after leaving full screen"
# The runs after this one drive the page from before, which closing the sized page shows again.
send key w cmd
expect "^browser: selected $shown_before\( in .*\)\{0,1\}\$" "closing the sized page didn't show the page before it"
# Where it was before, as the display's coming and going can leave it elsewhere.
[ -z "${frame:-}" ] || send place $frame
send click $page_point

# A page's full screen timed three times each way, entered by a key as a video's button is and left
# by the page, as its button leaves, and then by escape. The steps run from the core's line to the
# window's and the UI's, and the checks wait for the last of them. Each key also waits for the page
# to have heard the change before it: until it has, its document is still in the full screen it's
# leaving, or not yet in the one it's entering, and requestFullscreen or exitFullscreen does nothing.
page_heard() {
  expect "^browser: title [0-9]* fullscreen $1\$" "the page didn't hear its full screen $2"
}
timed_from=$(($(wc -l < "$log") + 1))
for _ in 1 2 3; do
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen to be timed"
  full_screen_done enter
  page_heard 1 "begin, to be timed"
  send key x
  expect '^browser: page left full screen$' "exitFullscreen didn't take the page out of full screen to be timed"
  full_screen_done exit
  page_heard 0 "end, to be timed"
done
timed_escape=$(($(wc -l < "$log") + 1))
for _ in 1 2 3; do
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen to be timed"
  full_screen_done enter
  page_heard 1 "begin, to be timed"
  send key escape
  expect '^browser: page left full screen$' "escape didn't take the page out of full screen to be timed"
  full_screen_done exit
  page_heard 0 "end, to be timed"
done
timed_to=$(($(wc -l < "$log")))
enter_ms=$(full_screen_times "$timed_from" "$timed_to" enter)
exit_ms=$(full_screen_times "$timed_from" "$((timed_escape - 1))" exit)
escape_ms=$(full_screen_times "$timed_escape" "$timed_to" exit)

# Chromium makes its window key when a new page's view replaces the focused one.
send key n
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
expect '^browser: loaded [0-9]* https://example.com/$' "the page didn't leave when the command bar opened"
send state
[ "$key" = GPUIPanel ] || fail "a page that committed under the command bar took the keyboard, $key has it"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar after a page committed under it"
send state
in_browser RenderWidgetHostViewCocoa "after the command bar closed over a page that committed under it"

# The window's background moves the window, as a title bar does: the header row outside its
# buttons, the sidebar below the rows and the card's margins. The buttons, the URL field and the
# rows and the Space's name keep their presses. Each drag moves 5 points, about a pointer's first step. Posted events
# don't move the window itself: the window server follows the real pointer.
moves=0
drag() {
  send drag "$1" "$2" $(($1 + 5)) "$2"
  count=$(grep -c '^browser: window move$' "$log" || true)
  if [ "$3" = moves ]; then
    [ "$count" -gt "$moves" ] || fail "a drag on $4 didn't move the window"
  else
    [ "$count" = "$moves" ] || fail "a drag on $4 moved the window"
  fi
  moves=$count
}
drag 115 24 moves "the header"
drag 138 24 stays "the back button"
drag 110 64 stays "the URL field"
drag 110 112 stays "the Space's name"
drag 110 195 stays "a tab's row"
drag 110 400 moves "the empty sidebar"
# On Windows the frame keeps the top 8 points, Windows' resize border (platform/windows/host.cc's
# FrameHit), as Windows' windows resize by theirs, and the card's margin under it moves the window:
# design/tokens.toml's card-inset is 10.
if windows; then
  drag 600 5 stays "the frame's top edge, which resizes"
fi
# Not nearer the top: KWin maximizes a window moved with the pointer within 5 pixels of the screen's
# top, its quick-maximize zone, as it does under a hand.
drag 600 9 moves "the card's top margin"
drag $((width - 8)) 300 moves "the card's right margin"
# A double-click there does what System Settings › Desktop & Dock says a title bar's does: zoom,
# unless it's set to something else. Sway, the Wayland session's compositor, maximizes no window:
# its windows tile or float, so there's no zoom to see.
action=$(defaults read -g AppleActionOnDoubleClick 2>/dev/null || echo Maximize)
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  zoom="isn't checked, since sway maximizes no window"
  echo "skip: a double-click on the header's zoom: sway maximizes no window."
elif [ "$action" = Maximize ]; then
  send doubleclick 115 24
  expect '^browser: window zoomed 1$' "a double-click on the header didn't zoom the window"
  # On Windows the UI lays out at the maximized size after Windows has zoomed the window, and a
  # press before it draws there is lost.
  ! windows || expect '^browser: drawn at ' "the UI didn't draw the zoomed window"
  # A click on the same spot within the double-click time (500 ms by default) goes on counting, so
  # the second pair would be a third and fourth click, which no title bar takes for a double-click.
  # The zoomed window can draw sooner than that.
  sleep 0.6
  send doubleclick 115 24
  expect '^browser: window zoomed 0$' "a second double-click on the header didn't zoom it back"
  zoom="zooms it"
else
  zoom="isn't checked, since a title bar's double-click is set to $action here"
fi
send doubleclick 110 400
expect '^browser: command new-tab via sidebar$' "a double-click on the empty sidebar didn't open a new tab"
expect '^browser: command bar shown$' "a double-click on the empty sidebar didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# ⌘S hides the sidebar, with the page focused or the UI, and the card takes its place, 10 in from
# every side, as the window's buttons go. ⌘S and the sidebar button bring both back.
# toggle <from> <hidden|shown> <what>: waits for the sidebar to finish moving after the command.
toggle() {
  expect "^browser: command toggle-sidebar via $1\$" "$3 didn't reach the UI"
  expect "^browser: sidebar $2\$" "$3 didn't leave the sidebar $2"
  if [ "$2" = hidden ]; then
    check_layout "after $3" inset
    check_buttons "after $3" hidden
  else
    check_layout "after $3"
    check_buttons "after $3"
  fi
}
# moved <what>: the sidebar's motion moved the page's frame every step, so the core heard the page
# moves before the first and is still after the last (issue #166). A check can't see the jiggle that
# stops. Not for docking the sidebar out, which takes its place in one frame. Before toggle, whose
# layout starts a new driver step that both lines precede.
moved() {
  expect '^browser: page moving 1$' "$1 didn't tell the core the page moves"
  expect '^browser: page moving 0$' "$1 didn't tell the core the page is still"
}
send click $page_point
send key s cmd
# The capture waits for the line, not a time, so it lands in the motion if the motion outlasts the
# capture, and just after it if not.
expect '^browser: page moving 1$' "⌘S with the page focused didn't tell the core the page moves"
../tools/window-screenshot.sh "$pid" "$out/sidebar-moving.png" || fail "no window to capture"
moved "⌘S with the page focused"
toggle page hidden "⌘S with the page focused"
../tools/window-screenshot.sh "$pid" "$out/sidebar-hidden.png" || fail "no window to capture"
send key s cmd
moved "⌘S again with the page focused"
toggle page shown "⌘S again with the page focused"
send click 110 400
send key s cmd
moved "⌘S with the UI focused"
toggle ui hidden "⌘S with the UI focused"
send key s cmd
moved "⌘S again with the UI focused"
toggle ui shown "⌘S again with the UI focused"
send click 93 24
toggle sidebar hidden "a click on the sidebar button"
send key s cmd
toggle ui shown "⌘S after the sidebar button"
# The page stays inset through a resize while the sidebar is hidden.
send resize 900 600
send key s cmd
toggle ui hidden "⌘S in a smaller window"
send resize 1000 700
check_layout "after a resize with the sidebar hidden" inset
send key s cmd
toggle ui shown "⌘S after a resize"

# With the sidebar hidden, the pointer at the window's left edge brings it out over the page in a
# window of its own, which takes clicks and leaves the keyboard to the browser window, and the
# pointer well past it sends it away. A click on its sidebar button, or ⌘S, docks the sidebar.
# check_reveal <what> [shown]: the reveal's window covers the left edge, or there's none.
check_reveal() {
  # The UI logs its rest before the host takes its window in from the spring's overshoot, a turn
  # of the run loop later, so a layout or two may still see the wider window.
  for _ in $(seq 10); do
    send layout
    child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout child' |
      cut -d' ' -f4-)
    height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
      sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
    expected=
    [ "${2:-}" = shown ] && expected="0,0 233x$height"
    [ "$child" = "$expected" ] && return
  done
  fail "$1: the reveal's window is at ${child:-none}, not ${expected:-none}"
}
# reveal_at_edge <what> [from]: the pointer comes onto the window's edge and moves along it, and the
# sidebar comes out, which is looked for from the first move, or from command <from>: the wait the
# pointer starts at the edge can be over before the next move is sent.
reveal_at_edge() {
  local from=${2:-$((step + 1))}
  send move 3 300
  send move 3 306
  expect '^browser: reveal shown$' "$1" "$from"
}
send key s cmd
toggle ui hidden "⌘S before the reveal"
send move 400 300
# The pointer has to move a few points up or down at the edge and stay a moment: one jumping
# there, or passing it, doesn't bring the sidebar out.
send move 3 300
none_while '^browser: reveal waits$' || fail "the pointer jumping to the window's edge brought the sidebar out"
step=$((step + 1))
passed=$step
echo "move 3 306" >&3
send move 400 300
if [ "$(uname)" = Linux ]; then
  # On Linux the shell hears neither the pointer over the page nor, under X11, its leaving for the
  # page, so the pass brings the sidebar out once its wait is up, which may be before the pointer
  # is back at the edge: the line is looked for from the pass on.
  echo "skip: the pointer passing the window's edge, whose leaving for the page Linux's shell doesn't hear."
  reveal_at_edge "the pointer at the window's edge didn't bring the sidebar out" "$passed"
else
  reveal_at_edge "the pointer at the window's edge didn't bring the sidebar out"
  [ "$(sed -n "/^browser: drive $passed: /,\$p" "$log" | grep -c '^browser: reveal shown$')" = 1 ] ||
    fail "the pointer passing the window's edge brought the sidebar out"
fi
check_reveal "with the sidebar out" shown
check_layout "with the sidebar out" inset
check_buttons "with the sidebar out" hidden
../tools/window-screenshot.sh "$pid" "$out/sidebar-reveal.png" || fail "no window to capture"
send move 200 300
send move 250 300
if [ "$(uname)" = Linux ]; then
  # On Linux the pointer over the page is Chromium's, which the shell doesn't hear, so the panel
  # has no hold zone there and goes as the pointer leaves it (platform/linux/reveal_window.rs).
  expect '^browser: reveal hidden$' "the pointer leaving the sidebar out didn't send it away"
else
  check_reveal "with the pointer in the hold zone past the sidebar out" shown
  send move 400 300
  expect '^browser: reveal hidden$' "the pointer past the sidebar out didn't send it away"
fi
check_reveal "after the sidebar went away"
reveal_at_edge "the pointer at the edge didn't bring the sidebar back out"
send click 93 24
toggle sidebar shown "a click on the sidebar button of the sidebar out"
check_reveal "with the sidebar docked from its sidebar button"
send key s cmd
toggle ui hidden "⌘S after docking the sidebar out"
reveal_at_edge "the pointer at the edge didn't bring the sidebar out again"
send key s cmd
toggle ui shown "⌘S with the sidebar out"
check_reveal "with the sidebar docked by ⌘S"
# Settings › Tabs & Sidebar's edge switch off (#1636): the pointer moving along the edge of the
# window with its sidebar hidden brings nothing out, and ⌘S still shows it. On again after.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar General")
../tools/window-screenshot.sh "$pid" "$out/settings-general.png" || fail "no window to capture"
send clickkey $(drawn "toolbar About")
settings_drew About
../tools/window-screenshot.sh "$pid" "$out/settings-about.png" || fail "no window to capture"
send clickkey $(drawn "toolbar Tabs & Sidebar")
settings_drew "Tabs & Sidebar"
../tools/window-screenshot.sh "$pid" "$out/settings-tabs.png" || fail "no window to capture"
in_sight "switch edge reveals sidebar" || fail "Settings didn't draw the edge's switch"
send clickkey $at
expect '^browser: edge reveals sidebar off$' "a click on the edge's switch didn't turn it off"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send key s cmd
toggle ui hidden "⌘S with the edge off"
send move 400 300
from=$((step + 1))
send move 3 300
send move 3 306
send move 3 312
# Past the wait the edge would have before the sidebar came out.
sleep 1
! sed -n "/^browser: drive $from: /,\$p" "$log" | grep -q '^browser: reveal \(waits\|shown\)$' ||
  fail "the pointer at the edge brought the sidebar out with the edge off"
send key s cmd
toggle ui shown "⌘S with the edge off"
send menu Settings…
expect '^browser: settings shown Tabs & Sidebar$' "Settings… didn't show Tabs & Sidebar again"
settings_drew "Tabs & Sidebar"
in_sight "switch edge reveals sidebar" || fail "Settings didn't draw the edge's switch"
send clickkey $at
expect '^browser: edge reveals sidebar on$' "a click on the edge's switch didn't turn it on again"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# ⌥ pressed and let go alone brings the hidden sidebar out with the pointer over the page, which
# keeps it out, and again sends it away. ⌥ held with ⌘, as for a shortcut, or let go after a key,
# as for a typed character, doesn't, and with the sidebar docked it does nothing.
# On Windows ⌥ is Alt, which the driver presses alone as winalt: its alt is the Mac's ⌥, Ctrl+Alt
# there, as AltGr is. The key after it there is Shift, as Alt+← goes back.
alone=alt
after_key() { send key left alt; }
if windows; then
  alone=winalt
  after_key() { send flags winalt shift; }
fi
docked=$((step + 1))
send flags $alone
send flags
send key s cmd
toggle ui hidden "⌘S before ⌥"
send move 600 300
send flags $alone
send flags
expect '^browser: reveal by option$' "⌥ pressed alone didn't bring the hidden sidebar out"
[ "$(since "$docked" | grep -c '^browser: reveal by option$')" = 1 ] ||
  fail "⌥ with the sidebar docked brought it out"
expect '^browser: reveal shown$' "⌥ didn't bring the sidebar's panel out"
check_reveal "with the sidebar out for ⌥" shown
../tools/window-screenshot.sh "$pid" "$out/sidebar-option.png" || fail "no window to capture"
# Only the panel ⌥ brought out, still out, goes away for it: one the pointer took away would come
# out again.
send move 650 320
send flags $alone
send flags
expect '^browser: reveal away by option$' \
  "the pointer over the page, or a second ⌥, sent the sidebar away"
expect '^browser: reveal hidden$' "a second ⌥ didn't take the sidebar's panel away"
not_alone=$((step + 1))
send flags $alone
send flags $alone cmd
send flags
if windows; then
  send flags cmd
  send flags cmd winalt
  send flags
fi
send flags $alone
after_key
send flags
# The same with the page focused, whose keys on X11 GPUI doesn't hear, so the core tells of its ⌥,
# and on the Mac come through the app again when the page doesn't use them.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page with the sidebar hidden"
send flags $alone
send flags
expect '^browser: reveal by option$' \
  "⌥ pressed alone with the page focused didn't bring the hidden sidebar out"
[ "$(since "$not_alone" | grep -c '^browser: reveal by option$')" = 1 ] ||
  fail "⌥ held with ⌘, let go after a key, or as AltGr brought the sidebar out"
expect '^browser: reveal shown$' "⌥ with the page focused didn't bring the sidebar's panel out"
send flags $alone
send flags
expect '^browser: reveal away by option$' "a second ⌥ with the page focused didn't send it away"
expect '^browser: reveal hidden$' "a second ⌥ with the page focused didn't take the panel away"
after_page_key=$((step + 1))
send flags $alone
after_key
send flags
send key s cmd
toggle page shown "⌘S from the page after ⌥"
none_since '^browser: reveal by option$' "$after_page_key" ||
  fail "⌥ let go after a key with the page focused brought the sidebar out"
# A shortcut the page hands back hides the row under the pointer until it moves, and the sidebar
# out, by the edge or by ⌥, hears that move in its own window, so its row under the pointer shows
# its hover (#2133). While it rests out, the browser takes about the CPU it takes docked: the two
# windows drawing the sidebar's list each asked the other for a frame, on and on, which took about
# 0.65 s of CPU in 4 s on GitHub's hosted Mac, against 0.1 s once fixed.
# hover_out <what>: the pointer on the first tab's row of the sidebar out draws its hover.
hover_out() {
  send move $(row 0)
  expect '^browser: hover tab [0-9]* ' "$1 didn't draw the hover of the row under the pointer"
}
# drawing_procs: "<seconds>\t<command line>" of each process of what draws. uBlock Origin works
# when it will, which isn't the windows' drawing measured here: its background page, in an
# extension's process, took about 0.8 s of 4 s on Wayland, and what it asks of the browser runs in
# Chromium's worker pools and IO threads, where it took 1.6 s of 4 s on Wayland while neither window
# drew a frame (#2400). On Linux the threads say what they are, so those are left out of every
# process; elsewhere only the extension's process is.
drawing_procs() {
  if windows; then
    powershell.exe -NoProfile -NonInteractive -Command "Get-CimInstance Win32_Process \
      -Filter \"Name = 'chrome.exe'\" | Where-Object { \$_.ExecutablePath -like '*\\build\\Browser\\*' -and \
      \$_.CommandLine -notlike '*--extension-process*' } | ForEach-Object {
      ('{0:F2}' -f ((\$_.UserModeTime + \$_.KernelModeTime) / 1e7)) + [char]9 + \$_.CommandLine }" | tr -d '\r'
  elif [ -d /proc ]; then
    for p in $(pgrep -f -- "$procs"); do
      line=$(tr '\0' ' ' < "/proc/$p/cmdline" 2> /dev/null)
      case $line in *--extension-process*) continue ;; esac
      for t in /proc/$p/task/*; do
        case $(cat "$t/comm" 2> /dev/null) in ThreadPool* | Chrome_IOThread | Chrome_ChildIOT) continue ;; esac
        sed 's/^.*) //' "$t/stat" 2> /dev/null
      done | awk -v hz="$(getconf CLK_TCK)" -v line="$line" '{ t += $12 + $13 } END { printf "%.2f\t%s\n", t / hz, line }'
    done
  else
    ps -Ao time=,args= | grep -F -e "$procs" | grep -v -e 'grep -F' -e --extension-process | awk '{
      s = 0; k = split($1, part, ":"); for (i = 1; i <= k; i++) s = s * 60 + part[i]
      $1 = ""; printf "%.2f\t%s\n", s, substr($0, 2) }'
  fi
}
# drawing_cpu: "<seconds> <processes> <kind>=<seconds>...", the CPU of what draws and each kind of
# process's share: the browser's own, the GPU's, the network service's, the pages' renderers, other
# utilities and anything else (#2502).
drawing_cpu() {
  drawing_procs | awk -F '\t' '{
    kind = "browser"
    if (match($2, /--type=[^ ]*/)) kind = substr($2, RSTART + 7, RLENGTH - 7)
    else if (index($2, "crashpad")) kind = "other"
    if (kind == "gpu-process") kind = "gpu"
    else if (kind == "utility" && index($2, "NetworkService")) kind = "network"
    else if (kind != "browser" && kind != "renderer" && kind != "utility") kind = "other"
    n++; t += $1; by[kind] += $1 }
    END { printf "%.2f %d", t, n; for (k in by) printf " %s=%.2f", k, by[k]; print "" }'
}
# rest_cpu [name]: $rest, the CPU in seconds what draws takes in 4 s with the pointer still, below
# the rows, $rest_procs the processes it counts, and $rest_kinds each kind's share, such as
# "browser 0.02 s, GPU 0.01 s". With a name, on the Mac, whose ps counts no threads apart, the
# browser's stacks over the same 4 s go in window-rest-<name>-sample.txt, every 10 ms: a burst
# inside the 4 s was over by the time a sample after the step could see it (#2502). Not in a
# subshell, which would lose send's step.
rest_cpu() {
  send move 100 600
  sleep 1 # the hover's fade out, before the window measured
  local from to sampler=
  if [ -n "${1:-}" ] && [ "$(uname)" = Darwin ]; then
    sample "$pid" 4 10 -file "$out/window-rest-$1-sample.txt" \
      > "$out/window-rest-$1-sample.log" 2>&1 &
    sampler=$!
  fi
  from=$(drawing_cpu)
  sleep 4 # the window measured
  to=$(drawing_cpu)
  [ -z "$sampler" ] || wait "$sampler" ||
    echo "window: the $1 rest's sample failed: $(tail -1 "$out/window-rest-$1-sample.log")"
  rest=$(printf '%s\n%s\n' "$from" "$to" | awk 'NR == 1 { from = $1 } NR == 2 { printf "%.2f", $1 - from }')
  rest_procs=$(echo "$to" | cut -d' ' -f2)
  rest_kinds=$(printf '%s\n%s\n' "$from" "$to" | awk '{
    for (i = 3; i <= NF; i++) { split($i, kv, "="); if (NR == 1) from[kv[1]] = kv[2]; else to[kv[1]] = kv[2] } }
    END {
      n = split("browser gpu network renderer utility other", kinds, " ")
      split("browser,GPU,network,renderers,other utilities,other", names, ",")
      for (i = 1; i <= n; i++) if (kinds[i] in to)
        printf "%s%s %.2f s", (said++ ? ", " : ""), names[i], to[kinds[i]] - from[kinds[i]] }')
}
# The sampling adds nothing the rest reads: sampled, docked read 0.00 s on the Mac (#2632).
rest_cpu docked
docked_cpu=$rest docked_kinds=$rest_kinds
send key s cmd
toggle page hidden "⌘S from the page before the sidebar out is hovered"
send move 400 300
reveal_at_edge "the pointer at the edge didn't bring the sidebar out after ⌘S from the page"
hover_out "the sidebar out by the edge"
../tools/window-screenshot.sh "$pid" "$out/sidebar-reveal-hover.png" || fail "no window to capture"
rest_cpu out
out_cpu=$rest
echo "window: at rest for 4 s, the browser's $rest_procs processes took ${docked_cpu} s of CPU" \
  "with the sidebar docked ($docked_kinds) and ${out_cpu} s with it out ($rest_kinds)"
if awk -v out="$out_cpu" -v docked="$docked_cpu" 'BEGIN { exit !(out > docked + 0.3) }'; then
  # Its age, for work that comes at a time after launch, as uBlock Origin's selfie did (#2502).
  up=$(ps -o etime= -p "$pid" | tr -d ' ')
  sampled=
  [ ! -s "$out/window-rest-out-sample.txt" ] ||
    sampled=", the browser's stacks over the rest in window-rest-out-sample.txt"
  fail "the sidebar out at rest took ${out_cpu} s of CPU in 4 s ($rest_kinds), against ${docked_cpu} s docked ($docked_kinds), the browser up $up$sampled"
fi
send move 600 300
expect '^browser: reveal hidden$' "the pointer past the sidebar out didn't send it away"
send key s cmd
toggle page shown "⌘S from the page with the sidebar in"
send key s cmd
toggle page hidden "⌘S from the page before ⌥ brings the sidebar out"
send flags $alone
send flags
expect '^browser: reveal by option$' "⌥ after ⌘S from the page didn't bring the hidden sidebar out"
expect '^browser: reveal shown$' "⌥ after ⌘S from the page didn't bring the sidebar's panel out"
hover_out "the sidebar out by ⌥"
../tools/window-screenshot.sh "$pid" "$out/sidebar-option-hover.png" || fail "no window to capture"
# ⌘S docks the panel in one update, under the pointer, so the layout waits for the frame that
# draws it and says the pointer is over it: on Linux the driver's layout can come before it.
send key s cmd
expect '^browser: sidebar hovered 1$' "⌘S from the page didn't dock the sidebar out by ⌥ under the pointer"
toggle page shown "⌘S from the page with the sidebar out by ⌥"
# With the sidebar hidden, a tab opening says so in a toast, as its row isn't seen. A second asked
# for while it's in place, as Arc's second ⌘↩ is, keeps it there and bumps it, unless motion is
# reduced; recorded on the Mac, and the window captured after, as a capture ends a recording on
# GitHub's hosted Mac. Its Show brings the sidebar back.
send key s cmd
toggle page hidden "⌘S from the page before opening a tab"
record new-tab-toast 6
for site_name in example.org example.com; do
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T with the sidebar hidden didn't open the command bar"
  send type $site_name
  send key return
  # The plate is told once, at the first toast's rest: the second keeps it where it is.
  [ "$site_name" = example.org ] && toast_step=$step
  expect "^browser: today [0-9][0-9]* $before\$" \
    "the tab for $site_name didn't open at the top of Today"
  expect '^browser: toast shown New Tab Created \[show\]$' \
    "a tab opened with the sidebar hidden, for $site_name, had no toast"
  expect '^browser: toast settled$' "the new tab's toast, for $site_name, didn't settle"
done
if [ "$(last 'reduce motion')" = on ]; then
  none_since '^browser: toast bumped$' || fail "under Reduce Motion, the toast asked for again bumped"
else
  logged '^browser: toast bumped$' || fail "the toast asked for again while in place didn't bump"
fi
# Show, the chip at the plate's right end: 13.5 in from it and about 50 wide. The plate is logged
# in the toast's window, whose width comes after it, at the content's right. The pointer goes on it
# before the recording's end is waited for, which outlasts the toast's 4.8 s, as a toast with the
# pointer on its buttons stays past its time; the window is captured with it there. The toast's
# window lets the pointer through until a move over its plate, which goes to the window under it,
# so the first move has it take the pointer and the toast hears the second, as a real one's next.
expect '^browser: toast plate [0-9]* [0-9]* [0-9]* [0-9]* in [0-9]* [0-9]*$' \
  "the new tab's toast didn't tell where its plate is" "$toast_step"
set -- $(grep '^browser: toast plate' "$log" | tail -1 | sed 's/^browser: toast plate //; s/ in / /')
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*\)x[0-9]*,.*/\1/')
show="$((content - $5 + $1 + $3 - 14 - 25)) $(($2 + $4 / 2))"
send move $show
first_move=$step
send move $show
expect '^browser: toast hovered true$' "the pointer at $show wasn't on the new tab's toast" "$first_move"
recorded
../tools/window-screenshot.sh "$pid" "$out/new-tab-toast.png" || fail "no window to capture"
send click $show
expect '^browser: toast button show$' "a click on the new tab's toast's Show at $show didn't take"
expect '^browser: sidebar shown$' "the new tab's toast's Show didn't bring the sidebar back"
check_layout "after a click on the new tab's toast"
# The sidebar's edge, the 6 pt right of its rows, drags its width between 164 and 500, Arc's, and
# stops at them. Dragged on past half the narrowest, it hides the sidebar at once, which keeps the
# width it had, and ⌘S brings it back. A double-click on the edge puts back 228.
send drag 225 400 325 400
expect '^browser: sidebar width 328$' "dragging the sidebar's edge didn't widen the sidebar"
check_layout "with the sidebar dragged wider" 328
send drag 325 400 700 400
expect '^browser: sidebar width 500$' "dragged far past its widest, the sidebar didn't stop at 500"
check_layout "with the sidebar at its widest" 500
../tools/window-screenshot.sh "$pid" "$out/sidebar-wide.png" || fail "no window to capture"
send drag 497 400 120 400
expect '^browser: sidebar width 164$' "dragged past its narrowest, the sidebar didn't stop at 164"
check_layout "with the sidebar at its narrowest" 164
../tools/window-screenshot.sh "$pid" "$out/sidebar-narrow.png" || fail "no window to capture"
# Clear of the window's edge, where the pointer would bring the hidden sidebar out.
send drag 161 400 50 400
expect '^browser: sidebar hidden$' "dragged on past its narrowest, the sidebar didn't hide"
none_since '^browser: sidebar width' || fail "the sidebar its edge hid changed its width"
check_layout "with the sidebar hidden by its edge" inset
check_buttons "with the sidebar hidden by its edge" hidden
send key s cmd
expect '^browser: sidebar shown$' "⌘S didn't bring back the sidebar its edge hid"
check_layout "with the sidebar its edge hid shown again" 164
send doubleclick 161 400
expect '^browser: sidebar width 228$' "a double-click on the sidebar's edge didn't put its width back"
none_since '^browser: command new-tab' || fail "a double-click on the sidebar's edge opened a tab"
check_layout "after a double-click on the sidebar's edge"

# The sidebar over a backdrop of black, grey 20, grey 60 and white (sRGB levels) is Dia's composite
# within 3 levels (docs/ux-spec.md › Window material), in the Mac's dark appearance and then its
# light one: Dia's dark material reads 24 / 32 / 45 / 104 there, under its tint of black 0.4, and
# its light one 137 / 146 / 165 / 243, under white 0.8, with the gradient over that. The blur comes
# from the window server, which a capture of the window alone leaves out, so this captures the
# screen, and only a check VM's screen, and its appearance, are the check's to change: check.sh
# runs it dark, and this turns it light for the light pass and back after.
# Colours the Space black from the theme picker's field, which is black at its middle.
black_space() {
  post rightclick 100 112
  expect "^browser: menu shown $(cased 'Change Space Icon…')" "a right-click on the Space's name showed no menu"
  send choose "$(cased 'Edit Theme Color…')"
  expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
  expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
  send clickkey 191.5 178.5
  expect '^browser: colored space [0-9]* 000000$' "the field's middle didn't colour the Space black"
  send key escape
  expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
  send move $page_point
}
material="not measured outside a check VM"
if [ "$(sysctl -n kern.hv_vmm_present)" = 1 ]; then
  # Black is neutral, so the field's gradient is faint and known: stops at 0.06 in dark and 0.09
  # in light, from black at the top to 0.25 · 255 at the bottom.
  black_space
  [ "$active" = 1 ] || fail "the app isn't active, so the window shows AppKit's inactive fill"
  srgb="/System/Library/ColorSync/Profiles/sRGB Profile.icc"
  material=
  off=
  opaque=
  # Each pass: its appearance, then each backdrop's grey and Dia's material over it.
  for pass in "dark 0:24 20:32 60:45 255:104" "light 0:137 20:146 60:165 255:243"; do
    tone=${pass%% *}
    if [ "$tone" = light ]; then
      light_shots=", $out/light.png, $out/light-menu.png, $out/appearance-dark.png"
      appearance false || fail "System Events didn't turn the Mac light"
      expect '^browser: window light true$' "the window didn't turn light with the Mac"
      ../tools/window-screenshot.sh "$pid" "$out/light.png" || fail "no window to capture"
      post rightclick 100 112
      expect "^browser: menu shown $(cased 'Change Space Icon…')" "a right-click on the Space's name showed no menu"
      ../tools/window-screenshot.sh "$pid" "$out/light-menu.png" || fail "no window to capture"
      send key escape
      expect '^browser: menu closed$' "escape didn't close the Space's menu"
      send move $page_point
    fi
    for pair in ${pass#* }; do
      level=${pair%:*}
      send backdrop "$level"
      expect "^browser: backdrop $level drawn" "the backdrop in grey $level didn't draw"
      # The window's frame and its screen's size, in points from the screen's top left.
      place=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
        grep -m1 "^browser: backdrop $level drawn" | sed 's/.* at //; s/[,x]/ /g; s/ on / /')
      [ "$(echo "$place" | wc -w)" -eq 6 ] || fail "the backdrop didn't say where the window is"
      shot="$out/material-$tone-$level.png"
      screencapture -x "$shot" || fail "screencapture couldn't capture the screen"
      sips -m "$srgb" "$shot" > /dev/null || fail "sips couldn't match $shot to sRGB"
      # A point of the empty sidebar, away from its text, and one of the backdrop beside the window.
      sidebar=$(pixel "$shot" $(echo "$place" | awk '{ print ($1 + 60) / $5, ($2 + 450) / $6 }'))
      backdrop=$(pixel "$shot" $(echo "$place" |
        awk '{ x = $1 >= 50 ? $1 - 50 : $1 + $3 + 50; print x / $5, ($2 + 450) / $6 }'))
      # Dia's composite there, 450 pt down: in dark 0.94 of its material under black 0.4, then 0.06
      # of the gradient; in light 0.91 of it under white 0.8, then 0.09. The material alone is ours
      # under our tint, black 0.25 or white 0.8, to tune the tint by.
      line=$(echo "${pair#*:} $place $sidebar $backdrop" | awk -v level="$level" -v tone="$tone" '{
        ours = ($8 + $9 + $10) / 3
        seen = ($11 + $12 + $13) / 3
        if (tone == "dark") {
          gradient = 0.06 * 63.75 * 450 / $5
          dia = 0.94 * 0.6 * $1 + gradient
          alone = (ours - gradient) / (0.94 * 0.749)
        } else {
          gradient = 0.09 * 63.75 * 450 / $5
          dia = 0.91 * (0.8 * 255 + 0.2 * $1) + gradient
          alone = ((ours - gradient) / 0.91 - 0.8 * 255) / 0.2
        }
        printf "%.0f (Dia %.0f), the material alone %.0f (Dia %d), the backdrop read as %.0f",
          ours, dia, alone, $1, seen
        if (ours - dia > 3 || dia - ours > 3) printf ", off"
        if (seen - level > 2 || level - seen > 2) printf ", capture off"
      }')
      echo "$tone material over $level: $line"
      case "$line" in *", off"* | *"capture off"*) off="$off $tone $level," ;; esac
      # Dia's reads 45 lighter over white than over black in dark, and 20 in light; an opaque
      # material, as Reduce Transparency draws it, reads the same over both.
      case $level in
      0) over_black=${line%% *} ;;
      255) [ $((${line%% *} - over_black)) -ge 10 ] || opaque="$opaque $tone," ;;
      esac
      material="$material$tone over $level $line; "
    done
  done
  send backdrop off
  # View › Appearance stands in for the Mac's: Dark turns the window dark while the Mac is light,
  # and Automatic gives it the Mac's light again.
  send menu "Appearance › Dark"
  expect '^browser: window light false$' "View › Appearance › Dark didn't turn the window dark"
  ../tools/window-screenshot.sh "$pid" "$out/appearance-dark.png" || fail "no window to capture"
  send menu "Appearance › Automatic"
  expect '^browser: window light true$' "View › Appearance › Automatic didn't give back the Mac's light"
  appearance true || fail "System Events didn't turn the Mac dark again"
  expect '^browser: window light false$' "the window didn't turn dark again with the Mac"
  [ -z "$opaque" ] ||
    fail "the sidebar read the same over black and white in${opaque%,}, so it's opaque: $material"
  # GitHub's hosted Mac draws AppKit's material behind the window darker than the Mac Dia was
  # measured on, 119 against 137 over black in light (#1554), so ours there is no measure of Dia's:
  # the run goes on, saying so, and the check VMs keep the comparison.
  if [ "${GITHUB_ACTIONS:-}" = true ]; then
    echo "skip: the sidebar against Dia's composite, as the hosted Mac's AppKit material isn't the one Dia was measured over"
  else
    [ -z "$off" ] || fail "the sidebar isn't Dia's composite within 3 levels over grey${off%,}: $material"
  fi
  material="over grey ${material%; }"
fi

# The Dock icon's dot shows the Space in front, and with Settings › General's "Dock icon takes the
# Space's color" off it stays green (docs/ux-spec.md › Dock dot), read from the image the app gives
# the Dock. The Space is black, which no default Space color is, and black over the dot's green
# with a color blend leaves it grey. In the Mac's Tinted and Clear icon styles macOS draws the icon
# in the style's colors and the app leaves it as it is.
dock="not on this OS"
if [ "$(uname)" = Darwin ]; then
  grep -q '^browser: colored space [0-9]* 000000$' "$log" || black_space
  # The last name the dot was drawn in, once it's $1 or macOS's own.
  dock_dot() {
    for _ in $(seq 100); do
      shown=$(sed -n 's/^browser: dock dot \([^ ]*\) in .*/\1/p' "$log" | tail -1)
      [ "$shown" = "$1" ] || [ "$shown" = system ] && return 0
      sleep 0.1
    done
    fail "the Dock's dot wasn't drawn $1 but ${shown:-never}"
  }
  # The dot's sRGB levels in the icon, saved at $1, as "r g b" in $icon. Not echoed for a $(),
  # whose subshell would count the driver's step for itself alone.
  dock_icon() {
    send dockicon "$1"
    icon=$(since | sed -n 's/^browser: dock icon //p' | tail -1)
  }
  green() { echo "$1" | awk '{ exit !($2 > $1 + 50 && $2 > $3 + 25) }'; }
  grey() { echo "$1" | awk '{ hi = $1; lo = $1; for (i = 2; i <= 3; i++) { if ($i > hi) hi = $i; if ($i < lo) lo = $i }; exit !(hi - lo <= 12) }'; }
  dock_dot 000000
  if [ "$shown" = system ]; then
    echo "skip: the Dock icon's dot, as this Mac's icon style draws it in its own colors"
    dock="left to macOS's icon style"
  else
    dock_icon "$out/dock-icon-black.png"
    black=$icon
    grey "$black" || fail "the Dock icon's dot in a black Space reads $black, not grey"
    send menu Settings…
    expect '^browser: settings shown ' "Settings… didn't show Settings for the Dock icon's switch"
    send clickkey $(drawn "toolbar General")
    settings_drew General
    in_sight "switch dock icon color" || fail "Settings didn't draw the Dock icon's switch"
    send clickkey $at
    expect '^browser: dock icon color off$' "a click on the Dock icon's switch didn't turn it off"
    send key w cmd
    expect '^browser: settings hidden$' "⌘W didn't hide Settings"
    dock_dot green
    dock_icon "$out/dock-icon-off.png"
    plain=$icon
    green "$plain" || fail "the Dock icon's dot with its switch off reads $plain, not green"
    send menu Settings…
    expect '^browser: settings shown General$' "Settings… didn't show General again"
    settings_drew General
    in_sight "switch dock icon color" || fail "Settings didn't draw the Dock icon's switch"
    send clickkey $at
    expect '^browser: dock icon color on$' "a click on the Dock icon's switch didn't turn it on again"
    send key w cmd
    expect '^browser: settings hidden$' "⌘W didn't hide Settings"
    dock_dot 000000
    dock_icon "$out/dock-icon-on.png"
    again=$icon
    grey "$again" || fail "the Dock icon's dot back on reads $again, not the Space's grey"
    dock="dot $black in a black Space, $plain with its switch off, $again on again"
    dock_shots=", $out/dock-icon-black.png, $out/dock-icon-off.png, $out/dock-icon-on.png"
  fi
elif windows; then
  # The taskbar button carries the same dot, and Settings names its switch for it (#2681). The
  # taskbar's overlay isn't read back, so this proves the switch is there and turns off and on.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings for the taskbar button's switch"
  send clickkey $(drawn "toolbar General")
  settings_drew General
  in_sight "switch dock icon color" || fail "Settings didn't draw the taskbar button's switch"
  send clickkey $at
  expect '^browser: dock icon color off$' "a click on the taskbar button's switch didn't turn it off"
  ../tools/window-screenshot.sh "$pid" "$out/taskbar-dot-switch.png" all || fail "no window to capture"
  send clickkey $at
  expect '^browser: dock icon color on$' \
    "a click on the taskbar button's switch didn't turn it on again"
  send key w cmd
  expect '^browser: settings hidden$' "Ctrl+W didn't hide Settings"
  echo "skip: the taskbar button's dot in the Space's color: the taskbar's overlay isn't read back yet, so only its switch is checked."
  dock="the taskbar button's switch off and on again in Settings"
  dock_shots=", $out/taskbar-dot-switch.png"
else
  echo "skip: the Dock icon's dot in the Space's color: Linux's host draws none."
fi

# Esc in full screen goes to the page first, so a page that keeps it stays full screen on one
# press. Two within 500 ms leave anyway, and so does one held 600 ms (Arc's).
printf '%s' '<title>keeps escape</title><p>Escape is mine</p><script>let n = 0;
onkeydown = e => { if (e.key == "Escape") { e.preventDefault(); document.title = "kept " + ++n; } };
</script>' > "$out/site/keeps-escape.html"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar for the page that keeps Esc"
send insert "${site}keeps-escape.html"
send key return
expect "^browser: loaded [0-9]* ${site}keeps-escape.html\$" "the page that keeps Esc didn't load"
keeps=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}keeps-escape.html\$|\1|p" "$log" | tail -1)
send click $page_point
send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen for the page that keeps Esc"
send key escape
expect "^browser: title $keeps kept 1\$" "the page didn't get Esc first in full screen"
none_since '^browser: window left full screen$' || fail "one Esc the page kept took the window out of full screen"
# The pair goes back to back, the page's answer to its first read after, as a slow runner can take
# 500 ms between two steps (#2009). It waits out the second press's 1 s (SECOND_PRESS in
# shell/keys.rs) from the press above first, which would otherwise take the pair's first press
# out of full screen as a second press of a key the page kept.
sleep 1.1
send key escape
first=$step
send key escape
expect "^browser: title $keeps kept 2\$" "the page didn't get the first of two Escs" "$first"
expect '^browser: escape twice leaves full screen$' "a second Esc at once didn't leave full screen"
expect '^browser: window left full screen$' "Esc twice didn't take the window out of full screen"
none_since "^browser: title $keeps kept 3\$" || fail "the second Esc that left full screen reached the page too"
# Held: the Mac's driver sends the keyboard's repeat, which says it's held; off the Mac a held key's
# repeats come as presses, so a held Esc leaves there as one pressed twice.
if [ "$(uname)" = Darwin ]; then
  send click $page_point
  send fullscreen
  expect '^browser: window entered full screen$' "the window didn't go full screen again"
  # The shell hears of full screen after the window does, and its note says it has: an Esc before
  # that is no press it counts, so the repeat after it isn't one held.
  expect '^browser: toast shown To exit full screen' "the window full screen again didn't say how to leave"
  send keydown escape
  expect "^browser: title $keeps kept [0-9]*\$" "the page didn't get the held Esc's press"
  sleep 0.7 # Esc held past the key repeat's delay
  send keydown escape repeat
  expect '^browser: escape held leaves full screen$' "Esc held 700 ms didn't leave full screen"
  send keyup escape
  expect '^browser: window left full screen$' "Esc held didn't take the window out of full screen"
else
  echo "skip: Esc held in full screen, as off the Mac the host can't tell a held key's repeats from presses, which leave as twice."
fi

# With two windows, only the key window's hidden sidebar comes out at its edge, as Arc 1.171's: a
# window behind leaves its edge to the pointer on its way to the one in front (#1402). The driver
# sends the window behind its moves without bringing it forward, as dialogs' hover card behind
# shows they reach it; once in front, the same moves bring its sidebar out. Linux's driver can't
# drive a window behind another.
if [ "$(uname)" = Linux ]; then
  echo "skip: the edge of a window behind another, which Linux's driver can't drive without bringing it forward."
else
  send click 110 400
  send key s cmd
  toggle ui hidden "⌘S before the window behind's edge"
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
  second_window=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  # The driver counts every window opened, closed ones too.
  driven_second=$(grep -c '^browser: window opened ' "$log")
  # A new window opens ⌘T's bar, which keeps its window's keyboard until escape closes it.
  expect_late '^browser: command bar shown$' "⌘N's window didn't open the command bar"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close ⌘N's command bar"
  send window 1 behind
  behind=$step
  send move 400 300
  send move 3 300
  send move 3 306
  ../tools/window-screenshot.sh "$pid" "$out/reveal-behind.png" all || fail "no window to capture"
  # The window's wait starts as the pointer moves in its edge, logged then, so none by the time the
  # driver brings it forward means it never started.
  send window 1
  sed -n "/^browser: drive $behind: /,/^browser: drive $step: /p" "$log" |
    grep -q '^browser: reveal \(waits\|shown\)$' &&
    fail "the pointer at the edge of the window behind brought its sidebar out"
  # Windows' pointer is real: it rests at the edge as the window comes in front, which hears it
  # there as a hand's, so the sidebar can come out with the step that brings the window forward.
  in_front=$((step + 1))
  windows && in_front=$step
  reveal_at_edge "the pointer at the edge of the first window, in front, didn't bring its sidebar out" \
    "$in_front"
  check_reveal "with the first window's sidebar out in front of the second" shown
  send move 400 300
  expect '^browser: reveal hidden$' "the pointer past the first window's sidebar out didn't send it away"
  send window "$driven_second"
  send key w cmd shift
  expect "^browser: window closed $second_window\$" "⇧⌘W didn't close the second window"
  send window 1
  send key s cmd
  toggle ui shown "⌘S after the window behind's edge"
fi

# ⌘N with the page's keyboard, in full screen, opens a window that goes full screen too, then opens
# ⌘T's command bar in it, whose URL loads there (Arc). The window it came from shows what it did.
# Last, as what it leaves would reach the steps after: its page stays in the Space's Today, where
# closing a tab shows it, and its full-screen Space goes as it closes, with a slide that macOS takes
# its time over in a check VM, and a full-screen change asked for in it ends at once and is then
# turned down (#889).
{
  send click $page_point
  send fullscreen
  expect '^browser: window entered full screen$' "the window didn't go full screen for ⌘N"
  send key n cmd
  expect '^browser: command new-window via ' "⌘N in full screen with the page's keyboard didn't run New Window"
  expect '^browser: window opened [0-9]' "⌘N in full screen didn't open a window"
  full_new=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  expect_late '^browser: new window first frame at [0-9.]* ms$' "⌘N's window in full screen drew no frame"
  expect_late '^browser: new window went full screen$' "⌘N's window from full screen didn't go full screen"
  expect_late '^browser: command bar shown$' "⌘N's full-screen window didn't open the command bar"
  # The bar takes the note on leaving full screen away as it opens, settled or not.
  expect_late '^browser: toast \(settled\|hidden\)$' "the new full-screen window's note didn't show"
  ../tools/window-screenshot.sh "$pid" "$out/new-window-full-screen.png" || fail "no window to capture"
  send insert "${site}notes.txt"
  send key return
  expect "^browser: window $full_new shows [1-9][0-9]*\$" "the URL typed in ⌘N's command bar didn't show in its window"
  # Its own full screen ends as the first's does, by the green button, and it closes.
  send key w cmd shift
  expect "^browser: window closed $full_new\$" "⇧⌘W didn't close ⌘N's full-screen window"
  send window 1
}
exec 3>&-
quit
echo "edge: $edge"
echo "ok: the page view keeps its margins and the window's buttons stay at Arc's through a resize and full screen, widened, $gap, full screen says how to leave and a page's doesn't, a full-screen page takes the window with it and escape or the page leaving brings both back, the page resized once each way, the green button takes a full-screen page out with the window and brings back the sidebar and the card, from where the page's full screen takes the window again, ${displays:+$displays, }the command bar opens in full screen and keeps the keyboard when a page commits under it, then gives it to the new page. Drags on the window's background move it and drags on its controls don't. A double-click on the header $zoom, and one on the empty sidebar opens a new tab. ⌘S from the page and the UI, and the sidebar button, hide the sidebar and the window's buttons, the card takes its place through a resize, and ⌘S brings them back. The pointer kept at the window's edge brings the hidden sidebar out over the page, one passing it doesn't, and the pointer past it sends it away, and the sidebar button and ⌘S dock it from there. ⌥ pressed alone, with the UI or the page focused, brings the hidden sidebar out and sends it away again, and ⌥ held with ⌘ or a key doesn't. A tab opened with the sidebar hidden says so in a toast, a second one bumps it, and a click on it brings the sidebar back. The sidebar's edge drags its width between 164 and 500, hides it dragged on past the narrowest, and a double-click puts it back. A page that hasn't drawn shows black or white as websites are told the appearance is:${unpainted_seen%,}. Page full screen, ms from the page's request, median of 6 going in, 3 leaving by the page, 3 by escape: going in ${enter_ms}leaving ${exit_ms}by escape ${escape_ms%, }. Scrolling: $timing. Material: $material. Dock icon: $dock. Windows: $out/fullscreen-note.png, $out/fullscreen.png, $out/page-fullscreen.png, $out/green-button.png, $out/green-button-again.png, ${display_shots:-}$out/sidebar-hidden.png, $out/sidebar-reveal.png, $out/sidebar-option.png, $out/new-tab-toast.png, $out/sidebar-wide.png, $out/sidebar-narrow.png, $out/before-paint-light.png, $out/before-paint-dark.png, $out/resize-gap.png${light_shots:-}${dock_shots:-}"
