# Spike (#359), on spike/live-mirror-359-3 only, never for main: what a live mirror of a page in the
# windows it left costs, on the Mac (a second CALayerHost of the page's compositor) and Windows (a
# DWM thumbnail), against the picture they draw today. The driver's mirror command puts it over each
# window's resting picture (platform/*/drive*). A page that paints every frame moves through two
# windows, then four, side by side, so the windows it left draw it whole: one mirror, then three.
# In each, with the page painting and then still, MIRROR_ROUNDS (3) rounds of MIRROR_SECONDS (15)
# measure the picture and then the mirror, in turns, so drift hits both alike: the CPU and wakeups
# of the app's processes and of the window server (WindowServer, dwm.exe), their memory, and on the
# Mac the GPU's active residency, where powermetrics has it. Lines go to build/check/mirror.txt.

seconds="${MIRROR_SECONDS:-15}"
rounds="${MIRROR_ROUNDS:-3}"
# Fills itself with a new colour and moves a square every frame, from the main thread; a click
# stops and starts it. No percent signs: the URL would read them as escapes.
page='data:text/html,<title>moving</title>
<body style="margin:0;overflow:hidden"><canvas id="c"></canvas>
<script>
const c = document.getElementById("c"), g = c.getContext("2d");
let on = true, t = 0;
function size() { c.width = innerWidth; c.height = innerHeight; }
size();
onresize = size;
function frame() {
  if (!on) return;
  t++;
  g.fillStyle = "rgb(" + (t * 3 & 255) + ",80,160)";
  g.fillRect(0, 0, c.width, c.height);
  g.fillStyle = "white";
  g.fillRect(t * 4 - c.width * Math.floor(t * 4 / c.width), 100, 80, 80);
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
onclick = () => {
  on = !on;
  document.title = on ? "moving" : "still";
  if (on) requestAnimationFrame(frame);
};
</script>'
log="$out/mirror.log"
results="$out/mirror.txt"
: > "$results"
step=0
rm -f "$saved" "$launch_frame"
case "$(uname)" in
  Darwin) server=WindowServer ;;
  MINGW* | MSYS*) server=C:/Windows/System32/dwm.exe ;;
  *)
    echo "skip: mirror. It's the Mac's and Windows'."
    exit 0
    ;;
esac
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
a=$(today)
send click $sidebar_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: mirror. Browser wasn't the active app."
  exit 0
fi
send place 0 40 900 500

