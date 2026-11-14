# check.sh runs this at 30.
# A tab opened from the command bar goes at the top of the Space's Today tabs and shows. The command
# bar lists the Space's other tabs before anything is typed, those shown last first, and suggests
# pages from history as it's typed in, growing to fit them, with the tabs already at them, never the
# one shown, and well-known sites under them. An open tab whose name the typing starts is the first
# row. ↩ or a click on a tab's row shows that tab without opening one. Typing at the end completes
# the text to the first row's page, and ↩ goes there. ⌘L opens the bar centred and a click on the
# URL field over the field. ⇧⌘A opens it for Search Tabs, listing the open tabs alone, which typing
# filters, and ↩ switches to the one left. ⌘T or ⌘L closes the open bar again, ⌘L's field holds the
# whole address, and ↩ on it as it opened reloads the page. Tab in the empty field, or "/ ", lists
# only the commands, which typing filters, until Esc or ⌫ at the start leaves, and "2. tab" goes to
# the second sidebar item. ⌘1–⌘9, ⌥⌘↓, ⌥⌘↑ and clicks on rows show tabs. ⌃Tab, with the page or
# the UI focused, shows the tab shown before, and with ⌃ held goes on back through the tabs in the
# order they were shown, and ⌃⇧Tab the other way,
# until ⌃ goes up or a click. A tab a page opens goes under it and closing it goes back there, and
# ⌘W otherwise shows the tab below, or above at the bottom. Tabs opened behind close to their
# sibling, then their opener; after a switch to an unrelated tab, a tab opened in front closes to
# the tab below; and a pinned tab's newest tab opened in front closes back to it, past an older one.
# On Linux two quick ⇧⌘T from the page, the second before the first's tab has its page focused,
# reopen both tabs closed last.
# Closing the last tab leaves the Space empty, with no pane, its window resizing, and on Windows
# maximizing and restoring, with no page, and ⌘T still works there. Mute/Unmute Tab mutes a tab,
# its row shows a speaker while it plays, crossed out while it's muted, and a click on it unmutes
# the tab. There ⇧⌦ takes a page out of history, as a click on the trash the pointer draws on its row
# does, and ⌥⌘⌫ a search typed before, ⇧⌘↩ searches for what was typed, ⌘↩ opens it in a tab behind
# that loads once it's first shown, Tab on the row that hints a search engine, or a space after its
# keyword, searches with that engine alone until ⌫ leaves, ↩ on a sum's row copies its result, and a
# right-click in its field shows the field's menu, with Paste and Go. Site Search Settings opens
# Chromium's search engines page. The command bar's tests, in src/command_bar/tests.rs, have the
# rest of its keys and what the menu's items do. A tab's menu walks by its keys past lines and
# dimmed items, selects by typing, opens a submenu by → and by the pointer, which crosses a row on
# its way there without taking it, picks what a press-drag-release ends on and closes on a click
# outside. A tab unseen for 2 hours lets go of its page as another is shown, unless it plays sound,
# its page asks before it's left or a split shows it, and loads it back with its history once shown,
# and Settings' Unload switch turns that off and on. The header's glyphs stay lit with the
# command bar open and with the page having the keyboard, and dim behind Settings till it goes. Back right-clicked or held lists the pages
# back, nearest first, with their favicons, a row picked goes there, and ⌘-clicked it opens the page
# back in a tab behind. Tabs › Reset all tabs in this Space takes the loaded pinned tabs away from
# their pinned URLs back there, and leaves an unloaded one. ⌘-click and ⇧-click choose rows,
# whose menu and ⇧⌘C copy their links, one a line, and ⌘D, New Folder and ⌘W pin, folder and
# close them all, each one step for ⌘Z. Reports how long switches take.

# A Space's Today tabs. The page opens a tab of its own from its button, the way a link to a new tab
# does, p in it plays a quiet tone until x, and u has it ask before it's left, or stop asking. The
# app asks Chrome to trace its tab switches, timed as Chrome times its own: from the tab strip's
# change to the first frame shown. The trace is written as the app quits. In a handler attribute a
# bare open() is document.open(), which would blank the page instead.
page='data:text/html,<title>loading</title>
<button onclick=window.open() style="font:24px system-ui">Open a tab</button>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let sound;
onkeydown = e => {
  if (e.key == "p") {
    sound = new AudioContext();
    const tone = sound.createOscillator();
    const gain = sound.createGain();
    gain.gain.value = 0.002;
    tone.connect(gain).connect(sound.destination);
    tone.start();
    document.title = "playing";
  }
  if (e.key == "x") sound.close();
  if (e.key == "u") {
    onbeforeunload = onbeforeunload ? null : e => e.preventDefault();
    document.title = onbeforeunload ? "asks" : "lets go";
  }
};
</script>'
log="$out/tabs.log"
trace="$PWD/$out/trace.json"
step=0
rm -f "$saved" "$launch_frame"
# shellcheck disable=SC2086 # the record mode, or nothing
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" --enable-tracing="$trace_categories" $trace_mode \
  --enable-tracing-format=json --trace-startup-file="$trace" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the tabs test page didn't draw"

await '^browser: today [0-9][0-9]*$' || fail "the first tab isn't in Today"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: tabs. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# Mute/Unmute Tab mutes the tab, and its row shows the speaker crossed out. The page plays only
# once it's muted, so the run makes no sound, and Chrome counts its sound for 2 s after it stops.
# A click on the speaker unmutes the tab, and the speaker goes.
bar_command toggle-mute "Mute/Unmute Tab" mute
expect "^browser: page $a audible 0 muted 1\$" "Mute/Unmute Tab didn't mute the tab"
send click $page_point
send key p
expect "^browser: title $a playing\$" "p in the page didn't play its sound"
expect "^browser: page $a audible 1 muted 1\$" "the muted tab's sound didn't show"
../tools/window-screenshot.sh "$pid" "$out/muted.png" || fail "no window to capture"
send key x
expect "^browser: page $a audible 0 muted 1\$" "the tab's sound didn't stop"
# The speaker is after the favicon, at the title's x, 43.
send click 52 $(row 0 | cut -d' ' -f2)
expect '^browser: command toggle-mute via sidebar$' "a click on the speaker didn't reach the UI"
expect "^browser: page $a audible 0 muted 0\$" "a click on the speaker didn't unmute the tab"
open_from_bar example.com
b=$tab
open_from_bar example.org
c=$tab
expect_tabs "$c $b $a" "$c" "after opening two tabs"
../tools/window-screenshot.sh "$pid" "$out/tabs.png" || fail "no window to capture"

# The command bar's last suggestions, as "<url>[ tab <id>][ site <keyword>]" a line.
suggestions() {
  grep '^browser: command bar suggestions' "$log" | tail -1 | sed 's/^[^:]*: [^:]* ms: //' |
    tr -s ', ' '\n ' | sed 's/^ //'
}
# The command bar's height logged last.
bar_height() {
  grep '^browser: command bar height' "$log" | tail -1 | sed 's/.* //'
}
# Waits for the bar to take its last suggestions' height, which the host sets after the UI logs
# them, so a capture then shows the rows drawn. It may have been as many rows before. Arc's panel:
# the input 64, its line and the gap under it 4.5, 50 a row up to five, 10 below.
await_bar_height() {
  rows=$(suggestions | wc -l | tr -d ' ')
  want=$(awk -v rows="$rows" 'BEGIN { if (rows > 5) rows = 5; print 78.5 + 50 * rows }')
  for _ in $(seq 50); do
    [ "$(bar_height)" = "$want" ] && return
    sleep 0.1
  done
  fail "the command bar is $(bar_height) tall, not $want for $rows rows"
}
# Opens the command bar and types "example", which completes to a page and suggests example.com
# and example.org. ⌫ takes the completion back, so the first row is the search for what was typed.
# Waits for $1's tab among the rows, and checks that $2's isn't, since $2 is shown. "example" starts
# the open tab's site, so tab $3 is the first row, with Switch to Tab, where ↩ switches to it.
suggest_example() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type example
  expect '^browser: command bar completes "example" with "\.' "the command bar didn't complete \"example\""
  send key backspace
  expect "^browser: command bar suggestions \"example\" in [0-9.]* ms: https://duckduckgo.com/?q=example, .*https://$1/ tab " \
    "the command bar didn't suggest $1's tab: $(suggestions | tr '\n' ' ')"
  expect "^browser: command bar puts tab $3 first\$" "the command bar didn't put $1's tab first"
  await_bar_height
  none_since "https://$2/ tab " || fail "the command bar suggested $2's tab, which is shown"
}
suggest_example example.com example.org "$b"
../tools/window-screenshot.sh "$pid" "$out/suggestions.png" || fail "no window to capture"
send key return
expect "^browser: command bar switch to tab $b\$" "↩ on example.com's row didn't switch to its tab"
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "↩ on example.com's row didn't show its tab"
[ "$(today)" = "$c $b $a" ] || fail "switching to a tab changed Today to \"$(today)\""
suggest_example example.org example.com "$c"
# Rows are 50 tall, under the 64 of the input, its line and a gap of 4. The tab's is the first.
send clickkey 320 $((68 + 25))
expect "^browser: command bar switch to tab $c\$" "a click on example.org's row didn't switch to its tab"
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "a click on example.org's row didn't show its tab"
[ "$(today)" = "$c $b $a" ] || fail "switching to a tab changed Today to \"$(today)\""

# Typing at the end completes the text to the first row's page, as Chrome's address bar does:
# example.com, typed before, completes "example.c", and ↩ goes where it goes: to its tab, since a
# tab has it open (issue #247).
send key l cmd
expect '^browser: command bar opens centred$' "⌘L didn't open the command bar centred"
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type example.c
expect '^browser: command bar completes "example.c" with "om"$' "the command bar didn't complete \"example.c\" to example.com"
expect '^browser: command bar drew "example.com"$' "the command bar didn't show the completion"
../tools/window-screenshot.sh "$pid" "$out/completion.png" || fail "no window to capture"
send key return
expect "^browser: command bar switch to tab $b\$" "↩ didn't switch to the tab the completion's page is open in"
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "↩ on the completion didn't show its tab"
[ "$(today)" = "$c $b $a" ] || fail "switching to the completion's tab changed Today to \"$(today)\""
# ⇧⌘A opens Search Tabs on the Space's tabs but the one shown, $1 and $2, those shown last first.
# "exa" names only $1, at example.com or example.org, where ↩ switches to it. It reaches the
# browser via $3, the page or the UI, which has the keyboard.
search_tabs() {
  send key a shift cmd
  expect "^browser: command search-tabs via $3\$" "⇧⌘A with $3 focused didn't run Search Tabs"
  expect "^browser: command bar searches tabs $1 $2\$" \
    "Search Tabs didn't list tabs $1 and $2: $(grep '^browser: command bar searches tabs' "$log" | tail -1)"
  expect '^browser: command bar shown$' "⇧⌘A didn't show the command bar"
  send type exa
  expect "^browser: command bar finds tabs \"exa\": $1\$" \
    "Search Tabs didn't keep tab $1 alone for \"exa\": $(grep '^browser: command bar finds tabs' "$log" | tail -1)"
  [ "$1" != "$c" ] || ../tools/window-screenshot.sh "$pid" "$out/search-tabs.png" || fail "no window to capture"
  send key return
  expect "^browser: command bar switch to tab $1\$" "↩ in Search Tabs didn't switch to tab $1"
  expect "^browser: selected $1\( in .*\)\{0,1\}\$" "↩ in Search Tabs didn't show tab $1"
  [ "$(today)" = "$c $b $a" ] || fail "Search Tabs changed Today to \"$(today)\""
}
send click $page_point
search_tabs "$c" "$a" page
search_tabs "$b" "$a" '[a-z]*'
# A click on the URL field opens the bar on the tab's URL over the field, as Arc's does: 350 wide,
# its left at the field's and its top 2 above the field's, where ⌘L's is centred.
send click 100 64
expect '^browser: command edit-url via sidebar$' \
  "a click on the URL field didn't open the command bar"
expect '^browser: command bar opens over the URL field$' \
  "a click on the URL field didn't open the command bar over the field"
expect '^browser: command bar shown$' "a click on the URL field didn't open the command bar"
expect '^browser: command bar drew "https://example.com/"$' \
  "the command bar from the URL field didn't show the tab's URL"
at=$(grep '^browser: command bar at' "$log" | tail -1 | cut -d' ' -f5-)
# The input alone is a row, 50 tall, over the field at 8, 46.
[ "$at" = "8 44 350 50" ] ||
  fail "the command bar from the URL field opened at $at, not at 8 44 and 350 by 50"
