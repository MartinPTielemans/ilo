# The sidebar on the window's right (#2375), on the Mac, where AppKit draws the window's buttons:
# a click on Right's drawing in Settings › Tabs & Sidebar › Sidebar position (#2407) moves it to
# the right while the app runs, and AppKit's buttons follow at once, close 20 points from the
# right edge as it is from the left, through a resize, the sidebar hidden and revealed from the
# right edge, and full screen; the command bar lists only the move that acts, which brings it
# back, buttons and all, and the setting's file goes with it. Elsewhere the UI draws the buttons,
# which the UI's tests and the parity screens check.

if [ "$(uname)" != Darwin ]; then
  echo "skip: sidebar-right checks AppKit's buttons, the Mac's alone."
else
page='data:text/html;charset=utf-8,<title>ready</title><body style="margin:0;height:100vh;background:%23e8edf0">'
log="$out/sidebar-right.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# side_buttons <stage> <side>: the buttons' centres, as the driver's layout line gives them from
# the window's top-left, are where AppKit puts them on <side>: 20, 43 and 66 from that edge.
side_buttons() {
  send layout
  width=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
  line=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout buttons' |
    cut -d' ' -f4-)
  echo "$1: window $width wide, close minimize zoom at $line" >> "$out/sidebar-right.txt"
  set -- "$1" "$2" $(echo "$line" | tr ' ' '\n' | cut -d. -f1 | tr '\n' ' ')
  case $2 in
    left) want="20 43 66" ;;
    right) want="$((width - 20)) $((width - 43)) $((width - 66))" ;;
  esac
  [ "$3 $4 $5" = "$want" ] ||
    fail "$1: the window's buttons are at $line, not at $want across, as on the $2"
}
shot() {
  ../tools/window-screenshot.sh "$pid" "$out/sidebar-right-$1.png" || fail "no window to capture"
}
: > "$out/sidebar-right.txt"
[ ! -e "$profile_dir/sidebar-right" ] || fail "the profile keeps the sidebar on the right already"
side_buttons "at launch, left" left

send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
if [ "$(sed -n 's/^browser: settings shown //p' "$log" | tail -1)" != "Tabs & Sidebar" ]; then
  send clickkey $(drawn "toolbar Tabs & Sidebar")
  expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its tab"
  settings_drew 'Tabs & Sidebar'
fi
# Sidebar position is two drawings of the window (#2407): a click on Right's picks it. The picker
# is at the pane's foot, under the card's edge where the screen is short, as on the Mac's check
# VMs: in_sight scrolls it up (#2503).
in_sight "sidebar position option Right" || fail "Settings didn't draw Right's drawing"
send clickkey $at
expect '^browser: sidebar on the right$' "a click on Right's drawing didn't move the sidebar"
[ -e "$profile_dir/sidebar-right" ] || fail "Right isn't kept: no sidebar-right in the profile"
expect '^browser: settings sidebar position Right at ' "Sidebar position didn't show Right"
shot settings
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
side_buttons "moved right" right
shot rest

send resize 1000 700
side_buttons "resized" right
send resize 1280 800
side_buttons "resized back" right

send key s cmd
expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar"
send move $((width - 400)) 300
send move $((width - 3)) 300
send move $((width - 3)) 306
expect '^browser: reveal shown$' "the pointer at the right edge didn't bring the sidebar out"
expect '^browser: reveal window shown$' "the sidebar out didn't come on screen"
shot revealed
send move $((width - 400)) 300
send key s cmd
expect '^browser: sidebar shown$' "⌘S didn't bring the sidebar back"
side_buttons "docked again" right

send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen"
shot fullscreen
send fullscreen
expect '^browser: window left full screen$' "the window didn't leave full screen"
side_buttons "after full screen" right

send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type move sidebar
expect '^browser: command bar commands "move sidebar": Move Sidebar to the Left$' \
  "the command bar didn't list only Move Sidebar to the Left: $(grep '^browser: command bar commands' "$log" | tail -1)"
shot command-bar
send key return
expect '^browser: sidebar on the left$' "Move Sidebar to the Left didn't move it back"
side_buttons "moved back left" left
[ ! -e "$profile_dir/sidebar-right" ] || fail "Left isn't kept: sidebar-right is still in the profile"
shot back-left
quit
echo "ok: the window's buttons followed the sidebar to the right and back: $out/sidebar-right.txt"
fi
