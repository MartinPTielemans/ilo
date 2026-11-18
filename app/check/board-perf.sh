# check.sh runs this at 695, only when named.
# A Board's numbers (#3073), a run of their own so board keeps to its time. A Board of 2,000 notes,
# each some 1.5 KB of text, a little over 3 MB of the 4 MiB a Board's file may hold, opens beside two
# plain pages, and:
# - Tab switches, by Chrome's measure: DevTools traces 10 round trips between the two plain pages,
#   then 10 between a plain page and the Board, and board_trace.py reads TabSwitching::Latency's
#   spans, from the tab strip's change to the first frame presented, against the 16 ms gate.
# - Memory, summed over the browser's processes as perf's is: with the two plain pages, with the
#   Board shown, and with it in the background.
# - Adding a picture, dropped on the Board as a file: a 1 MB PNG and one just under the 50 MB a
#   picture may be, three of each, each traced alone. The UI thread's longest task from the drop to
#   the Board saved, as the page writes the file in 256 KiB slices to the Board's own sequence, and
#   the add's time from the drop to the core's "board added" line: for the big one, its file and
#   then the small copy the page draws from.
# Numbers over the gate are findings, said in the run's lines, not failures: the run fails only
# when a number can't be read.

log="$out/board-perf.log"
cdp_log="$out/board-perf-cdp.log"
dir="$out/board-perf"
rm -rf "$dir"
mkdir -p "$dir"
step=0
rm -f "$saved" "$launch_frame"

