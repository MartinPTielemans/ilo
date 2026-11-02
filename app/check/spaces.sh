# New Space in the Spaces menu makes a Space and shows it, empty and without a page, with a form
# that takes the keyboard from the page. Japanese input composes in its field, return while
# composing only commits, and return names the Space and sends the form away; ⌘T then opens a tab in
# it. ⌃1–⌃9, ⌥⌘← and ⌥⌘→, with the page or the UI focused, and clicks on the switcher's dots show
# another Space, which shows the tab it showed last. Two fingers across the sidebar page to the
# Space a flick or the pages' place points at, never past the last, fingers along it don't, and a
# wheel's tick across pages once. A right-click on a row as a Space's rows slide in shows its
# tab's menu. A tab's menu moves the tab to another Space, which the window shows with it, and
# into another Space's folder, listed under its Today, which ⌘Z undoes, and a folder's menu moves
# it whole to another Space, which ⌘Z undoes too. Return
# with no name does nothing, New Space from the footer's + shows a form
# already open, and Cancel, or two fingers back, deletes its Space and goes back to the Space it
# was made from; fingers going on rubber-band the form. A right-click on the Space's name, a
# dot or its card in the Library shows Rename Space, Change Icon…, Edit Theme Color… and Delete
# Space, and Remove Theme Color once the Space has a colour, and one on the sidebar's empty space
# shows New Tab and New Folder… before them; an emoji committed after Change Icon…
# is the Space's icon, in its dot's place, until Remove Icon. Rename, the Spaces
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
page='data:text/html,<title>loading</title>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let frames = 0, first = 0, last = 0;
const count = (now) => {
  if (document.hidden) {
    if (!frames) first = now;
    frames++;
    last = now;
    requestAnimationFrame(count);
  }
};
document.onvisibilitychange = () => {
  document.title = document.hidden ? "hidden"
    : "visible " + frames + (frames ? " in " + Math.round(last - first) : "");
  if (document.hidden) {
    frames = 0;
    requestAnimationFrame(count);
  }
};
</script>'
log="$out/spaces.log"
step=0
# X11 sends a touchpad's scrolling with no start or end, so two fingers never page there (#554):
# its steps go to the Spaces and the Library by key, click and wheel instead. Its clicks land on
# whole pixels, at scale 1 here.
x11=
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  x11=1
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
send key return
none_since '^browser: create space' || fail "return while composing named the Space"
[ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
send ime off
# The form's Choose a Theme row opens the theme picker beside it, a child window with the
# keyboard, so its clicks go to the key window in points from its top left, its arrow 13 wide.
# A click in its field is the Space's first colour, where it's clicked, and the window takes it.
send click 100 299
expect "^browser: theme picker for $second\$" "the form's Choose a Theme row didn't open the picker"
expect '^browser: theme picker shown$' "the theme picker didn't show"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
at=$(grep '^browser: theme picker at' "$log" | tail -1 | cut -d' ' -f5-)
[ "$(echo "$at" | awk '{ print $3, $4 }')" = "370 509" ] ||
  fail "the theme picker opened at $at, not 370 by 509"
# Half way from the field's middle to its right edge is red: hue 0 at lightness 0.5. A whole
# pixel's click lands half a point right of it and below.
red_at="x=0.5000 y=0.0000"
red_color=ff0000
[ -z "$x11" ] || { red_at="x=0.5030 y=0.0030"; red_color=ff0302; }
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
send key escape
expect '^browser: theme picker dismissed$' "escape didn't close the theme picker"
expect '^browser: theme picker hidden$' "escape didn't hide the theme picker"
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

# A page out of sight is hidden, as in Chrome, and runs no animation frames: a tab behind the one
# shown, and the tab a Space not shown shows. Back in sight, it's visible.
#
# But for the frames its hover card's picture draws. The tabs a switch hides are pictured once the
# switches have been quiet for 0.4 s (shell/hover_cards.rs), and the core has a hidden page draw,
# still hidden to it, until the copy answers (capture.cc), as Chrome does a background tab's
# preview. That's one run of frames, 0.1 s at most, once the app asks for the tab's picture,
# which may answer only after the tab is shown again, so a step that keeps it hidden past 0.4 s
# sees one. Any other frame while hidden fails.
ran_hidden() {
  local seen
  seen=$(awk -v a="$a" '
    $0 == "browser: title " a " hidden" { pictured = 0 }
    index($0, "browser: tab " a " picturing ") == 1 { pictured = 1 }
    /^browser: handover picture copied / && index($0, "(tab " a ")") { pictured = 1 }
    index($0, "browser: title " a " visible ") == 1 { title = substr($0, 17 + length(a)); was = pictured }
    END { print was + 0, title }' "$log")
  case $seen in
  *" visible 0") ;;
  "1 visible "*" in "[0-9] | "1 visible "*" in "[0-9][0-9]) ;;
  *) fail "$1: $seen" ;;
  esac
}
open_from_bar example.org
expect "^browser: title $a hidden\$" "the tab behind the one shown isn't hidden"
send key w cmd
expect_tabs "$a" "$a" "after closing the tab over the page"
expect "^browser: title $a visible " "the tab shown again isn't visible"
ran_hidden "the tab shown again ran frames while hidden, other than its picture's"
show_space "click 131 $dots_y" select-space sidebar "$second" "$b"
expect "^browser: title $a hidden\$" "the tab in the Space not shown isn't hidden"
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
send click "$(row 0)"
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "a click on the top row didn't show tab $c"
send key w cmd
expect_tabs "$a" "$a" "after closing the last-used check's tab"

