# Every Space switch, by ⌃1 and ⌃2 and by the switcher's dots, and every switch between a Space's
# tabs by a click on its row, shows a page in the page area at every frame: the one it leaves until
# the one it shows has drawn, and never the empty page area between them, for 60 to 230 ms on
# GitHub's hosted Mac (#1438). A burst of the window's pictures (tools/window-frames.sh) looks at
# the page area in each: one flat colour is no page. Its pages are never flat, the first striped and
# the others example.com's and example.org's text on their backgrounds.
# The pictures need the Mac's ScreenCaptureKit, check.sh's Xvfb on Linux, or a CI runner's screen on
# Windows; elsewhere the run says it can't take them.
page="data:text/html,<title>ready</title><body style=\"margin:0;height:100vh;background:repeating-linear-gradient(45deg,%23cc3333 0 24px,%23ffffff 24px 48px)\">"
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
# The switcher's dots are centred under the sidebar, 25 above the window's bottom.
dots_y=$((height - 25))
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
send type frames
send key return
open_from_bar example.org
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
  send click 131 $dots_y
  expect "^browser: space $second in " "the second dot didn't show the second Space"
  sleep 0.7
  mark "dot1 $i"
  send click 97 $dots_y
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
mark stop
touch "$stop"
wait "$capture" || fail "the window's pictures failed: $(tail -1 "$out/switch-frames-capture.log")"
exec 3>&-
quit
# Each picture goes with the switch before it, until the next.
status=0
python3 -I - "$marks" "$frames" > "$out/switch-frames-summary.txt" << 'PY' || status=$?
import statistics, sys

marks = []
for line in open(sys.argv[1], encoding="utf-8"):
    at, what = line.rstrip("\n").split(" ", 1)
    marks.append((float(at), what))
frames = []
for line in open(sys.argv[2], encoding="utf-8"):
    parts = line.split()
    if len(parts) == 6 and parts[1] in ("flat", "page"):
        frames.append((float(parts[0]), parts[1] == "flat", " ".join(parts[2:])))
start = next(at for at, what in marks if what == "start")
stop = next(at for at, what in marks if what == "stop")
switches = [(at, what) for at, what in marks if what not in ("start", "stop")]
if len(frames) < 2:
    print("problem: %d pictures" % len(frames))
    sys.exit(1)
gaps = [b[0] - a[0] for a, b in zip(frames, frames[1:])]
print("%d pictures, one every %.0f ms (median), %.0f ms at most" % (
    len(frames), statistics.median(gaps) * 1000, max(gaps) * 1000))
before = [f for f in frames if f[0] < start]
problems = []
if not before or any(f[1] for f in before):
    problems.append("the page area was flat before any switch, or never pictured")
for index, (at, what) in enumerate(switches):
    until = switches[index + 1][0] if index + 1 < len(switches) else stop
    seen = [f for f in frames if at <= f[0] < until]
    flats = [f for f in seen if f[1]]
    runs, begin = [], None
    for f in seen + [(until, False, "")]:
        if f[1] and begin is None:
            begin = f[0]
        elif not f[1] and begin is not None:
            runs.append(f[0] - begin)
            begin = None
    print("%-7s %2d pictures, %d flat%s%s" % (
        what, len(seen), len(flats),
        ", for %.0f ms" % (max(runs) * 1000) if runs else "",
        "".join(", +%.0f ms %s" % ((f[0] - at) * 1000, f[2]) for f in flats[:4])))
    if len(seen) < 3:
        problems.append("%s: %d pictures, too few to tell" % (what, len(seen)))
    elif flats:
        problems.append("%s: no page for %.0f ms, %.0f ms after it" % (
            what, max(runs) * 1000, (flats[0][0] - at) * 1000))
for problem in problems:
    print("problem: " + problem)
sys.exit(1 if problems else 0)
PY
cat "$out/switch-frames-summary.txt"
[ "$status" = 0 ] ||
  fail "the page area showed no page through a switch: $(grep '^problem: ' "$out/switch-frames-summary.txt" | cut -c10- | paste -sd';' -), its pictures in $out/switch-flat"
echo "ok: every Space and tab switch showed a page in the page area at every picture"
