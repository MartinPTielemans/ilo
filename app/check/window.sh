# The page view keeps its margins and GPUI's view fills the window through a resize and full screen,
# and the window's buttons stay in the header row. A page that goes full screen takes the window
# with it, and escape or the page leaving brings both back, the page resized once each way, straight
# to its new size, and its leaving reported once, timed from the page's request to the window's and
# the UI's steps, going in and out. The window leaving full screen by its green button takes the page
# out with it and brings back the sidebar and the card, and the page's full screen takes the window
# again after. A full-screen window's display going away keeps the window and the page together,
# both full screen or both out, and escape and the page's full screen work after; going away while
# the page takes the window full screen leaves both windowed. The footer's menu
# flips up at the screen's bottom in full screen.
# The command bar opens in full screen and keeps the keyboard when a page commits under it, then
# gives the keyboard to the new page, and scrolling a long page doesn't stutter. Dragging the
# window's background moves the window, and the buttons, URL field and rows keep their presses. A
# double-click there zooms it, and one in the
# sidebar below the rows opens a new tab. ⌘S, with the page or the UI focused, and the sidebar
# button hide the sidebar and the window's buttons, and the card takes its place, through a resize;
# ⌘S brings them back, and the core hears the page moves for each motion and is still after it. The
# pointer kept at the window's edge brings the hidden sidebar out, and one passing it doesn't. A tab
# opened with the sidebar hidden says so in a toast, and a click on it brings the sidebar back. The
# sidebar's edge drags its width between 164 and 500, stopping at them, and dragged on past half the
# narrowest it hides the sidebar, keeping its width; a double-click on it puts the width back. A
# page that hasn't drawn yet shows the card's colour, not white. In a check VM, the sidebar over grey
# backdrops is Dia's composite, dark and then light, with the Mac's appearance, and View ›
# Appearance turns the light window dark and back.

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
  requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
</script>
HTML
)
log="$out/window.log"
step=0
rm -f "$saved" "$launch_frame"
serve
url="data:text/html;base64,$page#${site}wait"
BROWSER_DRIVE=1 BROWSER_URL="$url" "$browser" < "$out/drive" > "$log" 2>&1 &
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

# A page shows the card's colour until its first frame, never white, and its tab is loading from the
# start. The page's w opens one whose server never answers, so it never commits, and its URL, which
# a page asked for, stays hidden until it does.
first=$(today)
send click $page_point
send key w
expect "^browser: tab [0-9]* opened by $first in the foreground\$" "the page didn't open a tab"
tab=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: page $tab back 0 forward 0 loading 1 \$" \
  "a tab opened to a page that hasn't come doesn't say it's loading"
# Whether the colour "r g b" is the card's (tokens.toml › shell › card): dark on a dark window, as
# the Mac's check VM runs it, and on a light one, as Linux's is, white at 0.7 over any colour, but
# never the white of a page.
card_color() {
  [ "$(grep '^browser: window light ' "$log" | tail -1)" = "browser: window light true" ] ||
    { dark "$1"; return; }
  set -- $1
  [ "$1" -ge 179 ] && [ "$2" -ge 179 ] && [ "$3" -ge 179 ] && [ "$1 $2 $3" != "255 255 255" ]
}
for _ in $(seq 20); do
  ../tools/window-screenshot.sh "$pid" "$out/before-paint.png" || fail "no window to capture"
  color=$(pixel "$out/before-paint.png" 0.6 0.5)
  card_color "$color" && break
done
card_color "$color" || fail "a page that hasn't drawn shows $color, not the card's colour"
send key w cmd
expect_tabs "$first" "$first" "after closing the tab of the page that hasn't come"
send click $page_point

