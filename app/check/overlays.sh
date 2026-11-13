# check.sh runs this at 675.
# Every window the Windows host floats, through the four events that left one stale (#2563): a menu
# opened by a real click (#2516), the display adapter restarting (#2536), a scale change (#2541),
# and Win+D and back (#2545). After each, each overlay is opened and closed as a user does, and the
# driver's lives say where it is: shown, it's on screen, shown or with the keyboard, at its browser
# window's scale; closed, it's hidden, off screen, with neither the keyboard nor the pointer's
# capture. For the scale change, each hidden overlay is told another scale first, as one made
# hidden on another display is. The names come from the host's code, and a name this run has no way
# to open fails it, so a new overlay is walked as soon as it's added. The foot bar, which nothing
# but BROWSER_FOOT_BAR shows yet and nothing hides, is only seen shown. Each shown overlay is
# captured with the windows over the browser window, as overlays-<event>-<name>.png.
# Real input needs CI's hosted desktop, and lives are the Windows host's; the Mac's and Linux's
# events are #2563's audit.

if ! windows; then
  echo "skip: overlays. The driver's lives are the Windows host's; #2563 audits the Mac and Linux."
  exit 0
fi
if [ -z "${CI:-}" ]; then
  echo "skip: overlays. Win+D and the adapter restart need CI's desktop, which is the check's own."
  exit 0
fi

# The names from the host's code: the five in kOverlayNames, each window it adopts, and the menu,
# one a line.
names=$({
  sed -n '/^const char\* const kOverlayNames/,/};/p' platform/windows/host.cc | grep -o '"[^"]*"'
  grep -o 'Adopt([^,]*, [^,]*, "[^"]*"' platform/windows/host.cc | grep -o '"[^"]*"'
  grep -o 'each(context, "[^"]*"' platform/windows/host.cc | grep -o '"[^"]*"'
} | tr -d '"' | awk '!seen[$0]++')
[ "$(printf '%s\n' "$names" | grep -c .)" -ge 20 ] ||
  fail "the host's code gave fewer than the 20 names it had (#2563): $(printf '%s\n' "$names" | tr '\n' ',')"
# The order the run walks them in. Each but the foot bar is opened and closed by open and close below.
order='command bar
find bar
hud
toast
tooltip
hover card
link target
menu
icon picker
theme picker
site controls
capture
dialog
link preview
boost editor
folder preview
offer
drop targets
reveal
foot bar'
unknown=$(printf '%s\n' "$names" | while read -r name; do
  printf '%s\n' "$order" | grep -qx "$name" || printf '%s, ' "$name"
done)
[ -z "$unknown" ] ||
  fail "the host floats ${unknown%, } as well, which this run has no way to open: add it to its order, open and close"

