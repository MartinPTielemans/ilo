# check.sh runs this at 460.
# Every Space switch, by ⌃1 and ⌃2 and by the switcher's dots, and every switch between a Space's
# tabs by a click on its row, shows a page in the page area at every frame: the one it leaves until
# the one it shows has drawn, and never the empty page area between them, for 60 to 230 ms on
# GitHub's hosted Mac (#1438). A burst of the window's pictures (tools/window-frames.sh) looks at
# the page area in each: one flat colour is no page. Its pages are never flat, the first striped and
# the others example.com's and example.org's text on their backgrounds.
# On the Mac a page a switch lets go stays in the window, out of sight, until 250 ms after the last
# let-go, as taking it out at once left the page area empty for a frame (#2248). With an engine
# whose core does that, the second Space has two pages that move, a background scrolling under
# their stripes, which the Space switches show, and ten ⌃Tabs back to back between them show a page
# throughout too. Then the pages let go leave the window together, leaving only the page shown, and
# in those 250 ms a pointer move after a ⌃Tab, with the pointer resting over the page area, reaches
# only the page shown, a click and a key reach the page shown, ⌘L opens the command bar, ⇧-click
# opens a link in Peek, full screen keeps the page shown, the tab of a page let go closes, and a
# quit ends the app.
# The pictures need the Mac's ScreenCaptureKit, check.sh's Xvfb or tools/headless.sh's sway session
# on Linux, or a CI runner's screen on Windows; elsewhere the run says it can't take them.
page="data:text/html,<title>ready</title><body onkeydown=\"document.title='key '+event.key\" style=\"margin:0;height:100vh;background:repeating-linear-gradient(45deg,%23cc3333 0 24px,%23ffffff 24px 48px)\">"
log="$out/switch-frames.log"
frames="$out/switch-frames.txt"
marks="$out/switch-marks.txt"
stop="$out/switch-frames.stop"
step=0
rm -f "$saved" "$launch_frame" "$stop"
: > "$marks"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the striped page didn't draw"
first=$(space)
a=$(today)
# Whether the engine keeps the pages let go, from the line its core logs as they leave, in the
# library the running app mapped: a dev engine's dylibs can stay in build/ after a Release build,
# which the app doesn't load, and a check VM's guest has the app but no core. With the dev engine
# the core is in libchrome_dll.dylib, else in the framework's own library.
held=
if [ "$(uname)" = Darwin ]; then
  mapped=$(lsof -p "$pid" -Fn 2> /dev/null | sed -n 's/^n//p')
  engine_library=$(printf '%s\n' "$mapped" | grep -m1 '/libchrome_dll\.dylib$') ||
    engine_library=$(printf '%s\n' "$mapped" |
      grep -m1 -E '/Contents/Frameworks/[^/]*Framework\.framework/Versions/[^/]*/[^/]*Framework$') ||
    fail "the app ($pid) has mapped neither libchrome_dll.dylib nor its framework's library, so whether its engine keeps the pages let go is unknown"
fi
if [ "$(uname)" = Darwin ] &&
  LC_ALL=C grep -qaF "switch's pages let go left the window" "$engine_library"; then
  held=1
elif [ "$(uname)" = Darwin ]; then
  echo "skip: switch-frames' steps for the 250 ms after a let-go: this engine takes the page a" \
    "switch left out of the window at once (#2248)."
fi
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: switch-frames. Browser wasn't the active app."
  exit 0
