# check.sh runs this at 70.
# New Space in the Spaces menu makes a Space and shows it, empty and without a page, with a form
# that takes the keyboard from the page. Japanese input composes in its field, return while
# composing only commits, and return names the Space and sends the form away; ⌘T then opens a tab in
# it. ⌃1–⌃9, ⌥⌘← and ⌥⌘→, with the page or the UI focused, and clicks on the switcher's dots show
# another Space, which shows the tab it showed last. A dot dragged along the switcher, or a Library
# card by its handle, moves its Space, which ⌘Z undoes. Two fingers across the sidebar page to the
# Space a flick or the pages' place points at, never past the last, fingers along it don't, and a
# wheel's tick across pages once. A right-click on a row as a Space's rows slide in shows its
# tab's menu. A tab's menu moves the tab to another Space, which the window shows with it, and
# into another Space's folder, listed under its Today, which ⌘Z undoes, as the command bar does
# by the Space's or folder's name, and a folder's menu moves
# it whole to another Space, which ⌘Z undoes too. Export “name” into Folder › another Space makes
# the Space a folder there, which one ⌘Z takes back. Return
# with no name does nothing, New Space from the footer's + shows a form
# already open, and Cancel, or two fingers back, deletes its Space and goes back to the Space it
# was made from; fingers going on rubber-band the form. A right-click on the Space's name, a
# dot or its card in the Library shows Change Space Icon…, Rename Space…, Edit Theme Color… and Delete
# Space, and Remove Theme Color once the Space has a colour, and one on the sidebar's empty space
# shows New Tab and New Folder… before them, and, while the first launch's Space is the only one
# and unnamed, Hide Space Header, which hides its name row, and then Show Space Header; an emoji
# committed after Change Icon… is the Space's icon, in its dot's place, until Remove Icon. Rename,
# the Spaces
# menu's too, and a double-click on the name, turn it into a field, where a right-click shows the
# field's menu, return keeps the name and escape cancels. A Library card's field lights with the
# pointer on its brush or icon slot too. The brush opens the theme picker beside itself, which
# makes the Space light and dark, and the slot hangs the icon picker from itself, the Library
# staying open under them; another card's slot moves the picker there, and escape closes the
# picker, then the Library. A click on the card's name makes it a field in the card, the Library
# staying open. Delete asks first for a Space with tabs,
# in a dialog that a click around cancels and return confirms, and the Space goes once the dialog
# has, which a toast says. ⌘Z brings it back where it was, with its name and tabs. Its tabs close
# with it and reopen with ⇧⌘T in the Space shown, which ⌘Z then leaves out, and the last Space
# can't be deleted. The Spaces menu lists the Spaces, and its names show them. A page out of sight,
# behind another tab, in a Space not shown, in a minimized window or a hidden app, is hidden and
# runs no animation frames, and it's visible once it's back.
# A Space made from the first of two goes right after it, as Arc's does.

# Spaces. A new one is empty, and each keeps its tabs and the one it showed last. The page puts
# "hidden" in its title as it hides, and "visible <n>" as it shows again, with the n animation
# frames it ran while hidden, where Chrome runs none, and "in <ms>" from the first to the last.
# Hidden, it says "hidden f<frames> t<ticks>" at each frame and each second its timer ticks, so the
# log has each frame in order with what the app asked of the page (ran_hidden).
page='data:text/html,<title>loading</title>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let frames = 0, ticks = 0, first = 0, last = 0, ticking = 0;
const say = () => document.title = "hidden f" + frames + " t" + ticks;
const count = (now) => {
  if (document.hidden) {
    if (!frames) first = now;
    frames++;
    last = now;
    say();
    requestAnimationFrame(count);
  }
};
document.onvisibilitychange = () => {
  clearInterval(ticking);
  document.title = document.hidden ? "hidden"
    : "visible " + frames + (frames ? " in " + Math.round(last - first) : "");
  if (document.hidden) {
    frames = ticks = 0;
    ticking = setInterval(() => (ticks++, say()), 1000);
    requestAnimationFrame(count);
  }
};
</script>'
log="$out/spaces.log"
step=0
# X11 sends a touchpad's scrolling with no start or end, so two fingers never page there (#554):
# its steps go to the Spaces and the Library by key, click and wheel instead. Its clicks land on
# whole pixels, at scale 1 here. Windows' driver hands GPUI two fingers as Direct Manipulation
# does a precision touchpad's (#1212).
x11=
fingers=1
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  x11=1
  fingers=
  echo "skip: two fingers across the sidebar, on X11, which sends no touch phases (#554)."