# The id of the window the driver numbers $1, in the order they opened.
window_id() {
  grep '^browser: window opened ' "$log" | sed -n "${1}p" | cut -d' ' -f4
}
# Opens window $1 at $2 $3 and moves the page into it from window $1 - 1, which pictures it.
take_page() {
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open window $1"
  send window "$1"
  send place "$2" "$3" 900 500
  send click $(row 0)
  expect "^browser: window $(window_id "$1") shows $a\$" "the page didn't move into window $1"
  left=$(window_id $(($1 - 1)))
  expect "^browser: window $left pictures tab $a\$" "window $(($1 - 1)) got no picture of the page"
  expect "^browser: window $left picture at " "window $(($1 - 1))'s picture didn't rest"
}
# Mirrors the page in window $1 in each window before it, over its picture.
mirrors_on() {
  local k
  for k in $(seq 1 $(($1 - 1))); do
    # shellcheck disable=SC2046 # x y width height
    send mirror "$1" "$k" $(grep "^browser: window $(window_id "$k") picture at " "$log" | tail -1 |
      cut -d' ' -f6-)
    grep -q '^browser: mirror \(hosts\|thumbnails\)' "$log" || fail "no mirror in window $k"
  done
}
# The page in window $1 painting ($2 moving) or not (still).
page_state() {
  send window "$1"
  if [ "$(grep "^browser: title $a " "$log" | tail -1 | cut -d' ' -f4)" != "$2" ]; then
    send click $page_point
    expect "^browser: title $a $2\$" "a click on the page didn't make it $2"
  fi
}
server_snap() {
  if [ "$server" = WindowServer ]; then
    sudo -n python3 -I check/usage.py snap "$server" "$1"
  else
    python3 -I check/usage.py snap "$server" "$1"
  fi
}
# Measures $seconds as label $1: "app cpu wakeups MiB; server cpu wakeups MiB; gpu".
measure() {
  python3 -I check/usage.py snap "$procs" "$out/mirror-app-a.json"
  server_snap "$out/mirror-server-a.json"
  metrics=
  if [ "$server" = WindowServer ] && sudo -n true 2> /dev/null; then
    sudo -n powermetrics --samplers gpu_power -i 1000 -n "$seconds" > "$out/mirror-gpu.txt" 2>&1 &
    metrics=$!
  fi
  end=$(($(date +%s) + seconds))
  while [ "$(date +%s)" -lt "$end" ]; do
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.5
  done
  python3 -I check/usage.py snap "$procs" "$out/mirror-app-b.json"
  server_snap "$out/mirror-server-b.json"
  gpu=n/a
  if [ -n "$metrics" ]; then
    wait "$metrics" || true
    gpu=$(awk '/GPU HW active residency/ { gsub("%", "", $5); s += $5; n++ }
      END { if (n) printf "%.2f", s / n; else print "n/a" }' "$out/mirror-gpu.txt")
  fi
  app=$(python3 -I check/usage.py idle "$out/mirror-app-a.json" "$out/mirror-app-b.json")
  srv=$(python3 -I check/usage.py idle "$out/mirror-server-a.json" "$out/mirror-server-b.json")
  app_mib=$(python3 -I check/usage.py memory "$out/mirror-app-b.json" | cut -d' ' -f1)
  srv_mib=$(python3 -I check/usage.py memory "$out/mirror-server-b.json" | cut -d' ' -f1)
  echo "mirror: $1: app $app $app_mib; server $srv $srv_mib; gpu $gpu" | tee -a "$results"
}
# Rounds of the picture and then the mirror, in turns, with $1 windows, the page $2.
rounds() {
  local r
  page_state "$1" "$2"
  for r in $(seq "$rounds"); do
    measure "$1 windows $2 picture"
    mirrors_on "$1"
    measure "$1 windows $2 mirror"
    send mirror off
  done
}
# Whether window 1's card changes between two captures of every window, with the page painting.
card_changes() {
  ../tools/window-screenshot.sh "$pid" "$out/mirror-$1-a.png" all || fail "no window to capture"
  ../tools/window-screenshot.sh "$pid" "$out/mirror-$1-b.png" all || fail "no window to capture"
  echo "mirror: card $1: $(python3 -I ../spikes/live-mirror/card_changed.py \
    "$out/mirror-$1-a.png" "$out/mirror-$1-b.png" 880 480 1820)" | tee -a "$results"
}

if [ "$server" = WindowServer ]; then
  sudo -n powermetrics --samplers gpu_power -i 500 -n 1 2>&1 | grep -i 'gpu' | head -5 |
    sed 's/^/mirror: powermetrics: /' | tee -a "$results" || true
fi
take_page 2 920 40
page_state 2 moving
card_changes picture
mirrors_on 2
card_changes mirror
../tools/window-screenshot.sh "$pid" "$out/mirror-2.png" all || fail "no window to capture"
send mirror off
rounds 2 moving
rounds 2 still
page_state 2 moving
take_page 3 0 560
take_page 4 920 560
mirrors_on 4
../tools/window-screenshot.sh "$pid" "$out/mirror-4.png" all || fail "no window to capture"
send mirror off
rounds 4 moving
rounds 4 still
exec 3>&-
quit

# Each label's means: app CPU percent of a core, wakeups a second and MiB, then the server's.
awk -F': ' '/^mirror: [24] windows/ {
    split($3, part, "; ")
    split(part[1], app, " "); split(part[2], srv, " "); split(part[3], g, " ")
    n[$2]++
    for (i = 2; i <= 4; i++) { a[$2, i] += app[i]; s[$2, i] += srv[i] }
    if (g[2] != "n/a") { gs[$2] += g[2]; gn[$2]++ }
    order[$2] = ++seen
  }
  END {
    for (k in n) {
      printf "mirror: mean %s, %d rounds: app %.2f percent, %.1f wakeups/s, %.0f MiB;", k, n[k],
        a[k, 2] / n[k], a[k, 3] / n[k], a[k, 4] / n[k]
      printf " server %.2f percent, %.1f wakeups/s, %.0f MiB; gpu %s\n", s[k, 2] / n[k],
        s[k, 3] / n[k], s[k, 4] / n[k], gn[k] ? sprintf("%.2f percent", gs[k] / gn[k]) : "n/a"
    }
  }' "$results" | sort | tee -a "$results"
grep -q '^mirror: card mirror: changed' "$results" || fail "window 1's mirror didn't change while the page painted"
echo "ok: mirror, $(grep -c '^mirror: [24] windows' "$results") measures in $results." \
  "Windows: $out/mirror-2.png, $out/mirror-4.png"
