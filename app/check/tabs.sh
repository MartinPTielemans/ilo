# A tab opened from the command bar goes at the top of the Space's Today tabs and shows. The command
# bar lists the Space's other tabs before anything is typed, those shown last first, and suggests
# pages from history as it's typed in, growing to fit them, with the tabs already at them, never the
# one shown, and well-known sites under them. An open tab whose name the typing starts is the first
# row. ↩ or a click on a tab's row shows that tab without opening one. Typing at the end completes
# the text to the first row's page, and ↩ goes there. ⌘L opens the bar centred and a click on the
# URL field over the field. ⇧⌘A opens it for Search Tabs, listing the open tabs alone, which typing
# filters, and ↩ switches to the one left. ⌘T or ⌘L closes the open bar again, ⌘L's field holds the
# whole address, and ↩ on it as it opened reloads the page. Tab in the empty field, or "/ ", lists
# only the commands, which typing filters, until Esc or ⌫ at the start leaves. ⌘1–⌘9, ⌥⌘↓, ⌥⌘↑ and
# clicks on rows show tabs. ⌃Tab, with the page or the UI focused, shows the tab shown before, and
# with ⌃ held goes on back through the tabs in the order they were shown, and ⌃⇧Tab the other way,
# until ⌃ goes up or a click. A tab a page opens goes under it and closing it goes back there, and
# ⌘W otherwise shows the tab below, or above at the bottom. Tabs opened behind close to their
# sibling, then their opener; after a switch to an unrelated tab, a tab opened in front closes to
# the tab below; and a pinned tab's newest tab opened in front closes back to it, past an older one.
# Closing the last tab leaves the Space empty, with no pane, its window resizing, and on Windows
# maximizing and restoring, with no page, and ⌘T still works there. Mute/Unmute Tab mutes a tab,
# its row shows a speaker while it plays, crossed out while it's muted, and a click on it unmutes
# the tab. There ⇧⌦ takes a page out of history, as a click on the ✕ the pointer draws on its row
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
# and Settings' Unload switch turns that off and on. Back right-clicked or held lists the pages
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
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" --enable-tracing=latency \
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

switch "3 cmd" "$a"
send click 290 34
expect "^browser: tab [0-9]* opened by $a in the foreground\$" "the page's button didn't open a tab"
d=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
expect_tabs "$c $b $a $d" "$d" "a tab the page opened"
send key w cmd
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
# Lines logged since the driver's command $1.
since() {
  sed -n "/^browser: drive $1: /,\$p" "$log"
}
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
# With the switch off, a tab 2 hours unseen keeps its page, and turned on, it lets go. Settings
# stays open behind the browser window between the two clicks and comes back to the front for the
# second: a click in a Settings window closed and opened again, on the switch it last changed, never
# reached it.
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show General"
settings_drew General
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
expect '^browser: settings shown General$' "Settings… didn't bring Settings back to the front"
settings_drew General
send clickkey $unload_switch
expect '^browser: unload idle tabs on$' "a click on Unload tabs' switch didn't turn it on"
expect "browser: tab $c without its page\$" "turning the switch on didn't unload the tab unseen 2 hours"
settings_close
post clickkey $close
expect '^browser: settings hidden$' "the close button didn't hide Settings"
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
  expect '^browser: settings shown General$' "Settings… didn't show General after its close button"
  settings_drew General
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
# The pointer on a row from history draws a ✕ at the row's end, as Arc's does, and a click on it
# takes the row's page out rather than going there. Rows are 50 tall under the input's 68, and the
# ✕'s middle is 29 in from the bar's right edge; the pointer goes down them to example.com's.
# The bar's frame in the window, which can be in half points.
bar_at=$(grep '^browser: command bar at' "$log" | tail -1 | cut -d' ' -f5-)
# The ✕ of row $1, in the bar's window, or in the browser window's with "window".
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
expect "$drew" "the pointer on example.com's row didn't draw its ✕: $(suggestions | tr '\n' ' ')"
row=$(grep "$drew" "$log" | tail -1 | cut -d' ' -f8)
../tools/window-screenshot.sh "$pid" "$out/remove-suggestion.png" || fail "no window to capture"
send clickkey $(cross "$row")
expect '^browser: command bar removes https://example.com/$' "a click on the ✕ didn't take example.com's row out"
expect '^browser: command bar suggestions "example" in ' \
  "the command bar didn't list its rows again after the ✕"