fi
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the Spaces test page didn't draw"
first=$(space)
await "^browser: spaces $first\$" || fail "the app didn't start with one Space"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: Spaces. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The switcher's dots are centred under the sidebar, 25 above the window's bottom.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
dots_y=$((${content#*x} - 25))

# The first launch's Space is the browser's only one, "Space 1" with no icon, which Arc 1.171
# counts as unnamed and hides the name row of, what's under it rising 39. The runs keep the row
# (check.sh), so the sidebar's empty space's menu offers Hide Space Header after Edit Theme Color…,
# which puts New Tab, under the divider, 39 above where it was. Show Space Header brings the row
# back for the steps after (#1400).
post rightclick $(row 3)
expect "^browser: menu shown $(cased 'New Tab, New Folder…, Change Space Icon…, Rename Space…, Edit Theme Color…, Hide Space Header'), " \
  "the lone unnamed Space's menu didn't offer Hide Space Header after Edit Theme Color…"
send choose "$(cased 'Hide Space Header')"
expect '^browser: space header hidden$' "Hide Space Header didn't hide the name row"
expect '^browser: outline ' "the rows weren't drawn again without the name row"
../tools/window-screenshot.sh "$pid" "$out/space-header-hidden.png" || fail "no window to capture"
send click 100 $((90 + 44 + 15 + 20 - 39))
expect '^browser: command new-tab via sidebar$' "New Tab didn't rise into the hidden name row's place"
expect '^browser: command bar shown$' "a click on the risen New Tab didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
post rightclick $(row 3)
expect "^browser: menu shown .*$(cased 'Edit Theme Color…, Show Space Header'), " \
  "the hidden name row's Space's menu didn't offer Show Space Header"
send choose "$(cased 'Show Space Header')"
expect '^browser: space header kept$' "Show Space Header didn't bring the name row back"
expect '^browser: outline ' "the rows weren't drawn again with the name row"

send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
expect_tabs " *" 0 "in the new Space"
send layout
sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' | grep -q WebContentsViewCocoa &&
  fail "the new Space shows the first Space's page"
# The form's field has the keyboard, which the page had.
in_browser AccessKitSubclassOfGPUIView "in the new Space's form"
../tools/window-screenshot.sh "$pid" "$out/space-form.png" || fail "no window to capture"
send type reading
send key space
send ime on
send type nihongo
send key space
[ "$marked" = 8,3 ] || fail "the form's field didn't take the conversion: marked $marked"
../tools/window-screenshot.sh "$pid" "$out/space-form-ime.png" || fail "no window to capture"
committed=$((step + 1))
send key return
[ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
send ime off
# The form's Choose a Theme row opens the theme picker beside it, a child window with the
# keyboard, so its clicks go to the key window in points from its top left, its arrow 13 wide.
# A click in its field is the Space's first colour, where it's clicked, and the window takes it.
send click 100 299
expect "^browser: theme picker for $second\$" "the form's Choose a Theme row didn't open the picker"
expect '^browser: theme picker shown$' "the theme picker didn't show"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
none_since '^browser: create space' "$committed" || fail "return while composing named the Space"
at=$(grep '^browser: theme picker at' "$log" | tail -1 | cut -d' ' -f5-)
[ "$(echo "$at" | awk '{ print $3, $4 }')" = "370 509" ] ||
  fail "the theme picker opened at $at, not 370 by 509"
# Half way from the field's middle to its right edge is red: hue 0 at lightness 0.5. X11's and
# Windows' drivers click on whole pixels, which lands half a point right of it and below, a little
# less on Windows, whose picker opens a fraction of a point to the right (#1368).
red_at="x=0.5000 y=0.0000"
red_color=ff0000
[ -z "$x11" ] || { red_at="x=0.5030 y=0.0030"; red_color=ff0302; }
! windows || { red_at="x=0.5029 y=0.0029"; red_color=ff0302; }
rest="intensity=0.810 grain=0.000 appearance=automatic"
red="$red_at colors=1 $rest"
send clickkey 276.5 178.5
expect "^browser: theme picker set $second $red\$" "a click in the field didn't give a colour"
expect "^browser: colored space $second $red_color\$" "the field's colour wasn't the Space's colour"
expect "^browser: themed space $second $red\$" "the core didn't keep the Space's theme"
# + makes it two colours, the second the first's opposite.
send clickkey 211.5 316.5
expect "^browser: themed space $second $red_at colors=2 $rest\$" "+ didn't add a second colour"
../tools/window-screenshot.sh "$pid" "$out/theme-picker.png" || fail "no window to capture"
# On the Mac, AppKit can make an overlay's panel key as it shrinks away after the UI hid it, and
# what the user typed went to it unseen (#2594). The driver makes the picker key as its shrink
# starts: the keyboard still comes back to the form, for the return below.
if [ "$(uname)" = Darwin ]; then
  send makekey theme picker
fi
send key escape
expect '^browser: theme picker dismissed$' "escape didn't close the theme picker"
expect '^browser: theme picker hidden$' "escape didn't hide the theme picker"
if [ "$(uname)" = Darwin ]; then
  expect '^browser: made theme picker key$' "the theme picker wasn't made key as its shrink started"
  expect '^browser: theme picker hidden, its browser window takes the keyboard$' \
    "the theme picker, made key as it shrank, kept the keyboard"
else
  echo "skip: the theme picker made key as it shrinks away: AppKit making a shrinking panel key is the Mac's (#2594)."
fi
send state
in_browser AccessKitSubclassOfGPUIView "after the theme picker, in the new Space's form"
send key return
expect "^browser: create space $second reading 日本語\$" "return didn't name the Space: $(grep '^browser: create space' "$log" | tail -1)"
expect "^browser: renamed space $second reading 日本語\$" "the core didn't rename the Space"
expect '^browser: space form gone$' "the form didn't leave after Create"
../tools/window-screenshot.sh "$pid" "$out/new-space.png" || fail "no window to capture"
send key t cmd
expect '^browser: command new-tab via ui$' "⌘T in the new Space didn't reach the UI"
expect '^browser: command bar shown$' "⌘T in the new Space didn't open the command bar"
send type example.com
send key return
expect '^browser: today [0-9][0-9]*$' "the tab for example.com didn't open in the new Space"
b=$(today)
expect "^browser: selected $b\$" "the new Space's tab wasn't shown"
expect "^browser: loaded $b https://example.com/\$" "example.com didn't load in the new Space's tab"

# Opens the command bar with ⌘T and types $1, and once the core's rows are in among the bar's,
# captures it in $3 if given and picks the row named $2 with ↓ and ↩, checking that it ran.
by_name() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$1"
  expect "^browser: command bar lists \"$1\": .*https://duckduckgo.com/" "the core's rows for \"$1\" never came"
  index=$(grep "^browser: command bar lists \"$1\": " "$log" | tail -1 |
    sed 's/^browser: command bar lists "[^"]*": //' |
    awk -v want="$2" '{ n = split($0, rows, ", "); for (i = 1; i <= n; i++) if (rows[i] == want) { print i - 1; exit } }')
  [ -n "$index" ] || fail "the command bar didn't list $2 for \"$1\": $(grep "^browser: command bar lists \"$1\": " "$log" | tail -1)"
  if [ -n "$3" ]; then
    ../tools/window-screenshot.sh "$pid" "$out/$3" || fail "no window to capture"
  fi
  # Counted, as the Mac's seq 0 counts down to 0 rather than nothing.
  down=0
  while [ "$down" -lt "$index" ]; do
    send key down
    down=$((down + 1))
  done
  send key return
  expect "^browser: command bar runs $2\$" "↩ on $2 ran another row"
}

# How a name's field last lost the keyboard, if it did, for a field that ended unasked (#1664).
lost_keyboard() {
  grep '^browser: field lost the keyboard' "$log" | tail -1 | grep . || echo "it never lost it"
}

# $1 is the driver's key or click, $2 the command it runs, $3 where the UI hears it from, and $4
# the Space it shows, with its tab $5.
show_space() {
  send $1
  expect "^browser: command $2 via $3\$" "$1 with the $3 focused didn't run $2"
  expect "^browser: space $4 in " "$1 with the $3 focused didn't show Space $4"
  # The menu shows the shortcut too, and a key the shell ran never reaches it.
  [ "$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: command ')" = 1 ] ||
    fail "$1 with the $3 focused ran more than one command"
  expect_tabs "$5" "$5" "after $1 showed Space $4"
}
send click $page_point
show_space "key 1 ctrl" select-space page "$first" "$a"
send click $page_point
show_space "key 2 ctrl" select-space page "$second" "$b"
send click $page_point
show_space "key left alt cmd" previous-space page "$first" "$a"
send click $page_point
show_space "key right alt cmd" next-space page "$second" "$b"
# Spaces don't wrap around.
send key right alt cmd
expect '^browser: command next-space via page$' "⌥⌘→ at the last Space didn't reach the UI"
send state
none_since '^browser: space [0-9]' || fail "⌥⌘→ at the last Space showed another"
send click $sidebar_point
show_space "key 1 ctrl" select-space ui "$first" "$a"
show_space "key right alt cmd" next-space ui "$second" "$b"
show_space "key left alt cmd" previous-space ui "$first" "$a"
show_space "click 131 $dots_y" select-space sidebar "$second" "$b"
show_space "click 97 $dots_y" select-space sidebar "$first" "$a"
../tools/window-screenshot.sh "$pid" "$out/spaces.png" || fail "no window to capture"

# A dot dragged along the switcher moves its Space: the second's let go before the first's goes
# first, and ⌘Z puts it back. Held there, its copy is at the pointer and the line before the first
# (#1580). A drag with one move, as a flick, moves it too.
send press 131 $dots_y
send dragto 125 $dots_y
expect "^browser: drag space $second\$" "the second Space's dot didn't come away with the pointer"
send dragto 70 $dots_y
../tools/window-screenshot.sh "$pid" "$out/space-drag.png" || fail "no window to capture"
send release 70 $dots_y
expect "^browser: dropped space $second after 0\$" "the second Space's dot let go before the first's didn't move it"
expect "^browser: spaces $second $first\$" "the second Space didn't go first"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the Space's move"
expect "^browser: spaces $first $second\$" "⌘Z didn't put the Spaces back in their order"
# Until the dots have glided back, the second's is still drawn under the first's place (#2026).
expect '^browser: spaces settled$' "the dots didn't settle after ⌘Z"
send drag 97 $dots_y 160 $dots_y
expect "^browser: dropped space $first after $second\$" "the first Space's dot flicked past the second's didn't move it"
expect "^browser: spaces $second $first\$" "the first Space didn't go last"
send key z cmd
expect "^browser: spaces $first $second\$" "⌘Z didn't put the Spaces back after the flick"

# A page out of sight is hidden, as in Chrome, and runs no animation frames: a tab behind the one
# shown, and the tab a Space not shown shows. Back in sight, it's visible.
#
# But for the frames its hover card's picture draws. The tabs a switch hides are pictured once the
# switches have been quiet for 0.4 s (shell/hover_cards.rs), and the core has a hidden page draw,
# still hidden to it, until the copy answers (capture.cc), as Chrome does a background tab's
# preview. How long that takes is the machine's: 2 s once on a slow runner (#2239). So no time is
# judged here, only order. Each frame the hidden page runs is in the log, and it has a reason only
# once the app has asked for the tab's picture ("tab <a> picturing"). It keeps that reason past the
# picture's answer ("tab <a> pictured"), which may come with no copy, until the page has been quiet
# for a whole tick of its 1 s timer, as the copy's last frames can reach the log after it. A frame
# with no picture asked for fails, and so does a page that never goes quiet. The page counts afresh
# at each "hidden". The hidden stretch starts at whichever comes first, the core showing another
# tab or the page's "hidden", which can reach the log after the picture (#1678), so a picture
# between the two counts and one from before them doesn't.
hidden_state() {
  awk -v a="$a" '
    function start() { asked = drawing = 0; why = ""; counts() }
    function counts() { quiet = visible = frames = ticks = 0; tick_frames = -1 }
    BEGIN { start() }
    /^browser: selected [0-9]+( in |$)/ { if ($3 == a) shown = 1; else if (shown) { shown = 0; start() } }
    $0 == "browser: title " a " hidden" { if (shown) { shown = 0; start() } else counts() }
    index($0, "browser: title " a " visible ") == 1 { visible = 1; asked = 0 }
    index($0, "browser: tab " a " picturing ") == 1 { asked = drawing = 1; tick_frames = -1 }
    index($0, "browser: tab " a " pictured ") == 1 { drawing = 0; tick_frames = -1 }
    index($0, "browser: title " a " hidden f") == 1 {
      split(substr($0, length("browser: title " a " hidden f") + 1), n, " t")
      if (n[1] > frames && !asked && why == "")
        why = "frame " n[1] " came with no picture asked for, " (quiet ? "after the page had gone quiet" : "since it hid")
      if (n[2] > ticks && !drawing) {
        if (tick_frames == n[1]) { asked = 0; quiet = 1 } else tick_frames = n[1]
      }
      frames = n[1]; ticks = n[2]
    }
    END {
      state = why != "" ? "ran: " why : visible ? "visible" : quiet ? "quiet" : "running"
      print state, "(" frames " frames, " ticks " ticks)"
    }' "$log"
}
# Before the tab shows again: the page has gone quiet, with no frame it had no reason for, or it's
# shown already.
hidden_quiet() {
  local state
  for _ in $(seq 300); do
    state=$(hidden_state)
    case $state in
    ran:*) fail "$1: $state" ;;
    quiet* | visible*) return 0 ;;
    esac
    kill -0 "$pid" 2>/dev/null || gone
    sleep 0.1
  done
  fail "$1: it didn't go quiet, $state"
}
# Once it shows: nothing ran between its going quiet and its showing.
ran_hidden() {
  local state
  state=$(hidden_state)
  case $state in
  ran:*) fail "$1: $state" ;;
  esac
}
open_from_bar example.org
expect "^browser: title $a hidden\$" "the tab behind the one shown isn't hidden"
hidden_quiet "the tab behind the one shown ran frames while hidden, other than its picture's"
send key w cmd
expect_tabs "$a" "$a" "after closing the tab over the page"
expect "^browser: title $a visible " "the tab shown again isn't visible"
ran_hidden "the tab shown again ran frames while hidden, other than its picture's"
show_space "click 131 $dots_y" select-space sidebar "$second" "$b"
expect "^browser: title $a hidden\$" "the tab in the Space not shown isn't hidden"
hidden_quiet "the tab in the Space not shown ran frames while hidden, other than its picture's"
show_space "click 97 $dots_y" select-space sidebar "$first" "$a"
expect "^browser: title $a visible " "the tab in the Space shown again isn't visible"
ran_hidden "the tab in the Space shown again ran frames while hidden, other than its picture's"
# A Space shows its last-used tab again: with two in Today and the lower one shown, a switch away
# and back shows the lower one, not the top one.
open_from_bar example.org
c=$tab
send click "$(row 1)"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the lower row didn't show tab $a"
show_space "click 131 $dots_y" select-space sidebar "$second" "$b"
send click 97 $dots_y
expect '^browser: command select-space via sidebar$' "the first Space's dot didn't run select-space"
expect "^browser: space $first in " "the first Space's dot didn't show it"
# The swipes below would catch a page still sliding in, so it settles here: once the next command
# is sent, expect no longer reads the lines this switch logs.
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
expect_tabs "$c $a" "$a" "back in the first Space, its last-used tab"
../tools/window-screenshot.sh "$pid" "$out/space-last-used.png" || fail "no window to capture"
# A row carried across the sidebar's right edge out over the page is no longer at the edge, and
# held there past the edge's 0.5 s wait, the hand's hold the sleep is, it pages nowhere (#1466).
send press $(row 0)
send dragto 110 $(row 0 | cut -d' ' -f2)
send dragto 220 $(row 0 | cut -d' ' -f2)
send dragto $page_point
expect "^browser: drop targets for $a\$" "a row dragged over the page didn't bring the targets"
sleep 1 # the row held over the page past a Space edge's paging delay
none_since '^browser: drag paged to space' || fail "a row held over the page paged to the next Space"
send dragto $(row 0)
expect '^browser: drop targets gone$' "the targets stayed with the row back over the sidebar"
send release $(row 0)
# A row held at the sidebar's right edge pages to the next Space, and let go there it moves to it.
# Held on another row's middle half, it doesn't join that row in a split on the way, whose rest is
# shorter than the edge's wait (#1089). ⌘Z puts it back in its Space, where the window doesn't go
# with it, as the window shows another tab.
send press $(row 0)
send dragto 110 $(row 0 | cut -d' ' -f2)
send dragto 220 $(row 1 | cut -d' ' -f2)
expect "^browser: drag paged to space $second\$" "a row held at the sidebar's edge didn't page"
none_since '^browser: drop onto [0-9]' || fail "a row held at the edge rested on another's row"
send dragto $(row 0)
send release $(row 0)
expect "^browser: dragged $c to space $second today\$" "a row let go in the next Space didn't move"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the drag to the next Space"
expect "^browser: today $b\$" "⌘Z didn't take the dragged tab back: \"$(today)\""
send click 97 $dots_y
expect '^browser: command select-space via sidebar$' "the first Space's dot didn't run select-space"
expect "^browser: space $first in " "the first Space's dot didn't show it"
expect_tabs "$c $a" "$a" "after ⌘Z put the dragged tab back"
# Sent while the Space's page still slides in, the click acts on the row that rests under it.
send click "$(row 0)"
press=$step
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "a click on the top row didn't show tab $c"
# Where frames draw slower than the slide's 0.3 s (#732), the page settles before the app has the
# click, which then tests only the row at rest; the run says so, as for the right-click below.
if ! sed -n "/^browser: drive $((press - 1)): /,\$p" "$log" |
  awk -v sent="^browser: drive $press: " '/^browser: settled space/ { late = 1 }
    $0 ~ sent { clicked = 1 } clicked && /^browser: selected / { exit late }'; then
  { [ "$(uname)" = Linux ] || windows || [ -n "${GITHUB_ACTIONS:-}" ]; } ||
    fail "the page had settled before the click on the top row, so it didn't click mid-slide"
  echo "skip: a click on a row mid-slide: the page had settled before the app had the click" \
    "(#732, #1437)."
