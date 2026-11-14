# Probe (#2790), not a run: what the window manager makes of an overlay that stays while the user is
# elsewhere, on X11. Sourced by managed.sh and preview.sh. Each line it finds goes to
# $out/probe2790.txt as "probe2790 <overlay> <what> <json>", each capture to $out/probe2790-*.png.
X=check/x11probe.py
p2790() { python3 -I "$X" "$@"; }
note() { echo "probe2790 $*" >> "$out/probe2790.txt"; }
wm=${BROWSER_XVFB:-}
[ "$wm" = 1 ] && wm=openbox

# The JSON line of window $2 in snap file $1, or nothing.
line_of() { grep "\"id\": \"$2\"" "$1" | head -1; }
# A field $3 of window $2 in snap file $1.
field_of() {
  python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o = json.loads(l)
    if o.get("id") == sys.argv[2]:
        v = o.get(sys.argv[3])
        print(json.dumps(v) if not isinstance(v, str) else v)
        break' "$1" "$2" "$3"
}
# The browser window of process $1: its largest client with that pid, transient for none.
browser_window() {
  p2790 snap | python3 -I -c 'import json,sys
best = None
for l in sys.stdin:
    o = json.loads(l)
    if o.get("pid") == int(sys.argv[1]) and o["transient_for"] == "0x0" and o["viewable"]:
        if best is None or o["rect"][2] * o["rect"][3] > best["rect"][2] * best["rect"][3]:
            best = o
print(best["id"] if best else "")' "$1"
}
# The windows viewable in snap $2 that weren't in snap $1, inside browser $3's rect, one id a line.
# Not i3's frames, which are override-redirect windows of its own.
new_windows() {
  python3 -I -c 'import json,sys
def load(p):
    return {o["id"]: o for o in map(json.loads, open(p)) if "id" in o}
a, b = load(sys.argv[1]), load(sys.argv[2])
br = b.get(sys.argv[3], {}).get("rect", [0, 0, 99999, 99999])
for i, o in b.items():
    r = o["rect"]
    if o["viewable"] and not a.get(i, {}).get("viewable") and i != sys.argv[3] and r[2] > 1 \
            and not str(o.get("name") or "").startswith("[i3 con]") \
            and r[0] >= br[0] - 2 and r[1] >= br[1] - 2:
        print(i)' "$1" "$2" "$3"
}
# Waits up to 4 s for window $1's field $2 in a fresh snap to be $3, leaving the snap in $snap.
until_field() {
  for _ in $(seq 40); do
    p2790 snap > "$snap"
    [ "$(field_of "$snap" "$1" "$2")" = "$3" ] && return 0
    sleep 0.1
  done
  return 1
}
root_shot() { import -window root "$out/probe2790-$1.png" 2> /dev/null || true; }

# KWin's view of each window, from a script of its own (Alt-Tab lists what wants tab focus, isn't
# skipSwitcher and has no modal). Needs check.sh's kwin to have said where its bus and log are.
kwin_windows() {
  [ "$wm" = kwin ] && [ -f /tmp/kwin-2790.bus ] || return 0
  n=$((${kwin_n:-0} + 1))
  kwin_n=$n
  cat > "$out/probe2790-kwin-$n.js" <<'EOF'
var cs = workspace.clientList();
for (var i = 0; i < cs.length; i++) {
    var c = cs[i];
    print("kwin2790 " + c.windowId.toString(16) + " " + JSON.stringify({caption: c.caption,
        dialog: c.dialog, transient: c.transient, modal: c.modal, skipSwitcher: c.skipSwitcher,
        skipTaskbar: c.skipTaskbar, skipPager: c.skipPager, wantsInput: c.wantsInput,
        minimized: c.minimized, desktop: c.desktop, fullScreen: c.fullScreen, active: c.active,
        keepAbove: c.keepAbove, geometry: [c.x, c.y, c.width, c.height]}));
}
EOF
  bus=$(cat /tmp/kwin-2790.bus)
  id=$(DBUS_SESSION_BUS_ADDRESS=$bus dbus-send --session --print-reply --dest=org.kde.KWin /Scripting \
    org.kde.kwin.Scripting.loadScript string:"$out/probe2790-kwin-$n.js" string:"p2790$n" 2>&1 |
    awk '/int32/ { print $2 }')
  DBUS_SESSION_BUS_ADDRESS=$bus dbus-send --session --print-reply --dest=org.kde.KWin \
    "/Scripting/Script$id" org.kde.kwin.Script.run > /dev/null 2>&1 ||
    DBUS_SESSION_BUS_ADDRESS=$bus dbus-send --session --print-reply --dest=org.kde.KWin \
      "/$id" org.kde.kwin.Script.run > /dev/null 2>&1 || true
  sleep 0.5
  grep -a 'kwin2790 ' "$(cat /tmp/kwin-2790.where)" 2> /dev/null | sed "s/^.*kwin2790 /kwin $1 $n /" |
    while read -r l; do note "$l"; done
}