none_since '^browser: command bar \(go\|switch to tab\|behind\)' || fail "a click on the ✕ took its row too"
suggestions | grep -q '^https://example.com/' && fail "example.com's row stayed after its ✕"
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
# with it alone, ⌫ in its empty field leaves it, keeping the keyword and a space, and a space typed
# after the keyword enters it again.
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
send type cats
expect "$listed cats\" in [0-9.]* ms: https://duckduckgo.com/?q=cats" \
  "the site search didn't list DuckDuckGo's search first: $(suggestions | tr '\n' ' ')"
../tools/window-screenshot.sh "$pid" "$out/site-search.png" || fail "no window to capture"
for _ in 1 2 3 4 5; do send key backspace; done
expect '^browser: command bar leaves site search$' \
  "⌫ in the site search's empty field didn't leave it"
expect '^browser: command bar drew "duckduckgo.com "$' \
  "leaving the site search didn't put the keyword and the space back"
send key backspace
expect "$listed\" in $hint" \
  "the command bar didn't hint DuckDuckGo's site search again: $(suggestions | tr '\n' ' ')"
send type ' dogs'
expect '^browser: command bar site search duckduckgo.com by space$' \
  "a space after DuckDuckGo's keyword didn't enter its site search"
expect "$listed dogs\" in " "the site search didn't list DuckDuckGo's searches for what was typed"
send key return
went=$(grep '^browser: command bar go' "$log" | tail -1)
expect '^browser: command bar go https://duckduckgo.com/?q=dogs' \
  "↩ in the site search didn't search DuckDuckGo: $went"
expect '^browser: command bar hidden$' "↩ in the site search didn't close the command bar"
# History has the search once its page commits, which can come after the next ⌘T. The page's
# title says it has: its first page line comes as it starts.
expect '^browser: title [0-9]* .*dogs' "the search for dogs didn't reach DuckDuckGo"
# ⌥⌘⌫ takes the selected row out too, beside ⇧⌦, as Arc's does: here the search for dogs, typed
# before. Off the Mac it would be Ctrl+Alt+Backspace, which X11 can take to end the session, so
# there ⇧⌦ alone does (command_bar.rs).
dogs='https://duckduckgo.com/?q=dogs'
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type dog
expect "^browser: command bar suggestions \"dog\" in .*$dogs[^,]* removable" \
  "the command bar didn't suggest the search for dogs typed before: $(suggestions | tr '\n' ' ')"
for _ in 1 2 3 4 5 6 7 8; do
  send key down
  grep '^browser: command bar selected' "$log" | tail -1 | grep -q " $dogs" && break
done
if [ "$(uname)" = Darwin ]; then
  remove="backspace alt cmd" removed=⌥⌘⌫
else
  remove="delete shift" removed=⇧⌦