# Two fingers across the sidebar page between Spaces. A flick goes on to the next Space, a drag let
# go still goes to the Space nearest its place, and past the last Space the pages come back. Fingers
# along the list scroll it, and a wheel's tick across pages once. Held partway, the next Space's
# page shows beside the first's.
swipe_point="110 500"
if [ -z "$x11" ]; then
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
swipe_to "-12 0 10" "$second" "$b"
swipe_to "12 0 10" "$first" "$a"
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
# Past the first Space is the Library. Held partway it shows beside the page, and let go short of
# halfway it goes back. A flick brings it out and one the other way puts it back, and so do a
# wheel's ticks.
# $1 is how the driver moves, $2 its source and $3 whether the Library shows after it.
library_to() {
  send $1
  expect "^browser: command view-library via $2\$" "a $1 didn't reach the Library"
  expect "^browser: library $3\$" "a $1 didn't leave the Library $3"
}
if [ -z "$x11" ]; then
post swipe $swipe_point 30 0 10 1500
expect '^browser: library section ' "two fingers past the first Space didn't bring the Library out"
../tools/window-screenshot.sh "$pid" "$out/library-swipe.png" || fail "no window to capture"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
expect '^browser: library hidden$' "a swipe let go short of halfway didn't put the Library back"
none_since '^browser: command view-library' ||
  fail "a swipe let go short of halfway opened the Library"
library_to "swipe $swipe_point 12 0 10" swipe shown
# Fingers going on past where it opens take the Library on a little with the card, its column's
# fill still reaching the window's left edge, lighter than the window right of it.
post swipe $swipe_point 60 0 10 1500
expect '^browser: library past its end$' "fingers past the Library's end didn't reach it"
../tools/window-screenshot.sh "$pid" "$out/library-end.png" || fail "no window to capture"
column=$(pixel "$out/library-end.png" $(echo "6 / ${content%x*}" | bc -l) 0.85)
beside=$(pixel "$out/library-end.png" $(echo "200 / ${content%x*}" | bc -l) 0.85)
brighter "$column" "$beside" ||
  fail "past its end the Library's column left the window's edge: $column at it, $beside beside"