serve
# The page: a link to another site over its top 40%, which a pinned tab opens in a link preview,
# and below it a part that counts presses; "a" asks with an alert.
target="http://localhost:$port/notes.txt"
printf '%s' "<title>loading</title>
<a href=\"$target\" style=\"position:fixed;left:0;top:0;width:100%;height:40%;font:24px system-ui\">
A link to another site</a>
<div id=press style=\"position:fixed;left:0;top:40%;width:100%;height:60%;font:24px system-ui\">
Presses count here</div>
<script>
let presses = 0;
press.onmousedown = () => document.title = \"pressed \" + ++presses;
onkeydown = (event) => { if (event.key == \"a\") alert(\"overlays\"); };
requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"ready\"));
</script>" > "$out/site/overlays.html"
link_point="600 200"
# A Boost for the page's site, which Edit Boost's card customizes in the editor.
host=${site#http://}
host=${host%%:*}
mkdir -p "$profile_dir/Default/Boosts"
printf '%s\n' '/* overlays */' > "$profile_dir/Default/Boosts/$host.css"

log="$out/overlays.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_FOOT_BAR=arrange BROWSER_URL="${site}overlays.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the overlays page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: overlays. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
logged '^browser: drive: real press' || fail "CI's desktop gave no real input, which the run's events need"
await '^browser: foot bar shown over [0-9]*$' || fail "BROWSER_FOOT_BAR didn't show the foot bar"

# The tabs: the page pinned, then a folder under it holding a pinned tab, closed, and a login page
# in Today. The Today row is under the two pinned rows, the divider and "+ New Tab".
send key d cmd
expect "^browser: pinned $a\$" "⌘D didn't pin the overlays page"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "$target"
send key return
expect "^browser: loaded [0-9]* $target\$" "the folder's page didn't load"
n=$(today | cut -d' ' -f1)
send key d cmd
expect "^browser: pinned $a $n\$" "⌘D didn't pin the folder's page under the overlays page"
post rightclick $(pinned_row 1)
send choose "$(cased 'Add to Folder › New Folder')"
expect '^browser: rename folder [0-9]*$' "Add to Folder › New Folder gave no name field"
f=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
send key return
send click $(pinned_row 0)
expect "^browser: selected $a in " "a click on the pinned overlays page didn't show it"
bar_command collapse-folders "Collapse All Folders" "collapse all"
expect "^browser: outline $a $f-\\[$n\\]\$" "the folder didn't close under the pinned page"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "${site}login.html"
send key return
expect '^browser: title [0-9]* login$' "the login page didn't draw"
l=$(today | cut -d' ' -f1)
today_row="100 $((90 + 44 + 41 * 2 + 15 + 41 + 20))"
send click $(pinned_row 0)
expect "^browser: selected $a in " "a click on the pinned overlays page didn't show it again"

# Sets $got to the driver's lives entry for the window the host floats as $1, and $lives to all
# of them. Not in a command substitution, whose subshell would lose the step's count.
entry() {
  send lives
  lives=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: lives ')
  got=$(printf '%s\n' "${lives#browser: lives }" | sed 's/, /\n/g' |
    grep "^$1 \(hidden\|placed\|shown\|key\|hiding\)\( \|\$\)" || true)
}
# Fails unless the overlay $1 is $2, shown or hidden, by its lives entry, after the event $3.
lives_say() {
  entry "$1"
  case "$2" in
    shown)
      printf '%s\n' "$got" | grep -q "^$1 \(shown\|key\) on screen" &&
        ! printf '%s\n' "$got" | grep -q ' dpi [0-9]* over '
      ;;
    hidden)
      printf '%s\n' "$got" | grep -q "^$1 hidden" &&
        ! printf '%s\n' "$got" | grep -q ' on screen\| active\| focus\| capture'
      ;;
  esac || fail "after $3, the $1 should be $2: shown or key and on screen at its window's scale, or hidden, off screen, with neither the keyboard nor the capture. Lives say: ${got:-nothing of it}; all: $lives"
}
# Opens the overlay $1 as a user does, after the event $2.
open() {
  case "$1" in
    'command bar')
      send key t cmd
      expect '^browser: command bar shown$' "$2: ⌘T didn't open the command bar"
      ;;
    'find bar')
      send click $page_point
      post key f cmd
      expect '^browser: find bar shown$' "$2: ⌘F didn't open the find bar"
      ;;
    hud)
      send click $page_point
      send key - cmd
      expect '^browser: zoom hud shown$' "$2: ⌘- didn't show the zoom hud"
      ;;
    toast)
      send key c cmd shift
      expect "$url_toast" "$2: ⌘⇧C didn't show its toast"
      expect '^browser: toast window shown$' "$2: the toast's window didn't show"
      ;;
    tooltip)
      away
      send move 93 23
      expect '^browser: tooltip shown Hide Sidebar ' "$2: resting on Hide Sidebar showed no tooltip"
      expect '^browser: tooltip window shown$' "$2: the tooltip's window didn't show"
      ;;
    'hover card')
      away
      send move $today_row
      expect '^browser: hover card window shown$' "$2: resting on the Today tab showed no hover card"
      ;;
    'link target')
      send move $page_point
      send move $link_point
      expect '^browser: link target http' "$2: the pointer on the page's link showed no link target"
      expect '^browser: link target window shown$' "$2: the link target's window didn't show"
      ;;
    menu)
      post rightclick $today_row
      expect '^browser: menu window at ' "$2: a right-click on the Today tab showed no menu"
      ;;
    'icon picker')
      post rightclick $(pinned_row 0)
      send choose "$(cased 'Change Icon…')"
      expect '^browser: icon picker shown$' "$2: Change Icon… didn't open the icon picker"
      ;;
    'theme picker')
      post rightclick 100 112
      send choose "$(cased 'Edit Theme Color…')"
      expect '^browser: theme picker shown$' "$2: Edit Theme Color… didn't open the theme picker"
      ;;
    'site controls')
      send key l cmd
      expect '^browser: command bar shown$' "$2: ⌘L didn't open the command bar"
      send type site controls
      send key return
      expect '^browser: site controls settled$' "$2: Site Controls from the command bar didn't settle"
      ;;
    capture)
      send key 2 cmd shift
      expect '^browser: capture shown at ' "$2: ⌘⇧2 didn't show the capture"
      ;;
    dialog)
      send click $page_point
      send key a
      expect '^browser: dialog settled$' "$2: the page's alert didn't settle"
      ;;
    'link preview')
      send click $link_point
      expect '^browser: link preview shown$' "$2: the pinned page's link to another site didn't open its preview"
      expect '^browser: link preview settled$' "$2: the link preview didn't settle"
      ;;
    'boost editor')
      bar_command edit-boost "Edit Boost" "edit boost"
      expect "^browser: boost card $a at " "$2: Edit Boost drew no card"
      send layout
      read -r bx by bw bh << EOF