../tools/window-screenshot.sh "$pid" "$out/url-field-bar.png" || fail "no window to capture"
# The field's own menu is ilo's, and gives the keyboard back to the field.
post rightclickkey 175 25
expect "^browser: menu shown $(cased 'Cut[^,]*, Copy[^,]*, Paste, Paste and Go, Paste and Search, Select All')\$" \
  "a right-click in the command bar's field didn't show its menu"
expect '^browser: menu window at ' "the field's menu didn't open its window"
none_since '^browser: command bar hidden$' || fail "the field's menu closed the command bar"
send choose "$(cased 'Select All')"
expect '^browser: menu window hidden$' "choosing Select All didn't close the field's menu"
none_since '^browser: command bar hidden$' || fail "choosing in the field's menu closed the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# ⌘T or ⌘L closes the open bar, whichever opened it, as Arc's does. ⌘L's field holds the tab's
# whole address, https:// too, so a copy of it keeps the scheme; the run copies nothing, as the
# field's copy goes to the owner's pasteboard. ↩ on that address as it opened reloads the page.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
expect '^browser: command bar drew "https://example.com/"$' \
  "⌘L's field didn't hold the tab's whole address, https:// too"
send key t cmd
expect '^browser: command bar closes again on new-tab$' "⌘T didn't close the bar ⌘L opened"
expect '^browser: command bar hidden$' "⌘T left the command bar open"
none_since '^browser: command bar shown$' || fail "⌘T opened the command bar again as it closed it"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send key l cmd
expect '^browser: command bar closes again on edit-url$' "⌘L didn't close the bar ⌘T opened"
expect '^browser: command bar hidden$' "⌘L left the command bar open"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
expect '^browser: command bar drew "https://example.com/"$' "⌘L's field didn't hold the tab's address"
send key return
expect "^browser: command bar reloads tab $b\$" "↩ on the address ⌘L opened on didn't reload the tab"
expect "^browser: loaded $b https://example.com/\$" "↩ on the address ⌘L opened on didn't load it again"
expect '^browser: command bar hidden$' "↩ on the address ⌘L opened on didn't close the command bar"
[ "$(today)" = "$c $b $a" ] || fail "↩ on the address ⌘L opened on changed Today to \"$(today)\""
# Tab in the empty field lists only the commands, and what's typed next filters them, as Arc's
# bar does; Esc leaves the list, putting back what was typed, then closes the bar. "/ " typed lists
# them too, and ⌫ at the field's start leaves, putting "/ " back. The driver types no "/", so it
# commits one as an input method would, and then types the space.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send key tab
expect '^browser: command bar lists commands by tab$' "Tab in the empty field didn't list only the commands"
expect '^browser: command bar commands only "": Pin Tab, ' \
  "the commands-only list didn't start with Pin Tab: $(grep '^browser: command bar commands only' "$log" | tail -1)"
send type split
expect '^browser: command bar commands only "split": Add Split View' \
  "the commands-only list didn't keep the split commands for \"split\": $(grep '^browser: command bar commands only' "$log" | tail -1)"
none_since '^browser: command bar suggestions ' || fail "the commands-only list asked the core for pages"
../tools/window-screenshot.sh "$pid" "$out/commands-only.png" || fail "no window to capture"
send key escape
expect '^browser: command bar leaves commands$' "Esc didn't leave the commands-only list"
expect '^browser: command bar drew "split"$' "leaving the commands-only list didn't keep what was typed"
none_since '^browser: command bar hidden$' || fail "Esc in the commands-only list closed the bar"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar after the commands-only list"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert /
send key space
expect '^browser: command bar lists commands by slash$' "\"/ \" didn't list only the commands"
send type pin
expect '^browser: command bar commands only "pin": Pin Tab' \
  "the commands-only list didn't find Pin Tab for \"pin\": $(grep '^browser: command bar commands only' "$log" | tail -1)"
for _ in 1 2 3 4; do send key backspace; done
expect '^browser: command bar leaves commands$' "⌫ at the start didn't leave the commands-only list"
expect '^browser: command bar drew "/ "$' "leaving the commands-only list didn't put \"/ \" back"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"

# ilo's menu as #324 has it. The keys walk it past the line and the dimmed Mute, and typing selects
# by a title's start.
post rightclick $(row 0)
expect "^browser: menu shown $(cased 'Copy Link, .*, Mute (disabled), .*, Duplicate, Add to Folder › New Folder, Move to › ')" \
  "a right-click on the top Today row didn't show its tab's menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send key down
expect "^browser: menu highlights $(cased 'Copy Link') at " "↓ didn't highlight the menu's first item"
send key d
expect '^browser: menu highlights Duplicate at ' "typing d didn't select Duplicate"
send key escape
expect '^browser: menu closed$' "escape didn't close the menu"
# m passes the dimmed Mute for Move to, → opens its submenu on its first item, and escape closes
# only the submenu.
post rightclick $(row 0)
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click on the top Today row didn't show its menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send key m
expect '^browser: menu highlights Move to at ' "typing m didn't pass the dimmed Mute for Move to"
send key right
expect '^browser: menu submenu Move to shown at ' "→ didn't open Move to's submenu"
expect '^browser: menu highlights Move to › ' "→ didn't highlight the submenu's first item"
# On Linux the menu is a window over the browser window's. Under X11 with no compositor, as under
# Xvfb, a window can't blend and shows its clear pixels black, so there the panels are square and
# opaque with nothing round them (#1479): no corner of the menu or its submenu is black, and nor is
# the sidebar just left of it, where the shadow's room was.
if [ "$(uname)" = Linux ]; then
  expect '^browser: menu submenu Move to drawn$' "Move to's submenu wasn't drawn"
  set -- $(grep '^browser: menu window at ' "$log" | tail -1 | sed 's/.* at //; s/[,x]/ /g')
  left=$1 top=$2 bottom=$(($2 + $4))
  set -- $(grep '^browser: menu submenu Move to shown at ' "$log" | tail -1 | sed 's/.* at //')
  sub_right=$(($1 + $3 - 2)) sub_bottom=$(($2 + $4 - 2))
  points="$((left + 1)),$((top + 1)) $(($1 + 2)),$((top + 1)) $((left - 3)),$((top + 12))"
  points="$points $(($1 + 1)),$(($2 + 1)) $sub_right,$(($2 + 1)) $(($1 + 1)),$sub_bottom $sub_right,$sub_bottom"
  # The root's bottom is the window's where the submenu ends above it.
  [ "$sub_bottom" -ge $((bottom - 2)) ] || points="$points $((left + 1)),$((bottom - 2))"
  # A few captures, as the window takes its new shape as the submenu draws.
  black=
  for _ in 1 2 3 4 5; do
    ../tools/window-screenshot.sh "$pid" "$out/menu.png" || fail "no window to capture"
    [ "${BROWSER_HEADLESS:-}" = xvfb ] || break
    size=$(magick identify -format '%w %h' "$out/menu.png")
    width=${size% *} height=${size#* } black=
    for point in $points; do
      x=${point%,*} y=${point#*,}
      set -- $(pixel "$out/menu.png" "$(awk "BEGIN { print ($x + 0.5) / $width }")" \
        "$(awk "BEGIN { print ($y + 0.5) / $height }")")
      [ "$1" -ge 8 ] || [ "$2" -ge 8 ] || [ "$3" -ge 8 ] || black="$black $x,$y"
    done
    [ -n "$black" ] || break
  done
  [ -z "$black" ] || fail "with no compositor, the menu at $left,$top shows black at$black (#1479)"
fi
send key escape
expect '^browser: menu submenu Move to hidden$' "escape didn't close Move to's submenu"
none_since '^browser: menu closed$' || fail "escape in the submenu closed the whole menu"
send key escape
expect '^browser: menu closed$' "escape didn't close the menu after its submenu"
# The safe triangle: on its way to the submenu, the pointer crosses the row under Move to without
# taking the highlight, until it rests there.
post rightclick $(row 0)
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click on the top Today row didn't show its menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send key m
expect '^browser: menu highlights Move to at ' "typing m didn't select Move to"
set -- $(grep '^browser: menu highlights Move to at ' "$log" | tail -1 | sed 's/.* at //')
from_x=$(($1 + $3 / 2)) from_y=$(($2 + $4 / 2)) below=$(($2 + $4))
send move $from_x $from_y
expect '^browser: menu submenu Move to shown at ' "the pointer on Move to didn't open its submenu"
# The triangle runs from the pointer to the submenu's near edge. Aim just short of that edge, at
# the lowest point the triangle reaches there, which is over the rows under Move to.
set -- $(grep '^browser: menu submenu Move to shown at ' "$log" | tail -1 | sed 's/.* at //')
if [ "$1" -ge "$from_x" ]; then edge=$1 to_x=$(($1 - 6)); else edge=$(($1 + $3)) to_x=$(($1 + $3 + 6)); fi
to_y=$((from_y + (to_x - from_x) * ($2 + $4 - from_y) / (edge - from_x) - 2))
[ "$to_y" -ge $((below + 2)) ] ||
  fail "Move to's submenu at $* leaves no room in its triangle for the row under Move to"
# Read as the move is handled: the rest ends the triangle 300 ms on (menu.rs's SAFE_TIMEOUT).
send move $to_x $to_y
none_while '^browser: menu highlights ' ||
  fail "the pointer on its way to the submenu took the highlight of the row it crossed"
none_while '^browser: menu submenu Move to hidden$' ||
  fail "the pointer on its way to the submenu closed it"
expect '^browser: menu submenu Move to hidden$' "resting under Move to didn't close its submenu"
send key escape
expect '^browser: menu closed$' "escape didn't close the menu"
# Press, drag and release: a right press opens the menu, and letting go on an item picks it.
set -- $(row 0)
send rightpress $1 $2
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right press on the top Today row didn't show its menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send rightdragto $(($1 + 60)) $(($2 + 16))
expect "^browser: menu highlights $(cased 'Copy Link') at " "dragging onto Copy Link didn't highlight it"
send rightrelease $(($1 + 60)) $(($2 + 16))
expect "^browser: menu chose $(cased 'Copy Link')\$" "letting go on Copy Link didn't pick it"
# The first click after an item chosen with the pointer lands in the page, and the window stays
# where it was: on Windows the press that opened the menu, whose release the menu took, left the
# browser window holding the mouse captured, so the click went to the UI over the page (#2516).
send order
[ "$focus" != RenderWidgetHostViewCocoa ] ||
  fail "the page had the keyboard before the click into it, which then can't say where the click went"
chosen_frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
send click $page_point
in_browser RenderWidgetHostViewCocoa "a click into the page right after a menu item chosen with the pointer"
send order
[ "$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)" = "$chosen_frame" ] ||
  fail "a click into the page right after a menu item chosen with the pointer moved the window from $chosen_frame"
# A click outside closes the menu, and goes no further.
post rightclick $(row 0)
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click on the top Today row didn't show its menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send click $page_point
expect '^browser: menu closed$' "a click outside didn't close the menu"
expect '^browser: menu window hidden$' "a click outside left the menu's window up"
# A right-click elsewhere opens that menu in its place: on the next row, left of the menu, which
# opens right of the pointer. ↓ shows the new one has the keys, not closed as it opened.
post rightclick $(row 0)
expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click on the top Today row didn't show its menu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
set -- $(row 1)
post rightclick $(($1 - 60)) $2
expect "^browser: menu shown $(cased 'Copy Link, ')" \
  "a right-click on the next row with the menu up didn't show that row's menu"
expect '^browser: menu window at ' "the next row's menu didn't take the keyboard"
send key down
expect "^browser: menu highlights $(cased 'Copy Link') at " \
  "the next row's menu closed as it opened, or didn't take ↓"
send key escape
expect '^browser: menu closed$' "escape didn't close the next row's menu"

# Before anything is typed, the bar lists the Space's other tabs, as Arc's does: those shown most
# recently first, then the rest, such as favorites never shown.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
listed=$(grep '^browser: command bar tabs' "$log" | tail -1 | sed 's/^[^:]*: command bar tabs *//')
case "$listed" in
  "$c $a" | "$c $a "*) ;;
  *) fail "the command bar listed tabs \"$listed\" before anything was typed, not $c then $a" ;;
