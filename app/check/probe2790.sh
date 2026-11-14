# Probe (#2790), not a run: what the window manager makes of an overlay that stays while the user is
# elsewhere, on X11. Sourced by managed.sh and preview.sh. Each line it finds goes to
# $out/probe2790.txt as "probe2790 <overlay> <what> <json>", each capture to $out/probe2790-*.png.
X=check/x11probe.py
p2790() { python3 -I "$X" "$@"; }
note() { echo "probe2790 $*" >> "$out/probe2790.txt"; }
wm=${BROWSER_XVFB:-}
[ "$wm" = 1 ] && wm=openbox
# i3, and i3 told to tile dialogs (i3tile), where an overlay's place is refused.
tiling=
case $wm in i3*) tiling=1 ;; esac

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
  [ -n "$tiling" ] || covered "$b" "$o" "$name"
  # Following: ten moves, then five resizes, each timed from the browser frame's configure.
  if [ -n "$tiling" ]; then
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
  if [ -z "$tiling" ]; then
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
  if [ -n "$tiling" ]; then
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
  root_shot "$name-home"
  # Alt-Tab, held while it shows its list, then let go and ilo's window taken back.
  if [ -z "$tiling" ]; then
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

# Another app's window raised over browser $1 while overlay $2, named $3, shows: a white window over
# the overlay's middle, made active as a click on it would. The overlay should be under it, drawn
# and stacked: the root's pixel at the overlay's middle is the window's white, and the overlay is
# below it in the manager's stacking order. Then the browser's made active again.
# Another app's window raised over overlay $2 of browser $1, centred on the overlay, or on point
# $4,$5 when given, as for the toast, whose window is mostly see-through around its plate.
covered() {
  b=$1 o=$2 name=$3 at=${4:+$4,$5}
  p2790 snap > "$out/probe2790-cov0.json"
  rect=$(field_of "$out/probe2790-cov0.json" "$o" rect)
  [ -n "$rect" ] && [ "$rect" != null ] || { note "$name covered {\"error\": \"no overlay\"}"; return 0; }
  rm -f "$out/p2790-cover"
  mkfifo "$out/p2790-cover"
  python3 -I check/other-window.py < "$out/p2790-cover" > "$out/p2790-cover.id" &
  exec 9> "$out/p2790-cover"
  for _ in $(seq 50); do [ -s "$out/p2790-cover.id" ] && break; sleep 0.05; done
  other=$(printf '0x%x' "$(cat "$out/p2790-cover.id")")
  # shellcheck disable=SC2046
  set -- $(python3 -I -c 'import json,sys
r = json.loads(sys.argv[1])
w, h = min(r[2], 400), max(min(r[3], 300), 120)
x, y = (map(int, sys.argv[2].split(","))) if sys.argv[2] else (r[0] + r[2] // 2, r[1] + r[3] // 2)
# On the 1280 by 800 screen, as KWin keeps the top of a window from going above it.
print(min(max(x - w // 2, 0), 1280 - w), min(max(y - h // 2, 0), 800 - h), w, h, x, y)' "$rect" "$at")
  sleep 0.3
  p2790 moveresize "$other" "$1" "$2" "$3" "$4"
  sleep 0.3
  xdotool windowactivate "$(printf %d "$other")" 2> /dev/null || p2790 activate "$other"
  sleep 0.8
  p2790 snap > "$out/probe2790-cov1.json"
  root_shot "$name-covered"
  note "$name covered $(python3 -I -c 'import json,sys
snap = [json.loads(l) for l in open(sys.argv[1])]
root = [o for o in snap if o.get("root")][0]
by = {o.get("id"): o for o in snap}
o, other = sys.argv[2], sys.argv[3]
st = root["stacking"]
print(json.dumps({"other": other, "other_rect": by.get(other, {}).get("rect"),
                  "overlay_rect": by.get(o, {}).get("rect"), "overlay_viewable": by.get(o, {}).get("viewable"),
                  "pixel_at_overlay_middle": sys.argv[4], "at": sys.argv[5],
                  "stacking_overlay": st.index(o) if o in st else None,
                  "stacking_other": st.index(other) if other in st else None,
                  "tree_overlay": by.get(o, {}).get("order"), "tree_other": by.get(other, {}).get("order"),
                  "active": root["active"]}))' "$out/probe2790-cov1.json" "$o" "$other" "$(p2790 pixel "$5" "$6")" "$5,$6")"
  kwin_windows "$name-covered"
  exec 9>&-
  sleep 0.3
  p2790 activate "$b"
  sleep 0.6
}

# The root's whole image each time it changes, and its children's maps and configures, for
# $2 seconds from just before the command $3... is sent, to probe2790-film-$1.*, with the command's
# time in the notes.
film_open() {
  name=$1 secs=$2
  shift 2
  p2790 film "$secs" "$out/probe2790-film-$name" > "$out/p2790-film-$name.out" &
  fp=$!
  for _ in $(seq 50); do grep -q ready "$out/p2790-film-$name.out" 2> /dev/null && break; sleep 0.05; done
  note "$name film {\"sent\": $(date +%s.%N)}"
  "$@"
  wait $fp 2> /dev/null || true
  note "$name film-done $(tail -1 "$out/p2790-film-$name.out")"
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