$(grep "^browser: boost card $a at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
      send click $((bx + bw / 2)) $((by + bh - 26))
      expect '^browser: boost editor drawn$' "$2: Customize… didn't open the Boost editor"
      ;;
    'folder preview')
      away
      send move $(pinned_row 1)
      expect "^browser: folder preview shown $f at " "$2: resting on the closed folder didn't preview it"
      ;;
    offer)
      send click $today_row
      expect "^browser: selected $l in " "$2: a click on the login tab didn't show it"
      go_from_bar "${site#http://}login.html"
      expect "^browser: title $l login\$" "$2: the login page didn't draw"
      send click 600 60
      offers=$((offers + 1))
      log_in "overlays-$offers"
      expect '^browser: offer shown$' "$2: the login brought no offer"
      ;;
    'drop targets')
      send layout
      frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
        grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
      send press $today_row
      send dragto 110 ${today_row#* }
      send dragto $((${frame%%,*} + 140)) $((${frame#*,} + 200))
      expect '^browser: drop targets shown$' "$2: the Today tab dragged onto the page brought no targets"
      ;;
    reveal)
      send key s cmd
      expect '^browser: sidebar hidden$' "$2: ⌘S didn't hide the sidebar"
      send move 600 300
      send flags winalt
      send flags
      expect '^browser: reveal shown$' "$2: Alt alone didn't bring the hidden sidebar out"
      ;;
  esac
}
# Closes the overlay $1, after the event $2.
close() {
  case "$1" in
    'command bar' | 'find bar' | 'theme picker' | 'site controls' | capture | 'link preview' | \
      'boost editor')
      send key escape
      expect "^browser: $1 hidden\$" "$2: Escape didn't close the $1"
      ;;
    menu)
      send key escape
      expect '^browser: menu window hidden$' "$2: Escape didn't close the menu"
      ;;
    # The offer doesn't take the keyboard, so Escape would go to the page: another tab shown
    # closes it, as passwords' does.
    offer)
      send click $(pinned_row 0)
      expect '^browser: offer hidden$' "$2: showing the pinned page didn't close the offer"
      ;;
    hud)
      send key 0 cmd
      expect '^browser: zoom hud hidden$' "$2: the zoom hud didn't go by itself"
      ;;
    toast)
      expect '^browser: toast hidden$' "$2: the toast didn't go by itself"
      ;;
    tooltip)
      away
      expect '^browser: tooltip hidden$' "$2: the pointer leaving Hide Sidebar left its tooltip"
      ;;
    'hover card' | 'link target' | 'folder preview')
      send move $page_point
      expect "^browser: $1 hidden\$" "$2: the pointer leaving left the $1"
      ;;
    'icon picker')
      send insert glowing star
      send key return
      expect '^browser: icon picker hidden$' "$2: choosing an icon didn't close the icon picker"
      ;;
    dialog)
      send key return
      expect '^browser: dialog hidden$' "$2: Return didn't answer the page's alert"
      ;;
    'drop targets')
      send dragto $today_row
      expect '^browser: drop targets hidden$' "$2: the tab back over the sidebar left the drop targets"
      send release $today_row
      ;;
    reveal)
      send move 650 320
      send flags winalt
      send flags
      expect '^browser: reveal hidden$' "$2: Alt alone again didn't take the sidebar away"
      ;;
  esac
}
offers=0
# Logs in on the login page, its username field with the keyboard, with password $1, as
# passwords' does, as a username of its own: Chrome counts the offers ignored for a site and
# username, and stops offering once there have been a few.
log_in() {
  send key a cmd
  send type "check-$1@example.test"
  send key tab
  send key a cmd
  send type "$1"
  send key return
}
# Opens and closes each overlay after the event $1, named $2 in its captures. With $3, each hidden
# one made so far is told the scale $3 first.
walk() {
  # The names come in on 7, as a loop in a pipe would run in a subshell, where a failure wouldn't
  # end the run and the steps' count would be lost.
  while read -r name <&7; do
    if [ -n "${3:-}" ] && [ "$name" != 'foot bar' ]; then
      entry "$name"
      [ -z "$got" ] || {
        send dpi "$3" "$name"
        expect "^browser: dpi [0-9]* told $name\$" "$1: the hidden $name wasn't told another scale"
      }
    fi
    open "$name" "$1"
    lives_say "$name" shown "$1"
    ../tools/window-screenshot.sh "$pid" "$out/overlays-$2-$(echo "$name" | tr ' ' -).png" all ||
      fail "no window to capture"
    [ "$name" != 'foot bar' ] || continue
    close "$name" "$1"
    lives_say "$name" hidden "$1"
    # Each overlay starts from the pinned page shown, as some take the tab elsewhere.
    if [ "$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)" != "$a" ]; then
      send click $(pinned_row 0)
      expect "^browser: selected $a in " "$1: a click on the pinned overlays page didn't show it"
    fi
    [ "$name" != reveal ] || {
      send key s cmd
      expect '^browser: sidebar shown$' "$1: ⌘S didn't bring the sidebar back"
    }
  done 7<< EOF
$order
EOF
}