fi
send key w cmd
expect_tabs "$a" "$a" "after closing the last-used check's tab"

# Two fingers across the sidebar page between Spaces. Past halfway they go on to the next Space, a
# drag let go there still goes to the Space nearest its place, and past the last Space the pages come
# back. A flick short of halfway is the UI's test's (swipe.rs): the swipe's speed is read as the app
# handles its moves, and a hosted runner's frame, which can take longer than that reading's 0.1 s,
# can fall between the last move and the lift, which reads as fingers held still (#1869). Fingers
# along the list scroll it, and a wheel's tick across pages once. Held partway, the next Space's
# page shows beside the first's.
swipe_point="110 500"
if [ -n "$fingers" ]; then
post swipe $swipe_point -10 0 10 1500
await '^browser: swipe across$' || fail "two fingers across the sidebar didn't page"
../tools/window-screenshot.sh "$pid" "$out/swipe.png" || fail "no window to capture"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
none_since '^browser: space [0-9]' || fail "a swipe let go short of halfway showed another Space"
# $1 is the driver's swipe, $2 the Space it shows and $3 that Space's tab.
swipe_to() {
  send swipe $swipe_point $1
  expect '^browser: command select-space via swipe$' "a swipe of $1 didn't page"
  expect "^browser: space $2 in " "a swipe of $1 didn't show Space $2"
  expect_tabs "$3" "$3" "after a swipe of $1 showed Space $2"
}
swipe_to "-30 0 10" "$second" "$b"
swipe_to "30 0 10" "$first" "$a"
swipe_to "-30 0 20 200" "$second" "$b"
send swipe $swipe_point -12 0 10
expect '^browser: swipe across$' "two fingers across the sidebar didn't page"
none_since '^browser: space [0-9]' || fail "a flick past the last Space showed another"
send swipe $swipe_point 1 -10 10
expect '^browser: swipe along$' "two fingers along the list didn't read as scrolling"
none_since '^browser: space [0-9]' || fail "two fingers along the list showed another Space"
else
  # The page has the keyboard from the ⌘W above.
  show_space "key 2 ctrl" select-space page "$second" "$b"
  expect "^browser: settled space $second\$" "the second Space's page didn't settle"
fi
send wheel $swipe_point 1
expect '^browser: command select-space via wheel$' "a wheel's tick across the sidebar didn't page"
expect "^browser: space $first in " "a wheel's tick across the sidebar didn't show the first Space"
expect_tabs "$a" "$a" "after a wheel's tick showed the first Space"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
# A mouse's side buttons over the sidebar show the next Space and the previous, as Arc's (#688).
# Windows' driver can't press them yet (drive.cc).
if windows; then
  echo "skip: side buttons over the sidebar: the Windows driver can't press them yet."
else
  send sideclick $swipe_point forward
  expect '^browser: command select-space via mouse$' "the forward side button over the sidebar didn't page"
  expect "^browser: space $second in " "the forward side button over the sidebar didn't show the second Space"
  send sideclick $swipe_point back
  expect '^browser: command select-space via mouse$' "the back side button over the sidebar didn't page"
  expect "^browser: space $first in " "the back side button over the sidebar didn't show the first Space"
  expect_tabs "$a" "$a" "after the back side button showed the first Space"
  expect "^browser: settled space $first\$" "the first Space's page didn't settle after the side buttons"
fi
# Past the first Space is the Library. Held partway it shows beside the page, and let go short of
# halfway it goes back. A flick brings it out and one the other way puts it back, and so do a
# wheel's ticks.
# $1 is how the driver moves, $2 its source and $3 whether the Library shows after it.
library_to() {
  send $1
  expect "^browser: command view-library via $2\$" "a $1 didn't reach the Library"
  expect "^browser: library $3\$" "a $1 didn't leave the Library $3"
}
if [ -n "$fingers" ]; then
post swipe $swipe_point 30 0 10 1500
expect '^browser: library section ' "two fingers past the first Space didn't bring the Library out"
../tools/window-screenshot.sh "$pid" "$out/library-swipe.png" || fail "no window to capture"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
expect '^browser: library hidden$' "a swipe let go short of halfway didn't put the Library back"
none_since '^browser: command view-library' ||
  fail "a swipe let go short of halfway opened the Library"