# Unix ms now, in $stamp: bash's own clock where it has one, as the Mac's bash 3.2 doesn't.
board_stamp() {
  if [ -n "${EPOCHREALTIME:-}" ]; then
    stamp=$EPOCHREALTIME
    stamp_us=${stamp#*[.,]}
    stamp="${stamp%[.,]*}${stamp_us%???}"
  else
    stamp=$(python3 -I -c 'import time; print(int(time.time() * 1000))')
  fi
}
# The place of tab $1 among the pinned tabs and Today, for its ⌘ digit.
board_index() {
  echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$1" | cut -d: -f1
}
# Opens $1 in a new tab from the command bar and sets $tab to it once it's loaded.
board_open() {
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $2"
  send insert "$1"
  send key return
  expect "^browser: today [0-9][0-9]* $before\$" "$2 didn't open at the top of Today"
  tab=$(today | cut -d' ' -f1)
  expect "^browser: loaded $tab " "$2 didn't load in its tab"
}
# Traces categories $1 into $dir/$2 around the command after.
board_traced() {
  trace_categories=$1 trace_file=$dir/$2
  shift 2
  since=$(wc -l < "$cdp_log")
  echo "trace start $trace_categories" >&5
  board_cdp_await "$since" '^cdp: tracing$' "DevTools didn't start tracing"
  "$@"
  since=$(wc -l < "$cdp_log")
  echo "trace end $trace_file" >&5
  board_cdp_await "$since" "^cdp: traced $trace_file" "DevTools didn't write $trace_file"
  ! tail -n +$((since + 1)) "$cdp_log" | grep -q "^cdp: traced .* lossy$" ||
    echo "board-perf: $2: the browser dropped some of the trace"
}
# Waits up to 60 s for a line matching $2 in the DevTools log after its line $1, failing with $3.
board_cdp_await() {
  for _ in $(seq 600); do
    tail -n +$(($1 + 1)) "$cdp_log" | grep -q "$2" && return 0
    kill -0 "$pid" 2> /dev/null || fail "the browser exited, waiting for: $3"
    sleep 0.1
  done
  fail "$3: $(tail -3 "$cdp_log" | tr '\n' ' ')"
}
# $1 round trips between tabs $2 and $3, starting on $2, each switch by its ⌘ digit.
board_switches() {
  for _ in $(seq "$1"); do
    switch "$(board_index "$3") cmd" "$3"
    switch "$(board_index "$2") cmd" "$2"
  done
}
# The memory now, labelled $1, as "MiB processes" with the kinds of process after.
board_memory() {
  python3 -I check/usage.py snap "$procs" "$dir/memory-$1.json" kinds || fail "the browser's usage didn't read"
  echo "board-perf: memory, $1: $(python3 -I check/usage.py memory "$dir/memory-$1.json") MiB processes" \
    "($(python3 -I check/usage.py kinds "$dir/memory-$1.json" | tr '\n' ' ' | sed 's/ $//'))"
}
# The median and largest of the numbers on standard input, one a line, and how many.
board_stats() {
  sort -n | awk '{ v[NR] = $1 } END {
    if (!NR) { print "none"; exit }
    m = NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2
    printf "median %.1f, largest %.1f, n=%d", m, v[NR], NR }'
}
# Says the spans of the switches in trace $1, labelled $2, the odd ones apart from the even ones
# when $3 and $4 name them.
board_spans() {
  python3 -I check/board_trace.py switches "$dir/$1" > "$dir/$1.spans" ||
    fail "the switch trace $1 didn't read"
  incomplete=$(grep -c '^incomplete$' "$dir/$1.spans" || true)
  grep -v '^incomplete$' "$dir/$1.spans" > "$dir/$1.ms" || true
  [ -s "$dir/$1.ms" ] || fail "the trace $1 has no switch span: $(tr '\n' ' ' < "$dir/$1.spans")"
  if [ -n "${3:-}" ]; then
    echo "board-perf: tab switch, $3, Chrome's span: $(awk 'NR % 2 == 1' "$dir/$1.spans" |
      grep -v incomplete | board_stats) ms; gate 16 ms"
    echo "board-perf: tab switch, $4, Chrome's span: $(awk 'NR % 2 == 0' "$dir/$1.spans" |
      grep -v incomplete | board_stats) ms; gate 16 ms"
  else
    echo "board-perf: tab switch, $2, Chrome's span: $(board_stats < "$dir/$1.ms") ms; gate 16 ms"
  fi
  [ "$incomplete" = 0 ] || echo "board-perf: $1: $incomplete of the switches were cut off before their frame"
  over=$(awk '$1 > 16' "$dir/$1.ms" | wc -l | tr -d ' ')
  [ "$over" = 0 ] || echo "board-perf: FINDING: $over of $1's switches took over 16 ms: $(tr '\n' ' ' < "$dir/$1.ms")"
}
# Writes a PNG of random pixels, $2 by $2, stored without compression so its size is the pixels',
# to $1.
board_png() {
  python3 -I - "$1" "$2" << 'PYEOF' || fail "the $2 by $2 picture wasn't written"
import os, struct, sys, zlib
side = int(sys.argv[2])
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
row = side * 4 + 1
pixels = bytearray(os.urandom(side * row))
pixels[::row] = bytes(side)  # each row's filter byte: none
with open(sys.argv[1], "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", side, side, 8, 6, 0, 0, 0)) +
            chunk(b"IDAT", zlib.compress(bytes(pixels), 0)) + chunk(b"IEND", b""))
PYEOF
}
# Drops the picture $1 on the Board, its $2nd of kind $3, which stores $4 files, and waits for the
# Board saved after: the add's times go in $dir/add-$3-$2.
board_add() {
  : > "$dir/add-$3-$2.stamp"
  board_stamp
  dropped=$stamp
  send dragdrop $page_point file "$(app_path "$1")"
  dropped_step=$step
  for _ in $(seq 1200); do
    [ "$(since "$dropped_step" | grep -c 'browser: board added ')" -ge "$4" ] && break
    since "$dropped_step" | grep -q 'browser: board refused a write$' &&
      fail "the Board refused the $3 picture's write"
    kill -0 "$pid" 2> /dev/null || fail "the browser exited adding the $3 picture"
    sleep 0.1
  done
  [ "$(since "$dropped_step" | grep -c 'browser: board added ')" -ge "$4" ] ||
    fail "the $3 picture's add didn't finish in 2 minutes: $(since "$dropped_step" | grep 'browser: board ' | tr '\n' ' ')"
  expect 'browser: board saved$' "the Board wasn't saved after the $3 picture was added" "$dropped_step"
  since "$dropped_step" > "$dir/add-$3-$2.log"
  python3 -I check/board_trace.py since "$dropped" "$dir/add-$3-$2.log" 'browser: board added ' \
    > "$dir/add-$3-$2" || fail "the $3 picture's add times didn't read"
}