# A menu opened by a real click, as #2516's left the pointer's capture with the browser window.
post rightclick $today_row
expect '^browser: menu window at ' "a real right-click on the Today tab showed no menu"
send key escape
expect '^browser: menu window hidden$' "Escape didn't close the menu"
walk "a menu opened by a real click" menu
events="a menu opened by a real click"

# The display adapter restarting, as a driver update does, which took GPUI's device away (#2536).
# pnputil needs an administrator, as CI's runner is.
restarted=$(powershell.exe -NoProfile -Command \
  'Get-PnpDevice -Class Display -Status OK | ForEach-Object { pnputil /restart-device $_.InstanceId }' \
  2>&1 || true)
case "$restarted" in
  *"restarted successfully"*)
    send overlays
    expect '^browser: gpui redrew its windows on a new device$' \
      "the windows drew nothing after the display adapter restarted"
    walk "the display adapter restarting" adapter
    events="$events, the display adapter restarting"
    ;;
  *) echo "skip: overlays after the display adapter restarting, as pnputil couldn't: $restarted" ;;
esac

# The browser window at 125%, each hidden overlay told 150% first, as one made hidden on a display
# of another scale and shown over the browser window's (#2541).
send dpi 125
walk "the scale changing to 125%" scale 150
send dpi 100
events="$events, the scale changing"

# Win+D and back, real keys, as #2545's left the hidden command bar the active window. The page
# hearing a real click says the window is back.
send realkey d win
send realkey d win
pressed=$(grep -c "^browser: title $a pressed " "$log" || true)
send click $page_point
for _ in $(seq 100); do
  [ "$(grep -c "^browser: title $a pressed " "$log" || true)" -gt "$pressed" ] && break
  sleep 0.1
done
[ "$(grep -c "^browser: title $a pressed " "$log" || true)" -gt "$pressed" ] ||
  fail "the page didn't hear a click once the window was back from Win+D"
walk "Win+D and back" win-d
events="$events, Win+D and back"

exec 3>&-
quit
echo "ok: overlays. Each of the $(printf '%s\n' "$order" | grep -c .) the host floats, after $events: shown on screen at its window's scale, and closed off screen with neither the keyboard nor the capture."