fi
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
width=${content%x*}
height=${content#*x}
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
send type frames
send key return
open_from_bar example.org
o=$tab
if [ -n "$held" ]; then
  # The page that moves, with a link to itself in its bottom right 200 by 100 for Peek, which is
  # the card's bottom right, 10 in from the window's edges. It counts the pointer's moves in its
  # title.
  serve
  printf '%s' '<title>loading</title><style>
@keyframes scroll { to { background-position: 96px 0, 0 0 } }
body { margin: 0; height: 100vh; animation: scroll 0.5s linear infinite;
  background: repeating-linear-gradient(-45deg, #3355cc 0 24px, transparent 24px 48px),
    linear-gradient(90deg, #ffffff, #ffcc33) }
a { position: fixed; right: 0; bottom: 0; width: 200px; height: 100px; font: 24px system-ui }
</style><a href="switch-moving.html?peek">Peek</a>
<script>
let moves = 0;
onmousemove = () => document.title = "moved " + ++moves;
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "moving"));
</script>' > "$out/site/switch-moving.html"
  # Opens the page that moves at $1 from the command bar, as $tab.
  open_moving() {
    before=$(today)
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send type "${site}switch-moving.html$1"
    send key return
    expect "^browser: today [0-9][0-9]* $before\$" "the page that moves didn't open at the top of Today"
    tab=$(today | cut -d' ' -f1)
    expect "^browser: title $tab moving\$" "the page that moves didn't draw"
  }
  open_moving "?one"
  m=$tab
  open_moving "?two"
  n=$tab
fi
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
open_from_bar example.com
c=$tab
# Today's rows, newest first: example.com's over the striped page's.
[ "$(today)" = "$c $a" ] || fail "Today is \"$(today)\", not example.com's tab over the striped page's"

# The page area, 16 in from the card's edges: past the sidebar's 228, and 10 from the window's
# other sides.
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
mark() { echo "$(now) $*" >> "$marks"; }
# shellcheck disable=SC2086 # numbers
../tools/window-frames.sh "$pid" 120 "$stop" 244 26 $((width - 270)) $((height - 52)) \
  "$out/switch-flat" > "$frames" 2> "$out/switch-frames-capture.log" &
capture=$!
for _ in $(seq 600); do
  grep -q '^capturing' "$frames" && break
  kill -0 "$capture" 2> /dev/null || break
  sleep 0.1
done
if ! grep -q '^capturing' "$frames"; then
  why=$(tail -1 "$out/switch-frames-capture.log")
  case "$why" in
  "Reads only"* | "Copies the screen only"*)
    exec 3>&-
    quit
    echo "skip: switch-frames, which can't take the window's pictures here: $why"
    exit 0
    ;;
  esac
  fail "the window's pictures didn't start: $why"
fi
# A second of the page before any switch, which the pictures must see as one.
sleep 1
mark start
for i in 1 2 3 4; do
  mark "ctrl2 $i"
  send key 2 ctrl
  expect "^browser: space $second in " "⌃2 didn't show the second Space"
  sleep 0.7
  mark "ctrl1 $i"
  send key 1 ctrl
  expect "^browser: space $first in " "⌃1 didn't show the first Space"
  sleep 0.7
done
for i in 1 2; do
  mark "dot2 $i"
  send click "$(sed -n "s/^browser: switcher dot $second in window [0-9]* at //p" "$log" | tail -1)"
  expect "^browser: space $second in " "the second dot didn't show the second Space"
  sleep 0.7
  mark "dot1 $i"
  send click "$(sed -n "s/^browser: switcher dot $first in window [0-9]* at //p" "$log" | tail -1)"
  expect "^browser: space $first in " "the first dot didn't show the first Space"
  sleep 0.7
done
for i in 1 2; do
  mark "row1 $i"
  send click $(row 1)
  expect "^browser: selected $a in " "a click on the striped page's row didn't show it"
  sleep 0.7
  mark "row0 $i"
  send click $(row 0)
  expect "^browser: selected $c in " "a click on example.com's row didn't show it"
  sleep 0.7
done
# ⌃Tab as a hand's quick one types it, which the pictures are for: ⌃ goes up just after the Tab, so
# it switches once, on the press, shows nothing over the page, and ends the walk it starts, so the
# next goes back to the tab it left. The driver's key holds ⌃ only for its own key, so flags lets it
# go. The key is posted, flags straight behind it, as the driver runs its commands in order: a send's
# wait for the key's line took long enough that ⌃, held past [tab-switcher.motion] delay (0.16 s),
# showed the Recent Tabs Switcher in half the goes. What the key did is read from its step, $tapped,
# as the switch's lines can come before flags' own.
ctrl_tab() {
  post key tab ctrl
  tapped=$step
  send flags
  grep -q "^browser: drive $tapped: can't do that" "$log" && fail "the driver can't do ⌃Tab"
  [ -n "${first_tap:-}" ] || first_tap=$tapped
}
# None of the ⌃Tabs since the first held ⌃ long enough to show the Recent Tabs Switcher, whose
# window over the page would be in the pictures and take the keyboard from the browser window.
quick_ctrl_tabs() {
  none_since '^browser: tab switcher lists ' "$first_tap" ||
    fail "a quick ⌃Tab $1 showed the Recent Tabs Switcher, as ⌃ went up after its delay"
}
if [ -n "$held" ]; then
  mark "ctrl2 moving"
  send key 2 ctrl
  expect "^browser: space $second in " "⌃2 didn't show the second Space"
  # Its page in the pictures for as long as each switch before had.
  sleep 0.7
  # One mark for the ten, as each alone is too short for enough pictures. Each lets ⌃ go, so they go
  # back and forth between the Space's two tabs shown last, the pages that move.
  mark ctrltabs
  for i in 1 2 3 4 5 6 7 8 9 10; do
    ctrl_tab
    expect "^browser: selected [0-9][0-9]* in " "⌃Tab $i in a run of them didn't show a tab" "$tapped"
  done
  # The last switch's page in the pictures for as long as each switch before had.
  sleep 0.7
fi
mark stop
touch "$stop"
wait "$capture" || fail "the window's pictures failed: $(tail -1 "$out/switch-frames-capture.log")"
if [ -n "$held" ]; then
  quick_ctrl_tabs "in the pictures"
  leaving="^browser: switch's pages let go left the window"
  # The pages let go since the let-go at step $went have left the window, leaving the page shown,
  # after step $2 began, or the step wasn't within the 250 ms, which a slow Mac can take to send.
  left() {
    expect "$leaving, 1 views in it\$" "the pages let go didn't leave the window $1, or others stayed" \
      "$went"
    since "$went" | awk -v step="^browser: drive $2: " -v leaving="$leaving" '
      $0 ~ step { sent = 1 } $0 ~ leaving { exit !sent }' ||
      echo "skip: switch-frames' $1, which came after the page left had left the window"
  }
  # The core's let-go of the page the last switch left, as the page it shows has drawn, looked for
  # from the step $2 if given, the ⌃Tab's key: expect_soon looks from the last command sent.
  let_go() {
    sent=$step
    step=${2:-$step}
    expect_soon '^browser: switch let the page it left go ' "$1 didn't let the page it left go"
    step=$sent
    went=$step
  }
  went=$step
  left "after the ⌃Tabs" "$step"
  # The page shown last: one of the two that move.
  shown=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
  other=$m
  [ "$shown" != "$m" ] || other=$n
  # With the pointer resting over the page area, the move after a ⌃Tab reaches the page it shows,
  # and not the page it let go, still in the window. Two pages taking the pointer, setting the
  # cursor each, would show for the 250 ms.
  send move $page_point
  expect "^browser: title $shown moved [0-9]*\$" "a pointer move over the page area didn't reach the page shown"
  ctrl_tab
  expect "^browser: selected $other in " "⌃Tab didn't show the other page that moves" "$tapped"
  let_go "⌃Tab with the pointer over the page area" "$tapped"
  turned=$step
  send move $(echo $page_point | awk '{ print $1 + 10, $2 + 10 }')
  expect "^browser: title $other moved [0-9]*\$" "a pointer move just after ⌃Tab didn't reach the page shown"
  none_since "^browser: title $shown moved " "$turned" ||
    fail "a pointer move just after ⌃Tab reached the page it let go too"
  left "after a pointer move" "$step"
  ctrl_tab
  expect "^browser: selected $shown in " "⌃Tab didn't show the first page that moves again" "$tapped"
  let_go "⌃Tab back" "$tapped"
  send click $((width - 110)) $((height - 60)) shift
  expect '^browser: link preview shown$' "⇧-click just after a switch didn't open the link in Peek"
  left "with Peek open" "$step"
  send key escape
  expect '^browser: link preview closing (escape)$' "Esc didn't close Peek"
  send key 1 ctrl
  expect "^browser: space $first in " "⌃1 didn't show the first Space"
  send click $(row 1)
  expect "^browser: selected $a in " "a click on the striped page's row didn't show it"
  let_go "a click on the striped page's row"
  send click $page_point
  send key k
  expect "^browser: title $a key k\$" "a key just after a switch didn't reach the page shown"
  left "after a click and a key in the page shown" $((went + 1))
  send key 2 ctrl
  expect "^browser: space $second in " "⌃2 didn't show the second Space"
  let_go "⌃2"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L just after a switch didn't open the command bar"
  left "after ⌘L" "$step"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  send key 1 ctrl
  expect "^browser: space $first in " "⌃1 didn't show the first Space"
  let_go "⌃1"
  # Full screen is a Space of its own. AppKit says the window has entered or left it before macOS
  # has slid there or back, and until then its transition window is over the browser window and
  # takes the clicks (#2248). Left before the slide in has ended, macOS slides there and back after
  # (#889), so each step waits for its slide.
  send fullscreen
  entered=$step
  expect '^browser: window entered full screen$' "the window didn't go full screen just after a switch"
  expect '^browser: desktop space changed$' "the window's full-screen Space didn't come forward" \
    "$entered"
  left "in full screen" "$entered"
  send fullscreen
  exited=$step
  expect '^browser: window left full screen$' "the window didn't leave full screen"
  expect '^browser: desktop space changed$' "the desktop's Space didn't come back after full screen" \
    "$exited"
  # The striped page, shown again while it's let go, and example.com's tab, closed while its page is
  # let go: the window lets it go without it.
  send click $(row 0)
  expect "^browser: selected $c in " "a click on example.com's row didn't show it"
  let_go "a click on example.com's row"
  send click $(row 1)
  expect "^browser: selected $a in " "a click on the striped page's row didn't show it"
  let_go "a click on the striped page's row"
  send middleclick $(row 0)
  expect "^browser: tab $c closed\$" "a middle-click on example.com's row didn't close its tab"
  left "as its tab closed" "$step"
  # A quit with the page a switch let go still in the window.
  send key 2 ctrl
  expect "^browser: space $second in " "⌃2 didn't show the second Space"
  let_go "⌃2 before the quit"
  quick_ctrl_tabs "after the pictures"
fi
exec 3>&-
quit
if [ -n "$held" ]; then
  if since "$went" | grep -q "$leaving"; then
    echo "skip: switch-frames' quit just after a switch, which came after the page left had left the window"
  fi
fi
# A picture shows the window at a moment between when it was asked for and when it came back, so it
# goes with the last switch begun by when it came back: one taken across a switch's start, as a
# blank there would be, is that switch's, and says how far each side of its start it reached.
status=0
python3 -I - "$marks" "$frames" > "$out/switch-frames-summary.txt" << 'PY' || status=$?
import statistics, sys

marks = []
for line in open(sys.argv[1], encoding="utf-8"):
    at, what = line.rstrip("\n").split(" ", 1)
    marks.append((float(at), what))
# Asked for, came back, flat, and what it showed.
frames = []
for line in open(sys.argv[2], encoding="utf-8"):
    parts = line.split()
    if len(parts) == 7 and parts[2] in ("flat", "page"):
        frames.append((float(parts[0]), float(parts[1]), parts[2] == "flat", " ".join(parts[3:])))
start = next(at for at, what in marks if what == "start")
stop = next(at for at, what in marks if what == "stop")
switches = [(at, what) for at, what in marks if what not in ("start", "stop")]
if len(frames) < 2:
    print("problem: %d pictures" % len(frames))
    sys.exit(1)
gaps = [b[0] - a[0] for a, b in zip(frames, frames[1:])]
print("%d pictures, one every %.0f ms (median), %.0f ms at most, each %.0f ms in coming (median)" % (
    len(frames), statistics.median(gaps) * 1000, max(gaps) * 1000,
    statistics.median(f[1] - f[0] for f in frames) * 1000))
before = [f for f in frames if f[1] < start]
problems = []
if not before or any(f[2] for f in before):
    problems.append("the page area was flat before any switch, or never pictured")
for index, (at, what) in enumerate(switches):
    until = switches[index + 1][0] if index + 1 < len(switches) else stop
    seen = [f for f in frames if at <= f[1] < until]
    flats = [f for f in seen if f[2]]
    print("%-7s %2d pictures, %d flat%s" % (what, len(seen), len(flats), "".join(
        ", %+.0f..%+.0f ms %s" % ((f[0] - at) * 1000, (f[1] - at) * 1000, f[3])
        for f in flats[:4])))
    if len(seen) < 3:
        problems.append("%s: %d pictures, too few to tell" % (what, len(seen)))
    elif flats:
        problems.append("%s: no page in %d picture%s, between %+.0f and %+.0f ms from its start" % (
            what, len(flats), "" if len(flats) == 1 else "s", (flats[0][0] - at) * 1000,
            (flats[-1][1] - at) * 1000))
for problem in problems:
    print("problem: " + problem)
sys.exit(1 if problems else 0)
PY
# How long the core kept the page each switch left over the one it showed, from its lines: on the
# Mac until that one's renderer had a frame on screen.
awk '/^browser: switch keeps/ { kept++ }
  /^browser: switch let the page it left go after/ { after = after " " $(NF - 1) }
  /^browser: switch let the page it left go at the limit/ { limit++ }
  END { if (kept) printf "kept the page left %d times, let go after%s ms, %d at the limit\n",
    kept, after, limit }' "$log" >> "$out/switch-frames-summary.txt"
cat "$out/switch-frames-summary.txt"
[ "$status" = 0 ] ||
  fail "the page area showed no page through a switch: $(grep '^problem: ' "$out/switch-frames-summary.txt" | cut -c10- | paste -sd';' -), its pictures in $out/switch-flat"
echo "ok: every Space and tab switch showed a page in the page area at every picture"
[ -z "$held" ] ||
  echo "ok: on the Mac, ten ⌃Tabs back to back and switches to a page that moves showed a page" \
    "throughout, the pages let go left the window together leaving the page shown, and in the" \
    "250 ms after a let-go a pointer move reached only the page shown, and a click, a key, ⌘L," \
    "Peek, full screen, closing a tab and a quit worked"