library_to "swipe $swipe_point 12 0 10" swipe shown
# Fingers going on past where it opens take the Library on a little with the card, its column's
# fill still reaching the window's left edge: the edge is the column's colour 40 across, and not
# the section's beside it. The column is the ink over the window, darker in a light window and
# lighter in a dark one, so the step compares colours, not which is brighter.
post swipe $swipe_point 60 0 10 1500
expect '^browser: library past its end$' "fingers past the Library's end didn't reach it"
../tools/window-screenshot.sh "$pid" "$out/library-end.png" || fail "no window to capture"
# awk, as Git Bash on Windows has no bc.
end_at() { pixel "$out/library-end.png" "$(awk "BEGIN { print $1 / ${content%x*} }")" 0.85; }
alike() {
  set -- $1 $2
  for d in $(($1 - $4)) $(($2 - $5)) $(($3 - $6)); do
    [ "${d#-}" -le 4 ] || return 1
  done
}
column=$(end_at 6)
inside=$(end_at 40)
beside=$(end_at 200)
{ alike "$column" "$inside" && ! alike "$column" "$beside"; } || fail "past its end the \
Library's column left the window's edge: $column at it, $inside in it, $beside beside"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
none_since '^browser: command view-library' || fail "fingers past the Library's end closed it"
expect '^browser: library shown$' "the Library didn't settle back where it opens"
library_to "swipe $swipe_point -12 0 10" swipe hidden
fi
library_to "wheel $swipe_point 1" wheel shown
# Its Spaces section is as wide as its two cards, 216 + 264 a Space as Arc's, unless that would
# leave the page less than 56, as Arc 1.171's. The section's button is the third of six, with
# Agents (#1080) and Boosts (#695) last, 100 apart and centred 5 above the window's middle, 56 in.
send click 56 $((${content#*x} / 2 - 55))
expect '^browser: library section spaces$' "a click on Spaces didn't show the Library's Spaces"
spaces_wide=$((216 + 264 * 2))
[ $spaces_wide -gt $((${content%x*} - 56)) ] && spaces_wide=$((${content%x*} - 56))
expect "^browser: library spaces 2 wide $spaces_wide\$" \
  "the Library's Spaces section isn't $spaces_wide wide with two Spaces in a $content window"
expect "^browser: library resized $spaces_wide\$" "the Library didn't settle at its Spaces' width"
../tools/window-screenshot.sh "$pid" "$out/library-spaces.png" || fail "no window to capture"
# At rest a card's field is its icon and name alone; its plate, the slot's dashes and the brush
# come with the pointer on the field, here the second card's, 25 down its 48 from the top, and go
# with the pointer on the card below the field. The capture above is at rest, the pointer on the
# Spaces button.
grep -q "^browser: library field $second at rest drawn\$" "$log" ||
  fail "the second card's field wasn't drawn at rest"
send move $((404 + 120)) $((48 + 25))
expect "^browser: library field $second shown drawn\$" "the pointer on the second card's field didn't show its plate"
../tools/window-screenshot.sh "$pid" "$out/library-field-hover.png" || fail "no window to capture"
send move 524 $((${content#*x} / 2))
expect "^browser: library field $second at rest drawn\$" "the pointer leaving the second card's field left its plate"
# The brush, 4 in from the field's end and 28 wide, and the icon's slot, 6 in and 24 wide, are on
# the field, and light it too (#1074).
card_y=$((48 + 25))
brush_x=$((404 + 240 - 8 - 4 - 14))
slot_x=$((404 + 8 + 6 + 12))
send move $brush_x $card_y
expect "^browser: library field $second shown drawn\$" "the pointer on the second card's brush didn't light its field"
send move 524 $((${content#*x} / 2))
expect "^browser: library field $second at rest drawn\$" "the pointer leaving the second card's brush left its plate"
send move $slot_x $card_y
expect "^browser: library field $second shown drawn\$" "the pointer on the second card's icon slot didn't light its field"
send move 524 $((${content#*x} / 2))
expect "^browser: library field $second at rest drawn\$" "the pointer leaving the second card's icon slot left its plate"
# A right-click on the second card, 404 in and half its 240 wide, shows that Space's menu, with
# the colour it was given, and leaves the Library open on the Space shown.
post rightclick 524 $((${content#*x} / 2))
card_menu="^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$"
expect "$card_menu" "a right-click on the second Space's card didn't show its menu"
none_since '^browser: library space ' || fail "a right-click on a card showed its Space"
send dismiss
# The ••• at the second card's foot, its middle 22 in from the card's right and 21.5 above its foot,
# shows with the pointer on the card, here on the ••• itself, which the capture shows before the
# menu covers it. It shows the same menu and leaves the Library open on the Space shown (#1401).
more_point="$((404 + 240 - 22)) $((${content#*x} - 48 - 22))"
send move $more_point
../tools/window-screenshot.sh "$pid" "$out/library-card-more.png" || fail "no window to capture"
post click $more_point
expect "$card_menu" "a click on the second card's ••• didn't show its menu"
none_since '^browser: library space ' || fail "a click on a card's ••• showed its Space"
send dismiss
# The card's field is 8 in from its sides and 36 tall around the middle of its name's row, 25
# down. Its brush opens the theme picker for the Space beside itself, over the Library, which
# stays open (#1074), where the second gets its light and its dark appearance in turn, and then
# automatic again. The capture comes while it's dark, the card taking the theme as it's made and
# the window only the look of the Space it shows, the first. $1 is the appearance and $2 its
# button's x in the picker's window, which holds the 13 wide arrow left of the card. With open as
# $3 the picker stays open.
appear() {
  send click $brush_x $card_y
  expect "^browser: theme picker for $second\$" "the second card's brush didn't open the theme picker"
  expect '^browser: theme picker drawn$' "the brush's theme picker didn't draw itself on screen"
  none_since '^browser: library hidden$' || fail "the brush's theme picker closed the Library"
  send clickkey $2 49
  expect "^browser: themed space $second .* appearance=$1\$" "the picker didn't make the Space $1"
  if [ "$1" = dark ]; then
    ../tools/window-screenshot.sh "$pid" "$out/library-theme-picker.png" ||
      fail "no window to capture"
  fi
  [ "${3:-}" != open ] || return 0
  send key escape
  expect '^browser: theme picker hidden$' "escape didn't hide the brush's theme picker"
  none_since '^browser: library hidden$' || fail "escape in the card's theme picker closed the Library"
}
# A click on the second card's name, past its icon's slot, makes the name a field there with the
# keyboard, leaving the Library open and the Space where it was, and escape puts the name back.
# $1 is the capture's name.
rename_card() {
  send click $((404 + 8 + 6 + 24 + 6 + 20)) $card_y
  expect "^browser: rename space $second in library\$" "a click on the card's name didn't edit it"
  none_since '^browser: library space ' || fail "a click on a card's name showed its Space"
  none_since '^browser: library hidden$' || fail "a click on a card's name closed the Library"
  send type play
  ../tools/window-screenshot.sh "$pid" "$out/$1" || fail "no window to capture"
  send key escape
  expect '^browser: rename cancelled$' "escape didn't put the card's name back"
  none_since '^browser: renamed space ' || fail "escape in a card's name renamed the Space"
  none_since '^browser: library hidden$' || fail "escape in a card's name closed the Library"
}
appear light 191.3
appear dark 231.7 open
# Escape, and the brush straight after it, as the picker fades out: it shows again and stays, and
# takes a click, as a press while it fades leaves the showing that goes, not the next (#2237).
post key escape
appear light 191.3
rename_card library-rename.png
appear automatic 150.9
# The second card's slot hangs the icon picker from itself over the Library, which stays open,
# and a click on the first card's slot, 264 to the left, moves the picker there. Escape closes the
# picker, and then the Library.
send click $slot_x $card_y
expect "^browser: icon picker for $second on " "the second card's icon slot didn't open the icon picker"
none_since '^browser: library hidden$' || fail "the card's icon picker closed the Library"
../tools/window-screenshot.sh "$pid" "$out/library-icon-picker.png" || fail "no window to capture"
send click $((slot_x - 264)) $card_y
expect "^browser: icon picker for $first on " "a click on the first card's slot didn't move the icon picker"
none_since '^browser: library hidden$' || fail "moving the icon picker closed the Library"
send key escape
expect '^browser: icon picker hidden$' "escape didn't close the card's icon picker"
none_since '^browser: library hidden$' || fail "escape in the card's icon picker closed the Library"
# The second card's handle, at its foot 22 in from its left and 22 above it, drags it along the
# cards: let go over the first card's left half it goes first, the Library staying open with the
# Space shown where it was, and ⌘Z puts it back. Held there, its copy hangs from the pointer, its
# place dims and the line stands before the first card; the recording has the cards gliding to
# their places as it's let go (#1580). It starts after the still, which would end it (#2026).
handle_point="$((404 + 22)) $((${content#*x} - 48 - 22))"
handle_y=${handle_point#* }
send move $handle_point
send press $handle_point
send dragto 420 $handle_y
expect "^browser: drag space $second\$" "the second card's handle didn't drag the card"
send dragto 180 $handle_y
../tools/window-screenshot.sh "$pid" "$out/library-space-drag.png" || fail "no window to capture"
record library-space-drag 3
send release 180 $handle_y
expect "^browser: dropped space $second after 0\$" "the second card let go over the first didn't move its Space"
expect "^browser: spaces $second $first\$" "the second Space didn't go first"
recorded
none_since '^browser: library hidden$' || fail "a card's drag closed the Library"
none_since '^browser: library space ' || fail "a card's drag showed its Space"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the card's move"
expect "^browser: spaces $first $second\$" "⌘Z didn't put the cards back in their order"
# In a window 700 wide the two cards, 744 across, don't fit: the section leaves the page 56, 644,
# and its cards scroll across under two fingers, give past their last and spring back to it once
# let go, 100 along, the Library staying open (Arc 1.171, #1401).
if [ -n "$fingers" ]; then
  send resize 700 ${content#*x}
  expect "^browser: drawn at 700x${content#*x}\$" "the window didn't draw 700 wide"
  record library-cards 3
  send swipe 500 $((${content#*x} / 2)) -20 0 10
  expect '^browser: swipe cards$' "two fingers over the cards that don't fit didn't scroll them"
  expect '^browser: library cards at 100$' "the cards let go past their last didn't spring back to it"
  recorded
  none_since '^browser: library hidden$' || fail "two fingers over the cards closed the Library"
  ../tools/window-screenshot.sh "$pid" "$out/library-cards.png" || fail "no window to capture"
  send resize ${content%x*} ${content#*x}
  expect "^browser: drawn at $content\$" "the window didn't draw back at its size"
fi
send key escape
expect '^browser: library hidden$' "escape didn't close the Library once the picker had gone"
# The cards' pickers left the Space shown as it was.
shown=$(grep '^browser: space [0-9]* in ' "$log" | tail -1 | cut -d' ' -f3)
[ "$shown" = "$first" ] || fail "the cards' pickers showed Space $shown, not the first"
send key l cmd shift
expect '^browser: library shown$' "⇧⌘L didn't open the Library again"
library_to "wheel $swipe_point -1" wheel hidden

# A tab's menu moves it to another Space, and the window goes with the tab it shows.
post rightclick $(row 0)
expect "^browser: menu shown $(cased "Copy Link$share, Change Icon…, Rename…, Mute (disabled), Add Split View, Duplicate, Add to Folder › New Folder, Move to › Space 1 (checked), Move to › reading 日本語, Move to › New Space…, Archive Tab, Archive Tabs Below (disabled)")\$" "the tab's menu didn't offer the other Space"
send choose reading 日本語
expect '^browser: command move-to-space via tab menu$' "Move to didn't run"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "Move to didn't show the tab's new Space"
expect_tabs "$a $b" "$a" "after Move to"
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
post rightclick $(row 0)
expect "^browser: menu shown $(cased "Copy Link$share, Change Icon…, Rename…, Mute (disabled), Add Split View, Duplicate, Add to Folder › New Folder, Move to › Space 1, Move to › reading 日本語 (checked), Move to › New Space…, Archive Tab, Archive Tabs Below")\$" "the moved tab's menu didn't offer the first Space"
send choose Space 1
expect "^browser: space $first\( in .*\)\{0,1\}\$" "moving the tab back didn't show the first Space"
expect_tabs "$a" "$a" "after moving the tab back"
# ⌘Z sends the tab to the second Space again, where it was, and ⇧⌘Z moves it back.
expect '^browser: toast shown Tab Moved to Space \[undo\]$' "Move to Space didn't offer Undo"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Move to Space"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z didn't take the tab back to the second Space"
expect_tabs "$a $b" "$a" "after undoing Move to Space"
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Move to Space"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "⇧⌘Z didn't move the tab to the first Space"
expect_tabs "$a" "$a" "after redoing Move to Space"
# The command bar finds the second Space by its name: Focus shows it, and Move to takes the tab
# shown there, which ⌘Z brings back, as the menu's Move to does (#1396).
by_name reading "Focus reading 日本語" bar-space-by-name.png
expect '^browser: command select-space via command bar$' "the bar's Focus reading 日本語 didn't run"
expect "^browser: space $second in " "the bar's Focus didn't show the second Space"
expect_tabs "$b" "$b" "after the bar's Focus"
grep '^browser: command bar lists "reading": ' "$log" | tail -1 |
  grep -q ', Pin to reading 日本語, Move to reading 日本語' ||
  fail "the bar didn't list Pin to and Move to the second Space after Focus: $(grep '^browser: command bar lists "reading": ' "$log" | tail -1)"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space after the bar's Focus"
expect_tabs "$a" "$a" "back in the first Space after the bar's Focus"
by_name reading "Move to reading 日本語"
expect '^browser: command move-to-space via command bar$' "the bar's Move to reading 日本語 didn't run"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "the bar's Move to didn't show the tab's new Space"
expect_tabs "$a $b" "$a" "after the bar's Move to"
expect '^browser: toast shown Tab Moved to Space \[undo\]$' "the bar's Move to didn't offer Undo"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the bar's Move to"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "⌘Z didn't take the tab back to the first Space"
expect_tabs "$a" "$a" "after undoing the bar's Move to"
# Move to › lists another Space's folders under its Today, as Arc's does, and moves the tab into
# the one picked, which ⌘Z undoes. The second Space's folder goes again after.
send key 2 ctrl
expect "^browser: space $second in " "⌃2 didn't show the second Space"
send menu New Folder…
expect '^browser: rename folder [0-9]*$' "New Folder in the second Space gave no name field"
g=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
send type shelf
send key return
expect "^browser: renamed folder $g shelf\$" "the second Space's folder didn't take its name"
# The command bar finds the folder by its name and moves the tab shown into it, which ⌘Z undoes.
by_name shelf "Move to shelf" bar-folder-by-name.png
expect '^browser: command move-to-folder via command bar$' "the bar's Move to shelf didn't run"
expect "^browser: outline $g+\\[$b\\]\$" \
  "the bar's Move to shelf didn't put the tab in the folder: $(grep '^browser: outline' "$log" | tail -1)"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the bar's Move to shelf"
expect "^browser: outline $g+\\[\\]\$" "⌘Z left the tab in shelf: $(grep '^browser: outline' "$log" | tail -1)"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
post rightclick $(row 0)
expect "^browser: menu shown .*, $(cased 'Move to') › reading 日本語 › Today, \
$(cased 'Move to') › reading 日本語 › shelf, $(cased 'Move to › New Space…'), " \
  "Move to › didn't list the second Space's Today and its folder: $(grep '^browser: menu shown' "$log" | tail -1)"
# The capture has the second Space's submenu open on its Today and folder, picked by its keys.
expect '^browser: menu window at ' "the tab's menu didn't take the keyboard"
send key m
expect "^browser: menu highlights $(cased 'Move to') at " "typing m didn't select Move to, past Mute dimmed"
send key right
expect "^browser: menu submenu $(cased 'Move to') shown at " "→ didn't open the Move to submenu"
# The submenu opens on the first Space, the Space shown, with the second under it.
send key down
expect '^browser: menu highlights .*reading 日本語 at ' "↓ didn't select the second Space"
send key right
expect '^browser: menu submenu reading 日本語 shown at ' "→ didn't open the second Space's submenu"
../tools/window-screenshot.sh "$pid" "$out/move-to-folder.png" || fail "no window to capture"
send choose shelf
expect '^browser: command move-to-folder via tab menu$' "Move to › reading 日本語 › shelf didn't run"
expect '^browser: today $' "Move to › shelf left the tab in the first Space's Today"
# The window follows the tab into its Space, as Move to › a Space does.
expect "^browser: space $second\$" "the window didn't show the second Space after Move to › shelf"
expect "^browser: outline $g+\\[$a\\]\$" \
  "Move to › shelf didn't put the tab in the second Space's folder: $(grep '^browser: outline' "$log" | tail -1)"
# The folder's menu moves it, with the tab in it, to the first Space's pinned tabs, and the window
# goes with the tab it shows; ⌘Z takes the folder back (#1353).
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Rename Folder'), .*$(cased 'Move to Space') › Space 1, " \
  "the folder's menu didn't offer the first Space: $(grep '^browser: menu shown' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/folder-move-menu.png" || fail "no window to capture"
send choose Space 1
expect '^browser: command move-to-space via tab menu$' "the folder's Move to Space › Space 1 didn't run"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "the window didn't go with the moved folder's tab"
expect "^browser: outline \(.* \)\{0,1\}$g+\\[$a\\]\$" \
  "the folder didn't land whole in the first Space: $(grep '^browser: outline' "$log" | tail -1)"
expect '^browser: toast shown Folder Moved to Space \[undo\]$' "the folder's move didn't offer Undo"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
../tools/window-screenshot.sh "$pid" "$out/folder-moved.png" || fail "no window to capture"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the folder's move"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z didn't take the folder back to the second Space"
expect "^browser: outline $g+\\[$a\\]\$" \
  "⌘Z didn't put the folder back whole: $(grep '^browser: outline' "$log" | tail -1)"
undo=$((step + 1))
send key z cmd
send key 1 ctrl
expect "^browser: today $a\$" "⌘Z didn't bring the tab back to the first Space's Today" "$undo"
send key 2 ctrl
expect "^browser: outline $g+\\[\\]\$" "⌘Z left the tab in the folder: $(grep '^browser: outline' "$log" | tail -1)" "$undo"
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Rename Folder'), " "a right-click on the second Space's folder showed no menu"
send choose "$(cased 'Delete Folder')"
expect '^browser: outline *$' "Delete Folder didn't take the second Space's folder away"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space again"
expect_tabs "$a" "$a" "back in the first Space after Move to › shelf"
show_space "key 2 ctrl" select-space ui "$second" "$b"

# Fingers pull the last Space's page past its place and bring in a circle, which readies a new
# Space once its ring closes. Let go short of that, the page goes back; let go readied, they make
# the Space with its form, as New Space… does.
expect "^browser: settled space $second\$" "the second Space's page didn't settle"

# A right-click on a row as soon as ⌃2 has shown the second Space, while its rows still slide in
# from the right, shows the row's tab's menu, as it does once they've settled: a press acts on
# the row that's there to be pressed, the Space having switched. The pointer is 12 in, where the
# page takes longest to reach it. The menu's Rename…, then escape, leave the tab as it was.
show_space "key 1 ctrl" select-space ui "$first" "$a"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
post key 2 ctrl
slide=$step
expect_soon "^browser: space $second in " "⌃2 didn't show the second Space"
post rightclick 12 $(row 0 | cut -d' ' -f2)
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click while the second Space's rows slid in showed no menu"
# Settled before the menu showed, and the press came too late to be a test of the slide. The
# slide's 0.3 s is the clock's, and a GPU-less runner, Linux's, Windows' or GitHub's hosted Mac,
# may draw frames slower than that, which the press waits behind (#732). A Mac of our own still
# fails here, as it keeps the mid-slide press of #1513 tested.
if ! sed -n "/^browser: drive $slide: /,\$p" "$log" |
  awk '/^browser: settled space/ { late = 1 } /^browser: menu shown/ { exit late }'; then
  { [ "$(uname)" = Linux ] || windows || [ -n "${GITHUB_ACTIONS:-}" ]; } ||
    fail "the rows had settled before the right-click, so it didn't press mid-slide"
  echo "skip: a right-click mid-slide: the rows had settled before the app had the press" \
    "(#732, #1513)."
fi
send choose Rename…
expect "^browser: rename tab $b\$" "the menu a mid-slide right-click showed was another row's"
send key escape
expect '^browser: rename cancelled$' "escape in the tab's field didn't cancel"
# The page may have settled while the menu was up, before the commands since.
now=$step
step=$slide
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
step=$now

if [ -n "$fingers" ]; then
send swipe $swipe_point -10 0 10
expect '^browser: swipe across$' "two fingers across the sidebar didn't page"
none_since '^browser: new space ready$' || fail "a short pull past the last Space readied a Space"
post swipe $swipe_point -40 0 10 1500
expect '^browser: new space ready$' "a long pull past the last Space didn't ready a Space"
../tools/window-screenshot.sh "$pid" "$out/new-space-pull.png" || fail "no window to capture"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
expect '^browser: command new-space via swipe$' "letting go of a readied pull didn't make a Space"
expect "^browser: spaces $first $second [0-9]*\$" "the pull didn't make a third Space"
else
  send menu New Space…
  expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
  expect "^browser: spaces $first $second [0-9]*\$" "New Space didn't make a third Space"
fi
third=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: space $third in " "the window didn't show the third Space"
unnamed=$((step + 1))
send key return
show_space "key 1 ctrl" select-space ui "$first" "$a"
none_since '^browser: create space' "$unnamed" || fail "return with no name named the Space"
# The footer's + offers New Space…, New Folder…, New Split View and New Note, then what adds
# from the desktop, 8 in from the sidebar's edge.
post click 204 $((${content#*x} - 25))
plus="New Space…, New Folder…, New Split View, New Note, $(plus_adds)"
expect "^browser: menu shown $(cased "$plus")\$" \
  "the footer's + didn't offer New Space…, New Folder…, New Split View, New Note and $(plus_adds)"
send choose New Space…
expect '^browser: command new-space via footer$' "New Space… in the +'s menu didn't run"
expect "^browser: space $third in " "New Space didn't show the form already open"
# Clicks wait for the Space's page to slide into place.
expect "^browser: settled space $third\$" "the form's page didn't settle"
none_since '^browser: spaces ' || fail "New Space with a form open made another Space"
send state
in_browser AccessKitSubclassOfGPUIView "back in the third Space's form"
send click 114 $((${content#*x} - 74))
expect "^browser: cancel space $third\$" "a click on Cancel didn't cancel"
# The pull opened the form from the second Space.
expect "^browser: space $second in " "Cancel didn't show the second Space again"
expect "^browser: spaces $first $second\$" "Cancel didn't delete the third Space"
expect_tabs "$b" "$b" "after Cancel"
expect "^browser: settled space $second\$" "the second Space's page didn't settle after Cancel"
show_space "key 1 ctrl" select-space ui "$first" "$a"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
# Made from the first of two, the Space goes right after it, as Arc's does, not after the others.
# Fingers going on past the form rubber-band it, ready no new Space and leave its field the
# keyboard. Going back, they page to the Space it was made from, the first, and its Space goes
# once its page is out.
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]* $second\$" "New Space didn't make a Space after the first"
third=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: settled space $third\$" "the form's page didn't settle"
if [ -n "$fingers" ]; then
send swipe $swipe_point -40 0 10
expect '^browser: swipe across$' "two fingers across the form didn't page"
expect "^browser: settled space $third\$" "the form's page didn't settle back"
none_since '^browser: new space ready$' || fail "fingers past the form readied a Space"
none_since '^browser: cancel space' || fail "fingers going on past the form cancelled it"
send state
in_browser AccessKitSubclassOfGPUIView "in the form after a swipe"
# Held partway, the first Space's URL field and Favorites come in with its rows, and let go there
# the form springs back.
post swipe $swipe_point 12 0 18 1500
expect '^browser: swipe across$' "two fingers back off the form didn't page"
../tools/window-screenshot.sh "$pid" "$out/space-form-back.png" || fail "no window to capture"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
expect "^browser: settled space $third\$" "the form's page didn't spring back"
send swipe $swipe_point 12 0 10
expect "^browser: cancel space $third\$" "two fingers back off the form didn't cancel it"
else
  send click 114 $((${content#*x} - 74))
  expect "^browser: cancel space $third\$" "a click on Cancel didn't cancel"
fi
expect "^browser: space $first in " "a swipe back off the form didn't show the Space it came from"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
expect "^browser: spaces $first $second\$" "a swipe back off the form didn't delete its Space"
expect_tabs "$a" "$a" "after a swipe back off the form"
# The Spaces menu lists the Spaces by name, and a name shows its Space.
show_space "menu reading 日本語" select-space ui "$second" "$b"
expect "^browser: settled space $second\$" "the Space the menu showed didn't settle"
show_space "key 1 ctrl" select-space ui "$first" "$a"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"

# A right-click on the sidebar's empty space, three rows under the Space's one Today row, shows
# New Tab, New Folder… and the Space's menu, and New Tab opens the command bar, as ⌘T does.
post rightclick $(row 3)
expect "^browser: menu shown $(cased "New Tab, New Folder…, Change Space Icon…, Rename Space…, $colors, $profile, \
${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click under the Space's rows didn't show New Tab, New Folder… and the Space's menu"
../tools/window-screenshot.sh "$pid" "$out/space-empty-menu.png" || fail "no window to capture"
send choose "$(cased 'New Tab')"
expect '^browser: command new-tab via sidebar$' "New Tab in the empty space's menu didn't run new-tab"
expect '^browser: command bar shown$' "New Tab in the empty space's menu didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar New Tab opened"

# A right-click on the Space's name or its dot shows the Space's menu, as in Arc.
name_point="100 112"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on the Space's name didn't show its menu"
../tools/window-screenshot.sh "$pid" "$out/space-menu.png" || fail "no window to capture"
send dismiss
expect '^browser: menu closed$' "the Space's menu didn't close"
# So does a click on the … at the end of the name's row, its down and up together as a tap sends
# them, and the menu stays (#1090): with the UI holding the keyboard, then the page.
edit_point="202 112"
for holder in ui page; do
  if [ $holder = page ]; then
    send click $page_point
    in_browser RenderWidgetHostViewCocoa "after a click on the page"
  fi
  send move $edit_point
  send click $edit_point
  expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
    "a click on the … didn't show the Space's menu, the $holder holding the keyboard"
  expect '^browser: menu drawn ' "the Space's menu from a click on the … didn't come on screen ($holder)"
  ../tools/window-screenshot.sh "$pid" "$out/space-edit-click-$holder.png" || fail "no window to capture"
  none_since '^browser: menu c' || fail "the Space's menu from a click on the … closed ($holder)"
  send dismiss
  expect '^browser: menu closed$' "the Space's menu from the … didn't close"
done
# The same with the sidebar hidden and out over the page (#1090), whose panel is a window of its
# own, 5 in from the browser window's top left, that the menu goes over: a tap on the …, a press
# let go once the menu shows, and a right-click on the name. The menu stays, and the panel with it.
send key s cmd
expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar for the … out over the page"
send move $page_point
send move 3 300
send move 3 306
expect '^browser: reveal shown$' "the pointer at the window's edge didn't bring the sidebar out"
expect '^browser: reveal window shown$' "the sidebar out didn't come on screen"
reveal_edit="207 117"
for how in tap press rightclick; do
  case $how in
    tap)
      send move $reveal_edit
      send click $reveal_edit
      ;;
    press)
      send move $reveal_edit
      send press $reveal_edit
      ;;
    *)
      send move 105 117
      post rightclick 105 117
      ;;
  esac
  expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
    "a $how on the sidebar out over the page didn't show the Space's menu"
  expect '^browser: menu drawn ' "the Space's menu from a $how on the sidebar out didn't come on screen"
  if [ $how = press ]; then send release $reveal_edit; fi
  ../tools/window-screenshot.sh "$pid" "$out/space-edit-reveal-$how.png" || fail "no window to capture"
  none_since '^browser: menu c' || fail "the Space's menu from a $how on the sidebar out closed"
  none_since '^browser: reveal hidden$' || fail "the Space's menu from a $how sent the sidebar out away"
  send dismiss
  expect '^browser: menu closed$' "the Space's menu over the sidebar out didn't close"
done
send move $page_point
expect '^browser: reveal hidden$' "the pointer past the sidebar out didn't send it away after its menu"
send key s cmd
expect '^browser: sidebar shown$' "⌘S didn't bring the sidebar back after its menus out over the page"
# In a second window, and in a window full screen, a click on the … shows the Space's menu too
# (#1090).
send key n cmd
expect '^browser: window opened [0-9]*$' "⌘N didn't open a second window for the …"
new_window_bar
# The driver's fullscreen takes the window it drives, which stays the first till it's told.
send window 2
for where in second-window full-screen; do
  if [ $where = full-screen ]; then
    send fullscreen
    expect '^browser: window entered full screen$' "the second window didn't go full screen"
  fi
  send move $page_point
  send move $edit_point
  ../tools/window-screenshot.sh "$pid" "$out/space-edit-$where-before.png" || fail "no window to capture"
  send click $edit_point
  shown="^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$"
  for _ in $(seq 100); do logged "$shown" && break; sleep 0.1; done
  # The issue asks whether a second click works when the first does nothing.
  if ! logged "$shown"; then
    first=$step
    send click $edit_point
    for _ in $(seq 100); do logged "$shown" && break; sleep 0.1; done
    if logged "$shown" "$first"; then again=did; else again="didn't"; fi
    fail "a click on the … didn't show the Space's menu in the $where, and a second click $again"
  fi
  expect '^browser: menu drawn ' "the Space's menu from the … didn't come on screen in the $where"
  ../tools/window-screenshot.sh "$pid" "$out/space-edit-$where.png" || fail "no window to capture"
  none_since '^browser: menu c' || fail "the Space's menu from the … closed in the $where"
  send dismiss
  expect '^browser: menu closed$' "the Space's menu from the … didn't close in the $where"
done
send fullscreen
expect '^browser: window left full screen$' "the second window didn't leave full screen"
send key w cmd shift
expect '^browser: window closed [0-9]' "⇧⌘W didn't close the second window"
send window 1
# A press shows it too, and it stays once the button comes up after the menu shows, as it does
# when the user holds the button.
send move $edit_point
send press $edit_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a press on the … at the end of the Space's name didn't show its menu"
expect '^browser: menu drawn ' "the Space's menu from the … didn't come on screen"
send release $edit_point
../tools/window-screenshot.sh "$pid" "$out/space-edit-menu.png" || fail "no window to capture"
none_since '^browser: menu c' || fail "the Space's menu from the … closed as the button came up"
send choose Rename Space…
expect "^browser: rename space $first\$" "Rename Space didn't open the name's field"
send state
in_browser AccessKitSubclassOfGPUIView "in the name's field"
send type work
send key return
expect "^browser: renamed space $first work\$" "return in the name's field didn't rename the Space"
# Rename Space… again, where Done takes the •••'s place and keeps what's typed, as return does
# (Arc 1.171). A press on the row folds the pinned rows instead, which this Space has none of.
post rightclick $name_point
expect "^browser: menu shown $(cased 'Change Space Icon…'), " "a right-click on the Space's name showed no menu"
send choose Rename Space…
expect "^browser: rename space $first\$" "Rename Space… didn't open the name's field again"
send type Work
../tools/window-screenshot.sh "$pid" "$out/space-done.png" || fail "no window to capture"
send click 198 112
expect '^browser: space name done$' "a click on Done didn't reach it"
expect "^browser: renamed space $first Work\$" "Done didn't keep the name typed"
post rightclick $name_point
expect "^browser: menu shown $(cased 'Change Space Icon…'), " "a right-click on the Space's name showed no menu"
send choose Rename Space…
expect "^browser: rename space $first\$" "Rename Space… didn't open the name's field a third time"
# A right-click in the field shows the field's menu, not the Space's, and the field stays.
post rightclick $name_point
expect "^browser: menu shown $(cased 'Cut, Copy, Paste, Select All')\$" \
  "a right-click in the name's field didn't show the field's menu"
none_since "^browser: menu shown $(cased 'Change Space Icon…')" || fail "the right-click showed the Space's menu too"
send choose "$(cased 'Select All')"
send type gone
send key escape
expect '^browser: rename cancelled$' \
  "escape in the name's field didn't cancel: $(lost_keyboard)"
none_since '^browser: renamed space' || fail "escape in the name's field renamed the Space"
# Rename Space in the Spaces menu turns the name of the Space shown into a field too.
send menu Rename Space
expect '^browser: command rename-space via ui$' "Rename Space in the menu didn't run rename-space"
expect "^browser: rename space $first\$" "Rename Space in the menu didn't open the name's field"
send key escape
expect '^browser: rename cancelled$' \
  "escape in the name's field didn't cancel: $(lost_keyboard)"
# Change Icon… in the Space's menu opens the icon picker under the Space's name, its arrow at the
# icon's slot, on the Icon tab for a Space with no emoji. It's a child window with the keyboard, so
# its clicks go to the key window, in points from its top left. Escape closes it with no change,
# and the menu offers Remove Icon only once there is an icon.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Change Icon…"
send choose "$(cased 'Change Space Icon…')"
expect "^browser: icon picker for $first on icon\$" "Change Icon… didn't open the icon picker"
expect '^browser: icon picker shown$' "the icon picker didn't show"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
at=$(grep '^browser: icon picker at' "$log" | tail -1 | cut -d' ' -f5-)
# 273 wide, with the arrow's tip 13 in, under the icon's middle at x 26.
[ "$(echo "$at" | awk '{ print $1, $3, $4 }')" = "13 273 286" ] ||
  fail "the icon picker opened at $at, not 273 by 286 from x 13"
send key escape
expect '^browser: icon picker hidden$' "escape didn't close the icon picker"
none_since '^browser: iconed space' || fail "escape in the icon picker gave the Space an icon"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on the Space's name offered Remove Icon without an icon"
send choose "$(cased 'Change Space Icon…')"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
../tools/window-screenshot.sh "$pid" "$out/space-icon-picker.png" || fail "no window to capture"
# A click on a glyph is the Space's icon at once and closes the picker: the bolt, fifth in the first
# row, the grid's first column 27.75 in and its first row 76.25 down.
send clickkey 151.75 76.25
expect "^browser: iconed space $first glyph:bolt\$" \
  "a click on a glyph didn't become the Space's icon"
expect '^browser: icon picker hidden$' "a click on a glyph didn't close the icon picker"
../tools/window-screenshot.sh "$pid" "$out/space-icon.png" || fail "no window to capture"
# A click on the icon opens the picker too. On its Emoji tab, the left half of Emoji | Icon, typing
# narrows the list and ↩ gives the Space the emoji on the plate, the first match, at once. (Keys,
# not a click: the tab slides in over 0.2 s, and a click's place depends on where it has got to.)
send click 23 112
expect "^browser: icon picker for $first on icon\$" \
  "a click on the Space's icon didn't open the picker"
expect '^browser: icon picker shown$' "a click on the Space's icon didn't show the picker"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send clickkey 92.5 39.5
expect '^browser: icon picker on emoji$' "a click on Emoji didn't show the Emoji tab"
send insert fox
send key return
expect "^browser: iconed space $first 🦊\$" \
  "an emoji picked on the Emoji tab didn't become the Space's icon"
expect '^browser: icon picker hidden$' "an emoji picked on the Emoji tab didn't close the icon picker"
../tools/window-screenshot.sh "$pid" "$out/space-emoji.png" || fail "no window to capture"
# For a Space with an emoji, the picker opens on the Emoji tab, and its trash, after Emoji | Icon,
# takes the icon away.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Remove Icon, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" "a right-click on the Space's name didn't offer Remove Icon"
send choose "$(cased 'Change Space Icon…')"
expect "^browser: icon picker for $first on emoji\$" \
  "Change Icon… didn't open the picker on the Emoji tab for a Space with an emoji"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
../tools/window-screenshot.sh "$pid" "$out/space-emoji-grid.png" all || fail "no window to capture"
# The hand at the search's right end, 13 + 44 + 12 down, puts out the six tones along the search,
# and the fifth, medium-dark, shows each emoji that has tones in it: "thumbs" then finds 👍🏾 first,
# which ↩ makes the Space's icon. The default tone, the first, puts them back. Each is 20 wide,
# ending 2 in from the search's end at 12.25 + 8 × 31.
tone_y=69
send clickkey 248.25 $tone_y
send state
send clickkey 228.25 $tone_y
send insert thumbs
../tools/window-screenshot.sh "$pid" "$out/emoji-tone.png" all || fail "no window to capture"
send key return
expect "^browser: iconed space $first 👍🏾\$" \
  "the medium-dark tone didn't make thumbs' first match 👍🏾: $(grep '^browser: iconed space' "$log" | tail -1)"
expect '^browser: icon picker hidden$' "the emoji picked in its tone didn't close the icon picker"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Remove Icon, ")" "a right-click on the Space's name didn't offer Remove Icon"
send choose "$(cased 'Change Space Icon…')"
expect "^browser: icon picker for $first on emoji\$" "Change Icon… didn't open the picker on the Emoji tab again"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send clickkey 248.25 $tone_y
send state
send clickkey 148.25 $tone_y
send clickkey 197 39.5
expect "^browser: iconed space $first none\$" "the icon picker's trash didn't take the icon away"
expect '^browser: icon picker hidden$' "the icon picker's trash didn't close it"
# Remove Icon in the menu does the same.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "the Space's menu offered Remove Icon after the trash"
send choose "$(cased 'Change Space Icon…')"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send clickkey 151.75 76.25
expect "^browser: iconed space $first glyph:bolt\$" \
  "a click on a glyph didn't become the Space's icon"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Remove Icon, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" "a right-click on the Space's name didn't offer Remove Icon"
send choose "$(cased 'Remove Icon')"
expect "^browser: iconed space $first none\$" "Remove Icon didn't take the Space's icon away"
# Edit Theme Color… in the Space's menu opens the theme picker too. A swatch is the Space's colour,
# its own hex, at its hue and lightness in the field, and the wave's knob dragged left washes the
# window's colours out, the window following as it goes; the core keeps the theme once it's let go.
# A click outside the picker closes it.
# The menu opens as a hand opens it, the release once it shows, which the menu takes: on Windows the
# press left the browser window holding the mouse captured, so every drag in the picker went to it
# and moved it (#2516). The picker's drags, and one from where it has nothing, leave the window's
# frame where it was.
post rightpress $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Edit Theme Color…"
send rightrelease $name_point
send order
window_frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
frame_kept() {
  send order
  [ "$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)" = "$window_frame" ] ||
    fail "$1 moved the window from $window_frame: $(grep '^browser: frame window 1 ' "$log" | tail -1)"
}
record space-theme-open 2
send choose "$(cased 'Edit Theme Color…')"
expect "^browser: theme picker for $first\$" "Edit Theme Color… didn't open the theme picker"
expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
recorded
# From the menu it has no arrow, its card 14 inside the sidebar's edge and the window's top, as
# Arc 1.171's is inside its 382 × 534 window there (#1400). Its window keeps the arrow's 13 left of
# the card, so it starts a point past the edge. Linux and Windows put it on a whole point or a
# fraction off, so a point either way passes.
at=$(grep '^browser: theme picker at' "$log" | tail -1 | cut -d' ' -f5-)
edge=$(grep '^browser: sidebar width ' "$log" | tail -1 | cut -d' ' -f4)
echo "$at" | awk -v edge="${edge:-228}" '{ x = $1 - (edge + 1); y = $2 - 14 }
  END { exit !(x * x <= 1 && y * y <= 1 && $3 == 370 && $4 == 509) }' ||
  fail "Edit Theme Color… opened the theme picker at $at, not against the sidebar's edge at the top"
# Its shadow's window is round it, the card's shape cut out of it where the card is, or the page
# shows through under the card (#2075). Only the Mac's host draws its own.
if [ "$(uname)" = Darwin ]; then
  shadow=$(grep '^browser: theme picker shadow off by' "$log" | tail -1)
  [ "$shadow" = "browser: theme picker shadow off by 0 0 0 0" ] ||
    fail "the theme picker's shadow isn't round it: ${shadow#browser: theme picker }"
else
  echo "skip: the theme picker's shadow round it: only the Mac's host draws its own."
fi
# Between the swatches and the wave there's nothing: a drag from there changes nothing.
themes=$(grep -c "^browser: themed space $first " "$log" || true)
empty=$(echo "$at" | awk '{ print $1 + 13 + 178.5, $2 + 410 }')
emptied=$(echo "$at" | awk '{ print $1 + 13 + 218.5, $2 + 410 }')
send press $empty
send dragto $emptied
send release $emptied
frame_kept "a drag from the picker's empty space"
[ "$(grep -c "^browser: themed space $first " "$log")" = "$themes" ] ||
  fail "a drag from the picker's empty space changed the theme: $(grep "^browser: themed space $first " "$log" | tail -1)"
[ "$(grep '^browser: theme picker \(shown\|hidden\)$' "$log" | tail -1)" = "browser: theme picker shown" ] ||
  fail "a drag from the picker's empty space closed it"
send clickkey "$(swatch_x 7)" $swatch_y
blue='x=-0.619[0-9] y=-0.242[0-9] colors=1'
expect "^browser: theme picker set $first $blue intensity=0.810 " "the blue swatch gave no colour"
expect "^browser: colored space $first 7eb6d5\$" "the swatch didn't give the Space its colour"
expect "^browser: themed space $first $blue intensity=0.810 " "the core didn't keep the swatch"
# The knob's middle is 30 to 231 from the card's left, 13 in, 452 down: 192.8 at 0.81.
knob=$(echo "$at" | awk '{ print $1 + 13 + 192.8, $2 + 452 }')
washed=$(echo "$at" | awk '{ print $1 + 13 + 100, $2 + 452 }')
send press $knob
expect '^browser: theme picker handles 1 drawn, intensity held$' \
  "the wave's knob wasn't drawn held as it was pressed"
send dragto $washed
send release $washed
# The pointer lands on a whole point, and the card's left needn't be one, so the drag ends a little
# either side.
expect "^browser: theme picker set $first $blue intensity=0.3[45][0-9] " \
  "the wave's knob dragged left didn't wash the colour out"
expect "^browser: themed space $first $blue intensity=0.3[45][0-9] " \
  "the core didn't keep the theme's intensity"
frame_kept "a drag on the wave's knob"
# The grain's dial, 298.5 across the card and 452 down, turns for a drag across its top.
dial=$(echo "$at" | awk '{ print $1 + 13 + 298.5, $2 + 425 }')
turned=$(echo "$at" | awk '{ print $1 + 13 + 338.5, $2 + 425 }')
send press $dial
send dragto $turned
send release $turned
expect "^browser: themed space $first $blue intensity=0.3[45][0-9] grain=0\.[0-9]*[1-9]" \
  "a drag on the grain's dial didn't give the theme grain"
frame_kept "a drag on the grain's dial"
# + twice puts three colours round the blue, the others 150° and 210° on, and the third dragged
# alone goes to a place of its own, the other two staying, which the core keeps.
send clickkey 211.5 316.5
expect "^browser: themed space $first x=-0.619[0-9] y=-0.242[0-9] colors=2 " "+ didn't add a second colour"
send clickkey 211.5 316.5
expect "^browser: themed space $first x=-0.619[0-9] y=-0.242[0-9] colors=3 " "+ didn't add a third colour"
# Pressed before its frame is drawn, a handle wouldn't follow the drag's moves.
expect '^browser: theme picker handles 3 drawn$' "the picker didn't draw the three colours at rest"
# The blue is 201.4° round and 0.665 out, so the third is at 51.4°, (0.415, 0.519) in the field,
# whose middle is 178.5 from the card's top and 13 + 178.5 from its left, at 170 a unit. Dragged
# to (0.3, -0.55), it's 97.2° on from the blue and 0.038 nearer the middle.
third=$(echo "$at" | awk '{ print $1 + 13 + 178.5 + 170 * 0.415, $2 + 178.5 + 170 * 0.519 }')
placed=$(echo "$at" | awk '{ print $1 + 13 + 178.5 + 170 * 0.3, $2 + 178.5 - 170 * 0.55 }')
send press $third
expect '^browser: theme picker handles 3 drawn, 2 held$' "the third colour wasn't drawn held as it was pressed"
send dragto $placed
send release $placed
alone="x=-0.619[0-9] y=-0.242[0-9] colors=3 .* places=150.00,0.0000,9[6-8]\.[0-9]*,-0.0[2-5][0-9]*"
expect "^browser: theme picker set $first $alone\$" "the third colour dragged alone didn't go to its own place"
expect "^browser: themed space $first $alone\$" "the core didn't keep the third colour's place"
frame_kept "a drag on a colour's dot"
../tools/window-screenshot.sh "$pid" "$out/space-theme.png" || fail "no window to capture"
send click $page_point
# The host closes the picker for a click outside it, and only escape's own dismissal logs one.
expect '^browser: theme picker hidden$' "a click outside the theme picker didn't close it"
# Remove Theme Color, offered once the Space has a colour, takes its theme and colour away, and the
# menu offers it no more.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Remove Theme Color"
send choose "$(cased 'Remove Theme Color')"
expect "^browser: colored space $first none\$" \
  "Remove Theme Color didn't take the Space's colour away"
expect "^browser: themed space $first none\$" \
  "Remove Theme Color didn't take the Space's theme away"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "the Space's menu offered Remove Theme Color without a colour"
send choose Rename Space…
send key escape
expect '^browser: rename cancelled$' \
  "escape in the name's field didn't cancel: $(lost_keyboard)"
# Deleting a Space with tabs asks first, in a dialog over the window, and a click around it cancels.
# Return deletes the Space once the dialog has gone. Its tabs close with it, and ⇧⌘T reopens them
# here.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
post choose Delete Space
expect '^browser: dialog shown Delete “reading 日本語”?$' "Delete Space didn't ask first"
expect '^browser: dialog settled$' "the dialog didn't settle"
send click 300 40
expect '^browser: dialog cancelled$' "a click around the dialog didn't cancel it"
expect '^browser: dialog hidden$' "the dialog didn't go"
send state
[ "$key" = BrowserCoreNSWindow ] || fail "the window didn't get the keyboard back: $key has it"
none_since '^browser: spaces' || fail "a click around the dialog deleted the Space"
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
post choose Delete Space
expect '^browser: dialog settled$' "Delete Space didn't ask again"
../tools/window-screenshot.sh "$pid" "$out/space-delete.png" || fail "no window to capture"
send key return
expect '^browser: dialog confirmed$' "return didn't choose Delete"
expect "^browser: spaces $first\$" "Delete didn't delete the Space"
sed -n "/^browser: drive $step: /,\$p" "$log" | grep -A 999 '^browser: dialog hidden$' |
  grep -q "^browser: spaces $first\$" || fail "the Space went before the dialog did"
[ "$(today)" = "$a" ] || fail "deleting another Space left this one's Today as \"$(today)\""
# The toast says so, in Chrome's words off the Mac (#772), and ⌘Z brings the Space back where it
# was, with its name and its tab, and shows it.
undo=⌘Z
{ windows || [ "$(uname)" = Linux ]; } && undo=Ctrl+Z
deleted="^browser: toast shown Space deleted! Use $undo to undo\\.\$"
expect "$deleted" "the toast didn't say the Space went"
expect '^browser: toast settled$' "the toast didn't settle"
../tools/window-screenshot.sh "$pid" "$out/space-deleted.png" || fail "no window to capture"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the delete"
# The toast goes with the ⌘Z it named, long before its 5 s are up, so it can't prompt a second
# one (#1375).
for _ in $(seq 15); do logged '^browser: toast hidden$' && break; sleep 0.1; done
logged '^browser: toast hidden$' || fail "the toast still said to use ⌘Z after ⌘Z"
expect "^browser: spaces $first $second\$" "⌘Z didn't bring the Space back where it was"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z didn't show the Space it brought back"
expect "^browser: loaded [0-9]* https://example.com/\$" "the Space came back without its tab"
expect "^browser: settled space $second\$" "the Space ⌘Z brought back didn't settle"
# Export “reading 日本語” into Folder › Work, its own dimmed, makes the Space a folder at the bottom
# of Work's pinned tabs and its tab the top of Work's Today, shows Work and deletes the Space, and a
# toast offers the Undo. ⌘Z brings the Space back where it was, with its tab, and shows it (#1400).
t=$(today)
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, Export “reading 日本語” into Folder › Work, \
Export “reading 日本語” into Folder › reading 日本語 (disabled), Export as Markdown…, Manage Spaces…, \
Delete Space")\$" "a right-click on a dot didn't offer Export into Folder › Work"
../tools/window-screenshot.sh "$pid" "$out/space-export-menu.png" || fail "no window to capture"
post choose Work
expect "^browser: command export-into-folder $second $first via space menu\$" \
  "Export into Folder › Work didn't run"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "Export into Folder didn't show Work"
expect "^browser: space $second exported into folder [0-9][0-9]* in $first\$" \
  "Export into Folder didn't make the Space a folder"
f=$(sed -n "s/^browser: space $second exported into folder \([0-9]*\) in $first\$/\1/p" "$log" | tail -1)
expect "^browser: spaces $first\$" "Export into Folder didn't delete the Space"
expect '^browser: toast shown Space Exported into Folder \[undo\]$' "Export into Folder didn't offer Undo"
expect "^browser: today $t $a\$" "the Space's tab didn't go to the top of Work's Today: Today is \"$(today)\""
expect "^browser: outline \(.* \)\{0,1\}$f+\[\]\$" "the folder isn't at the bottom of Work's pinned tabs, open"
../tools/window-screenshot.sh "$pid" "$out/space-exported.png" || fail "no window to capture"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Export into Folder"
expect "^browser: spaces $first $second\$" "⌘Z didn't bring the exported Space back where it was"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z didn't show the exported Space"
expect "^browser: today $t\$" "⌘Z didn't put the exported Space's tab back: Today is \"$(today)\""
expect "^browser: settled space $second\$" "the exported Space ⌘Z brought back didn't settle"
# Deleted again, its tab reopens here with ⇧⌘T, and ⌘Z brings back the Space without it.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
post choose Delete Space
expect '^browser: dialog shown Delete “reading 日本語”?$' "the Space came back without its name"
expect '^browser: dialog settled$' "the dialog didn't settle"
send key return
expect "^browser: spaces $first\$" "Delete didn't delete the Space it brought back"
expect "^browser: today $a\$" "the Space shown didn't come back: Today is \"$(today)\""
send key t cmd shift
# From the UI or the page, whichever the dialog gave the keyboard back to: the page, on X11, since
# the export's ⌘Z showed it.
expect '^browser: command reopen-closed-tab via \(ui\|page\)$' "⇧⌘T didn't reach the browser"
expect "^browser: today [0-9]* $a\$" "⇧⌘T didn't reopen the deleted Space's tab here: Today is \"$(today)\""
expect "^browser: loaded $(today | cut -d' ' -f1) https://example.com/\$" \
  "the reopened tab didn't load its page"
send key z cmd
expect "^browser: spaces $first $second\$" "⌘Z after ⇧⌘T didn't bring the Space back"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z after ⇧⌘T didn't show the Space"
expect '^browser: today $' "⌘Z brought back the tab ⇧⌘T had reopened: Today is \"$(today)\""
# Each window keeps its own ⌘Z: with a Space deleted in this window and the archive cleared in a
# second one, the second window's ⌘Z brings back the archive and not the Space, and this one's the
# Space and not the archive.
open_from_bar example.org
send key w cmd
expect '^browser: today $' "⌘W didn't take the tab out of Today: Today is \"$(today)\""
expect '^browser: archive [1-9][0-9]* tabs$' "⌘W didn't archive the tab"
archived=$(grep '^browser: archive [0-9]* tabs$' "$log" | tail -1 | cut -d' ' -f3)
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a window"
other=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
new_window_bar
send window 1
post rightclick 131 $dots_y
# ⌘Z brought the Space back with its theme.
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $empty_profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on a dot in the first window didn't show its Space's menu"
send choose Delete Space
expect "^browser: spaces $first\$" "Delete in the first window didn't delete the Space"
# The window ⌘N opened, the third: the driver counts every window opened, closed ones too.
send window 3
send menu Clear Archive
expect '^browser: dialog shown Clear the archive?$' "Clear Archive in the second window didn't ask first"
send key return
expect '^browser: archive 0 tabs$' "Clear Archive in the second window didn't empty the archive"
send key z cmd
expect "^browser: archive $archived tabs\$" "the second window's ⌘Z didn't bring back the archive"
none_since "^browser: spaces $first $second\$" ||
  fail "the second window's ⌘Z brought back the first window's Space"
send window 1
send key z cmd
expect "^browser: spaces $first $second\$" "the first window's ⌘Z didn't bring back its Space"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "the first window's ⌘Z didn't show the Space"
none_since '^browser: archive 0 tabs$' || fail "the first window's ⌘Z took the archive back out"
send window 3
send key w cmd shift
expect "^browser: window closed $other\$" "⇧⌘W didn't close the second window"
send window 1
# In the Library's Spaces, the first Space's top Today row dragged onto the second's card, which is
# empty, moves its tab there as Move to does, and the Library stays open; dragged back from there,
# it goes home (Arc's help centre, #688). Each card's tabs are under its name, 48 + 6 + 38 down, and
# with no pinned tabs under the divider's 15, so its first row's middle is 127 down.
# The Library opens on the section it showed last, its Spaces.
send key l cmd shift
expect '^browser: library section spaces$' "⇧⌘L didn't open the Library on its Spaces, as it was left"
expect '^browser: library shown$' "⇧⌘L didn't open the Library over the cards"
card_drag() {
  send press $1 127
  send dragto $(($1 + 10)) 127
  send dragto $2 127
  send release $2 127
}
card_drag 260 524
expect "^browser: library dragged [0-9]* to space $second\$" \
  "the first card's tab dragged onto the second's didn't go to its Space"
carded=$(grep '^browser: library dragged ' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: today $carded\$" "the tab dragged onto the second card isn't its Space's Today: \"$(today)\""
none_since '^browser: library hidden$' || fail "the drag onto the second card closed the Library"
../tools/window-screenshot.sh "$pid" "$out/library-card-drag.png" || fail "no window to capture"
card_drag 524 260
expect "^browser: library dragged $carded to space $first\$" \
  "the tab dragged back from the second card didn't go back to the first Space"
expect '^browser: today $' "the tab dragged back left the second Space's Today \"$(today)\""
none_since '^browser: library hidden$' || fail "the drag back onto the first card closed the Library"
send key escape
expect '^browser: library hidden$' "escape didn't close the Library after the cards' drags"
# Empty, it goes without asking, and says so too.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, $empty_profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
send choose Delete Space
expect "^browser: spaces $first\$" "Delete didn't delete the empty Space"
none_since '^browser: dialog shown' || fail "deleting an empty Space asked first"
expect "$deleted" "the toast didn't say the empty Space went"
# The last Space can't go.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, $profile, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space (disabled)")\$" \
  "the last Space's menu offered Delete"
send choose Rename Space…
send key escape
expect '^browser: rename cancelled$' \
  "escape in the name's field didn't cancel: $(lost_keyboard)"
# Edit Theme… in ⌘T's bar, which "theme" finds first, opens the theme picker for the Space shown.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar for Edit Theme…"
send type theme
send key return
expect '^browser: command edit-theme via ' "Edit Theme… in the command bar didn't run"
expect "^browser: theme picker for $first\$" "Edit Theme… didn't open the theme picker for the Space shown"
expect '^browser: theme picker shown$' "Edit Theme… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker from the bar didn't draw itself on screen"
../tools/window-screenshot.sh "$pid" "$out/theme-from-bar.png" || fail "no window to capture"
send key escape
expect '^browser: theme picker hidden$' "escape didn't hide the theme picker from the bar"
# The page shown is hidden while its window is minimized, and while the app is hidden, as Chrome's
# is, and visible once they're back. Last, as the app may not be active after it's been hidden.
# Today is the tab ⇧⌘T reopened over the page's.
hidden_quiet "the page behind Today's tab ran frames while hidden, other than its picture's"
send click "$(row 1)"
last_title=$(grep "^browser: title $a " "$log" | tail -1 | cut -d' ' -f4-)
[ "${last_title%% *}" = visible ] ||
  expect "^browser: title $a visible " "a click on the page's row didn't show it"
ran_hidden "the page shown by its row had run frames while hidden, other than its picture's"
# Linux and Windows have no hiding an app, and the run's sway session keeps its focused window from
# minimizing.
if [ "$(uname)" = Linux ]; then
  echo "skip: a minimized window and a hidden app, which sway and Linux have no way to make."
else
  send minimize
  expect "^browser: title $a hidden\$" "the page in the minimized window isn't hidden"
  send minimize
  expect "^browser: title $a visible 0\$" \
    "the page isn't visible after the window came back, or ran frames while minimized"
  if windows; then
    echo "skip: a hidden app, which Windows has no way to make."
  else
    send hide
    expect "^browser: title $a hidden\$" "the page isn't hidden while the app is"
    send hide
    expect "^browser: title $a visible 0\$" \
      "the page isn't visible after the app came back, or ran frames while hidden"
  fi
fi
exec 3>&-
quit
spaces_ms=$(sed -n 's/^browser: space [0-9]* in \([0-9.]*\) ms$/\1/p' "$log" | summary)
# What the sidebar took to draw each frame while the pages moved, from its build to its paint:
# the median of each page change's median, and the slowest frame.
paged=$(sed -n '/^browser: paged frames /s/[^0-9.][^0-9.]*/ /gp' "$log" | sort -k2 -n |
  awk '{ n += $1; m[NR] = $2; if ($3 > x) x = $3 }
    END { if (NR) printf "%d page changes, %d frames, median %.2f ms, max %.2f ms",
      NR, n, m[int((NR + 1) / 2)], x }')
echo "ok: New Space makes an empty Space whose form takes the keyboard and Japanese input, and names it, and ⌘T opens a tab in it; ⌃1 ⌃2 ⌥⌘← ⌥⌘→ with the page or the UI focused, and the switcher's dots, show each Space with the tab it showed last; a dot dragged along the switcher, or a Library card by its handle, moves its Space, which ⌘Z undoes; two fingers across the sidebar page to the Space a flick or the pages' place points at, never past the last, and a wheel's tick across pages once; past the first Space, fingers and a wheel's tick bring the Library out and put it back, its Spaces section as wide as its cards, and fingers let go short of halfway put it back, and past where it opens take it on a little, its column's fill at the window's edge; a right-click on a row as the Space's rows slide in shows its tab's menu; a tab's menu moves it to another Space, which shows with it; the command bar finds the second Space by its name, Focus shows it and Move to moves the tab there, and finds a folder by its name and moves the tab into it, each of which ⌘Z undoes; a row held at the sidebar's edge pages to the next Space without joining the row under it in a split, and let go there moves to it; fingers held past the last Space ready a new Space and let go make it, with its form, and let go short of that don't; Cancel, and two fingers back, delete a new Space and go back to the Space it was made from, and fingers going on past its form rubber-band it. The first launch's lone Space, unnamed, hides its name row with Hide Space Header in the sidebar's empty space's menu, its rows rising into its place, and Show Space Header brings it back. A right-click on a Space's name, dot or Library card shows Change Space Icon…, Rename Space…, Edit Theme Color…, Set Profile, New Folder…, Manage Spaces… and Delete Space, as its ••• does, and one on the sidebar's empty space New Tab before them, which opens the command bar; the theme picker, from the form's row and the menu, the menu's against the sidebar's edge at the window's top, gives a Space its colours from its field, a swatch's own, + and the wave, and a colour dragged alone to a place of its own, the window taking them as they're made, and Remove Theme Color, offered once it has one, takes them away; an emoji picked in its icon picker, by a search and ↩, is its icon, in its dot's place in the switcher, until Remove Icon, and escape leaves it as it was; Done in the •••'s place keeps the name typed, as return does, a right-click in its field shows the field's menu, and escape cancels; a Library card's field lights with the pointer on its brush or icon slot, the brush opens the theme picker beside itself, which makes the Space light and dark, and the slot the icon picker, which another card's slot moves, both over the Library, which stays open until escape closes the picker and then it, and a click on the card's name edits it in the card with the Library left open; in a window too narrow for its cards they scroll across under two fingers and spring back from past their last. Delete asks first for a Space with tabs, in a dialog a click around cancels and return confirms, closes them once it has gone, and the toast says so until ⌘Z brings the Space back where it was with its name and tabs, ⇧⌘T reopens them in the Space shown, and ⌘Z then leaves those out; the last Space can't go. The Spaces menu names each Space and shows it, and its Rename Space opens the name's field. Move to Space offers Undo, ⌘Z takes the tab back to its Space and ⇧⌘Z moves it again. Move to › lists the other Space's Today and its folder, and moves the tab into the folder, which ⌘Z undoes. The folder's Move to Space › moves it with its tab to the other Space, the window going with it, and ⌘Z takes it back. Export “name” into Folder › another Space makes the Space a folder there, its Today tab going too, and ⌘Z brings it back whole ($out/space-export-menu.png, $out/space-exported.png). With two windows, each one's ⌘Z undoes only what was done in it: a Space deleted in one, the archive cleared in the other. A page behind another tab, in a Space not shown, in a minimized window or a hidden app is hidden and runs no animation frames, and is visible once back. Paging, the sidebar a frame: ${paged:-not logged}. Windows: $out/swipe.png, $out/library-swipe.png, $out/library-end.png, $out/library-spaces.png, $out/library-field-hover.png, $out/library-theme-picker.png, $out/library-icon-picker.png, $out/library-space-drag.png and its motion in $out/library-space-drag.mp4, $out/library-rename.png, $out/library-cards.png and its motion in $out/library-cards.mp4, $out/new-space-pull.png, $out/space-empty-menu.png, $out/space-menu.png, $out/space-edit-menu.png, $out/space-done.png, $out/space-delete.png, $out/space-deleted.png, $out/bar-space-by-name.png, $out/bar-folder-by-name.png"
echo "  Space switch, key to the core's report: $spaces_ms. Windows: $out/space-header-hidden.png, $out/space-form.png, $out/space-form-ime.png, $out/theme-picker.png, $out/space-theme.png and its opening in $out/space-theme-open.mp4, $out/new-space.png, $out/spaces.png, $out/space-drag.png, $out/space-icon-picker.png, $out/space-icon.png, $out/space-emoji.png, $out/move-to-folder.png, $out/folder-move-menu.png, $out/folder-moved.png, $out/library-card-drag.png"
