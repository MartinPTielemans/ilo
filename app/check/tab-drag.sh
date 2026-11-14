# check.sh runs this at 830, only when named.
# Only Wayland's sessions drag with the data device, so it runs only when named, and skips
# elsewhere, where the windows run checks the drag.
# On Wayland, a tab's row dragged out of its window is the data device's drag, the app's own, as
# the windows run checks on the Mac and X11 with the driver standing in for the drag between windows
# (#1859): there AppKit's drag and XDND's follow the real pointer, which the driver doesn't move, but
# under Wayland the driver's pointer is a device of sway's seat, so the row's own drag goes all the
# way. Two windows side by side: the row dragged past its window's bottom edge leaves it as the
# system's drag, with a picture of the row, and let go over the other window's list, the tab moves
# there where the line marks, its page live with what was typed in it, which ⌘Z sends back. Then,
# let go over that window's page, which takes no tab, it opens in a window of its own, which ⌘Z
# closes, sending the tab back. Only in Wayland's sessions: elsewhere the windows run checks it.
if [ "$(uname)" != Linux ] || [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: tab-drag. A row's own drag between windows runs only in Wayland's sessions; the windows run checks the rest."
  return 0
fi
serve
log="$out/tab-drag.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$(echo "$site" | sed 's/127\.0\.0\.1/localhost/')autofocus.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the page to drag didn't draw"
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
c=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
# Its field, at the page's top left, takes what's typed, which a page loaded again would lose.
send click 380 28
send type kept
expect "^browser: title $c value kept\$" "the page to drag didn't take what was typed"
kept() {
  [ "$(grep -c "^browser: loaded $c " "$log")" = 1 ] || fail "$1: the page loaded again"
  send click 380 28
  send type x
  expect "^browser: title $c value kept\(x*\)x\$" "$1: the page's field lost what was typed"
}
# The row of tab $1 in the driven window's Today.
row_of() {
  index=0
  for each in $(today); do
    [ "$each" = "$1" ] && row $index && return
    index=$((index + 1))
  done
}
# A tab of its own in the first window, so it keeps one as the page leaves.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "${site}notes.txt"
send key return
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the first window's other tab didn't load"
send click $(row_of $c)
expect "^browser: window $first shows $c\$" "a click on the page's row didn't show it"
# The windows side by side, their lists in the open, as the windows run places them.
left=10 right=640 wide=620
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
new_window_bar
send window 2
send place $right 40 $wide 700
send window 1
send place $left 40 $wide 700
# Dragged past the first window's bottom edge, its row leaves as the system's drag, with a picture
# of the row from the frame GPUI drew, as the Mac's does.
set -- $(row_of $c)
send press $1 $2
send dragto $1 $(($2 + 10))
send dragto $1 705
expect "^browser: tab $c window drag\$" "the row dragged past the window's edge didn't leave it"
expect "^browser: tab $c drag picture [1-9][0-9]*x[1-9][0-9]*\$" "the tab's drag showed no picture of its row"
# Over the second window's list, above its first row, and let go there: the compositor hands the
# drag to that window's surface, whose list takes the tab.
over=$((right - left + 100))
set -- $(row 0)
send dragto $over $(($2 - 20))
expect '^browser: drop line in today 0$' "no line marked the tab's place in the second window's list"
../tools/window-screenshot.sh "$pid" "$out/tab-drag-line.png" all || fail "no window to capture"
send release $over $(($2 - 20))
expect "^browser: tab $c dropped from window $first in today 0\$" \
  "the second window's list didn't put the tab where its line was"
expect "^browser: window $second shows $c\$" "the tab dropped on the second window's list didn't show there"
../tools/window-screenshot.sh "$pid" "$out/tab-drag-dropped.png" all || fail "no window to capture"
send window 2
kept "dropped on the other window's list"
# ⌘Z there, from the UI, as ⌘Z in a page's text field is the page's, shows it in the first window
# again, and ⇧⌘Z in the second takes it back.
send click 100 600
send key z cmd
expect "^browser: window $first shows $c\$" "⌘Z didn't send the dropped tab back to the first window"
send key z cmd shift
expect "^browser: window $second shows $c\$" "⇧⌘Z didn't bring the tab back to the second window's list"
# Dragged past the second window's bottom edge and let go over its page, which takes no tab, it
# opens in a window of its own.
set -- $(row_of $c)
send press $1 $2
send dragto $1 $(($2 + 10))
send dragto $1 705
expect "^browser: tab $c window drag\$" "the row dragged past the second window's edge didn't leave it"
send dragto $((wide - 100)) 400
send release $((wide - 100)) 400
expect "^browser: tab $c window drag to a new window\$" "let go where nothing took it, the tab's drag opened no window"
expect "^browser: window [0-9]* took $c from window $second\$" "no window took the tab let go where nothing took it"
third=$(grep "^browser: window [0-9]* took $c from window $second\$" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: window $third shows $c\$" "the window opened for the tab didn't show it"
../tools/window-screenshot.sh "$pid" "$out/tab-drag-out.png" all || fail "no window to capture"
send window 3
kept "let go where nothing took it"
send click 100 600
send key z cmd
expect "^browser: window closed $third\$" "⌘Z didn't close the window the tab's drag opened"
expect "^browser: window $second shows $c\$" "⌘Z didn't send the tab back to the window it came from"
exec 3>&-
quit
echo "ok: on Wayland, a tab's row dragged past its window's edge leaves it as the data device's drag, with a picture of the row, and let go over the other window's list moves the tab there live, which ⌘Z sends back and ⇧⌘Z takes again, or let go over a page opens it live in a window of its own, which ⌘Z closes, sending it back. Windows: $out/tab-drag-line.png, $out/tab-drag-dropped.png, $out/tab-drag-out.png"