fi
send key $remove
expect "^browser: command bar removes $dogs" "$removed didn't take the search for dogs out"
expect '^browser: command bar suggestions "dog" in ' "the command bar didn't list its rows again after $removed"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
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
send release $back_button
none_since '^browser: command back via sidebar$' || fail "letting go of back held went back too"
send choose back
expect '^browser: command history -1 via sidebar$' "choosing the held menu's row didn't reach the UI"
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
expect "^browser: today [0-9][0-9]* [0-9][0-9]* [0-9][0-9]* $rest\$" \
  "⌘Z after ⌘W didn't bring the three chosen tabs back: $(today)"

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
spans=$(python3 -I - "$trace" << 'PY'
import json, sys
# Chrome's tab switch spans, begin and end events paired by id, in microseconds. A switch to a tab
# whose page hasn't loaded yet, as when a new tab shows, times the page load; Chrome reports those
# apart from switches, and so does this. Chrome times none between the tabs of a split view, as to
# the one Add Split View opens beside the tab shown, whose page is on screen already, and the core
# has it do the same for ours. A switch cut off before its frame showed, as by the next switch
# while ⌃ is held, never finished, so Chrome's histograms leave it out; it's only counted.
events = json.load(open(sys.argv[1]))
events = events["traceEvents"] if isinstance(events, dict) else events
begins = {}
for event in events:
    if event.get("name") != "TabSwitching::Latency":
        continue
    if event.get("ph") == "b":
        measurement = event.get("args", {}).get("tab_switch_measurement", {})
        begins[event.get("id")] = (event["ts"], "NOT_LOADED" in measurement.get("tab_state", ""),
                                   measurement.get("result") == "RESULT_INCOMPLETE")
    elif event.get("ph") == "e" and event.get("id") in begins:
        begin, not_loaded, incomplete = begins.pop(event.get("id"))
        if not not_loaded:
            print("incomplete" if incomplete else (event["ts"] - begin) / 1000)
PY
)
chrome_ms=$(echo "$spans" | grep -v incomplete | summary)
[ -n "$chrome_ms" ] || fail "the trace has no finished tab switches"
# grep -c finds none in a run where no switch was cut off, and exits 1 then.
chrome_ms="$chrome_ms, $(echo "$spans" | grep -c incomplete || true) incomplete, not counted"
echo "ok: new tabs open at the top and show, the command bar lists the Space's other tabs before anything is typed, suggests pages, their tabs and well-known sites and completes what's typed, ⌘L opens it centred and a click on the URL field over the field, ⇧⌘A lists the open tabs alone, typing filters them and ↩ switches, ⌘T and ⌘L close the open bar again, ⌘L's field holds the whole address and ↩ on it reloads, Tab in the empty field or \"/ \" lists only the commands until Esc or ⌫ leaves, keys and clicks switch, ⇧⌦ and a click on a row's ✕ take a page out of history and ⌥⌘⌫ a search typed before, ⇧⌘↩ searches and ⌘↩ opens a tab behind that loads once shown, Tab on a hinted row and a space after an engine's keyword search with it alone until ⌫ leaves, ↩ on a sum's row copies its result, and a right-click in its field shows Paste and Go, ⌃Tab goes back through the tabs shown while ⌃ is held, a page's tab goes under it and closing it goes back, tabs opened behind close to their sibling then their opener, a switch to an unrelated tab and a link opened in front forget older openers, so a pinned tab's newest tab closes back to it, ⌘W shows the tab below or above, an empty Space shows no pane, its window resizes, and on Windows maximizes and restores, and it still opens tabs, a tab mutes and its speaker unmutes it, a tab unseen 2 hours lets go of its page as another shows unless it plays sound, asks before it's left or shows in a split, and gets it back with its history once shown, and Settings' Unload switch turns that off and on, Site Search Settings opens Chromium's search engines page, back right-clicked or held lists the pages back, nearest first, with their favicons, and a row picked goes there, back held and reload pressed shrink a little and grow back, and reload let go off it doesn't reload, and ⌘-clicked opens the page back in a tab behind. Reset all tabs in this Space takes the loaded pinned tabs back to their pinned URLs and leaves the unloaded one. ⌘-click and ⇧-click choose three rows, whose menu and ⇧⌘C copy their links in order, and ⌘D pins them, New Folder folders them and ⌘W closes them, each undone by ⌘Z."
echo "  Switch to a loaded tab, tab strip to frame shown (Chrome's measure; gate 16 ms): $chrome_ms."
echo "  Switch, key to the core's report: $app_ms. Windows: $out/muted.png, $out/tabs.png, $out/suggestions.png, $out/completion.png, $out/search-tabs.png, $out/url-field-bar.png, $out/commands-only.png, $out/listed.png, $out/popular.png, $out/site-hint.png, $out/site-search.png, $out/field-menu.png, $out/empty.png, $out/remove-suggestion.png, $out/site-search-settings.png, $out/idle-unloaded.png, $out/back-held.png, $out/back-menu.png, $out/back-behind.png, $out/chosen.png, $out/chosen-menu.png"