esac
../tools/window-screenshot.sh "$pid" "$out/listed.png" || fail "no window to capture"
# Well-known sites fill the rows under the profile's own, never first, so ↩ still searches.
send type g
expect '^browser: command bar suggestions "g" in [0-9.]* ms: https://duckduckgo.com/?q=g, .*https://www.google.com/, https://mail.google.com/' \
  "the command bar didn't suggest well-known sites under the search: $(suggestions | tr '\n' ' ')"
../tools/window-screenshot.sh "$pid" "$out/popular.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

switch "3 cmd" "$a"
switch "1 cmd" "$c"
switch "9 cmd" "$a"
switch "up alt cmd" "$b"
switch "down alt cmd" "$a"
# Next wraps around, as in Chrome.
switch "down alt cmd" "$c"
# ⌃Tab goes back through the tabs in the order they were shown, c, a and b by now, as far as ⌃
# stays down. Then the tab it showed counts as shown last.
send click $page_point
switch "tab ctrl" "$a"
expect '^browser: command recent-tab via page$' "⌃Tab with the page focused didn't reach the UI"
switch "tab ctrl" "$b"
send flags
switch "tab ctrl" "$c"
send flags
switch "tab ctrl shift" "$a"
send flags
send click $sidebar_point
switch "tab ctrl" "$c"
expect '^browser: command recent-tab via ui$' "⌃Tab with the UI focused didn't reach the UI"
send flags
# A click ends the walk too.
switch "tab ctrl" "$a"
send click $(row 1)
expect "^browser: selected $b in " "a click on the second row didn't show its tab"
switch "tab ctrl shift" "$c"
send flags
send click 100 154
expect '^browser: command new-tab via sidebar$' "a click on \"+ New Tab\" didn't run New Tab"
expect '^browser: command bar shown$' "a click on \"+ New Tab\" didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# "2. tab", Arc's words for its numbered rows, finds Go to Sidebar Item 2 first, and ↩ shows the
# second item, b, as ⌘2 does (#1396). After the walk, so the order the tabs were shown in is its.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type 2. tab
expect '^browser: command bar commands "2. tab": Go to Sidebar Item 2' \
  "the command bar didn't find Go to Sidebar Item 2 first for \"2. tab\": $(grep '^browser: command bar commands ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/numbered.png" || fail "no window to capture"
send key return
expect '^browser: command select-tab via command bar$' "↩ on Go to Sidebar Item 2 didn't run it"
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "Go to Sidebar Item 2 didn't show the second item, $b"

switch "3 cmd" "$a"
send click 290 34
expect "^browser: tab [0-9]* opened by $a in the foreground\$" "the page's button didn't open a tab"
d=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
expect_tabs "$c $b $a $d" "$d" "a tab the page opened"
# Typing close and return in the command bar archives the tab, as Arc's bar lists Archive Tab
# first for it, and never closes the window (#1374).
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type close
expect '^browser: command bar commands "close": Archive Tab\(,\|$\)' \
  "the command bar didn't find Archive Tab first for close: $(grep '^browser: command bar commands' "$log" | tail -1)"
expect '^browser: command bar drew "close"$' "the command bar didn't draw its rows for close"
../tools/window-screenshot.sh "$pid" "$out/bar-close.png" || fail "no window to capture"
send key return
expect '^browser: command close-tab via command bar$' "close and return in the command bar didn't archive the tab"
expect_tabs "$c $b $a" "$a" "after closing the page's tab"

# A tab unseen for 2 hours lets go of its page as another is shown, keeping its row, and shown
# again it loads its page back with its history. One that plays sound, whose page asks before it's
# left, or that the window shows beside another in a split keeps its page. The driver's age makes
# every tab 2 hours older, which the run can't wait. Settings' switch turns it off, and back on.
# A copy of the page's tab, whose page asks before it's left.
bar_command duplicate-tab "Duplicate Current Tab" duplicate
expect "^browser: today $c $b $a [0-9][0-9]*\$" "Duplicate Current Tab didn't copy the tab under it"
asks=$(today | cut -d' ' -f4)
expect "^browser: title $asks ready\$" "the copy's page didn't draw"
send click $page_point
send key u
expect "^browser: title $asks asks\$" "u didn't have the copy's page ask before it's left"
# The page's tab plays its sound, muted.
switch "3 cmd" "$a"
bar_command toggle-mute "Mute/Unmute Tab" mute
expect "^browser: page $a audible 0 muted 1\$" "Mute/Unmute Tab didn't mute the tab"
send click $page_point
send key p
expect "^browser: page $a audible 1 muted 1\$" "the muted tab's sound didn't show"
# example.org beside b's page.
switch "2 cmd" "$b"
send menu Add Split View
expect '^browser: command bar shown$' "Add Split View didn't open the command bar"
send type example.org
send key return
expect "^browser: panes $b [0-9][0-9]*\$" "Add Split View didn't show a page beside b's"
n=$(grep '^browser: panes' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: loaded $n https://example.org/\$" "example.org didn't load beside b's page"
# The x of each pane, left to right.
send layout
panes_x=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9]*' | cut -d' ' -f2)
left_x=$(echo "$panes_x" | head -1)
right_x=$(echo "$panes_x" | tail -1)
c_page=$(grep "^browser: page $c back" "$log" | tail -1)
send age 7200
aged=$step
send click $((left_x + 100)) 400
expect "^browser: selected $b\$" "a click in the left pane didn't focus it"
expect 'browser: idle tabs checked$' "showing a tab didn't check for tabs unseen 2 hours"
since $aged | grep -q "browser: tab $c without its page\$" || fail "a tab unseen 2 hours kept its page"
expect "^browser: page $c unloaded 1\$" "c's row didn't show its tab without its page"
for kept in "$a playing sound" "$asks asking before it's left" "$b shown" "$n shown beside it"; do
  since $aged | grep -q "browser: tab ${kept%% *} without its page\$" &&
    fail "the tab ${kept#* } let go of its page"
done
../tools/window-screenshot.sh "$pid" "$out/idle-unloaded.png" || fail "no window to capture"
switch "1 cmd" "$c"
expect "browser: tab $c has its page\$" "c's tab didn't get its page back once shown"
expect "^browser: loaded $c ${c_page##* }\$" "c's page didn't load once shown at ${c_page##* }"
expect "^$c_page\$" "c's tab came back without its history: $(grep "^browser: page $c back" "$log" | tail -1)"
# The header's glyphs stay lit while the window is the one the user is in, with its command bar
# open over it or its page having the keyboard, which GPUI's window loses, and dim behind another
# window, Settings here, as Arc's do (host_watch_window_main).
window=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
nav_lit() {
  [ "$(last 'nav glyphs dimmed')" != true ] && none_since "^browser: window $window main false\$" ||
    fail "the header's glyphs dimmed $1"
}
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
nav_lit "under the window's command bar"
../tools/window-screenshot.sh "$pid" "$out/nav-lit-bar.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send click $page_point
await "^browser: drive $step done" || fail "the driver didn't finish the click in c's page"
nav_lit "with the page having the keyboard"
../tools/window-screenshot.sh "$pid" "$out/nav-lit.png" || fail "no window to capture"
# With the switch off, a tab 2 hours unseen keeps its page, and turned on, it lets go. Settings
# stays open behind the browser window between the two clicks and comes back to the front for the
# second: a click in a Settings window closed and opened again, on the switch it last changed, never
# reached it.
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show General"
settings_drew General
expect "^browser: window $window main false\$" "the window behind Settings wasn't told it isn't the user's"
expect '^browser: nav glyphs dimmed true$' "the window behind Settings didn't dim its header's glyphs"
expect "^browser: window $window drew main false\$" "the window behind Settings didn't draw its dimmed glyphs"
../tools/window-screenshot.sh "$pid" "$out/nav-dimmed.png" all || fail "no window to capture"
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
# Where Settings drew the switch, which differs by OS: it logs that once a place.
await '^browser: settings switch unload idle tabs at ' || fail "Settings didn't draw Unload tabs' switch"
unload_switch=$(sed -n 's/^browser: settings switch unload idle tabs at //p' "$log" | tail -1)
send clickkey $unload_switch
expect '^browser: unload idle tabs off$' "a click on Unload tabs' switch didn't turn it off"
send window 1
switch "2 cmd" "$b"
send age 7200
aged=$step
send click $((right_x + 100)) 400
expect "^browser: selected $n\$" "a click in the right pane didn't focus it"
expect 'browser: idle tabs checked$' "showing a tab didn't check for tabs unseen 2 hours"
since $aged | grep -q "browser: tab $c without its page\$" && fail "a tab let go of its page with the switch off"
send menu Settings…
# Settings comes back on the section it showed last.
expect '^browser: settings shown Tabs & Sidebar$' "Settings… didn't bring Settings back to the front"
settings_drew 'Tabs & Sidebar'
send clickkey $unload_switch
expect '^browser: unload idle tabs on$' "a click on Unload tabs' switch didn't turn it on"
expect "browser: tab $c without its page\$" "turning the switch on didn't unload the tab unseen 2 hours"
settings_close
post clickkey $close
expect '^browser: settings hidden$' "the close button didn't hide Settings"
expect "^browser: window $window main true\$" "the window wasn't told it's the user's again once Settings went"
expect '^browser: nav glyphs dimmed false$' "the window didn't light its header's glyphs again once Settings went"
# A key sent while the keyboard leaves the closing Settings window for the browser's is lost, so
# ⌘W waits for the click to finish and the browser window to have the keyboard. On the Mac the
# click's done can go with the window it closed, as in favorites.sh, so there the state says.
[ "$(uname)" = Darwin ] ||
  await "^browser: drive $step done" || fail "the driver didn't finish the click on Settings' close button"
send state
[ "$key" = BrowserCoreNSWindow ] ||
  fail "the browser window didn't get the keyboard back from Settings: it's in $key"
# On Windows Alt+F4, ⌃F4 to the driver, hides Settings too. It asks Windows to close the window, as
# the taskbar's Close window does, which Settings takes as hiding.
if windows; then
  send menu Settings…
  expect '^browser: settings shown Tabs & Sidebar$' \
    "Settings… didn't show Tabs & Sidebar again after its close button"
  settings_drew 'Tabs & Sidebar'
  post key f4 ctrl
  expect '^browser: settings hidden$' "Alt+F4 didn't hide Settings"
  await "^browser: drive $step done" || fail "the driver didn't finish Alt+F4 in Settings"
  send state
  [ "$key" = BrowserCoreNSWindow ] ||
    fail "the browser window didn't get the keyboard back from Settings after Alt+F4: it's in $key"
fi
# Back to the page's tab alone, as the run had them.
send key w cmd
expect "^browser: tab $n closed\$" "⌘W didn't close the right pane"
expect "^browser: panes $b\$" "⌘W on the right pane didn't leave b's page alone"
ran_once close-tab "one ⌘W on the right pane"
switch "4 cmd" "$asks"
send click $page_point
send key u
expect "^browser: title $asks lets go\$" "u didn't have the copy's page stop asking before it's left"
send key w cmd
expect_tabs "$c $b $a" "$a" "after closing the copy"
send click $page_point
send key x
bar_command toggle-mute "Mute/Unmute Tab" mute
expect "^browser: page $a audible [01] muted 0\$" "Mute/Unmute Tab didn't unmute the tab"

switch "1 cmd" "$c"
send key w cmd
expect_tabs "$b $a" "$b" "after closing the top tab"
switch "9 cmd" "$a"
send key w cmd
expect_tabs "$b" "$b" "after closing the bottom tab"
# Two quick ⇧⌘T from the page reopen both tabs just closed. Linux's driver presses the second as the
# app answers the first, its tab started and its renderer starting: a quick second press on a busy
# machine comes then, before the tasks the first posted, among them the one that focuses the tab's
# page, and went to the pane the page before left, where nothing heard it (#1519).
if [ "$(uname)" = Linux ]; then
  send key t cmd shift again
  logged '^browser: drive pressed again$' || fail "the driver didn't press ⇧⌘T again as the app answered the first"
  for _ in $(seq 100); do
    [ "$(since $step | grep -c '^browser: command reopen-closed-tab via page$')" -ge 2 ] && break
    sleep 0.1
  done
  [ "$(since $step | grep -c '^browser: command reopen-closed-tab via page$')" -ge 2 ] ||
    fail "the second of two quick ⇧⌘T didn't reach the UI: Today is \"$(today)\""
  logged '^browser: key for tab [0-9]*, with no page view to hear it, to the browser$' ||
    fail "the second ⇧⌘T came once the reopened tab's page had the keyboard, so the check missed #1519's race"
  expect "^browser: today [0-9][0-9]* $b [0-9][0-9]*\$" \
    "two quick ⇧⌘T didn't reopen both tabs where they were: Today is \"$(today)\""
  top=$(today | cut -d' ' -f1)
  bottom=$(today | cut -d' ' -f3)
  # Closed again, so the run goes on as it was.
  shown=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
  [ "$shown" = "$bottom" ] || switch "9 cmd" "$bottom"
  send key w cmd
  expect "^browser: today $top $b\$" "⌘W didn't close the bottom tab ⇧⌘T reopened"
  shown=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
  [ "$shown" = "$top" ] || switch "1 cmd" "$top"
  send key w cmd
  expect_tabs "$b" "$b" "after closing the tabs two quick ⇧⌘T reopened"
