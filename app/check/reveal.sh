# [DEBUG-rv] A probe for #2133: hovers rows in the docked sidebar, then in the revealed one, by the
# edge and by ⌥, through the driver's moves and through the window server's, and logs each frame
# the browser window and the reveal's window draw, so the gap from a move to the next frame shows.

serve
log="$out/reveal.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: today [0-9][0-9]*$' || fail "the first tab isn't in Today"
send click $page_point
phases="$out/reveal-phases.txt"
echo "active $active" > "$phases"
open_from_bar example.com

# hover <label> <how>: the pointer goes between the New Tab row and the first two tabs' rows, a
# third of a second on each, then rests.
hover() {
  echo "$((step + 1)) $1 $2" >> "$phases"
  for _ in 1 2 3; do
    for n in -1 0 1; do
      send "$2" $(row $n)
      sleep 0.3
    done
  done
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/reveal-$1-$2.png" || fail "no window to capture"
}

hover docked move
hover docked hidmove

send key s cmd
expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar"
send move 400 300
send move 3 300
send move 3 306
expect '^browser: reveal shown$' "the pointer at the window's edge didn't bring the sidebar out"
expect '^browser: reveal window shown$' "the sidebar out didn't come on screen"
echo "$((step + 1)) edge rest" >> "$phases"
sleep 1
hover edge move
hover edge hidmove

send move 600 300
expect '^browser: reveal hidden$' "the pointer past the sidebar out didn't send it away"
send flags alt
send flags
expect '^browser: reveal by option$' "⌥ pressed alone didn't bring the hidden sidebar out"
expect '^browser: reveal shown$' "⌥ didn't bring the sidebar's panel out"
echo "$((step + 1)) option rest" >> "$phases"
sleep 1
hover option hidmove
hover option move

echo "$((step + 1)) end" >> "$phases"
exec 3>&-
quit
echo "ok: probe done. Log: $log"