# 300 trackpad events of 10 points, 8 ms apart. X11's events carry no distance, so the Linux driver
# turns the wheel, which Chromium scrolls 120 points a notch there: 25 notches.
scroll="10 300"
[ "$(uname)" = Linux ] && scroll="1 25"
send scroll $page_point $scroll
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
# The window's resize border, about 3 outside its edge (Dia), is AppKit's to track, as for any
# resizable window. Whether AppKit takes a drag the driver posts there only a Mac's run tells: a
# drag from 2 outside the right edge either widens the window by 100 or leaves it, and the run
# says which.
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
../tools/window-screenshot.sh "$pid" "$out/before-green-button.png" || fail "no window to capture"
sidebar_at="$(awk -v w="$width" 'BEGIN { print 110 / w }') 0.6"
sidebar=$(pixel "$out/before-green-button.png" $sidebar_at)
# The sidebar in capture $1 has its colour from before, within 8 in each channel.
sidebar_back() {
  sidebar_now=$(pixel "$1" $sidebar_at)
  echo "$sidebar_now $sidebar" |
    awk '{ for (i = 1; i <= 3; i++) if ($i - $(i + 3) > 8 || $(i + 3) - $i > 8) exit 1 }'
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
../tools/window-screenshot.sh "$pid" "$out/green-button.png" || fail "no window to capture"
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
../tools/window-screenshot.sh "$pid" "$out/green-button-again.png" || fail "no window to capture"
sidebar_back "$out/green-button-again.png" ||
  fail "after the page's next full screen the sidebar is $sidebar_now, not $sidebar: the page still covers it"

# The page's window full screen on a second display, the main one, as for a video on an external
# monitor, and the display going away, then and again as the window leaves full screen: the window
# and the page stay together. AppKit keeps the window full screen on the other display, with the
# page, or takes it out, and the page with it, and the sidebar and the card come back. From there
# escape and the page's full screen work. The check VM's AppKit keeps the window full screen, and
# finishes a way out it started, but on the owner's Mac it took the window out without finishing,
# and the page stayed laid out full screen, for good (#415).
second_display() {
  send display add 1280 800 main
  added=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: display ')
  [ "$added" = "browser: display added at 0 0 1280 800" ] ||
    fail "no second display for the page's full screen: $added"
  send place 40 40 1000 700
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
if [ "$(uname)" = Linux ]; then
  echo "skip: second display: the Linux driver can't add one yet."
else
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

# A page's full screen timed three times each way, entered by a key as a video's button is and left
# by the page, as its button leaves, and then by escape. The steps run from the core's line to the
# window's and the UI's, and the checks wait for the last of them.
timed_from=$(($(wc -l < "$log") + 1))
for _ in 1 2 3; do
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen to be timed"
  full_screen_done enter
  send key x
  expect '^browser: page left full screen$' "exitFullscreen didn't take the page out of full screen to be timed"
  full_screen_done exit
done
timed_escape=$(($(wc -l < "$log") + 1))
for _ in 1 2 3; do
  send key f
  expect '^browser: page entered full screen$' "the page didn't go full screen to be timed"
  full_screen_done enter
  send key escape
  expect '^browser: page left full screen$' "escape didn't take the page out of full screen to be timed"
  full_screen_done exit
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
drag 600 5 moves "the card's top margin"
drag $((width - 8)) 300 moves "the card's right margin"
# A double-click there does what System Settings › Desktop & Dock says a title bar's does: zoom,
# unless it's set to something else.
action=$(defaults read -g AppleActionOnDoubleClick 2>/dev/null || echo Maximize)
if [ "$action" = Maximize ]; then
  send doubleclick 115 24
  expect '^browser: window zoomed 1$' "a double-click on the header didn't zoom the window"
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
send move 3 300
send move 3 306
expect '^browser: reveal shown$' "the pointer at the window's edge didn't bring the sidebar out"
[ "$(sed -n "/^browser: drive $passed: /,\$p" "$log" | grep -c '^browser: reveal shown$')" = 1 ] ||
  fail "the pointer passing the window's edge brought the sidebar out"
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
send move 3 300
send move 3 306
expect '^browser: reveal shown$' "the pointer at the edge didn't bring the sidebar back out"
send click 93 24
toggle sidebar shown "a click on the sidebar button of the sidebar out"
check_reveal "with the sidebar docked from its sidebar button"
send key s cmd
toggle ui hidden "⌘S after docking the sidebar out"
send move 3 300
send move 3 306
expect '^browser: reveal shown$' "the pointer at the edge didn't bring the sidebar out again"
send key s cmd
toggle ui shown "⌘S with the sidebar out"
check_reveal "with the sidebar docked by ⌘S"
# ⌥ pressed and let go alone brings the hidden sidebar out with the pointer over the page, which
# keeps it out, and again sends it away. ⌥ held with ⌘, as for a shortcut, or let go after a key,
# as for a typed character, doesn't, and with the sidebar docked it does nothing.
send flags alt
send flags
none_since '^browser: reveal by option$' || fail "⌥ with the sidebar docked brought it out"
send key s cmd
toggle ui hidden "⌘S before ⌥"
send move 600 300
send flags alt
send flags
expect '^browser: reveal by option$' "⌥ pressed alone didn't bring the hidden sidebar out"
expect '^browser: reveal shown$' "⌥ didn't bring the sidebar's panel out"
check_reveal "with the sidebar out for ⌥" shown
../tools/window-screenshot.sh "$pid" "$out/sidebar-option.png" || fail "no window to capture"
# Only the panel ⌥ brought out, still out, goes away for it: one the pointer took away would come
# out again.
send move 650 320
send flags alt
send flags
expect '^browser: reveal away by option$' \
  "the pointer over the page, or a second ⌥, sent the sidebar away"
expect '^browser: reveal hidden$' "a second ⌥ didn't take the sidebar's panel away"
send flags alt
send flags alt cmd
send flags
none_since '^browser: reveal by option$' || fail "⌥ held with ⌘ brought the sidebar out"
send flags alt
send key left alt
send flags
none_since '^browser: reveal by option$' || fail "⌥ let go after a key brought the sidebar out"
# The same with the page focused, whose keys on X11 GPUI doesn't hear, so the core tells of its ⌥,
# and on the Mac come through the app again when the page doesn't use them.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page with the sidebar hidden"
send flags alt
send flags
expect '^browser: reveal by option$' \
  "⌥ pressed alone with the page focused didn't bring the hidden sidebar out"
expect '^browser: reveal shown$' "⌥ with the page focused didn't bring the sidebar's panel out"
send flags alt
send flags
expect '^browser: reveal away by option$' "a second ⌥ with the page focused didn't send it away"
expect '^browser: reveal hidden$' "a second ⌥ with the page focused didn't take the panel away"
send flags alt
send key left alt
send flags
none_since '^browser: reveal by option$' ||
  fail "⌥ let go after a key with the page focused brought the sidebar out"
send key s cmd
toggle page shown "⌘S from the page after ⌥"
# With the sidebar hidden, a tab opening says so in a toast, as its row isn't seen, and its Show
# brings the sidebar back.
send key s cmd
toggle page hidden "⌘S from the page before opening a tab"
before=$(today)
send key t cmd
expect '^browser: command bar shown$' "⌘T with the sidebar hidden didn't open the command bar"
send type example.org
send key return
expect "^browser: today [0-9][0-9]* $before\$" "the tab for example.org didn't open at the top of Today"
expect '^browser: toast shown New Tab Created \[show\]$' \
  "a tab opened with the sidebar hidden had no toast"
expect '^browser: toast settled$' "the new tab's toast didn't settle"
../tools/window-screenshot.sh "$pid" "$out/new-tab-toast.png" || fail "no window to capture"
# A click on Show, the chip at the plate's right end: 13.5 in from it and about 50 wide. The plate is
# logged in the toast's window, whose width comes after it, at the content's right.
expect '^browser: toast plate [0-9]* [0-9]* [0-9]* [0-9]* in [0-9]* [0-9]*$' \
  "the new tab's toast didn't tell where its plate is"
set -- $(grep '^browser: toast plate' "$log" | tail -1 | sed 's/^browser: toast plate //; s/ in / /')
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*\)x[0-9]*,.*/\1/')
show="$((content - $5 + $1 + $3 - 14 - 25)) $(($2 + $4 / 2))"
send move $show
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
material="not measured outside a check VM"
if [ "$(sysctl -n kern.hv_vmm_present)" = 1 ]; then
  # The theme picker's field is black at its middle, and black is neutral, so its gradient is
  # faint and known: stops at 0.06 in dark and 0.09 in light, from black at the top to 0.25 · 255
  # at the bottom.
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
  [ "$active" = 1 ] || fail "the app isn't active, so the window shows AppKit's inactive fill"
  srgb="/System/Library/ColorSync/Profiles/sRGB Profile.icc"
  material=
  off=
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
  [ -z "$off" ] || fail "the sidebar isn't Dia's composite within 3 levels over grey${off%,}: $material"
  material="over grey ${material%; }"
fi
exec 3>&-
quit
echo "edge: $edge"
echo "ok: the page view keeps its margins and the window's buttons stay at Arc's through a resize and full screen, full screen says how to leave and a page's doesn't, a full-screen page takes the window with it and escape or the page leaving brings both back, the page resized once each way, the green button takes a full-screen page out with the window and brings back the sidebar and the card, from where the page's full screen takes the window again, a full-screen window's display going away, in full screen and as it leaves, keeps the window and the page together, and going away as the page takes the window full screen leaves both windowed, the command bar opens in full screen and keeps the keyboard when a page commits under it, then gives it to the new page. Drags on the window's background move it and drags on its controls don't. A double-click on the header $zoom, and one on the empty sidebar opens a new tab. ⌘S from the page and the UI, and the sidebar button, hide the sidebar and the window's buttons, the card takes its place through a resize, and ⌘S brings them back. The pointer kept at the window's edge brings the hidden sidebar out over the page, one passing it doesn't, and the pointer past it sends it away, and the sidebar button and ⌘S dock it from there. ⌥ pressed alone, with the UI or the page focused, brings the hidden sidebar out and sends it away again, and ⌥ held with ⌘ or a key doesn't. A tab opened with the sidebar hidden says so in a toast, and a click on it brings the sidebar back. The sidebar's edge drags its width between 164 and 500, hides it dragged on past the narrowest, and a double-click puts it back. A page that hasn't drawn shows the card's colour ($color). Page full screen, ms from the page's request, median of 6 going in, 3 leaving by the page, 3 by escape: going in ${enter_ms}leaving ${exit_ms}by escape ${escape_ms%, }. Scrolling: $timing. Material: $material. Windows: $out/fullscreen-note.png, $out/fullscreen.png, $out/page-fullscreen.png, $out/green-button.png, $out/green-button-again.png, $out/display-gone.png, $out/display-gone-again.png, $out/display-gone-leaving.png, $out/display-gone-leaving-again.png, $out/display-gone-entering.png, $out/display-gone-entering-again.png, $out/sidebar-hidden.png, $out/sidebar-reveal.png, $out/sidebar-option.png, $out/new-tab-toast.png, $out/sidebar-wide.png, $out/sidebar-narrow.png, $out/before-paint.png${light_shots:-}"