fi
send key w cmd
expect_tabs " *" 0 "after closing the last tab"
../tools/window-screenshot.sh "$pid" "$out/empty.png" || fail "no window to capture"
# The capture shows no pane, as in an empty Space: on Windows a pane is a window of its own, and the
# closed tab's went on drawing black over the Space (#1343).
send layout
none_since '^browser: layout content.*WebContentsViewCocoa' ||
  fail "a pane stayed over the empty Space: $(grep '^browser: layout content' "$log" | tail -1)"
# The window resized, maximized and restored with no page after the last tab closed, as after a
# close with only unloaded tabs left, goes on: on Windows the pane went on laying out the closed
# tab's freed page at each resize, and the browser crashed (#1340). Maximized by the system command,
# as by the title bar's button or Win+↑, as in the crash.
size=$(grep '^browser: layout content' "$log" | tail -1 |
  sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
smaller="$((${size% *} - 100)) $((${size#* } - 100))"
send resize $smaller
expect "^browser: drawn at ${smaller% *}x${smaller#* }\$" "the window with no page didn't draw at its new size"
if windows; then
  send maximize
  send layout
  maximized=$(grep '^browser: layout content' "$log" | tail -1 |
    sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
  [ "${maximized% *}" -gt "${smaller% *}" ] ||
    fail "the system command didn't maximize the window with no page: $maximized"
  send maximize
  expect "^browser: drawn at ${smaller% *}x${smaller#* }\$" "the window with no page didn't restore"
fi
send resize $size
expect "^browser: drawn at ${size% *}x${size#* }\$" "the window with no page didn't draw back at its size"
# ⇧⌦ takes a page from history out of the rows.
send key t cmd
expect '^browser: command new-tab via ui$' "⌘T in an empty Space didn't reach the UI"
expect '^browser: command bar shown$' "⌘T in an empty Space didn't open the command bar"
send type example
expect '^browser: command bar completes "example" with "\.' \
  "the command bar didn't complete \"example\""
send key backspace
expect '^browser: command bar suggestions "example" in .*?q=example, .*https://example.org/' \
  "the command bar didn't suggest example.org: $(suggestions | tr '\n' ' ')"
for _ in 1 2 3 4 5 6 7 8; do
  send key down
  grep '^browser: command bar selected' "$log" | tail -1 | grep -q ' https://example.org/$' && break
done
send key delete shift
expect '^browser: command bar removes https://example.org/$' "⇧⌦ didn't take example.org's row out"
expect '^browser: command bar suggestions "example" in ' \
  "the command bar didn't list its rows again after ⇧⌦"
suggestions | grep -q '^https://example.org/' && fail "example.org's row stayed after ⇧⌦"
# The pointer on a row from history draws a trash at the row's end, where Arc's has a ✕, and a
# click on it takes the row's page out rather than going there. Rows are 50 tall under the input's
# 68, and the trash's middle is 29 in from the bar's right edge; the pointer goes down them to example.com's.
# The bar's frame in the window, which can be in half points.
bar_at=$(grep '^browser: command bar at' "$log" | tail -1 | cut -d' ' -f5-)
# The trash of row $1, in the bar's window, or in the browser window's with "window".
cross() {
  echo "$bar_at" | awk -v row="$1" -v where="${2:-}" '{
    x = $3 - 29; y = 93 + 50 * row
    if (where == "window") { x += $1; y += $2 }
    print x, y
  }'
}
drew='^browser: command bar drew remove on row [0-9]* https://example.com/$'
for row in 0 1 2 3 4; do
  send move $(cross $row window)
  for _ in $(seq 10); do
    none_since "$drew" || break 2
    sleep 0.1
  done
done
expect "$drew" "the pointer on example.com's row didn't draw its trash: $(suggestions | tr '\n' ' ')"
row=$(grep "$drew" "$log" | tail -1 | cut -d' ' -f8)
../tools/window-screenshot.sh "$pid" "$out/remove-suggestion.png" || fail "no window to capture"
# Resting on it, its tooltip names it and its key. It hangs from it in the bar's window and goes
# over the bar, a window of its own over the page, which a tooltip otherwise goes under (#1394).
remove_keys=⌥⌘⌫
[ "$(uname)" = Darwin ] || remove_keys=Shift+Del
expect "^browser: tooltip shown Remove from History $remove_keys\$" \
  "resting on the trash didn't show Remove from History and $remove_keys"
expect '^browser: tooltip window shown$' "the trash's tooltip didn't show"
# Where the UI asked, in the bar, and where the host put it, which the Mac's and Linux's say.
asked=$(grep '^browser: tooltip at ' "$log" | tail -1 | cut -d' ' -f4,5)
placed=$(grep '^browser: tooltip window at ' "$log" | tail -1 | cut -d' ' -f5,6)
[ -z "$placed" ] || echo "$asked $placed" | awk '{ exit !(($1 - $3)^2 < 1 && ($2 - $4)^2 < 1) }' ||
  fail "the trash's tooltip is at $placed in the bar, not at $asked where the UI asked"
../tools/window-screenshot.sh "$pid" "$out/remove-tooltip.png" || fail "no window to capture"
send clickkey $(cross "$row")
expect '^browser: command bar removes https://example.com/$' "a click on the trash didn't take example.com's row out"
expect '^browser: tooltip hidden$' "a click on the trash didn't end its tooltip"
expect '^browser: command bar suggestions "example" in ' \
  "the command bar didn't list its rows again after the trash"
none_since '^browser: command bar \(go\|switch to tab\|behind\)' || fail "a click on the trash took its row too"
suggestions | grep -q '^https://example.com/' && fail "example.com's row stayed after its trash"
# The removal is the window's step: ⌘Z in the field puts it back as it was, its visits with it,
# and the core lists it again, and ⇧⌘Z takes it out again, as the rest of the run has it (#1394).
send key z cmd
expect '^browser: put back removal [0-9]*$' "⌘Z in the command bar didn't put example.com back"
expect '^browser: undo done$' "⌘Z in the command bar didn't undo the removal"
for _ in $(seq 50); do
  suggestions | grep -q '^https://example.com/' && break
  sleep 0.1
done
suggestions | grep -q '^https://example.com/' ||
  fail "⌘Z didn't list example.com's row again: $(suggestions | tr '\n' ' ')"
send key z shift cmd
expect '^browser: remove again removal [0-9]*$' "⇧⌘Z in the command bar didn't take example.com out again"
for _ in $(seq 50); do
  suggestions | grep -q '^https://example.com/' || break
  sleep 0.1
done
suggestions | grep -q '^https://example.com/' && fail "example.com's row stayed after ⇧⌘Z"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# ⇧⌘↩ searches for what was typed even when it names a page, as Chrome's does.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type example.com
send key return shift cmd
expect '^browser: command bar go https://duckduckgo.com/?q=example.com$' \
  "⇧⌘↩ didn't search for what was typed: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: command bar hidden$' "⇧⌘↩ didn't close the command bar"
# Site search, as Chrome's keyword mode: Tab on the row that hints the default engine searches
# with it alone, ⌫ in its empty field leaves it, putting back what was typed before Tab, and a
# space typed after the keyword is only a space, as in Arc, where Tab enters it again (#1397).
listed='^browser: command bar suggestions "duckduckgo.com'
hint="[0-9.]* ms: [^,]* site duckduckgo.com"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type duckduckgo.com
expect "$listed\" in $hint" \
  "the command bar's first row didn't hint DuckDuckGo's site search: $(suggestions | tr '\n' ' ')"
await_bar_height
../tools/window-screenshot.sh "$pid" "$out/site-hint.png" || fail "no window to capture"
send key tab
expect '^browser: command bar site search duckduckgo.com by tab$' \
  "Tab on the row that hints DuckDuckGo didn't enter its site search"
send type example
expect "$listed example\" in [0-9.]* ms: https://duckduckgo.com/?q=example" \
  "the site search didn't list DuckDuckGo's search first: $(suggestions | tr '\n' ' ')"
# It lists only that search, as Arc's does, though the core also lists DuckDuckGo's searches from
# history, such as ⇧⌘↩'s for example.com above: one row under the input, 78.5 + 50 tall.
listed_rows=$(suggestions | wc -l | tr -d ' ')
for _ in $(seq 50); do
  [ "$(bar_height)" = 128.5 ] && break
  sleep 0.1
done
[ "$(bar_height)" = 128.5 ] ||
  fail "the site search is $(bar_height) tall, not one row's 128.5, for the core's $listed_rows rows"
../tools/window-screenshot.sh "$pid" "$out/site-search.png" || fail "no window to capture"
for _ in 1 2 3 4 5 6 7 8; do send key backspace; done
expect '^browser: command bar leaves site search$' \
  "⌫ in the site search's empty field didn't leave it"
expect '^browser: command bar drew "duckduckgo.com"$' \
  "leaving the site search didn't put back what was typed before Tab"
expect "$listed\" in $hint" \
  "the command bar didn't hint DuckDuckGo's site search again: $(suggestions | tr '\n' ' ')"
send type ' '
expect "$listed \" in " "the command bar didn't list what a space after DuckDuckGo's keyword asks"
none_since '^browser: command bar site search' || fail "a space after DuckDuckGo's keyword entered its site search"
expect '^browser: command bar drew "duckduckgo.com "$' "a space after DuckDuckGo's keyword didn't stay in the field"
send key backspace
expect "$listed\" in $hint" \
  "the command bar didn't hint DuckDuckGo's site search again: $(suggestions | tr '\n' ' ')"
send key tab
expect '^browser: command bar site search duckduckgo.com by tab$' \
  "Tab didn't enter DuckDuckGo's site search again"
send type dogs
expect "$listed dogs\" in " "the site search didn't list DuckDuckGo's searches for what was typed"
send key return
went=$(grep '^browser: command bar go' "$log" | tail -1)
expect '^browser: command bar go https://duckduckgo.com/?q=dogs' \
  "↩ in the site search didn't search DuckDuckGo: $went"
expect '^browser: command bar hidden$' "↩ in the site search didn't close the command bar"
searching dogs "↩ in the site search didn't start DuckDuckGo's search for dogs: $went"
# A new profile has Arc's 19 site searches, and a word that starts one's host hints it on the first
# row, whatever that row is; Tab enters it, in YouTube's red (#1397). It needs an engine built with
# this core (core/site_searches.cc).
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type yout
expect '^browser: command bar suggestions "yout" in [0-9.]* ms: [^,]* site youtube.com' \
  "the command bar's first row didn't hint YouTube's site search: $(suggestions | tr '\n' ' ')"
await_bar_height
../tools/window-screenshot.sh "$pid" "$out/site-hint-youtube.png" || fail "no window to capture"
send key tab
expect '^browser: command bar site search youtube.com by tab$' \
  "Tab on the row that hints YouTube didn't enter its site search"
expect '^browser: command bar drew ""$' "the command bar didn't draw YouTube's site search"
../tools/window-screenshot.sh "$pid" "$out/site-search-youtube.png" || fail "no window to capture"
# ⌫ puts back what was typed before Tab, "yout" and not YouTube's keyword, and Esc the bar as it
# opened, as Arc's do (#1397). Each is captured once the bar has left, before the expect on what
# it put back, so a build that puts back something else shows it.
send key backspace
expect '^browser: command bar leaves site search$' "⌫ didn't leave YouTube's site search"
expect '^browser: command bar drew "you' "the command bar didn't draw what ⌫ put back"
../tools/window-screenshot.sh "$pid" "$out/site-left-youtube.png" || fail "no window to capture"
expect '^browser: command bar drew "yout"$' \
  "⌫ didn't put back what was typed before Tab: $(grep '^browser: command bar drew' "$log" | tail -1)"
expect '^browser: command bar suggestions "yout" in [0-9.]* ms: [^,]* site youtube.com' \
  "the command bar didn't hint YouTube's site search again: $(suggestions | tr '\n' ' ')"
send key tab
expect '^browser: command bar site search youtube.com by tab$' "Tab didn't enter YouTube's site search again"
send type cats
expect '^browser: command bar drew "cats"$' "the command bar didn't draw cats in YouTube's site search"
send key escape
expect '^browser: command bar leaves site search' "Esc didn't leave YouTube's site search"
expect '^browser: command bar drew "' "the command bar didn't draw what Esc put back"
../tools/window-screenshot.sh "$pid" "$out/site-escaped-youtube.png" || fail "no window to capture"
expect '^browser: command bar leaves site search as it opened$' \
  "Esc didn't leave YouTube's site search for the bar as it opened"
expect '^browser: command bar drew ""$' \
  "Esc didn't put back the bar as ⌘T opened it: $(grep '^browser: command bar drew' "$log" | tail -1)"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"
# ⌥⌘⌫ takes the selected row out too, beside ⇧⌦, as Arc's does: here the search for dogs, typed
# before. Off the Mac it would be Ctrl+Alt+Backspace, which X11 can take to end the session, so
# there ⇧⌦ alone does (command_bar.rs).
if [ "$(uname)" = Darwin ]; then
  remove="backspace alt cmd" removed=⌥⌘⌫
else
  remove="delete shift" removed=⇧⌦
fi
# History has the search only once its page commits, which can come after the next ⌘T. The page's
# title says it has: its first page line comes as it starts. Where DuckDuckGo doesn't answer the
# runner, history never has it (#1875), and the step says it skipped.
answered=
for _ in $(seq 100); do
  logged "^browser: title $searched .*dogs" && answered=1 && break
  sleep 0.1
done
dogs='https://duckduckgo.com/?q=dogs'
if [ -z "$answered" ]; then
  echo "skip: $removed on the search for dogs typed before: DuckDuckGo didn't answer, and history has a search only once its page commits."
else
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type dog
  expect "^browser: command bar suggestions \"dog\" in .*$dogs[^,]* removable" \
    "the command bar didn't suggest the search for dogs typed before: $(suggestions | tr '\n' ' ')"
  for _ in 1 2 3 4 5 6 7 8; do
    send key down
    grep '^browser: command bar selected' "$log" | tail -1 | grep -q " $dogs" && break
  done
  send key $remove
  expect "^browser: command bar removes $dogs" "$removed didn't take the search for dogs out"
  # Chrome says the page left the history, which the bar forgets what it learned of.
  expect '^browser: history deleted [1-9][0-9]* urls from profile [0-9]*$' \
    "Chrome didn't say $removed deleted the search for dogs from the history"
  expect '^browser: command bar suggestions "dog" in ' "the command bar didn't list its rows again after $removed"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
fi
# A sum's result is the row after the first, and ↩ on it copies the result, to a pasteboard of the
# run's own.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type 12+7
expect '^browser: command bar calculates "12+7": 19$' "the command bar didn't work out 12+7"
send key down
expect '^browser: command bar selected = 19$' "↓ didn't select the sum's row"
../tools/window-screenshot.sh "$pid" "$out/sum.png" || fail "no window to capture"
send key return
expect '^browser: command bar copies 19$' "↩ on the sum's row didn't copy its result"
expect '^browser: copied 19$' "the sum's result wasn't copied"
expect '^browser: command bar hidden$' "copying the sum's result didn't close the command bar"
# A right-click in the command bar's field shows its menu, with Paste and Go and Paste and Search.
# The check chooses only Select All: Cut, Copy and the Paste items use the owner's pasteboard.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type 8-3
expect '^browser: command bar calculates "8-3": 5$' "the command bar didn't work out 8-3"
post rightclickkey 200 27
pastes='Paste, Paste and Go, Paste and Search'
expect "^browser: menu shown $(cased "Cut (disabled), Copy (disabled), $pastes, Select All")\$" \
  "a right-click in the command bar's field didn't show its menu"
../tools/window-screenshot.sh "$pid" "$out/field-menu.png" || fail "no window to capture"
send choose "$(cased 'Select All')"
send type 2+2
expect '^browser: command bar calculates "2+2": 4$' \
  "Select All in the field's menu didn't select its text for typing to replace"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar after its menu"
# ⌘↩ opens what was typed in a tab behind, at the top of Today, which loads only once it's first
# shown (Dia), and the bar stays open.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
before=$(today)
send type example.org
send key return cmd
# The bar may go over http, from history, as the https page was taken out of it above.
expect '^browser: command bar behind https\{0,1\}://example.org/$' \
  "⌘↩ didn't open example.org behind"
expect "^browser: today [0-9][0-9]* $before\$" "⌘↩'s tab didn't open at the top of Today"
waiting=$(today | cut -d' ' -f1)
expect "^browser: page $waiting unloaded 1\$" "⌘↩'s tab didn't wait to load"
none_since '^browser: command bar hidden$' || fail "⌘↩ closed the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar after ⌘↩"
grep -q "^browser: loaded $waiting " "$log" && fail "⌘↩'s tab loaded before it was shown"
switch "1 cmd" "$waiting"
expect "^browser: loaded $waiting https\{0,1\}://example.org/\$" "⌘↩'s tab didn't load once shown"

# Back's history, as Arc's. A right-click on back lists the pages back, nearest first, each with its
# favicon, and a row picked goes there; held for 0.5 s it lists them too, and letting go doesn't go
# back; ⌘-clicked it opens the page back in a tab under, behind, which loads once shown, and the tab
# shown stays. On Linux the driver sends cmd as Ctrl, so that's the Ctrl-click. From two pages, each
# with its own favicon, whose links, over all of them, go on in the same tab: back to next, and next
# to a text file. Each page's title is its name once it has drawn, and the name its row shows.
# Held, back shrinks a little and grows back as its menu opens; reload pressed shrinks too, and let
# go off it grows back without reloading, as a button does.
serve
back_button='138 23.5'
reload_button='206 23.5'
printf '%s' iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFUlEQVR42mNgcPhPGhrVMKph+GoAAJiZPxAbf0wNAAAAAElFTkSuQmCC |
  base64 -d > "$out/site/next.png"
for page in back:next.html:icon.png next:notes.txt:next.png; do
  name=${page%%:*} rest_of=${page#*:}
  printf '%s' "<title>loading</title><link rel=\"icon\" href=\"${rest_of#*:}\">
<a href=\"${rest_of%%:*}\" style=\"position:fixed;inset:0\">$name</a>
<script>requestAnimationFrame(() => setTimeout(() => document.title = \"$name\"));</script>" \
    > "$out/site/$name.html"
done
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
rest=$(today)
send type "${site#http://}back.html"
send key return
expect "^browser: today [0-9][0-9]* $rest\$" "the back page's tab didn't open at the top of Today"
h=$(today | cut -d' ' -f1)
# A page has drawn, so takes clicks, once its title is its name, and its favicon is in the profile's
# favicons once the tab has it.
page_drawn() {
  expect "^browser: title $h $1\$" "the $1 page didn't draw"
  expect "^browser: favicon $h [0-9]* bytes\$" "the $1 page's tab didn't get its favicon"
}
# A click on the page's link, which goes on to $1 in the same tab.
go_on() {
  send click $page_point
  expect "^browser: page $h back 1 forward 0 loading 0 ${site}$1\$" \
    "a click on the page's link didn't go on to $1 in its tab"
  # A click hits the buttons the window drew last, where back was dimmed as the page before had
  # nowhere to go back to. Off the Mac that frame can come after the driver's click (#1048, #1179).
  for _ in $(seq 100); do
    [ "$(last 'nav back drawn')" = on ] && return
    sleep 0.1
  done
  fail "the header didn't draw back on once the page had gone on to $1"
}
page_drawn back
go_on next.html
page_drawn next
go_on notes.txt
post rightclick $back_button
expect "^browser: history $h back 2\$" "a right-click on back didn't ask for the pages back"
expect '^browser: menu drawn next (favicon), back (favicon)$' \
  "a right-click on back didn't draw the two pages back, nearest first, with their favicons: $(grep '^browser: menu ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/back-menu.png" || fail "no window to capture"
send choose back
expect '^browser: command history -2 via sidebar$' "choosing the back page's row didn't reach the UI"
expect "^browser: page $h back 0 forward 1 loading 0 ${site}back.html\$" "choosing the back page's row didn't go there"
# The page comes back from the back-forward cache, which logs no title or load again.
expect "^browser: favicon $h [0-9]* bytes\$" "the back page's tab didn't get its favicon"
go_on next.html
page_drawn next
send press $back_button
expect '^browser: nav back pressed drawn$' "back held didn't draw itself shrunk"
expect "^browser: history $h back 1\$" "holding back didn't ask for the pages back"
expect '^browser: menu drawn back (favicon)$' \
  "holding back didn't draw the back page's row: $(grep '^browser: menu ' "$log" | tail -1)"
expect '^browser: nav back at rest drawn$' "back held didn't grow back as its menu opened"
../tools/window-screenshot.sh "$pid" "$out/back-held.png" || fail "no window to capture"
let_go=$((step + 1))
send release $back_button
send choose back
expect '^browser: command history -1 via sidebar$' "choosing the held menu's row didn't reach the UI"
none_since '^browser: command back via sidebar$' "$let_go" || fail "letting go of back held went back too"
expect "^browser: page $h back 0 forward 1 loading 0 ${site}back.html\$" "choosing the held menu's row didn't go back"
expect "^browser: favicon $h [0-9]* bytes\$" "the back page's tab didn't get its favicon"
# A press hits the button drawn last, which is stop until the window draws the loaded page.
# Off the Mac that frame can come after the driver's press (#1048).
if [ "$(uname)" != Darwin ]; then
  for _ in $(seq 100); do
    [ "$(last 'nav shows')" = reload ] && break
    sleep 0.1
  done
  [ "$(last 'nav shows')" = reload ] || fail "the header didn't draw reload once the page loaded"
fi
send press $reload_button
expect '^browser: nav reload pressed drawn$' "reload pressed didn't draw itself shrunk"
../tools/window-screenshot.sh "$pid" "$out/reload-pressed.png" || fail "no window to capture"
# Let go on the URL field's left end, off reload.
send release 40 64
expect '^browser: nav reload at rest drawn$' "reload let go didn't grow back"
none_since '^browser: command reload via sidebar$' || fail "reload let go off it reloaded"
go_on next.html
page_drawn next
send click $back_button cmd
expect '^browser: command back in the background via sidebar$' "⌘-click on back didn't reach the UI"
expect "^browser: today $h [0-9][0-9]* $rest\$" "⌘-click on back didn't add a row under the tab"
behind=$(today | cut -d' ' -f2)
expect "^browser: page $behind unloaded 1\$" "⌘-click on back's tab didn't wait to load"
grep '^browser: selected' "$log" | tail -1 | grep -q "^browser: selected $h\( in .*\)\{0,1\}\$" ||
  fail "⌘-click on back showed another tab"
grep "^browser: page $h " "$log" | tail -1 | grep -q " ${site}next.html\$" ||
  fail "⌘-click on back went back in the tab shown"
../tools/window-screenshot.sh "$pid" "$out/back-behind.png" || fail "no window to capture"
send click $(row 1)
expect "^browser: selected $behind in " "a click on ⌘-click on back's row didn't show its tab"
expect "^browser: loaded $behind ${site}back.html\$" "⌘-click on back's tab didn't load the page back once shown"
send key w cmd
expect_tabs "$h $rest" "$h" "closing ⌘-click on back's tab didn't go back to its opener"
send key w cmd
expect "^browser: today $rest\$" "⌘W didn't close the back page's tab"

# Which tab closing one goes back to (issue #442), from a page whose link, over all of it, opens a
# tab in front, and with ⌘ behind. Last, as it pins a tab, which moves Today's rows down.
serve
printf '%s' '<title>opener</title>
<a target=_blank href="notes.txt" style="position:fixed;inset:0">opener</a>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));</script>' > "$out/site/opener.html"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
before=$(today)
send type "${site#http://}opener.html"
send key return
expect "^browser: today [0-9][0-9]* $before\$" \
  "the opener page's tab didn't open at the top of Today"
o=$(today | cut -d' ' -f1)
# Chromium drops a click on a page that hasn't drawn yet.
expect "^browser: title $o ready\$" "the opener page didn't draw"
# The tab a page opened last.
opened() {
  grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3
}
# Two tabs opened behind, shown one after the other, keep their opener: closing the second goes
# to the first, its sibling, and closing that to the opener.
send click $page_point cmd
expect "^browser: tab [0-9]* opened by $o in the background\$" \
  "a ⌘-click on the link didn't open a tab"
b1=$(opened)
send click $page_point cmd
expect "^browser: tab [0-9]* opened by $o in the background\$" "a second ⌘-click didn't open a tab"
b2=$(opened)
expect "^browser: today $o $b1 $b2 $before\$" \
  "the tabs opened behind didn't line up under their opener"
send click $(row 1)
expect "^browser: selected $b1 in " "a click on the first tab opened behind didn't show it"
send click $(row 2)
expect "^browser: selected $b2 in " "a click on the second tab opened behind didn't show it"
send key w cmd
expect_tabs "$o $b1 $before" "$b1" "closing the second tab opened behind didn't go to its sibling"
send key w cmd
expect_tabs "$o $before" "$o" "closing the first tab opened behind didn't go to its opener"
# A switch to a tab unrelated to the one shown forgets where tabs came from: back from there,
# closing the tab the link opened goes to the tab below, not its opener.
send click $page_point
expect "^browser: tab [0-9]* opened by $o in the foreground\$" \
  "a click on the link didn't open a tab"
f=$(opened)
expect_tabs "$o $f $before" "$f" "the link's tab didn't open in front under its opener"
u=$(echo "$before" | cut -d' ' -f1)
send click $(row 2)
expect "^browser: selected $u in " "a click on the tab below didn't show it"
send click $(row 1)
expect "^browser: selected $f in " "a click on the link's tab didn't show it again"
send key w cmd
expect_tabs "$o $before" "$u" \
  "after a switch away and back, closing the link's tab went to its opener"
# A pinned tab's link opens a tab in front twice, the pinned tab shown in between: closing the
# second goes back to the pinned tab, not to the first, as a tab opened in front forgets the
# openers before it.
send click $(row 0)
expect "^browser: selected $o in " "a click on the opener page's row didn't show it"
send key d cmd
expect_lists "$o" "$before" "after ⌘D on the opener page's tab"
send click $page_point
expect "^browser: tab [0-9]* opened by $o in the foreground\$" \
  "a click on the pinned tab's link didn't open a tab"
x=$(opened)
expect_tabs "$x $before" "$x" "the pinned tab's link didn't open its tab at the top of Today"
send click $(pinned_row 0)
expect "^browser: selected $o in " "a click on the pinned tab didn't show it"
send click $page_point
expect "^browser: tab [0-9]* opened by $o in the foreground\$" \
  "a second click on the pinned tab's link didn't open a tab"
y=$(opened)
expect_tabs "$y $x $before" "$y" "the pinned tab's second link didn't open its tab at the top"
send key w cmd
expect_tabs "$x $before" "$o" \
  "closing the pinned tab's newest link didn't go back to the pinned tab"
# Tabs › Reset all tabs in this Space takes the Space's pinned tabs away from their pinned URLs back
# there, as Arc's does, but only those loaded: one unloaded stays as it is until it's shown. Edit…
# gives two pinned tabs a pinned URL their pages aren't at, and ⌘W unloads the second.
printf '<title>reset</title>' > "$out/site/reset.html"
# Pinned URL › Edit… on pinned row $1, tab $2, takes $3 typed over its pinned URL.
edit_pinned_url() {
  post rightclick $(pinned_row "$1")
  expect "^browser: menu shown $(cased 'Unpin Tab, ')" "a right-click on pinned row $1 didn't show its menu"
  send choose "$(cased 'Pinned URL › Edit…')"
  expect "^browser: edit pinned url $2\$" "Edit… didn't show tab $2's pinned URL in a field"
  send type "$3"
  send key return
  expect "^browser: page $2 away 1\$" "the pinned URL typed didn't take tab $2 away from it"
}
set -- $(row 0)
send click $1 $(($2 + 41))
expect "^browser: selected $x in " "a click on Today's top row didn't show its tab"
send key d cmd
expect_lists "$o $x" "$before" "after ⌘D on the link's tab"
expect '^browser: rows settled$' "the rows didn't settle after ⌘D"
edit_pinned_url 0 "$o" "${site}reset.html"
edit_pinned_url 1 "$x" "${site}reset.html"
send click $page_point
send key w cmd
expect "^browser: page $x unloaded 1\$" "⌘W didn't unload the second pinned tab"
send menu Reset all tabs in this Space
expect '^browser: command reset-space-tabs via ui$' "Tabs › Reset all tabs in this Space didn't run"
expect "^browser: loaded $o ${site}reset.html\$" "Reset all tabs didn't take the loaded pinned tab back to its pinned URL"
expect "^browser: page $o away 0\$" "the pinned tab Reset took back still read as away"
none_since "^browser: loaded $x " || fail "Reset all tabs loaded the unloaded pinned tab"
none_since "^browser: page $x \(away 0\|unloaded 0\)\$" || fail "Reset all tabs took the unloaded pinned tab back"
# Site Search Settings, from the bar, opens Chromium's search engines page in a new tab, where the
# profile's site searches are added and edited.
bar_command site-search-settings "Site Search Settings" "site search"
expect '^browser: loaded [0-9]* chrome://settings/searchEngines$' \
  "Site Search Settings didn't open Chromium's search engines page: $(grep '^browser: loaded' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/site-search-settings.png" || fail "no window to capture"
# Among equal matches the bar lists first the command picked more and lately, per profile (ux-spec.md
# › Command bar › What the bar learns): "view" finds View Archive… above View Media… until View
# Media… is picked, all three opening the Library.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type view
expect '^browser: command bar commands "view": .*View Archive…, .*View Media…' \
  "\"view\" didn't list View Archive… above View Media… before a pick: $(grep '^browser: command bar commands "view"' "$log" | tail -1)"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"
bar_command view-media "View Media…" "view media"
expect '^browser: library shown$' "View Media… didn't open the Library"
send key l cmd shift
expect '^browser: library hidden$' "⇧⌘L didn't close the Library"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type view
expect '^browser: command bar commands "view": .*View Media…, .*View Archive…' \
  "a pick didn't put View Media… above View Archive…: $(grep '^browser: command bar commands "view"' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/bar-picked.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"
# ⌘-click and ⇧-click choose several rows, as Arc's do, drawn with the secondary fill. Their menu
# and ⇧⌘C copy their links, one a line; ⌘D pins them and New Folder folders them, each one step
# for ⌘Z; ⌘W closes them all, and ⌘Z brings them back. Three pages of the site's, opened from ⌘T.
for name in one two three; do printf '<title>%s</title>' "$name" > "$out/site/$name.html"; done
# Opens the site's page $1 from ⌘T at the top of Today, as tab $opened.
open_site() {
  was=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
  send type "$site$1.html"
  send key return
  expect "^browser: today [0-9][0-9]*${was:+ $was}\$" "$1 didn't open at the top of Today"
  opened=$(today | cut -d' ' -f1)
  expect "^browser: loaded $opened $site$1.html\$" "$1 didn't load in its tab"
}
# The centre of Today's row $1, under the pinned tabs.
today_row() {
  set -- $(row "$1")
  echo "$1 $(($2 + 41 * $(echo $(pinned) | wc -w)))"
}
# The links copied last, from the line logged and the two after it.
copied() {
  grep -A2 '^browser: copied ' "$log" | tail -3 | sed 's/^browser: copied //'
}
open_site one
a=$opened
open_site two
b=$opened
open_site three
c=$opened
rest=$(today | cut -d' ' -f4-)
shown_pinned=$(pinned)
send click $(today_row 2) cmd
expect "^browser: chosen $c $a\$" "⌘-click on a row didn't choose it with the tab shown"
send click $(today_row 1) shift
expect "^browser: chosen $c $b $a\$" "⇧-click didn't choose the rows from the one ⌘-clicked through it"
none_since "^browser: selected " || fail "choosing rows showed another tab"
../tools/window-screenshot.sh "$pid" "$out/chosen.png" || fail "no window to capture"
post rightclick $(today_row 1)
expect "^browser: menu shown $(cased 'Pin Tabs, Add to Folder › New Folder')\(, [^,]*\)*, $(cased 'Copy URLs, Close Tabs')\$" \
  "a right-click on a chosen row didn't show their menu: $(grep '^browser: menu shown' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/chosen-menu.png" || fail "no window to capture"
links=$(printf '%sthree.html\n%stwo.html\n%sone.html' "$site" "$site" "$site")
send choose "$(cased 'Copy URLs')"
expect "^browser: copied ${site}three.html\$" "Copy URLs didn't copy the chosen rows' links"
[ "$(copied)" = "$links" ] || fail "Copy URLs copied \"$(copied)\", not the three links in order"
clipboard_holds "$links" || fail "Copy URLs left the clipboard without the three links"
send key c cmd shift
expect "^browser: copied ${site}three.html\$" "⇧⌘C with rows chosen didn't copy their links"
[ "$(copied)" = "$links" ] || fail "⇧⌘C copied \"$(copied)\", not the chosen rows' links"
send key d cmd
expect_lists "$shown_pinned $c $b $a" "$rest" "⌘D with rows chosen"
send key z cmd
expect_lists "$shown_pinned" "$c $b $a $rest" "⌘Z after ⌘D on the chosen rows"
expect '^browser: rows settled$' "the rows didn't settle after ⌘Z"
# A plain click lets them go, and ⌘-clicks choose them again.
send click $(today_row 0)
expect '^browser: chosen $' "a plain click on a row didn't let the chosen rows go"
send click $(today_row 1) cmd
send click $(today_row 2) cmd
expect "^browser: chosen $c $b $a\$" "⌘-clicks didn't choose the rows again"
post rightclick $(today_row 0)
expect "^browser: menu shown $(cased 'Pin Tabs, ')" "a right-click on a chosen row didn't show their menu"
send choose "$(cased 'Add to Folder › New Folder')"
expect '^browser: rename folder [0-9]*$' "New Folder didn't give the chosen rows' folder its name field"
f=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: outline .*$f+\[$c $b $a\]\$" "New Folder didn't pin the chosen rows into it in order"
expect "^browser: today $rest\$" "New Folder left a chosen row in Today: $(today)"
send key escape
send key z cmd
expect_lists "$shown_pinned" "$c $b $a $rest" "⌘Z after New Folder on the chosen rows"
expect '^browser: rows settled$' "the rows didn't settle after ⌘Z"
send click $(today_row 1) cmd
send click $(today_row 2) cmd
expect "^browser: chosen $c $b $a\$" "⌘-clicks didn't choose the rows once more"
send key w cmd
expect "^browser: today $rest\$" "⌘W with rows chosen didn't close them all: $(today)"
send key z cmd
reopened=$step
expect "^browser: today [0-9][0-9]* [0-9][0-9]* [0-9][0-9]* $rest\$" \
  "⌘Z after ⌘W didn't bring the three chosen tabs back: $(today)"

# On Windows a window the UI floats over the page is on screen only while the UI shows it, in
# whatever order keys, clicks, focus and minimizing come (#1346). Windows shows the windows a
# browser window owns again as it comes back from minimized, and it brought back the bar, a tooltip
# and a menu the UI had closed, empty and taking the page's clicks. The driver lists what's over
# the browser window by each window's own visibility, and its reshow shows the UI's hidden windows
# again as Windows does.
if windows; then
  # What's over the page is none, once a fade out has ended.
  overlays_none() {
    for _ in $(seq 20); do
      send overlays
      shown=$(grep '^browser: overlays shown ' "$log" | tail -1 | cut -d' ' -f4-)
      [ "$shown" = none ] && return 0
      sleep 0.1
    done
    fail "$1 left windows over the page: $shown"
  }
  # The bar's window goes with the bar, and the plate for New Tab with it.
  bar_gone() {
    expect '^browser: command bar hidden$' "$1 left the command bar open"
    expect '^browser: new tab plate 0$' "$1 left New Tab's plate selected"
    overlays_none "$1"
  }
  # Where each window over the page is, "x,y" in points, in the driver's order, once there's one.
  overlays_at() {
    for _ in $(seq 20); do
      send overlays
      at=$(grep '^browser: overlays shown ' "$log" | tail -1 | grep -o ' at [0-9-]*,[0-9-]*' |
        cut -c5- | tr '\n' ' ')
      [ -n "$at" ] && return 0
      sleep 0.1
    done
    fail "$1"
  }
  # The toast from ⌘Z and the hover card under the pointer go by themselves, once the pointer is
  # over the page. The driver's pointer is real (#2544), so it rests where a menu was opened, on its
  # row, which shows the row's hover card once the menu closes, as it does to a hand.
  settle() {
    away
    for _ in $(seq 100); do
      toast=$(grep '^browser: toast \(shown\|hidden\)' "$log" | tail -1)
      card=$(grep '^browser: hover card \(shown\|hidden\)' "$log" | tail -1)
      case "$toast $card" in *shown*) sleep 0.1 ;; *) break ;; esac
    done
  }
  settle
  overlays_none "the steps before"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send key escape
  bar_gone "Esc right after ⌘T"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send key t cmd
  bar_gone "⌘T twice"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send click $page_point
  bar_gone "a click on the page after ⌘T"
  send move 93 23
  expect '^browser: tooltip shown Hide Sidebar Ctrl+S$' "resting on the sidebar button didn't show its tooltip"
  expect '^browser: tooltip window shown$' "the sidebar button's tooltip didn't fade in"
  away
  expect '^browser: tooltip hidden$' "the pointer leaving the sidebar button didn't hide its tooltip"
  overlays_none "the tooltip, once hidden,"
  post rightclick $(row 0)
  expect '^browser: menu window at ' "a right-click on the top Today row didn't open its menu's window"
  send key escape
  expect '^browser: menu closed$' "escape didn't close the menu"
  expect '^browser: menu window hidden$' "escape left the menu's window up"
  settle
  overlays_none "the menu, once closed,"
  send reshow
  expect '^browser: reshown [1-9][0-9]*$' "the driver found none of the UI's windows hidden to show again"
  overlays_none "Windows showing the bar's, the tooltip's and the menu's windows again"
  # Minimized, the window closes its bar and its menu, as the user leaving them, and they stay
  # closed as it comes back.
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send minimize
  bar_gone "minimizing the window"
  send minimize
  overlays_none "the window coming back from minimized after its bar closed"
  send state
  if [ "$active" = 1 ]; then
    post rightclick $(row 0)
    expect '^browser: menu window at ' "a right-click on the top Today row didn't open its menu's window"
    send minimize
    expect '^browser: menu closed$' "minimizing the window left its menu open"
    expect '^browser: menu window hidden$' "minimizing the window left its menu's window up"
    send minimize
    settle
    overlays_none "the window coming back from minimized after its menu closed"
  else
    echo "skip: a menu in a minimized window, as the app wasn't active once its window came back."
  fi
  # Windows' own Win+D and back with the bar open: the bar closes with the window, and once the
  # window is back the keyboard is the window's, not the hidden bar's, where what the user typed
  # went unseen and Ctrl+T closed the bar again (#2545). Real keys, which need the desktop to be
  # the check's own.
  if [ -n "${CI:-}" ] && [ "$active" = 1 ]; then
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send realkey d win
    bar_gone "Win+D"
    send realkey d win
    expect '^browser: command bar hidden, its browser window takes the keyboard$' \
      "the window back from Win+D left its hidden command bar with the keyboard"
    send realkey t ctrl
    expect '^browser: command bar shown$' "Ctrl+T after Win+D and back didn't open the command bar"
    overlays_at "the command bar after Win+D isn't on screen"
    send realkey g
    expect '^browser: command bar drew "g"$' "what's typed after Win+D didn't go into the command bar"
    ../tools/window-screenshot.sh "$pid" "$out/command-bar-after-win-d.png" || fail "no window to capture"
    send key escape
    bar_gone "Esc in the command bar after Win+D"
  else
    echo "skip: Win+D with the command bar open, as real keys need CI's desktop and the app active."
  fi
  # Moved with the bar open, the window takes the bar along, over the same place, where it stayed
  # behind on the screen, and the window's menu closes, as the Mac's do (#1364).
  send order
  frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
  read -r frame_x frame_y frame_width frame_height <<FRAME