await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
none_since '^browser: command view-library' || fail "fingers past the Library's end closed it"
expect '^browser: library shown$' "the Library didn't settle back where it opens"
library_to "swipe $swipe_point -12 0 10" swipe hidden
fi
library_to "wheel $swipe_point 1" wheel shown
# Its Spaces section is as wide as its two cards, 216 + 264 a Space as Arc's, unless that would
# leave the page less than 400, and never narrower than one card's 480. The section's button is
# the third of four, 100 apart and centred 5 above the window's middle, 56 in.
send click 56 $((${content#*x} / 2 + 45))
expect '^browser: library section spaces$' "a click on Spaces didn't show the Library's Spaces"
spaces_wide=$((216 + 264 * 2))
[ $spaces_wide -gt $((${content%x*} - 400)) ] && spaces_wide=$((${content%x*} - 400))
[ $spaces_wide -lt 480 ] && spaces_wide=480
expect "^browser: library spaces 2 wide $spaces_wide\$" \
  "the Library's Spaces section isn't $spaces_wide wide with two Spaces in a $content window"
expect "^browser: library resized $spaces_wide\$" "the Library didn't settle at its Spaces' width"
../tools/window-screenshot.sh "$pid" "$out/library-spaces.png" || fail "no window to capture"
# At rest a card's field is its icon and name alone; its plate, the slot's dashes and the brush
# come with the pointer on the field, here the second card's, 33 down its 48 from the top, and go
# with the pointer on the card below the field. The capture above is at rest, the pointer on the
# Spaces button.
grep -q "^browser: library field $second at rest drawn\$" "$log" ||
  fail "the second card's field wasn't drawn at rest"
send move $((404 + 120)) $((48 + 33))
expect "^browser: library field $second shown drawn\$" "the pointer on the second card's field didn't show its plate"
../tools/window-screenshot.sh "$pid" "$out/library-field-hover.png" || fail "no window to capture"
send move 524 $((${content#*x} / 2))
expect "^browser: library field $second at rest drawn\$" "the pointer leaving the second card's field left its plate"
# The brush, 4 in from the field's end and 28 wide, and the icon's slot, 6 in and 24 wide, are on
# the field, and light it too (#1074).
card_y=$((48 + 33))
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
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the second Space's card didn't show its menu"
none_since '^browser: library space ' || fail "a right-click on a card showed its Space"
send dismiss
# The card's field is 8 in from its sides and 36 tall around the middle of its name's row, 33
# down. Its brush opens the theme picker for the Space beside itself, over the Library, which
# stays open (#1074), where the second gets its light and its dark appearance in turn, and then
# automatic again. The capture comes while it's dark, the card taking the theme as it's made and
# the window only the look of the Space it shows, the first. $1 is the appearance and $2 its
# button's x in the picker's window, which holds the 13 wide arrow left of the card.
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
appear dark 231.7
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
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
post rightclick $(row 0)
expect "^browser: menu shown .*, $(cased 'Move to') › reading 日本語 › Today, \
$(cased 'Move to') › reading 日本語 › shelf, $(cased 'Move to › New Space…'), " \
  "Move to › didn't list the second Space's Today and its folder: $(grep '^browser: menu shown' "$log" | tail -1)"
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
# slide's 0.3 s is the clock's, and a GPU-less Linux runner may draw frames slower than that, which
# the press waits behind (#732).
if ! sed -n "/^browser: drive $slide: /,\$p" "$log" |
  awk '/^browser: settled space/ { late = 1 } /^browser: menu shown/ { exit late }'; then
  [ "$(uname)" = Linux ] || fail "the rows had settled before the right-click, so it didn't press mid-slide"
  echo "skip: a right-click mid-slide: the rows had settled before the app had the press (#732)."
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

if [ -z "$x11" ]; then
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
send key return
none_since '^browser: create space' || fail "return with no name named the Space"
show_space "key 1 ctrl" select-space ui "$first" "$a"
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
if [ -z "$x11" ]; then
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
expect "^browser: menu shown $(cased "New Tab, New Folder…, Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
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
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the Space's name didn't show its menu"
../tools/window-screenshot.sh "$pid" "$out/space-menu.png" || fail "no window to capture"
send dismiss
expect '^browser: menu closed$' "the Space's menu didn't close"
# So does a click on the … at the end of the name's row, its down and up together as a tap sends
# them, and the menu stays (#1090): with the UI holding the keyboard, then the page.
edit_point="196 112"
for holder in ui page; do
  if [ $holder = page ]; then
    send click $page_point
    in_browser RenderWidgetHostViewCocoa "after a click on the page"
  fi
  send move $edit_point
  send click $edit_point
  expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
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
expect '^browser: reveal shown$' "the pointer at the window's edge didn't bring the sidebar out"
expect '^browser: reveal window shown$' "the sidebar out didn't come on screen"
reveal_edit="201 117"
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
  expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
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
  shown="^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$"
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
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a press on the … at the end of the Space's name didn't show its menu"
expect '^browser: menu drawn ' "the Space's menu from the … didn't come on screen"
send release $edit_point
../tools/window-screenshot.sh "$pid" "$out/space-edit-menu.png" || fail "no window to capture"
none_since '^browser: menu c' || fail "the Space's menu from the … closed as the button came up"
send choose Rename Space
expect "^browser: rename space $first\$" "Rename Space didn't open the name's field"
send state
in_browser AccessKitSubclassOfGPUIView "in the name's field"
send type work
send key return
expect "^browser: renamed space $first work\$" "return in the name's field didn't rename the Space"
send doubleclick $name_point
expect "^browser: rename space $first\$" "a double-click on the Space's name didn't open its field"
# A right-click in the field shows the field's menu, not the Space's, and the field stays.
post rightclick $name_point
expect "^browser: menu shown $(cased 'Cut, Copy, Paste, Select All')\$" \
  "a right-click in the name's field didn't show the field's menu"
none_since '^browser: menu shown Rename Space' || fail "the right-click showed the Space's menu too"
send choose "$(cased 'Select All')"
send type gone
send key escape
expect '^browser: rename cancelled$' "escape in the name's field didn't cancel"
none_since '^browser: renamed space' || fail "escape in the name's field renamed the Space"
# Rename Space in the Spaces menu turns the name of the Space shown into a field too.
send menu Rename Space
expect '^browser: command rename-space via ui$' "Rename Space in the menu didn't run rename-space"
expect "^browser: rename space $first\$" "Rename Space in the menu didn't open the name's field"
send key escape
expect '^browser: rename cancelled$' "escape in the name's field didn't cancel"
# Change Icon… in the Space's menu opens the icon picker under the Space's name, its arrow at the
# icon's slot, on the Icon tab for a Space with no emoji. It's a child window with the keyboard, so
# its clicks go to the key window, in points from its top left. Escape closes it with no change,
# and the menu offers Remove Icon only once there is an icon.
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for $first on icon\$" "Change Icon… didn't open the icon picker"
expect '^browser: icon picker shown$' "the icon picker didn't show"
at=$(grep '^browser: icon picker at' "$log" | tail -1 | cut -d' ' -f5-)
# 273 wide, with the arrow's tip 13 in, under the icon's middle at x 26.
[ "$(echo "$at" | awk '{ print $1, $3, $4 }')" = "13 273 286" ] ||
  fail "the icon picker opened at $at, not 273 by 286 from x 13"
send key escape
expect '^browser: icon picker hidden$' "escape didn't close the icon picker"
none_since '^browser: iconed space' || fail "escape in the icon picker gave the Space an icon"
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the Space's name offered Remove Icon without an icon"
send choose "$(cased 'Change Icon…')"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
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
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, Remove Icon, $colors, $profile, \
Export as Markdown…, Delete Space")\$" "a right-click on the Space's name didn't offer Remove Icon"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for $first on emoji\$" \
  "Change Icon… didn't open the picker on the Emoji tab for a Space with an emoji"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
../tools/window-screenshot.sh "$pid" "$out/space-emoji-grid.png" all || fail "no window to capture"
send clickkey 197 39.5
expect "^browser: iconed space $first none\$" "the icon picker's trash didn't take the icon away"
expect '^browser: icon picker hidden$' "the icon picker's trash didn't close it"
# Remove Icon in the menu does the same.
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "the Space's menu offered Remove Icon after the trash"
send choose "$(cased 'Change Icon…')"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
send clickkey 151.75 76.25
expect "^browser: iconed space $first glyph:bolt\$" \
  "a click on a glyph didn't become the Space's icon"
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, Remove Icon, $colors, $profile, \
Export as Markdown…, Delete Space")\$" "a right-click on the Space's name didn't offer Remove Icon"
send choose "$(cased 'Remove Icon')"
expect "^browser: iconed space $first none\$" "Remove Icon didn't take the Space's icon away"
# Edit Theme Color… in the Space's menu opens the theme picker too. A swatch is the Space's colour,
# its own hex, at its hue and lightness in the field, and the wave's knob dragged left washes the
# window's colours out, the window following as it goes; the core keeps the theme once it's let go.
# A click outside the picker closes it.
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Edit Theme Color…"
send choose "$(cased 'Edit Theme Color…')"
expect "^browser: theme picker for $first\$" "Edit Theme Color… didn't open the theme picker"
expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
send clickkey "$(swatch_x 7)" $swatch_y
blue='x=-0.619[0-9] y=-0.242[0-9] colors=1'
expect "^browser: theme picker set $first $blue intensity=0.810 " "the blue swatch gave no colour"
expect "^browser: colored space $first 7eb6d5\$" "the swatch didn't give the Space its colour"
expect "^browser: themed space $first $blue intensity=0.810 " "the core didn't keep the swatch"
at=$(grep '^browser: theme picker at' "$log" | tail -1 | cut -d' ' -f5-)
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
../tools/window-screenshot.sh "$pid" "$out/space-theme.png" || fail "no window to capture"
send click $page_point
# The host closes the picker for a click outside it, and only escape's own dismissal logs one.
expect '^browser: theme picker hidden$' "a click outside the theme picker didn't close it"
# Remove Theme Color, offered once the Space has a colour, takes its theme and colour away, and the
# menu offers it no more.
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on the Space's name didn't offer Remove Theme Color"
send choose "$(cased 'Remove Theme Color')"
expect "^browser: colored space $first none\$" \
  "Remove Theme Color didn't take the Space's colour away"
expect "^browser: themed space $first none\$" \
  "Remove Theme Color didn't take the Space's theme away"
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "the Space's menu offered Remove Theme Color without a colour"
send choose Rename Space
send key escape
expect '^browser: rename cancelled$' "escape in the name's field didn't cancel"
# Deleting a Space with tabs asks first, in a dialog over the window, and a click around it cancels.
# Return deletes the Space once the dialog has gone. Its tabs close with it, and ⇧⌘T reopens them
# here.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" \
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
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" \
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
expect "^browser: spaces $first $second\$" "⌘Z didn't bring the Space back where it was"
expect "^browser: space $second\( in .*\)\{0,1\}\$" "⌘Z didn't show the Space it brought back"
expect "^browser: loaded [0-9]* https://example.com/\$" "the Space came back without its tab"
expect "^browser: settled space $second\$" "the Space ⌘Z brought back didn't settle"
# Deleted again, its tab reopens here with ⇧⌘T, and ⌘Z brings back the Space without it.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
post choose Delete Space
expect '^browser: dialog shown Delete “reading 日本語”?$' "the Space came back without its name"
expect '^browser: dialog settled$' "the dialog didn't settle"
send key return
expect "^browser: spaces $first\$" "Delete didn't delete the Space it brought back"
expect "^browser: today $a\$" "the Space shown didn't come back: Today is \"$(today)\""
send key t cmd shift
expect '^browser: command reopen-closed-tab via ui$' "⇧⌘T didn't reach the UI"
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
send window 1
post rightclick 131 $dots_y
# ⌘Z brought the Space back with its theme.
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $empty_profile, \
Export as Markdown…, Delete Space")\$" \
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
# Empty, it goes without asking, and says so too.
post rightclick 131 $dots_y
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $empty_profile, \
Export as Markdown…, Delete Space")\$" \
  "a right-click on a dot didn't show its Space's menu"
send choose Delete Space
expect "^browser: spaces $first\$" "Delete didn't delete the empty Space"
none_since '^browser: dialog shown' || fail "deleting an empty Space asked first"
expect "$deleted" "the toast didn't say the empty Space went"
# The last Space can't go.
post rightclick $name_point
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space (disabled)")\$" \
  "the last Space's menu offered Delete"
send choose Rename Space
send key escape
expect '^browser: rename cancelled$' "escape in the name's field didn't cancel"
# The page shown is hidden while its window is minimized, and while the app is hidden, as Chrome's
# is, and visible once they're back. Last, as the app may not be active after it's been hidden.
# Today is the tab ⇧⌘T reopened over the page's.
send click "$(row 1)"
last_title=$(grep "^browser: title $a " "$log" | tail -1 | cut -d' ' -f4-)
[ "${last_title%% *}" = visible ] ||
  expect "^browser: title $a visible " "a click on the page's row didn't show it"
ran_hidden "the page shown by its row had run frames while hidden, other than its picture's"
# Linux has no hiding an app, and the run's sway session keeps its focused window from minimizing.
if [ "$(uname)" = Linux ]; then
  echo "skip: a minimized window and a hidden app, which sway and Linux have no way to make."
else
  send minimize
  expect "^browser: title $a hidden\$" "the page in the minimized window isn't hidden"
  send minimize
  expect "^browser: title $a visible 0\$" \
    "the page isn't visible after the window came back, or ran frames while minimized"
  send hide
  expect "^browser: title $a hidden\$" "the page isn't hidden while the app is"
  send hide
  expect "^browser: title $a visible 0\$" \
    "the page isn't visible after the app came back, or ran frames while hidden"
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
echo "ok: New Space makes an empty Space whose form takes the keyboard and Japanese input, and names it, and ⌘T opens a tab in it; ⌃1 ⌃2 ⌥⌘← ⌥⌘→ with the page or the UI focused, and the switcher's dots, show each Space with the tab it showed last; two fingers across the sidebar page to the Space a flick or the pages' place points at, never past the last, and a wheel's tick across pages once; past the first Space, fingers and a wheel's tick bring the Library out and put it back, its Spaces section as wide as its cards, and fingers let go short of halfway put it back, and past where it opens take it on a little, its column's fill at the window's edge; a right-click on a row as the Space's rows slide in shows its tab's menu; a tab's menu moves it to another Space, which shows with it; a row held at the sidebar's edge pages to the next Space without joining the row under it in a split, and let go there moves to it; fingers held past the last Space ready a new Space and let go make it, with its form, and let go short of that don't; Cancel, and two fingers back, delete a new Space and go back to the Space it was made from, and fingers going on past its form rubber-band it. A right-click on a Space's name, dot or Library card shows Rename, Change Icon…, Edit Theme Color… and Delete, and one on the sidebar's empty space New Tab before them, which opens the command bar; the theme picker, from the form's row and the menu, gives a Space its colours from its field, a swatch's own, + and the wave, and a colour dragged alone to a place of its own, the window taking them as they're made, and Remove Theme Color, offered once it has one, takes them away; an emoji picked in its icon picker, by a search and ↩, is its icon, in its dot's place in the switcher, until Remove Icon, and escape leaves it as it was; a double-click on the name renames it too, a right-click in its field shows the field's menu, return keeps the name and escape cancels; a Library card's field lights with the pointer on its brush or icon slot, the brush opens the theme picker beside itself, which makes the Space light and dark, and the slot the icon picker, which another card's slot moves, both over the Library, which stays open until escape closes the picker and then it, and a click on the card's name edits it in the card with the Library left open. Delete asks first for a Space with tabs, in a dialog a click around cancels and return confirms, closes them once it has gone, and the toast says so; ⌘Z brings the Space back where it was with its name and tabs, ⇧⌘T reopens them in the Space shown, and ⌘Z then leaves those out; the last Space can't go. The Spaces menu names each Space and shows it, and its Rename Space opens the name's field. Move to Space offers Undo, ⌘Z takes the tab back to its Space and ⇧⌘Z moves it again. Move to › lists the other Space's Today and its folder, and moves the tab into the folder, which ⌘Z undoes. The folder's Move to Space › moves it with its tab to the other Space, the window going with it, and ⌘Z takes it back. With two windows, each one's ⌘Z undoes only what was done in it: a Space deleted in one, the archive cleared in the other. A page behind another tab, in a Space not shown, in a minimized window or a hidden app is hidden and runs no animation frames, and is visible once back. Paging, the sidebar a frame: ${paged:-not logged}. Windows: $out/swipe.png, $out/library-swipe.png, $out/library-end.png, $out/library-spaces.png, $out/library-field-hover.png, $out/library-theme-picker.png, $out/library-icon-picker.png, $out/library-rename.png, $out/new-space-pull.png, $out/space-empty-menu.png, $out/space-menu.png, $out/space-delete.png, $out/space-deleted.png"
echo "  Space switch, key to the core's report: $spaces_ms. Windows: $out/space-form.png, $out/space-form-ime.png, $out/theme-picker.png, $out/space-theme.png, $out/new-space.png, $out/spaces.png, $out/space-icon-picker.png, $out/space-icon.png, $out/space-emoji.png, $out/move-to-folder.png, $out/folder-move-menu.png, $out/folder-moved.png"