# What the manager does with overlay $2 over browser $1, named $3: its hints as the manager left
# them, following the window's moves and sizes, full screen, minimised, another desktop, Alt-Tab.
battery() {
  b=$1 o=$2 name=$3
  snap="$out/probe2790-snap.json"
  p2790 snap > "$snap"
  note "$name props $(line_of "$snap" "$o")"
  note "$name browser $(line_of "$snap" "$b")"
  note "$name root $(grep '"root"' "$snap")"
  root_shot "$name-shown"
  kwin_windows "$name-shown"
  # Following: ten moves, then five resizes, each timed from the browser frame's configure.
  if [ "$wm" = i3 ]; then
    # Tiled, so i3 moves it only by retiling: another window opens beside it and goes.
    p2790 watch 4 > "$out/probe2790-$name-retile.json" &
    w=$!
    sleep 0.3
    rm -f "$out/p2790-other"
    mkfifo "$out/p2790-other"
    python3 -I check/other-window.py < "$out/p2790-other" > "$out/p2790-other.id" &
    exec 8> "$out/p2790-other"
    sleep 1.5
    p2790 snap > "$snap"
    note "$name retiled-overlay $(line_of "$snap" "$o")"
    note "$name retiled-browser $(line_of "$snap" "$b")"
    root_shot "$name-retiled"
    exec 8>&-
    wait $w 2> /dev/null || true
    p2790 snap > "$snap"
    note "$name back-overlay $(line_of "$snap" "$o")"
    note "$name back-browser $(line_of "$snap" "$b")"
  else
    p2790 follow "$b" "$o" 10 12 8 0 0 > "$out/probe2790-$name-move.json" 2>&1 || true
    note "$name move $(tail -1 "$out/probe2790-$name-move.json")"
    p2790 follow "$b" "$o" 5 0 0 -24 -16 > "$out/probe2790-$name-resize.json" 2>&1 || true
    note "$name resize $(tail -1 "$out/probe2790-$name-resize.json")"
    root_shot "$name-moved"
    start=$(head -1 "$out/probe2790-$name-move.json" | python3 -I -c 'import json,sys
print(" ".join(map(str, json.load(sys.stdin)["start_browser"])))')
    # shellcheck disable=SC2086
    [ -z "$start" ] || p2790 moveresize "$b" $start
    sleep 0.5
  fi
  # Full screen, as the manager makes a window, and back.
  p2790 state "$b" add FULLSCREEN
  sleep 1
  p2790 snap > "$snap"
  note "$name fullscreen-overlay $(line_of "$snap" "$o")"
  note "$name fullscreen-browser $(line_of "$snap" "$b")"
  root_shot "$name-fullscreen"
  kwin_windows "$name-fullscreen"
  p2790 state "$b" remove FULLSCREEN
  sleep 1
  # Minimised, and back.
  if [ "$wm" != i3 ]; then
    p2790 iconify "$b"
    until_field "$b" viewable false || true
    sleep 0.3
    p2790 snap > "$snap"
    note "$name minimised-overlay $(line_of "$snap" "$o")"
    note "$name minimised-browser $(line_of "$snap" "$b")"
    root_shot "$name-minimised"
    p2790 activate "$b"
    until_field "$b" viewable true || true
    sleep 0.5
    p2790 snap > "$snap"
    note "$name restored-overlay $(line_of "$snap" "$o")"
  else
    note "$name minimised i3 has no minimise"
  fi
  # Another desktop: the browser's sent to the second, which is then shown, and back.
  if [ "$wm" = i3 ]; then
    i3-msg "[id=$(printf %d "$b")] move container to workspace 2" > /dev/null 2>&1 || true
    sleep 0.6
    p2790 snap > "$snap"
    note "$name otherdesk-overlay $(line_of "$snap" "$o")"
    i3-msg "workspace 2" > /dev/null 2>&1 || true
    sleep 0.6
    p2790 snap > "$snap"
    note "$name thatdesk-overlay $(line_of "$snap" "$o")"
    root_shot "$name-thatdesk"
    i3-msg "[id=$(printf %d "$b")] move container to workspace 1; workspace 1" > /dev/null 2>&1 || true
  else
    p2790 desktops 2
    sleep 0.3
    p2790 todesktop "$b" 1
    sleep 0.8
    p2790 snap > "$snap"
    note "$name otherdesk-overlay $(line_of "$snap" "$o")"
    note "$name otherdesk-root $(grep '"root"' "$snap")"
    p2790 current 1
    sleep 0.8
    p2790 snap > "$snap"
    note "$name thatdesk-overlay $(line_of "$snap" "$o")"
    root_shot "$name-thatdesk"
    p2790 todesktop "$b" 0
    sleep 0.3
    p2790 current 0
  fi
  sleep 0.8
  p2790 activate "$b"
  sleep 0.5
  p2790 snap > "$snap"
  note "$name home-overlay $(line_of "$snap" "$o")"
  note "$name home-root $(grep '"root"' "$snap")"
  # Alt-Tab, held while it shows its list, then let go and ilo's window taken back.
  if [ "$wm" != i3 ]; then
    xdotool keydown alt key Tab 2> /dev/null || true
    sleep 0.8
    root_shot "$name-alt-tab"
    p2790 snap > "$snap"
    note "$name alttab-root $(grep '"root"' "$snap")"
    xdotool keyup alt 2> /dev/null || true
    sleep 0.5
    p2790 activate "$b"
    sleep 0.5
  fi
}

# The time from now to the root's pixel at $1,$2 changing, in ms, after the command $3... is sent.
# Prints "<ms to change> <ms to map>".
time_open() {
  px=$1 py=$2
  shift 2
  rm -f "$out/p2790-ff"
  p2790 firstframe "$px" "$py" 3 > "$out/p2790-ff" &
  ff=$!
  for _ in $(seq 50); do grep -q ready "$out/p2790-ff" 2> /dev/null && break; sleep 0.05; done
  t0=$(date +%s.%N)
  "$@"
  wait $ff 2> /dev/null || true
  python3 -I -c 'import json,sys
o = json.loads(open(sys.argv[1]).read().splitlines()[-1])
t0 = float(sys.argv[2])
f = lambda v: "none" if v is None else str(round((v - t0) * 1000))
print(f(o.get("changed")), f(o.get("mapped")))' "$out/p2790-ff" "$t0"
}