$frame
FRAME
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  overlays_at "the command bar's window isn't on screen"
  bar_before=$at
  hidden=$(grep -c '^browser: command bar hidden$' "$log" || true)
  send place $((frame_x + 60)) $((frame_y + 40)) "$frame_width" "$frame_height"
  overlays_at "the command bar's window left the screen as its window moved"
  ../tools/window-screenshot.sh "$pid" "$out/command-bar-moved.png" || fail "no window to capture"
  # Within a point, as each is rounded from pixels.
  echo "$bar_before" "$at" | awk '{ n = NF / 2; for (i = 1; i <= n; i++) {
      split($i, a, ","); split($(i + n), b, ",")
      if ((b[1] - a[1] - 60) ^ 2 > 1 || (b[2] - a[2] - 40) ^ 2 > 1) exit 1 } }' ||
    fail "moving the window by 60,40 points left the command bar behind: $bar_before, then $at"
  [ "$(grep -c '^browser: command bar hidden$' "$log")" = "$hidden" ] ||
    fail "moving the window closed its command bar"
  send place "$frame_x" "$frame_y" "$frame_width" "$frame_height"
  send key escape
  bar_gone "Esc in the command bar after its window moved"
  post rightclick $(row 0)
  expect '^browser: menu window at ' "a right-click on the top Today row didn't open its menu's window"
  send place $((frame_x + 60)) $((frame_y + 40)) "$frame_width" "$frame_height"
  expect '^browser: menu closed$' "moving the window left its menu open"
  expect '^browser: menu window hidden$' "moving the window left its menu's window up"
  send place "$frame_x" "$frame_y" "$frame_width" "$frame_height"
  settle
  overlays_none "the menu, closed as its window moved,"
  # The bar opened while a page loads in another site's renderer, so its view replaces the one
  # that had the keyboard as it commits: the bar stays open with the keyboard, as it closes only
  # when the user leaves it, where it closed as Chromium made its window the active one (#1363).
  rm -f "$out/site/held.go"
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "${site}notes.txt"
  send key return
  expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the tab for notes.txt didn't open at the top"
  committing=$(today | cut -d' ' -f1)
  expect "^browser: loaded $committing ${site}notes.txt\$" "notes.txt didn't load in its tab"
  go_from_bar "http://localhost:$port/held"
  served "localhost:$port/held" "the tab didn't ask for /held on localhost"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar over the loading page"
  hidden=$(grep -c '^browser: command bar hidden$' "$log" || true)
  touch "$out/site/held.go"
  expect "^browser: loaded $committing http://localhost:$port/held\$" "/held didn't load in its tab"
  # Twice round the UI thread, for what the commit posted to it to have run.
  send state
  send state
  [ "$(grep -c '^browser: command bar hidden$' "$log")" = "$hidden" ] ||
    fail "the command bar closed as the page under it committed"
  [ "$key" = GPUIPanel ] || fail "the command bar hasn't the keyboard after the page committed: $key"
  if grep -q '^browser: command bar takes the keyboard back from its window$' "$log"; then
    committed="took the keyboard back from its window"
  else
    committed="kept the keyboard"
  fi
  send key escape
  bar_gone "Esc in the command bar after the page under it committed"
  send key w cmd
  expect "^browser: tab $committing closed\$" "⌘W didn't close the tab that loaded /held"
  # Alt+F4 in the bar closes the browser window under it, as in Chrome's address field, and the
  # bar's window, which every browser window shares, lives on to open over the next one: Ctrl+T
  # there opens the bar again (#1362). The windows over the page that have no Alt+F4 of the UI's,
  # such as the dialog, close themselves on it (dialogs.sh).
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
  second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  new_window_bar
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar in the second window"
  post key f4 ctrl
  await "^browser: drive $step done" || fail "the driver didn't finish Alt+F4 in the command bar"
  expect '^browser: command close-window via ui$' "Alt+F4 in the command bar didn't reach the UI"
  expect "^browser: window closed $second\$" "Alt+F4 in the command bar didn't close its window"
  expect '^browser: command bar hidden$' "the command bar stayed open as its window closed"
  send window 1
  send key t cmd
  expect '^browser: command bar shown$' "Ctrl+T didn't open the command bar after Alt+F4 closed a window"
  send key escape
  bar_gone "Esc in the command bar after Alt+F4 closed the window it was over"
  # The display adapter restarting, as a driver update or a reset does, takes GPUI's device away.
  # With the bar open and the app idle, the bar's window stayed on screen with nothing drawn in it,
  # the page reading through its frame and shadow, and the sidebar black, until the next input asked
  # for a frame (#2536). The device's removal now wakes GPUI to draw its windows again on a new one.
  # The bar's view isn't rendered again for it, so its "drew" line doesn't say. pnputil needs an
  # administrator, as CI's runner is.
  send key t cmd
  expect '^browser: command bar shown$' "Ctrl+T didn't open the command bar"
  restarted=$(powershell.exe -NoProfile -Command \
    'Get-PnpDevice -Class Display -Status OK | ForEach-Object { pnputil /restart-device $_.InstanceId }' \
    2>&1 || true)
  case "$restarted" in
    *"restarted successfully"*)
      # A driver command that draws nothing, for what's logged after the restart.
      send overlays
      expect '^browser: gpui redrew its windows on a new device$' \
        "the windows drew nothing after the display adapter restarted under the bar, with no input"
      overlays_at "the command bar's window left the screen as the display adapter restarted"
      ../tools/window-screenshot.sh "$pid" "$out/bar-adapter-restarted.png" all ||
        fail "no window to capture"
      adapter=", and the open bar draws again after the display adapter restarts under it"
      ;;
    *) echo "skip: the display adapter restarting under the bar, as pnputil couldn't: $restarted" ;;
  esac
  send key escape
  bar_gone "Esc in the command bar after the display adapter restarted"