# A Board of 2,000 notes, in the profile before the launch.
id=$(printf 'c%.0s' $(seq 32))
mkdir -p "$profile_dir/Default/Boards/$id"
python3 -I - "$profile_dir/Default/Boards/$id/board.json" << 'PYEOF' || fail "the Board of 2,000 notes wasn't written"
import json, sys
words = "shelf sink tile grout oak brass hinge drawer lamp cord paint primer sample swatch quote".split()
cards = []
for i in range(2000):
    text = " ".join(words[(i + j) % len(words)] for j in range(300))[:1500]
    cards.append({"id": "n%04d" % i, "kind": "note", "x": 20 + 260 * (i % 50),
                  "y": 20 + 260 * (i // 50), "w": 240, "text": text})
board = {"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": cards}
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(board, f, separators=(",", ":"))
PYEOF
size=$(wc -c < "$profile_dir/Default/Boards/$id/board.json" | tr -d ' ')
[ "$size" -lt 4194304 ] || fail "the Board of 2,000 notes is $size bytes, over the 4 MiB a Board's file may hold"

scratch=$(mktemp -d)
start_with_cdp 'data:text/html,<title>plain a</title><p>A plain page.' "$profile_dir" ""
await_load
send click $page_point
plain_a=$(today | cut -d' ' -f1)
[ -n "$plain_a" ] || fail "the first page's tab isn't in Today"
board_open 'data:text/html,<title>plain b</title><p>Another plain page.' "the second plain page"
plain_b=$tab
board_memory "two plain pages"

board_open "chrome://board/$id" "the Board of 2,000 notes"
board=$tab
expect "^browser: loaded $board chrome://board/$id\$" "the Board of 2,000 notes didn't load"
# A Board saves as its tab hides, once it has read its file and drawn its cards, which is the line
# that says it has: nothing in the page logs.
switch "$(board_index "$plain_a") cmd" "$plain_a"
expect 'browser: board saved$' "the Board of 2,000 notes didn't save as its tab hid"
switch "$(board_index "$board") cmd" "$board"
board_memory "two plain pages and the Board shown"
# Back on a plain page, the Board's in the background, saved.
switch "$(board_index "$plain_a") cmd" "$plain_a"
expect 'browser: board saved$' "the Board of 2,000 notes didn't save as its tab hid again"
board_memory "two plain pages, the Board in the background"

board_traced latency switches-plain.json board_switches 10 "$plain_a" "$plain_b"
board_spans switches-plain.json "between two plain pages"
board_traced latency switches-board.json board_switches 10 "$plain_a" "$board"
board_spans switches-board.json "" "plain page to the Board" "the Board to a plain page"

# Each add from the Board shown, as a drop is on the page in view.
switch "$(board_index "$board") cmd" "$board"
board_png "$scratch/small.png" 512
board_png "$scratch/big.png" 3600
echo "board-perf: pictures $(wc -c < "$scratch/small.png" | tr -d ' ') and $(wc -c < "$scratch/big.png" | tr -d ' ') bytes"
for kind in small big; do
  files=1
  [ "$kind" = small ] || files=2
  for n in 1 2 3; do
    board_traced toplevel "add-$kind-$n.json" board_add "$scratch/$kind.png" "$n" "$kind" "$files"
    python3 -I check/board_trace.py ui "$dir/add-$kind-$n.json" > "$dir/add-$kind-$n.ui" ||
      fail "the UI thread's tasks in add-$kind-$n.json didn't read"
    read -r longest name from count < "$dir/add-$kind-$n.ui"
    echo "board-perf: add $kind $n: UI thread's longest task $longest ms ($name from $from, of $count);" \
      "board added at $(tr '\n' ' ' < "$dir/add-$kind-$n" | sed 's/ $//' | sed 's/ / and /') ms from the drop"
  done
done
../tools/window-screenshot.sh "$pid" "$out/board-perf.png" || fail "no window to capture"
exec 3>&- 5>&-
quit
rm -rf "$profile_dir" "$scratch"
echo "ok: the Board of 2,000 notes' tab switches, memory and picture adds are in the board-perf lines" \
  "above and in $dir. Window: $out/board-perf.png"
