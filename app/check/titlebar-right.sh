# #2375's question for a Mac VM, on its test branch alone: with the browser windows' class answering
# right to left for windowTitlebarLayoutDirection, which is read-only (BROWSER_SIDEBAR_RIGHT,
# platform/mac/host.mm), do AppKit's own buttons take the mirrored
# places, zoom, minimize, close, with close 13 points from the right edge, as the core's inset puts
# it from the left (core/platform/mac/native_widget.mm), and keep them through a resize, full screen
# with its title bar come down, and the sidebar hidden, revealed and docked again? Only the buttons
# may move: the Window menu names the window as before. The UI isn't mirrored here, so its sidebar
# stays on the left; the buttons are AppKit's, which is the question. Each stage runs left (before)
# and right (after), logs the buttons' centres from the window's top-left into
# $out/titlebar-right.txt and captures the window. Full screen's title bar comes down only for the
# real pointer at the screen's top, which check/pointer.mm moves, so only in a CI job.
# The left run then turns the title bar right and back while the app runs, as #2389's setting does.

if [ "$(uname)" != Darwin ]; then
  echo "skip: titlebar-right is the Mac's alone."
else
summary_file="$out/titlebar-right.txt"
: > "$summary_file"
pointer=
if [ "${GITHUB_ACTIONS:-}" = true ]; then
  clang++ -fobjc-arc -framework Cocoa -o "$out/pointer" check/pointer.mm 2> "$out/pointer.log" ||
    fail "check/pointer.mm didn't build: $(cat "$out/pointer.log")"
  pointer="$out/pointer"
fi
page='data:text/html;charset=utf-8,<title>ready</title><body style="margin:0;height:100vh;background:%23e8edf0">'
# buttons <side> <stage>: logs the buttons' centres, as the driver's layout line gives them.
buttons() {
  send layout
  line=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout buttons' |
    cut -d' ' -f4-)
  content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    sed 's/^browser: layout content \([0-9]*x[0-9]*\).*/\1/')
  echo "$1 $2: window ${content:-?}, close minimize zoom at $line" >> "$summary_file"
}
shot() {
  ../tools/window-screenshot.sh "$pid" "$out/titlebar-$1-$2.png" ${3:-} || fail "no window to capture"
}
for side in left right; do
  log="$out/titlebar-$side.log"
  step=0
  right=
  [ "$side" = right ] && right=1
  BROWSER_SIDEBAR_RIGHT=$right BROWSER_DRIVE=1 BROWSER_MOTION=full BROWSER_URL="$page" \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  if [ -n "$right" ]; then
    grep -q '^browser: titlebar right to left$' "$log" ||
      fail "the host didn't lay the title bar out right to left: $(grep '^browser: titlebar' "$log")"
  fi
  buttons $side rest
  shot $side rest
  send resize 1000 700
  buttons $side resized
  send resize 1280 800
  buttons $side "resized back"
  shot $side resized
  send fullscreen
  expect '^browser: window entered full screen$' "$side: the window didn't go full screen"
  buttons $side "full screen, bar up"
  if [ -n "$pointer" ]; then
    send screen
    read -r sx sy sw _ << EOF
$(grep '^browser: screen at ' "$log" | tail -1 | cut -d' ' -f4-)
EOF
    "$pointer" move $((sx + sw / 2)) $((sy + 1)) > "$out/pointer.txt" 2>&1 ||
      fail "check/pointer.mm didn't move: $(cat "$out/pointer.txt")"
    # The bar comes down on its own spring: the stage is logged once the buttons' row has moved,
    # within 3 s, and logged as it is if it hasn't.
    before=$(grep '^browser: layout buttons' "$log" | tail -1)
    for _ in $(seq 30); do
      send layout
      [ "$(grep '^browser: layout buttons' "$log" | tail -1)" != "$before" ] && break
    done
    buttons $side "full screen, bar down"
    shot $side fullscreen all
    "$pointer" move $((sx + sw / 2)) $((sy + 400)) > /dev/null 2>&1 || true
  else
    echo "$side: full screen's bar not brought down: check/pointer.mm moves the real pointer, only in a CI job" >> "$summary_file"
    shot $side fullscreen all
  fi
  send fullscreen
  expect '^browser: window left full screen$' "$side: the window didn't leave full screen"
  buttons $side "after full screen"
  shot $side after-fullscreen
  send key s cmd
  expect '^browser: sidebar hidden$' "$side: ⌘S didn't hide the sidebar"
  buttons $side "sidebar hidden"
  send move 400 300
  send move 3 300
  send move 3 306
  expect '^browser: reveal shown$' "$side: the pointer at the edge didn't bring the sidebar out"
  buttons $side "sidebar revealed"
  shot $side revealed
  send move 400 300
  send key s cmd
  expect '^browser: sidebar shown$' "$side: ⌘S didn't bring the sidebar back"
  buttons $side "sidebar docked again"
  if [ -z "$right" ]; then
    # #2389's setting turns the title bar while the app runs, and each window answers by its own
    # mark; marking the views alone left the buttons where they were until the next resize (run
    # 42941747251). Each way of asking AppKit to place them again turns right and back, the
    # buttons logged after each turn, then a resize settles them before the next way.
    for how in layout style tile buttons size frame; do
      send titlebar right $how
      expect "^browser: titlebar turned right to left by $how" "$how: the title bar didn't turn"
      buttons $side "turned right by $how"
      [ "$how" = frame ] && shot $side turned-right
      send titlebar left $how
      expect "^browser: titlebar turned left to right by $how" "$how: the title bar didn't turn back"
      buttons $side "turned back by $how"
      send resize 1000 700
      send resize 1280 800
    done
  fi
  send menubar Window
  echo "$side Window menu: $(grep '^browser: menu bar Window: ' "$log" | tail -1 | cut -d' ' -f5-)" \
    >> "$summary_file"
  quit
done
echo "ok: the buttons' places at each stage, left then right: $summary_file." \
  "Windows: $(ls "$out"/titlebar-*.png | tr '\n' ' ')"
fi