fi

# Showing the desktop with the command bar open (#2711). On X11 the bar is a window of its own that
# no window manager takes, so it has to leave the screen as its window does: openbox hides every
# window, and the bar goes with the browser window and comes back with it, with the keys. KWin
# raises the desktop shell's window over the others and gives it the keys, and the bar closes as
# it does for any window that takes them; without a shell KWin shows nothing, so the driver maps
# one. EWMH's _NET_SHOWING_DESKTOP asks either, as openbox's Super+D, KWin's Ctrl+F12 and a
# pager's button do. sway has no desktop to show and hides a window on another workspace: under
# Wayland the bar is part of the window's surface and goes with it, and closes as the app goes
# inactive, as on leaving it for another app's window. sway's Xwayland, where the app is only when
# X11 is forced in a Wayland session, draws the bar on every workspace and tells no X client its
# window left the screen (#2720), so the step doesn't run there.
if [ "$(uname)" = Linux ] && { [ -n "${WAYLAND_DISPLAY:-}" ] || [ "${BROWSER_HEADLESS:-xvfb}" = xvfb ]; }; then
  # The share, in thousandths, of the capture's most common colour: the empty screen's, with the
  # pointer the only other thing on it.
  plain() {
    ../tools/window-screenshot.sh "$pid" "$1" all || fail "no screen to capture"
    convert "$1" -format %c histogram:info:- |
      awk -v all="$(identify -format '%[fx:w*h]' "$1")" \
        '{ n = $1 + 0; if (n > top) top = n } END { printf "%d\n", top * 1000 / all }'
  }
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    opened=$step
    swaymsg workspace 9 > /dev/null || fail "sway didn't go to another workspace"
    for _ in $(seq 50); do
      [ "$(plain "$out/workspace-away.png")" -ge 995 ] && break
      sleep 0.1
    done
    [ "$(plain "$out/workspace-away.png")" -ge 995 ] ||
      fail "another workspace still showed the bar or its window: $out/workspace-away.png"
    expect '^browser: command bar hidden$' "the bar stayed open with its window off screen" "$opened"
    expect '^browser: app active 0$' "the app stayed active with its window off screen" "$opened"
    active=$(grep -c '^browser: app active 1$' "$log")
    swaymsg workspace 1 > /dev/null || fail "sway didn't go back to the window's workspace"
    for _ in $(seq 100); do
      [ "$(grep -c '^browser: app active 1$' "$log")" -gt "$active" ] && break
      sleep 0.1
    done
    [ "$(grep -c '^browser: app active 1$' "$log")" -gt "$active" ] ||
      fail "the app didn't log \"browser: app active 1\" in 10 s back on its window's workspace"
    ../tools/window-screenshot.sh "$pid" "$out/workspace-back.png" all || fail "no screen to capture"
    desktop="on another workspace the window and its bar are off screen, and the bar closes as the app does on leaving it"
  else
    # What the X server shows, as "window <0|1>, overlays <on screen> of <shown>, keyboard <whose>".
    onscreen() {
      for _ in $(seq 50); do
        send onscreen
        seen=$(grep '^browser: onscreen ' "$log" | tail -1)
        printf '%s\n' "$seen" | grep -q "^browser: onscreen $1\$" && return 0
        sleep 0.1
      done
      fail "$2: $seen"
    }
    send desktop shell
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    onscreen 'window 1, overlays \([1-9][0-9]*\) of \1, keyboard overlay' \
      "the bar didn't show over its window with the keys"
    send desktop on
    shown=$step
    onscreen 'window [01], overlays 0 of [0-9][0-9]*, keyboard none' \
      "showing the desktop left the bar on screen, or the keys with the app"
    [ "$(plain "$out/desktop-shown.png")" -ge 995 ] ||
      fail "the desktop showed more than itself: $out/desktop-shown.png"
    send desktop off
    onscreen 'window 1, overlays \(\([1-9][0-9]*\) of \2, keyboard overlay\|0 of 0, keyboard window\)' \
      "the window didn't come back with the keys once the windows showed again"
    if [ "${seen##*keyboard }" = overlay ]; then
      send type g
      expect '^browser: command bar drew "g"$' \
        "a key typed once the windows showed again didn't reach the bar"
      ../tools/window-screenshot.sh "$pid" "$out/desktop-back.png" all || fail "no screen to capture"
      send key escape
      expect '^browser: command bar hidden$' "Esc didn't close the bar once its window was back"
      desktop="showing the desktop takes the window and its bar off screen, and back with the keys"
    else
      expect '^browser: command bar hidden$' "the bar was off screen but not closed" "$shown"
      ../tools/window-screenshot.sh "$pid" "$out/desktop-back.png" all || fail "no screen to capture"
      desktop="showing the desktop closes the bar as the desktop takes the keys, and the window comes back with them"
    fi
    send desktop shell off
  fi
elif [ "$(uname)" = Linux ]; then
  echo "skip: another workspace with the command bar open, on sway's Xwayland: sway draws the bar on every workspace and tells no X client its window left the screen; the app is there only when X11 is forced (#2720)."
fi

# Arc's bar commands that call what the menus do: Mute All Tabs mutes every tab, and Unmute All
# Tabs, listed while one is muted, unmutes them; Enter Full Screen puts the window full screen and
# reads Exit Full Screen there, which takes it out; and there Exit, View › Exit's, with ⎋'s key,
# takes it out too.
# The ⌘Z above gave the tab shown a fresh page, and a key typed into a page whose renderer is still
# starting is lost (#1519), which the Wayland pass hits. There ⌘L waits for the page to load. #1974
# fixes the race in the core and removes this.
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: ⌘L in the page ⌘Z brought back before it has loaded: the key is lost while its renderer starts (#1519)."
  reopened_tab=$(sed -n "/^browser: drive $reopened: /,\$p" "$log" | grep '^browser: tab [0-9]* has its page$' | tail -1 | cut -d' ' -f3)
  expect "^browser: loaded ${reopened_tab:-[0-9]*} " "the tab ⌘Z brought back didn't load" "$reopened"
fi
send click $page_point
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar for Mute All Tabs"
send type 'all tabs'
expect '^browser: command bar commands "all tabs": Mute All Tabs' "the command bar didn't find Mute All Tabs first"
../tools/window-screenshot.sh "$pid" "$out/bar-mute-all.png" || fail "no window to capture"
send key return
expect '^browser: command mute-all-tabs via command bar$' "Mute All Tabs didn't run from the command bar"
expect '^browser: page [0-9]* audible [01] muted 1$' "Mute All Tabs muted no tab"
bar_command unmute-all-tabs "Unmute All Tabs" unmute
expect '^browser: page [0-9]* audible [01] muted 0$' "Unmute All Tabs unmuted no tab"
# A window that already fills its screen goes full screen at the size it has, as a tiled one does
# under sway or Hyprland, so the UI can't wait for a resize to hear it (#1864). The headless sway
# session's window fills its output already; under Xvfb, openbox's tiled case is a window
# maximized to the screen, so there it's maximized first.
# The double-click is the window manager's to answer with the real pointer, which is #2721's
# slice 2, so for now the window goes full screen from its own size there.
if [ "${BROWSER_HEADLESS:-}" = xvfb ]; then
  echo "skip: a double-click on the header maximizing the window first, under Xvfb: #2721 slice 2."
fi
bar_command toggle-full-screen "Enter Full Screen" "enter full"
expect '^browser: window entered full screen$' "Enter Full Screen didn't put the window full screen"
bar_command toggle-full-screen "Exit Full Screen" "exit full"
expect '^browser: window left full screen$' "Exit Full Screen didn't take the window out"
bar_command toggle-full-screen "Enter Full Screen" "enter full"
expect '^browser: window entered full screen$' "Enter Full Screen didn't put the window full screen again"
bar_command exit Exit escape
expect '^browser: window left full screen$' "Exit didn't take the window out of full screen"

exec 3>&-
# Chrome writes the trace before it quits, once every process has started tracing or 10 s have
# passed, so a renderer that closed early can hold the quit that long.
quit 30

app_ms=$(sed -n 's/^browser: selected [0-9]* in \([0-9.]*\) ms$/\1/p' "$log" | summary)
[ -s "$trace" ] || fail "the app didn't write its trace"
# Without the format switch, as when Linux's process title garbled the app's switches (#605),
# Chrome writes Perfetto's proto, its default.
head -c 1 "$trace" | grep -q '[[{]' ||
  fail "the trace isn't JSON, so Chrome didn't get --enable-tracing-format=json"
spans=$(python3 -I check/tab_switches.py "$trace")
chrome_ms=$(echo "$spans" | grep -v incomplete | summary)
# With more than latency, a trace whose buffer ran out may keep none, and the product's not at fault.
if [ "$trace_categories" = latency ]; then
  [ -n "$chrome_ms" ] || fail "the trace has no finished tab switches"
elif [ -z "$(echo "$spans" | grep -v incomplete | grep .)" ]; then
  chrome_ms=none
fi
# grep -c finds none in a run where no switch was cut off, and exits 1 then.
chrome_ms="$chrome_ms, $(echo "$spans" | grep -c incomplete || true) incomplete, not counted"
# With more than latency, the trace keeps the run's last part, where its buffer runs out, so the
# switches before are left out of it.
[ "$trace_categories" = latency ] || chrome_ms="$chrome_ms; with $trace_categories, $(echo "$spans" |
  grep -c . || true) of the $(grep -c '^browser: selected [0-9]* in ' "$log" || true) switches have a span"
# A tab that let go of its page, unseen 2 hours, gets its page back once shown with its
# sessionStorage, where a site keeps its sign-in, as Chrome's discarded tabs keep theirs (#1934).
# Its cookies are its profile's, so only the storage could go.
log="$out/tabs-sign-in.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}session-sign-in.html?tabs" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
signed=$(today)
await "^browser: title $signed sign-in cookie none storage none, drawn\$" ||
  fail "the session sign-in page didn't draw, or found a sign-in before it signed in"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: an unloaded tab's sessionStorage. Browser wasn't the active app, so the page ignored the click."
else
  send key s
  expect "^browser: title $signed sign-in cookie kept storage kept, set\$" \
    "s in the session sign-in page didn't set its session cookie and sessionStorage"
  # Another of the site's pages in front, then the driver makes the tabs 2 hours older, and a third
  # shown checks for tabs unseen that long.
  for page in notes.txt notes.txt?again; do
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send type "${site}$page"
    send key return
    expect "^browser: loaded [0-9]* ${site}$page\$" "${site}$page didn't load in a tab"
    [ "$page" != notes.txt ] || send age 7200
  done
  expect 'browser: idle tabs checked$' "showing a tab didn't check for tabs unseen 2 hours"
  await "browser: tab $signed without its page\$" || fail "the sign-in's tab, unseen 2 hours, kept its page"
  switch "3 cmd" "$signed"
  expect "browser: tab $signed has its page\$" "the sign-in's tab didn't get its page back once shown"
  expect "^browser: title $signed sign-in cookie [a-z]* storage [a-z]*, drawn\$" \
    "the sign-in's tab didn't draw its page once shown"
  found=$(grep "^browser: title $signed sign-in cookie " "$log" | tail -1)
  case "$found" in
    *"cookie kept storage kept, drawn") ;;
    *"storage none, drawn") fail "the tab that let go of its page came back without its sessionStorage, signed out" ;;
    *) fail "the tab that let go of its page came back without its session cookie: $found" ;;
  esac
  ../tools/window-screenshot.sh "$pid" "$out/unloaded-sign-in.png" || fail "no window to capture"
  exec 3>&-
  quit
fi
echo "ok: new tabs open at the top and show, the command bar lists the Space's other tabs before anything is typed, suggests pages, their tabs and well-known sites and completes what's typed, ⌘L opens it centred and a click on the URL field over the field, ⇧⌘A lists the open tabs alone, typing filters them and ↩ switches, ⌘T and ⌘L close the open bar again, ⌘L's field holds the whole address and ↩ on it reloads, Tab in the empty field or \"/ \" lists only the commands until Esc or ⌫ leaves, keys and clicks switch, ⇧⌦ and a click on a row's trash take a page out of history and ⌥⌘⌫ a search typed before, which Chrome then says left the history, and the button's tooltip names it and its key over the bar, ⌘Z puts the page back and ⇧⌘Z takes it out again, ⇧⌘↩ searches and ⌘↩ opens a tab behind that loads once shown, Tab on a hinted row searches with its engine alone until ⌫ leaves, a space after the keyword is only a space, \"yout\" hints YouTube and Tab searches it, ↩ on a sum's row copies its result, and a right-click in its field shows Paste and Go, ⌃Tab goes back through the tabs shown while ⌃ is held, a page's tab goes under it and closing it, by close and ↩ in the command bar, goes back, tabs opened behind close to their sibling then their opener, a switch to an unrelated tab and a link opened in front forget older openers, so a pinned tab's newest tab closes back to it, ⌘W shows the tab below or above, on Linux two quick ⇧⌘T reopen both tabs closed last, an empty Space shows no pane, its window resizes, and on Windows maximizes and restores, and it still opens tabs, a tab mutes and its speaker unmutes it, a tab unseen 2 hours lets go of its page as another shows unless it plays sound, asks before it's left or shows in a split, and gets it back with its history and its sessionStorage once shown, and Settings' Unload switch turns that off and on, Site Search Settings opens Chromium's search engines page, a pick of View Media… lists it above View Archive…, its equal for \"view\", back right-clicked or held lists the pages back, nearest first, with their favicons, and a row picked goes there, back held and reload pressed shrink a little and grow back, and reload let go off it doesn't reload, and ⌘-clicked opens the page back in a tab behind. Reset all tabs in this Space takes the loaded pinned tabs back to their pinned URLs and leaves the unloaded one. ⌘-click and ⇧-click choose three rows, whose menu and ⇧⌘C copy their links in order, and ⌘D pins them, New Folder folders them and ⌘W closes them, each undone by ⌘Z. On Windows the bar, a tooltip and a menu are on screen only while the UI shows them: after Esc, ⌘T again or a click on the page, as the pointer leaves, after Windows shows them again and after their window was minimized${adapter:-}.${desktop:+ On Linux $desktop.}"
[ -z "${committed:-}" ] || echo "  On Windows Alt+F4 in the bar closes its window and the bar opens" \
  "over the next one, the bar" \
  "moves with its window and the window's menu closes, and the bar stays open over a page that" \
  "commits in another site's renderer and $committed: $out/command-bar-moved.png"
echo "  Switch to a loaded tab, tab strip to frame shown (Chrome's measure; gate 16 ms): $chrome_ms."
echo "  A site search for example: the core listed $listed_rows rows, the bar one."
echo "  Switch, key to the core's report: $app_ms. Windows: $out/muted.png, $out/tabs.png, $out/bar-close.png, $out/suggestions.png, $out/completion.png, $out/search-tabs.png, $out/url-field-bar.png, $out/commands-only.png, $out/numbered.png, $out/listed.png, $out/popular.png, $out/site-hint.png, $out/site-search.png, $out/field-menu.png, $out/empty.png, $out/remove-suggestion.png, $out/remove-tooltip.png, $out/site-search-settings.png, $out/bar-picked.png, $out/idle-unloaded.png, $out/back-held.png, $out/back-menu.png, $out/back-behind.png, $out/chosen.png, $out/chosen-menu.png"
