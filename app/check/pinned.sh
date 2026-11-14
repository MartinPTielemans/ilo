# check.sh runs this at 130.
# ⌘D pins the tab shown, from the page and from the UI, at the bottom of the pinned tabs, and unpins
# it to the top of Today. ⌘1–⌘9 count the pinned tabs first. ⌘W, the − under the pointer, on the
# tab shown or not, or a middle-click on its row, unloads a pinned tab and keeps its row, and shows
# the tab shown before it that's still open, whose row it rings for a moment, else no page. The unloaded row has a remove button in the −'s place, which takes the tab out of
# the pinned tabs and not into Today, and ⌘Z pins it again at its place. Shown again, the tab loads. A link in its page takes it away from
# its pinned URL, and ⌘↩ from the page and the UI, and a click on its favicon, go back there. An
# address typed into it opens in a Today tab of its own, as in Arc, and closing that shows the
# pinned tab again. Launched again, the app brings back the pinned tab pinned and away from its
# pinned URL, without loading it. Dragging a row moves its tab in the pinned tabs, pins it and
# unpins it, without showing it, and escape puts it back. New Folder makes a folder under the pinned
# tabs, which takes the name typed, as New Folder… in the sidebar's empty space does, and a tab's
# Add to Folder › moves it into a folder or a new one in its place, named for its site, each one
# step for ⌘Z; the tab shown in that folder, closed in a closed one, is listed under the outer
# one, whose glyph has three dots, and stays listed as another tab shows until a click on the
# outer one closes it for good. A row dragged right under it goes in and dragged left comes out,
# a folder drags as a row, a click closes and opens it, and closed, it previews its tab, which the
# keyboard shows. Launched again, the app brings it back closed and a double-click renames it. In
# its menu, Copy All Links is Copy All Links as Markdown with ⌥ held and Change Icon… gives it an
# emoji. Settings' Export… writes it and its tab as Markdown links, and Delete Folder in its menu
# leaves its tab where it was. Links and files dragged onto the list from the page or another app open as
# tabs where the pointer lets go, and dragged text that isn't a link opens a search for it there,
# but a link that isn't a web link doesn't open. A split made with a pinned tab is pinned, by ⌥-click
# on its link or by dragging one pinned tab onto another. Away from
# its pinned URL, Pinned URL › Replace Pinned URL with Current and Edit… change its pinned URL,
# which ⌘Z gives back, and ⌘-click on its favicon goes back keeping the page in a new tab. Pinned at
# another host than its page's, its row names the page's host under its title. Export
# Space as Markdown… saves the Space's folders, pinned tabs and Today tabs as a list of links. The
# chevron after the Space's name collapses the pinned rows but the tab shown's, which the tabs'
# order then skips, and the command bar and the View menu expand and collapse them. Launched again,
# the app brings them back collapsed. Focus Sidebar rings the tab shown's row, whose ring the arrows
# move past the collapsed rows and return shows, and a key in the page or escape ends it; ⇧F10
# opens the ringed row's menu and ⌘W closes the ringed row, not the tab shown. A folder
# from the desktop dropped on the list becomes a folder of its files' tabs, none shown, a file's
# row dragged onto the page hands the file to the system's drag, and the +'s Add File or Folder…
# opens a file at the top of Today.

# Pinned tabs, from the served page, whose favicon fades once its tab is unloaded.
serve
log="$out/pinned.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the pinned tabs test page didn't draw"
a=$(today)
await "^browser: favicon $a [1-9][0-9]* bytes\$" || fail "the served page's favicon didn't reach the UI"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: pinned tabs. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
open_from_bar example.com
b=$tab
open_from_bar example.org
c=$tab
expect_tabs "$c $b $a" "$c" "after opening two tabs"
send key d cmd
expect '^browser: command toggle-pin via page$' "⌘D with the page focused didn't reach the UI"
expect_lists "$c" "$b $a" "after ⌘D from the page"
switch "3 cmd" "$a"
send click $sidebar_point
send key d cmd
expect '^browser: command toggle-pin via ui$' "⌘D with the UI focused didn't run"
expect_lists "$c $a" "$b" "after ⌘D from the UI"
switch "1 cmd" "$c"
switch "3 cmd" "$b"

# Dragging a row moves its tab, in the pinned tabs and between them and Today, and its press shows
# it, as Arc's does. Today's top row is under the pinned rows, the divider and "+ New Tab".
today_top="100 $((90 + 44 + 41 * 2 + 15 + 41 + 20))"
clicks=$(detents)
send drag $(pinned_row 1) 100 150
expect "^browser: dragged $a to pinned 0\$" "a drag of the second pinned tab up didn't move it"
# It passed one row, so the trackpad clicked once, and letting go added none.
passed=$(($(detents) - clicks))
[ "$passed" = 1 ] || fail "a row dragged past one gave $passed clicks"
expect_lists "$a $c" "$b" "after dragging a pinned tab up"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "pressing the row dragged didn't show its tab"
# In steps, as the pointer goes: Today's tab comes away, and over the pinned rows, which stay put,
# a line marks where it would go.
clicks=$(detents)
send press $today_top
expect '^browser: pressed [0-9]*$' "the press on Today's row didn't reach the UI"
none_since '^browser: detent$' || fail "pressing a row clicked the trackpad"
send dragto 100 270
send dragto 100 230
# Between the pinned rows, as a rest on a row's middle would join that tab in a split view, and the
# capture takes longer than that rest.
send dragto 100 175
../tools/window-screenshot.sh "$pid" "$out/drag.png" || fail "no window to capture"
none_since '^browser: dragged' || fail "a row dropped before the pointer let go"
[ "$(detents)" -gt "$clicks" ] || fail "a row dragged up past the pinned rows didn't click"
# Holding the row at its place doesn't click again, nor does letting go.
held=$((step + 1))
send dragto 100 176
send release 100 175
expect "^browser: dragged $b to pinned 1\$" "a drag of the Today tab up didn't pin it"
none_since '^browser: detent$' "$held" || fail "a row held at its place, or let go, clicked the trackpad"
expect_lists "$a $b $c" " *" "after dragging the Today tab into the pinned tabs"
# The rows make room only once it's let go, as Arc's do, so the next drag waits for them.
expect '^browser: rows settled$' "the pinned rows didn't settle after the drop"
send drag $(pinned_row 1) 100 300
expect "^browser: dragged $b to today 0\$" "a drag of a pinned tab down didn't unpin it"
expect_lists "$a $c" "$b" "after dragging a pinned tab into Today"
send drag $(pinned_row 0) 100 200
expect_lists "$c $a" "$b" "after dragging the first pinned tab down"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "pressing the row dragged didn't show its tab"
# The press now shows its row's tab, so it waits for the rows to stop under the pointer.
expect '^browser: rows settled$' "the pinned rows didn't settle after the drag down"
# Escape puts a dragged row back.
send press $(pinned_row 0)
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "pressing a row didn't show its tab"
send dragto 100 250
send key escape
expect '^browser: command stop via ' "escape during a drag didn't reach the UI"
clicks=$(detents)
send release 100 250
[ "$(detents)" = "$clicks" ] || fail "letting go after escape clicked the trackpad"
none_since '^browser: dragged' || fail "a drag escape put back dropped its row"
none_since '^browser: selected' || fail "letting go of a drag escape put back showed its tab"
[ "$(pinned)" = "$c $a" ] || fail "a drag escape put back moved a tab: the pinned tabs are \"$(pinned)\""

# ⌘W on a pinned tab unloads it and shows the tab shown before it, a pinned one still loaded ahead
# of the top Today tab, and rings its row for a moment. Showing it again loads it.
switch "2 cmd" "$a"
send key w cmd
expect "^browser: selected $c in " "⌘W on a pinned tab didn't show the tab shown before it"
expect "^browser: sidebar ring $c at " "⌘W on a pinned tab didn't ring the row of the tab it showed"
expect "^browser: page $a unloaded 1\$" "⌘W on a pinned tab didn't unload it"
none_since "^browser: tab $a closed\$" || fail "⌘W closed a pinned tab"
[ "$(pinned)" = "$c $a" ] || fail "⌘W on a pinned tab took its row: the pinned tabs are \"$(pinned)\""
../tools/window-screenshot.sh "$pid" "$out/pinned.png" || fail "no window to capture"
expect '^browser: sidebar ring hidden$' "the ring stayed on the tab ⌘W showed"
send click $(pinned_row 1)
expect "^browser: selected $a in " "a click on the unloaded pinned tab didn't show it"
expect "^browser: loaded $a $site\$" "the unloaded pinned tab didn't load once shown"
expect "^browser: page $a unloaded 0\$" "the pinned tab still read as unloaded once it loaded"

# The − under the pointer unloads a pinned tab not shown, and the remove button the unloaded row has instead
# waits for the pointer to leave it, so the second click shows the tab. The pointer comes from the
# page, where the card of the row it was on goes, so the capture has the − alone.
send move 600 400
[ "$(grep -E '^browser: hover card (window shown|hidden)$' "$log" | tail -1)" != \
  "browser: hover card window shown" ] ||
  expect '^browser: hover card hidden$' "the hover card stayed after the pointer left the rows"
send move $(pinned_close 0)
expect "^browser: hover tab $c close − favicon none\$" "the pinned tab's row didn't draw its −"
../tools/window-screenshot.sh "$pid" "$out/pinned-close.png" || fail "no window to capture"
send click $(pinned_close 0)
expect '^browser: command close-tab via sidebar$' "a click on a pinned tab's − didn't run Close Tab"
expect "^browser: page $c unloaded 1\$" "the − didn't unload the pinned tab"
none_since '^browser: selected' || fail "unloading a pinned tab not shown changed the tab shown"
expect "^browser: hover tab $c close 0 favicon none\$" "the unloaded pinned tab's row kept its −"
send click $(pinned_close 0)
expect "^browser: selected $c in " "a click where an unloaded pinned tab's − was didn't show the tab"
none_since '^browser: command close-tab' || fail "an unloaded pinned tab showed its −"
none_since '^browser: command remove-tab' || fail "a second click on the − removed the pinned tab"
expect "^browser: loaded $c https://example.org/\$" "the pinned tab didn't load once shown"
# Shown, its − unloads it too and shows the tab shown before it.
expect "^browser: hover tab $c close − favicon none\$" "the pinned tab shown didn't draw −"
send click $(pinned_close 0)
expect '^browser: command close-tab via sidebar$' "a click on a pinned tab's − didn't run Close Tab"
expect "^browser: page $c unloaded 1\$" "the − didn't unload the pinned tab"
expect "^browser: selected $a in " "the − on a pinned tab didn't show the tab shown before it"
[ "$(pinned)" = "$c $a" ] || fail "the − took the pinned tab's row: pinned tabs are \"$(pinned)\""
send click $(pinned_row 0)
expect "^browser: selected $c in " "a click on the unloaded pinned tab didn't show it"
expect "^browser: loaded $c https://example.org/\$" "the pinned tab didn't load once shown"
# A middle-click on a row closes its tab as its − does, which unloads a pinned one.
send middleclick $(pinned_row 1)
expect '^browser: command close-tab via sidebar$' "a middle-click on a pinned tab didn't run Close Tab"
expect "^browser: page $a unloaded 1\$" "a middle-click didn't unload the pinned tab"
none_since '^browser: selected' || fail "a middle-click on a pinned tab not shown changed the tab shown"
send click $(pinned_row 1)
expect "^browser: selected $a in " "a click on the pinned tab a middle-click unloaded didn't show it"
expect "^browser: loaded $a $site\$" "the pinned tab a middle-click unloaded didn't load once shown"

# Without Today tabs, closing the last shows the nearest pinned tab still loaded, and closing a pinned
# tab shows the one shown before it still loaded, else no page.
switch "3 cmd" "$b"
send key w cmd
expect "^browser: tab $b closed\$" "⌘W didn't close the last Today tab"
expect_lists "$c $a" " *" "after closing the last Today tab"
expect "^browser: selected $a\$" "closing the last Today tab didn't show the lowest pinned tab still loaded"
send key w cmd
expect "^browser: selected $c in " "⌘W on a pinned tab without Today tabs didn't show the other one loaded"
expect "^browser: page $a unloaded 1\$" "⌘W didn't unload the pinned tab"
send key w cmd
expect '^browser: selected 0$' "⌘W on the last pinned tab loaded didn't leave the window without a page"
expect "^browser: page $c unloaded 1\$" "⌘W didn't unload the last pinned tab loaded"
none_since '^browser: loaded' || fail "closing a pinned tab loaded another"
../tools/window-screenshot.sh "$pid" "$out/pinned-unloaded.png" || fail "no window to capture"

# Under the pointer, an unloaded pinned row has the remove button in its −'s place, which its
# tooltip names. A click takes the tab out of the pinned tabs, not into Today, without loading or
# showing it, and offers Undo in a toast (#1376), and ⌘Z pins it again at its place (#1349).
# The button comes with the row's hover, and the pointer's next move finds it, as a hand's does.
send move $(pinned_row 1)
send move $(pinned_row 0)
expect "^browser: hover tab $c close remove favicon none\$" \
  "the unloaded pinned tab's row didn't draw its remove button"
send move $(pinned_close 0)
expect '^browser: tooltip shown Remove Tab$' "the remove button's tooltip didn't name it"
expect '^browser: tooltip window shown$' "the remove button's tooltip didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/pinned-remove.png" || fail "no window to capture"
send click $(pinned_close 0)
expect '^browser: command remove-tab via sidebar$' "a click on the remove button didn't run Remove Tab"
expect "^browser: tab $c closed\$" "the remove button didn't close the pinned tab"
expect '^browser: toast shown Tab Removed \[undo\]$' "removing a pinned tab didn't offer Undo"
expect_lists "$a" " *" "after removing a pinned tab"
../tools/window-screenshot.sh "$pid" "$out/pinned-removed.png" || fail "no window to capture"
none_since '^browser: loaded' || fail "removing an unloaded pinned tab loaded it"
none_since '^browser: selected' || fail "removing a pinned tab changed the tab shown"
send key z cmd
expect '^browser: undo done$' "⌘Z didn't undo removing the pinned tab"
expect_lists "[0-9][0-9]* $a" " *" "after ⌘Z on removing a pinned tab"
c=$(pinned | cut -d' ' -f1)
none_since '^browser: loaded' || fail "⌘Z loaded the pinned tab it brought back"
../tools/window-screenshot.sh "$pid" "$out/pinned-remove-undone.png" || fail "no window to capture"

# ⌘D unpins a tab to the top of Today. The served page stays pinned, for its link, and the names
# swap with it: from here $c is the pinned tab, the served page, and $a the Today tab.
send click $(pinned_row 0)
expect "^browser: selected $c in " "a click on the unloaded pinned tab didn't show it"
expect "^browser: loaded $c https://example.org/\$" "the unloaded pinned tab didn't load once shown"
send key d cmd
expect_lists "$a" "$c" "after ⌘D on a pinned tab"
unpinned=$c
c=$a
a=$unpinned

# Away from its pinned URL, ⌘↩ goes back there, from the page and the UI. The click comes under
# the pointer that pressed the row ⌘D unpinned, as a trackpad tap does, while that row may still be
# drawn there, and shows the tab that rests there.
send click $(pinned_row 0)
expect "^browser: selected $c in " "a click on the unloaded pinned tab didn't show it"
expect "^browser: loaded $c $site\$" "the unloaded pinned tab didn't load once shown"
# The served page's link, 200 by 40 points, 100 in from the card's top left, which is 228 and 10 in
# from the window's (site.sh).
link_point="430 130"
# The pointer on the link, once the page takes it. Chromium drops a page's input until it has drawn,
# which a software renderer does well after the page has loaded, so it moves until the page reports
# the link under it. Each try comes from off the link: a page reports a link only as the pointer
# reaches it, and one shown again under a pointer already there doesn't.
on_link() {
  local at
  at=$(wc -l < "$log")
  for i in $(seq 50); do
    send move 600 400
    send move $((430 + i % 2)) 130
    tail -n "+$((at + 1))" "$log" | grep -q "^browser: link target ${site}notes.txt\$" && return 0
    sleep 0.1
  done
  fail "the pointer on the pinned tab's link reported no address"
}
# Away from it by its page's link.
link_away() {
  on_link
  send click $link_point
  expect "^browser: loaded $c ${site}notes.txt\$" "the page's link didn't load in the pinned tab"
  expect "^browser: page $c away 1\$" "a pinned tab at another page didn't read as away from its pinned URL"
}
# Away from it, then back with ⌘↩ from side $1, which a click gives the keyboard.
link_away_and_back() {
  link_away
  if [ "$1" = ui ]; then
    send click $sidebar_point
  else
    send click $page_point
  fi
  send key return cmd
  expect "^browser: command back-to-pinned-url via $1\$" "⌘↩ with the $1 focused didn't run Back to Pinned URL"
  expect "^browser: loaded $c $site\$" "⌘↩ with the $1 focused didn't load the pinned URL"
  expect "^browser: page $c away 0\$" "the pinned tab still read as away at its pinned URL"
}
link_away_and_back page
link_away_and_back ui
# A split made with a pinned tab is pinned, as Arc keeps one made in Pinned: ⌥-click on its
# page's link opens the link in a pane beside it, which joins the pinned tabs as one row. Separate
# All Pages ends it, and ⌘D and ⌘W take the pane's tab out again.
on_link
send click $link_point alt
expect "^browser: splits $c+[0-9][0-9]*\$" \
  "⌥-click on the pinned tab's link didn't open it beside the page: $(grep '^browser: splits' "$log" | tail -1)"
n=$(grep '^browser: splits' "$log" | tail -1 | sed 's/.*+//')
expect "^browser: pinned $c $n\$" "the pane beside the pinned tab isn't pinned: the pinned tabs are \"$(pinned)\""
expect "^browser: today $a\$" "the pane beside the pinned tab went into Today: Today is \"$(today)\""
expect "^browser: loaded $n ${site}notes.txt\$" "the link didn't load in the pane beside the pinned tab"
# The outline comes as the rows are drawn.
expect '^browser: outline ' "the pinned split's row wasn't drawn"
../tools/window-screenshot.sh "$pid" "$out/pinned-split.png" || fail "no window to capture"
bar_command separate-split "Separate All Tabs" "separate all"
expect '^browser: splits $' "Separate All Tabs didn't end the pinned split"
expect "^browser: panes $n\$" "Separate All Tabs didn't leave the focused pane's tab shown alone"
# A split made by dragging inside Pinned stays pinned too, as Arc's: the pane's tab, a pinned row
# of its own now, rested on the pinned tab's row's right half joins it there on the right, and ⌘Z
# takes it back to its row.
expect_lists "$c $n" "$a" "after Separate All Tabs on the pinned split"
y0=$(pinned_row 0 | cut -d' ' -f2)
send press $(pinned_row 1)
send dragto 110 $(pinned_row 1 | cut -d' ' -f2)
send dragto 170 $y0
expect "^browser: drop onto $c right\$" \
  "a pinned tab rested on a pinned row didn't take it as the drop"
send release 170 $y0
expect "^browser: dragged $n onto $c\$" "the pinned tab let go on another pinned tab's row didn't join it"
expect "^browser: splits $c+$n\$" "the pinned tabs dragged together aren't a split: $(grep '^browser: splits' "$log" | tail -1)"
expect_lists "$c $n" "$a" "after dragging a pinned tab onto another inside Pinned"
../tools/window-screenshot.sh "$pid" "$out/pinned-drag-split.png" || fail "no window to capture"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the split made inside Pinned"
expect '^browser: splits $' "⌘Z didn't take the dragged pinned tab out of the split: $(grep '^browser: splits' "$log" | tail -1)"
expect_lists "$c $n" "$a" "after ⌘Z on the split made inside Pinned"
send key d cmd
expect_lists "$c" "$n $a" "after ⌘D on the pinned split's pane"
send key w cmd
expect "^browser: tab $n closed\$" "⌘W didn't close the pinned split's pane"
# The next click waits for the tab ⌘W shows, which would otherwise come after it.
expect "^browser: selected $a\$" "⌘W on the pinned split's pane didn't show the top Today tab"
send click $(pinned_row 0)
expect "^browser: selected $c in " "a click on the pinned tab didn't show it"
# Away from its pinned URL, the menu's Pinned URL › Replace Pinned URL with Current makes the page
# it's at the one it goes back to, and ⌘Z gives it back the one it had. Edit… shows the pinned URL
# in a field in its row, which takes a URL typed over it, and ⌘Z takes that back too (Arc).
link_away
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Unpin Tab, Add to Favorites, Back to Pinned URL, Pinned URL › Replace Pinned URL with Current, Pinned URL › Edit…, Rename Tab, Change Icon…, Duplicate Tab, Add to Folder › New Folder, Move to Space (disabled), Close Tab')\$" \
  "a right-click on the pinned tab away from its pinned URL didn't show its Pinned URL submenu"
expect '^browser: menu window at ' "the menu didn't take the keyboard"
send key p
expect "^browser: menu highlights $(cased 'Pinned URL') at " "typing p didn't select Pinned URL"
send key right
expect '^browser: menu submenu Pinned URL shown at ' "→ didn't open the Pinned URL submenu"
expect "^browser: menu highlights $(cased 'Pinned URL › Replace Pinned URL with Current') at " \
  "→ didn't highlight Replace Pinned URL with Current"
../tools/window-screenshot.sh "$pid" "$out/pinned-url-menu.png" || fail "no window to capture"
send key return
expect '^browser: command replace-pinned-url via tab menu$' "Replace Pinned URL with Current didn't run"
expect "^browser: page $c away 0\$" "Replace Pinned URL with Current didn't make the page its pinned URL"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Replace Pinned URL with Current"
expect "^browser: page $c away 1\$" "⌘Z didn't give the pinned tab back its pinned URL"
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Replace Pinned URL with Current"
expect "^browser: page $c away 0\$" "⇧⌘Z didn't replace the pinned URL again"
send key z cmd
expect "^browser: page $c away 1\$" "⌘Z didn't give the pinned tab back its pinned URL again"
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Unpin Tab, Add to Favorites, Back to Pinned URL, Pinned URL › Replace Pinned URL with Current, Pinned URL › Edit…, Rename Tab, Change Icon…, Duplicate Tab, Add to Folder › New Folder, Move to Space (disabled), Close Tab')\$" "a right-click on the pinned tab didn't show its menu"
send choose "$(cased 'Pinned URL › Edit…')"
expect "^browser: edit pinned url $c\$" "Edit… didn't show the pinned URL in a field"
send type "${site}notes.txt"
../tools/window-screenshot.sh "$pid" "$out/pinned-url-edit.png" || fail "no window to capture"
send key return
expect "^browser: page $c away 0\$" "the pinned URL typed in Edit… didn't take"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the pinned URL typed"
expect "^browser: page $c away 1\$" "⌘Z didn't give the pinned tab back the pinned URL Edit… replaced"
# Pinned at another host than its page's, here the same server's other name, its row names the
# page's host under its title (Arc), and ⌘Z, giving back its pinned URL, takes the line away.
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Unpin Tab, ')" "a right-click on the pinned tab didn't show its menu"
send choose "$(cased 'Pinned URL › Edit…')"
expect "^browser: edit pinned url $c\$" "Edit… didn't show the pinned URL in a field"
send type "$(echo "$site" | sed 's/127\.0\.0\.1/localhost/')"
send key return
expect "^browser: page $c left pinned host 1\$" "a pinned tab off its pinned URL's host didn't name the host"
../tools/window-screenshot.sh "$pid" "$out/pinned-host.png" || fail "no window to capture"
send key z cmd
expect "^browser: page $c left pinned host 0\$" "⌘Z gave the pinned URL back but kept the host line"
# ⌘-click on its favicon goes back too, keeping the page it's at in a new tab at the top of Today,
# opened first, which shows (Arc).
# Its row draws −, shown or not.
send move $(pinned_favicon 0)
expect "^browser: hover tab $c close \(0\|−\) favicon $c\$" \
  "the pointer on the pinned tab's favicon wasn't drawn"
send click $(pinned_favicon 0) cmd
expect '^browser: command back-to-pinned-url-keeping-page via sidebar$' \
  "a ⌘-click on an away pinned tab's favicon didn't keep its page on the way back"
expect "^browser: today [0-9][0-9]* $a\$" "the ⌘-click didn't open a copy at the top of Today: Today is \"$(today)\""
k=$(today | cut -d' ' -f1)
expect "^browser: loaded $k ${site}notes.txt\$" "the copy didn't keep the page the pinned tab was at"
expect "^browser: loaded $c $site\$" "the ⌘-click didn't take the pinned tab back to its pinned URL"
expect "^browser: page $c away 0\$" "the pinned tab still read as away at its pinned URL"
since=$(grep -n "^browser: drive $step: " "$log" | cut -d: -f1)
opened=$(sed -n "$since,\$p" "$log" | grep -n "^browser: today $k $a\$" | head -1 | cut -d: -f1)
back=$(sed -n "$since,\$p" "$log" | grep -n "^browser: loaded $c $site\$" | head -1 | cut -d: -f1)
[ "$opened" -lt "$back" ] || fail "the pinned tab went back before its page's copy opened"
../tools/window-screenshot.sh "$pid" "$out/pinned-kept-page.png" || fail "no window to capture"
expect "^browser: selected $k\( in .*\)\{0,1\}\$" "the copy the ⌘-click kept didn't show"
send key w cmd
expect "^browser: tab $k closed\$" "⌘W didn't close the copy the ⌘-click kept"
expect "^browser: selected $a\$" "⌘W on the copy the ⌘-click kept didn't show the top Today tab"
send click $(pinned_row 0)
expect "^browser: selected $c in " "a click on the pinned tab didn't show it"
# An address typed into it opens in a tab it opened, at the top of Today, shown, and the pinned tab
# keeps its page. Closing that tab shows the pinned one again, its opener.
go_from_bar example.com
expect "^browser: tab [0-9]* opened by $c in the foreground\$" \
  "an address typed into the pinned tab didn't open a tab of its own"
typed=$(grep '^browser: tab [0-9]* opened by' "$log" | tail -1 | cut -d' ' -f3)
expect_lists "$c" "$typed $a" "after an address typed into the pinned tab"
expect "^browser: selected $typed\( in .*\)\{0,1\}\$" "the tab an address typed into the pinned tab opened wasn't shown"
expect "^browser: loaded $typed https://example.com/\$" "the address typed into the pinned tab didn't load in its tab"
none_since "^browser: loaded $c " || fail "the address typed into the pinned tab loaded in it"
send key w cmd
expect "^browser: tab $typed closed\$" "⌘W didn't close the tab the pinned tab opened"
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "closing the tab the pinned tab opened didn't show the pinned tab"
link_away
# The quit leaves Today's tab shown, so the pinned tab comes back unloaded.
switch "2 cmd" "$a"
# Its favicon under the pointer offers the way back.
send move $(pinned_favicon 0)
expect "^browser: hover tab $c close \(0\|−\|remove\) favicon $c\$" \
  "the pointer on the pinned tab's favicon wasn't drawn"
../tools/window-screenshot.sh "$pid" "$out/pinned-away.png" || fail "no window to capture"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't reach the UI"
exited

# Launched again, the pinned tab comes back pinned, at its page, unloaded, and away from its pinned
# URL. A click on its favicon goes back there and shows it.
log="$out/pinned-restored.log"
step=0
rm -f "$out/space.md"
# The export's file as an absolute path, which the app on Windows needs, as it resolves a relative
# one against its own folder (#997).
BROWSER_DRIVE=1 BROWSER_EXPORT_FILE="$(app_path "$PWD/$out/space.md")" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Space"
await '^browser: pinned [0-9][0-9]*$' || fail "the pinned tab didn't come back pinned"
await '^browser: today [0-9][0-9]*$' || fail "the Today tab didn't come back"
c=$(pinned)
a=$(today)
await "^browser: selected $a\$" || fail "the relaunch didn't show the tab shown last"
await "^browser: loaded $a https://example.org/\$" || fail "the tab shown didn't load"
await "^browser: page $c back 1 forward 0 loading 0 ${site}notes.txt\$" ||
  fail "the pinned tab didn't come back at its page: $(grep "^browser: page $c" "$log" | tail -1)"
await "^browser: page $c unloaded 1\$" || fail "the pinned tab didn't come back unloaded"
await "^browser: page $c away 1\$" || fail "the pinned tab didn't come back away from its pinned URL"
grep -q "^browser: loaded $c " "$log" && fail "the pinned tab loaded without being shown"
../tools/window-screenshot.sh "$pid" "$out/pinned-restored.png" || fail "no window to capture"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the restored pinned tab's way back. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The window opened under the pointer the quit left on the favicon, so it drew that hover with no
# move, and the driver's real click in the page jumped the pointer away with none either: off every
# row first, or the move back to the favicon changes no hover and draws no line (#2831).
send move $sidebar_point
drawn_hover() {
  grep '^browser: hover \(tab\|new\|none\) ' "$log" | tail -1
}
for _ in $(seq 100); do
  drawn_hover | grep -q '^browser: hover none ' && break
  sleep 0.1
done
drawn_hover | grep -q '^browser: hover none ' ||
  fail "the pointer off every row left a row hovered: $(drawn_hover)"
# Unloaded, its row has the remove button in its −'s place.
send move $(pinned_favicon 0)
expect "^browser: hover tab $c close remove favicon $c\$" \
  "the pointer on the pinned tab's favicon wasn't drawn"
send click $(pinned_favicon 0)
expect '^browser: command back-to-pinned-url via sidebar$' "a click on an away pinned tab's favicon didn't run Back to Pinned URL"
expect "^browser: selected $c in " "a click on the pinned tab's favicon didn't show it"
expect "^browser: loaded $c $site\$" "a click on the pinned tab's favicon didn't load its pinned URL"
expect "^browser: page $c away 0\$" "the pinned tab still read as away at its pinned URL"
none_since "^browser: loaded $c ${site}notes.txt" || fail "the unloaded tab loaded its old page on the way back"

# New Folder makes a folder under the pinned tabs with its name's field, which takes what's typed.
# The pinned tabs and folders the app last drew: each folder with + if it's open, else -, and
# what's in it in brackets.
outline() {
  grep '^browser: outline' "$log" | tail -1 | sed 's/^browser: outline *//'
}
# Waits for the rows moving since the outline last changed to come to rest, so a press lands on the
# row drawn there. An outline the same as the one before moves no row, and so brings no "rows
# settled" after it: the pages of a folder dropped from the desktop log theirs again as they load.
settled() {
  for _ in $(seq 300); do
    awk '/^browser: outline/ { if ($0 != last) s = 0; last = $0 } /^browser: rows settled/ { s = 1 }
      END { exit !s }' "$log" && return 0
    sleep 0.1
  done
  fail "the rows didn't come to rest"
}
send menu New Folder…
expect '^browser: command new-folder via ui$' "New Folder in the Tabs menu didn't run new-folder"
expect '^browser: rename folder [0-9]*$' "New Folder didn't give the folder's name field the keyboard"
f=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
[ "$(outline)" = "$c $f+[]" ] || fail "New Folder didn't make an open folder under the pinned tab: $(outline)"
send type work
send key return
expect "^browser: renamed folder $f work\$" "the new folder didn't take the name typed"
# Dragged right of where it was, under the folder, a pinned tab goes in, and dragged left of it,
# out. A folder drags as a row does.
send press $(pinned_row 0)
send dragto 110 180
expect '^browser: detent$' "a row dragged under the folder and in didn't click the trackpad"
held=$((step + 1))
send dragto 120 200
send release 120 200
expect "^browser: dragged $c to folder $f 0\$" "a pinned tab dragged right under the folder didn't go in"
none_since '^browser: detent$' "$held" || fail "a row held in the folder clicked again"
expect "^browser: outline $f+\\[$c\\]\$" "the tab dragged in isn't in the folder: $(outline)"
settled
send press $(pinned_row 1)
send dragto 90 197
expect '^browser: detent$' "a row dragged left out of the folder didn't click the trackpad"
held=$((step + 1))
send dragto 76 199
send release 76 199
expect "^browser: dragged $c to pinned 1\$" "a tab dragged left out of the folder didn't come out"
none_since '^browser: detent$' "$held" || fail "a row held out of the folder clicked again"
expect "^browser: outline $f+\\[\\] $c\$" "the tab dragged out is still in the folder: $(outline)"
settled
send drag $(pinned_row 0) 100 200
expect "^browser: dragged $f to pinned 1\$" "a drag of the folder down didn't move it"
# Its press closed it, as a press on a folder does, and a click opens it again.
expect "^browser: outline $c $f-\\[\\]\$" \
  "the folder dragged down isn't under the tab, closed: $(outline)"
settled
send click $(pinned_row 1)
expect "^browser: outline $c $f+\\[\\]\$" "a click on the folder dragged didn't open it: $(outline)"
# It's empty, so no row moves to wait for.
send press $(pinned_row 0)
send dragto 110 180
expect '^browser: detent$' "a row dragged under the folder and in didn't click the trackpad"
held=$((step + 1))
send dragto 120 200
send release 120 200
expect "^browser: outline $f+\\[$c\\]\$" "the tab dragged under the folder didn't go in: $(outline)"
none_since '^browser: detent$' "$held" || fail "a row held in the folder clicked again"
settled
none_since '^browser: selected' || fail "dragging into and out of a folder showed a tab"
../tools/window-screenshot.sh "$pid" "$out/folder.png" || fail "no window to capture"
# A click closes the folder and another opens it, recorded for its motion: closing critically
# damped, opening on its ease-out, neither overshooting. With the Today tab shown, the folder's tab
# isn't, so it folds away and the rows under it slide up, then back down.
switch "9 cmd" "$a"
record folder-motion 3
send click $(pinned_row 0)
expect '^browser: command toggle-folder via sidebar$' "a click on the folder didn't run toggle-folder"
expect "^browser: outline $f-\\[$c\\]\$" "a click on the open folder didn't close it: $(outline)"
# Each half plays out in the film before the next click.
settled
send click $(pinned_row 0)
expect "^browser: outline $f+\\[$c\\]\$" "a click on the closed folder didn't open it: $(outline)"
settled
recorded
# A right-click on the sidebar's empty space offers New Folder… after New Tab, which makes a folder
# as the Tabs menu's does (#1088).
post rightclick 100 500
expect "^browser: menu shown $(cased 'New Tab, New Folder…'), " \
  "a right-click on the sidebar's empty space didn't offer New Folder… after New Tab"
../tools/window-screenshot.sh "$pid" "$out/empty-space-menu.png" || fail "no window to capture"
send choose "$(cased 'New Folder…')"
expect '^browser: command new-folder via sidebar$' \
  "New Folder… in the empty space's menu didn't run"
expect '^browser: rename folder [0-9]*$' "New Folder… from the empty space gave no name field"
g=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: outline $f+\\[$c\\] $g+\\[\\]\$" \
  "New Folder… from the empty space didn't make a folder at the bottom: $(outline)"
send type read
send key return
expect "^browser: renamed folder $g read\$" "the folder from the empty space didn't take its name"
# A name moves no row, so there's nothing to wait for.
# A tab's menu has Add to Folder ›: New Folder, then the Space's folders. A folder takes the tab at
# its bottom.
post rightclick $(pinned_row 1)
expect "^browser: menu shown .*, $(cased 'Duplicate Tab, Add to Folder › New Folder'), \
$(cased 'Add to Folder') › work, $(cased 'Add to Folder') › read, $(cased 'Move to Space')" \
  "the tab's menu didn't list Add to Folder › New Folder, work and read"
send choose "$(cased 'Add to Folder') › read"
expect '^browser: command add-to-folder via tab menu$' "Add to Folder › read didn't run"
expect "^browser: outline $f+\\[\\] $g+\\[$c\\]\$" \
  "Add to Folder › read didn't move the tab in: $(outline)"
settled
../tools/window-screenshot.sh "$pid" "$out/add-to-folder.png" || fail "no window to capture"
# Add to Folder › New Folder puts the tab in a new folder in its place, named for its site.
host=$(printf '%s\n' "$site" | sed 's#^[a-z]*://##; s#^www\.##; s#[:/?#].*##')
post rightclick $(pinned_row 2)
expect "^browser: menu shown .*, $(cased 'Add to Folder › New Folder'), " "the tab's menu again"
send choose "$(cased 'Add to Folder › New Folder')"
expect '^browser: command new-folder via tab menu$' "Add to Folder › New Folder didn't run"
expect '^browser: rename folder [0-9]*$' "Add to Folder › New Folder gave no name field"
h=$(grep '^browser: rename folder' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: folder name field $host\$" \
  "the tab's folder wasn't named for $host: $(grep '^browser: folder name' "$log" | tail -1)"
expect "^browser: outline $f+\\[\\] $g+\\[$h+\\[$c\\]\\]\$" \
  "Add to Folder › New Folder didn't put the tab in a folder in its place: $(outline)"
settled
../tools/window-screenshot.sh "$pid" "$out/site-folder.png" || fail "no window to capture"
send key escape
# The tab shown two closed folders down is listed one under the outer one, the folder between
# hidden, and the outer folder's glyph has three dots (#1405). Captured before the expect on the
# dots, so a build without them shows the tab where it was.
send click $(pinned_row 3)
expect "^browser: selected $c in " "a click on the tab in the site's folder didn't show it"
send click $(pinned_row 2)
expect "^browser: outline $f+\\[\\] $g+\\[$h-\\[$c\\]\\]\$" \
  "a click on the site's folder didn't close it: $(outline)"
# Its tab stays listed where it was, so no row moves to wait for.
send click $(pinned_row 1)
expect "^browser: outline $f+\\[\\] $g-\\[$h-\\[$c\\]\\]\$" "a click on read didn't close it: $(outline)"
settled
../tools/window-screenshot.sh "$pid" "$out/folder-nested.png" || fail "no window to capture"
expect "^browser: folder $g dots over $c\$" \
  "read, closed over the site's closed folder, didn't draw its dots for the tab shown"
# With another tab shown it stays listed there, dots and all, so no row moves, and a click on read
# then closes it over the tab for good, as Arc's does, and the next one opens it (#1405).
switch "9 cmd" "$a"
../tools/window-screenshot.sh "$pid" "$out/folder-kept.png" || fail "no window to capture"
expect "^browser: folder $g keeps $c\$" "the tab shown in read didn't stay listed as another showed"
send click $(pinned_row 1)
expect "^browser: folder $g closed for good\$" "a click on read didn't close it over the tab kept"
none_since '^browser: command toggle-folder' || fail "a click on read closing it for good toggled it"
# The outline doesn't change, so settled would have nothing to wait for.
expect '^browser: rows settled$' "the tab kept in read didn't fold away"
../tools/window-screenshot.sh "$pid" "$out/folder-closed-for-good.png" || fail "no window to capture"
send click $(pinned_row 1)
expect "^browser: outline $f+\\[\\] $g+\\[$h-\\[$c\\]\\]\$" "a click on read didn't open it again: $(outline)"
settled
send click $(pinned_row 2)
expect "^browser: outline $f+\\[\\] $g+\\[$h+\\[$c\\]\\]\$" \
  "a click on the site's folder didn't open it again: $(outline)"
# Each is one step for ⌘Z, and the empty folder goes with Delete Folder.
send key z cmd
expect "^browser: outline $f+\\[\\] $g+\\[$c\\]\$" \
  "⌘Z didn't undo Add to Folder › New Folder: $(outline)"
send key z cmd
expect "^browser: outline $f+\\[$c\\] $g+\\[\\]\$" "⌘Z didn't undo Add to Folder › read: $(outline)"
settled
post rightclick $(pinned_row 2)
expect "^browser: menu shown $(cased 'Rename Folder'), " \
  "a right-click on the empty folder showed no menu"
send choose "$(cased 'Delete Folder')"
expect "^browser: outline $f+\\[$c\\]\$" \
  "Delete Folder didn't take the empty folder away: $(outline)"
settled
# The command bar finds commands by their names' letters in order, those that start with what's
# typed first, and one found by its letters alone last, after the suggestions. ↩ runs the command
# selected.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type clear
expect '^browser: command bar commands "clear": Clear Today, Clear Cache and Refresh, Clear Cookies and Refresh, Clear Archive, Clear Browsing Data…, Collapse All Folders$' "the command bar didn't find the five commands that start with \"clear\", then Collapse All Folders: $(grep '^browser: command bar commands' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/command-bar-commands.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar again"
send type cllps
expect '^browser: command bar commands "cllps": Collapse All Folders, Collapse Pinned Tabs$' "the command bar didn't find Collapse All Folders, then Collapse Pinned Tabs, for \"cllps\": $(grep '^browser: command bar commands' "$log" | tail -1)"
send key pagedown
expect '^browser: command bar selected Collapse Pinned Tabs$' "⇟ didn't select Collapse Pinned Tabs, the last row"
send key up
expect '^browser: command bar selected Collapse All Folders$' "↑ didn't select Collapse All Folders, the row above"
send key return
expect '^browser: command bar runs Collapse All Folders$' "↩ didn't run Collapse All Folders"
expect '^browser: command collapse-folders via command bar$' "Collapse All Folders didn't run collapse-folders"
expect "^browser: outline $f-\\[$c\\]\$" "Collapse All Folders didn't close the folder: $(outline)"
expect '^browser: new tab plate 0$' "running a command left New Tab's plate"
# Export Space as Markdown… saves the folder with its tab nested under it, the pinned tab's link to
# its pinned URL, then the Today tab. The run names the file, as it can't drive the save panel, and
# leaves out the Space's name and the pages' titles, which it doesn't choose.
bar_command export-space "Export Space as Markdown…" export
expect "^browser: exported 2 links to .*/space.md\$" \
  "Export Space as Markdown… didn't save the Space's two links"
exported=$(sed -e '1s/^# .*/#/' -e 's/\[.*\](/[](/' "$out/space.md")
[ "$exported" = "$(printf '#\n\n## Pinned\n\n- work\n  - []('"$site"')\n\n## Today\n\n- [](https://example.org/)')" ] ||
  fail "the Space wasn't saved as its folder, then its Today tab: $exported"
# Expand All Folders opens it again, and Collapse All Folders closes it for the launch that follows.
bar_command expand-folders "Expand All Folders" "expand all"
expect "^browser: outline $f+\\[$c\\]\$" "Expand All Folders didn't open the folder: $(outline)"
bar_command collapse-folders "Collapse All Folders" "collapse all"
expect "^browser: outline $f-\\[$c\\]\$" "Collapse All Folders didn't close the folder again: $(outline)"
# The preview coming and going, recorded for its motion: 0.45 s after the pointer comes onto the
# folder's row it fades in over 90 ms as it grows from 0.85 about the middle of its left edge, and
# 40 ms after the pointer leaves both it fades out over 80 ms (docs/ux-spec.md › Folders › Preview).
record folder-preview-motion 3
send move $(pinned_row 0)
expect "^browser: folder preview shown $f at " "resting on the closed folder didn't preview it to record"
send move $page_point
expect '^browser: folder preview hidden$' "the pointer leaving the folder's row didn't hide its preview"
recorded
# The pointer resting on the closed folder previews its tab with the keyboard in its search, where
# typing its title selects it and ↩ shows it, keeping the folder closed.
send move $(pinned_row 0)
expect "^browser: folder preview shown $f at " "resting on the closed folder didn't preview it"
../tools/window-screenshot.sh "$pid" "$out/folder-preview.png" || fail "no window to capture"
send type ready
expect '^browser: folder preview found 1 for ready$' "typing didn't reach the folder preview's search"
../tools/window-screenshot.sh "$pid" "$out/folder-preview-search.png" || fail "no window to capture"
# Its row slides in from under the closed folder over 0.15 s as it fades in (#1405), recorded.
record folder-peek-motion 2
send key return
expect "^browser: folder preview chose $c\$" "↩ in the folder's preview didn't choose the tab found"
expect '^browser: folder preview hidden$' "choosing a tab left the folder's preview up"
expect "^browser: selected $c in " "the tab chosen in the folder's preview didn't show"
recorded
# The folder staying closed logs no new outline, so the last one is the folder's state.
[ "$(outline)" = "$f-[$c]" ] || fail "showing a tab from the preview opened the folder: $(outline)"
# A click on its row chooses that tab as ↩ does. The pointer leaves the row and rests on it again.
send move $page_point
send move $(pinned_row 0)
expect "^browser: folder preview shown $f at " "resting on the closed folder again didn't preview it"
read -r px py pw _ << EOF
$(grep "^browser: folder preview shown $f at " "$log" | tail -1 | cut -d' ' -f7-)
EOF
# The first row's middle: under the search (48), its line (1) and the padding (4), half its fill.
send click $((px + pw / 2)) $((py + 75))
expect "^browser: folder preview chose $c\$" "a click on the folder preview's row didn't choose its tab"
expect '^browser: folder preview hidden$' "a click on the folder preview's row left it up"
send move $page_point
switch "9 cmd" "$a"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't reach the UI"
exited

# Launched again, the folder comes back closed with its tab. A double-click renames it, Export…
# writes it, and Delete Folder in its menu leaves its tab where it was.
log="$out/folder-restored.log"
step=0
# The run names the file Export… writes, as it can't drive the system's save panel.
rm -f "$out/spaces.md"
BROWSER_EXPORT_FILE="$(app_path "$PWD/$out/spaces.md")" BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Space"
await '^browser: outline [0-9]*-\[[0-9]*\]$' || fail "the folder didn't come back closed with its tab: $(outline)"
f=$(outline | cut -d- -f1)
c=$(outline | sed 's/.*\[\(.*\)\]/\1/')
# Ids are new after a launch.
await '^browser: today [0-9][0-9]*$' || fail "the Today tab didn't come back"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the restored folder. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The first click opens it.
send doubleclick $(pinned_row 0)
expect "^browser: rename folder $f\$" "a double-click on the folder didn't open its name field"
expect "^browser: outline $f+\\[$c\\]\$" "the double-click's first click didn't open the folder: $(outline)"
send type later
send key return
expect "^browser: renamed folder $f later\$" "the folder didn't take the name typed over its old one"
../tools/window-screenshot.sh "$pid" "$out/folder-restored.png" || fail "no window to capture"
post rightclick $(pinned_row 0)
# Paste URL as New Tab comes and goes with the clipboard, so it isn't pinned here.
# Move to Space is dimmed, as the pinned tab's is: this run has no other Space (#1353).
expect "^browser: menu shown $(cased 'Rename Folder, Change Icon…, Duplicate, Move to Space (disabled), Copy All Links'), .*$(cased 'Delete Folder')\$" \
  "the folder's menu isn't Rename Folder, Change Icon…, Duplicate, Move to Space, Copy All Links … Delete Folder"
expect '^browser: menu drawn ' "the folder's menu didn't draw"
../tools/window-screenshot.sh "$pid" "$out/folder-menu.png" || fail "no window to capture"
# With ⌥ held, Copy All Links is Copy All Links as Markdown, as in Arc's.
send flags alt
expect "^browser: menu drawn with ⌥ held $(cased 'Rename Folder, Change Icon…, Duplicate, Move to Space (disabled), Copy All Links as Markdown'), " \
  "⌥ didn't turn the folder's Copy All Links into Copy All Links as Markdown"
../tools/window-screenshot.sh "$pid" "$out/folder-menu-option.png" || fail "no window to capture"
send choose "$(cased 'Copy All Links')"
expect "^browser: menu chose $(cased 'Copy All Links as Markdown')\$" \
  "Copy All Links with ⌥ held didn't choose Copy All Links as Markdown"
expect '^browser: toast shown Copied Links as Markdown$' "Copy All Links as Markdown showed no toast"
send flags
# Change Icon… puts an emoji before the folder's name.
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Rename Folder, Change Icon…, Duplicate'), " \
  "a second right-click on the folder didn't show its menu"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for folder $f on emoji\$" "Change Icon… didn't open the icon picker for the folder"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker under the folder"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send insert glowing star
send key return
expect "^browser: iconed folder $f 🌟\$" "the picker's pick didn't become the folder's icon"
expect '^browser: icon picker hidden$' "picking an emoji didn't close the icon picker"
../tools/window-screenshot.sh "$pid" "$out/folder-icon.png" || fail "no window to capture"
# Export… in Settings › Import & Export, under its Export heading, writes the Space's pinned
# tabs as Markdown links, the folder a list of what it holds.
send key , cmd
expect '^browser: settings shown General$' "⌘, didn't show Settings on its General tab"
# The run's first ⌘, opens the window, which takes no click until it has drawn (#1101).
expect '^browser: settings drew General$' "Settings didn't draw General"
send clickkey $(drawn "toolbar Import & Export")
expect '^browser: settings tab Import & Export$' "a click on Import & Export didn't show its section"
settings_drew 'Import & Export'
# Copy All Links' toast is mostly still up, and the click lands all the same (#1134): a toast lets
# clicks through, but on its plate while it has buttons, which this one hasn't. A slow run says
# when the toast had gone first, so the click didn't have one to go past.
shown=$(grep -n '^browser: toast shown Copied Links as Markdown$' "$log" | tail -1 | cut -d: -f1)
section=$(grep -n '^browser: settings tab Import & Export$' "$log" | tail -1 | cut -d: -f1)
hidden=$(sed -n "${shown:-1},\$p" "$log" | grep -n '^browser: toast hidden$' | head -1 | cut -d: -f1)
if [ -n "$hidden" ] && [ $((${shown:-1} + hidden - 1)) -lt "$section" ]; then
  echo "skip: the click on Import & Export with a toast up, as Copy All Links' toast had gone first (#1134)."
fi
click_in_sight "export spaces"
expect '^browser: exported [0-9]* links to .*/spaces\.md$' "Export… didn't write the Spaces' links"
grep -qx -- '- later' "$out/spaces.md" ||
  fail "the export didn't list the folder: $(cat "$out/spaces.md")"
grep -qx -- "  - \[.*\]($site)" "$out/spaces.md" ||
  fail "the export didn't list the pinned tab in its folder: $(cat "$out/spaces.md")"
../tools/window-screenshot.sh "$pid" "$out/export.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Rename Folder, Change Icon…, Remove Icon, Duplicate'), .*$(cased 'Delete Folder')\$" \
  "the folder with an icon didn't offer Remove Icon"
send choose "$(cased 'Delete Folder')"
expect '^browser: command delete-folder via tab menu$' "Delete Folder didn't run delete-folder"
expect "^browser: outline $c\$" "Delete Folder didn't leave its tab in its place: $(outline)"
# The toast offers Undo; ⌘Z remakes the folder around its tab, under the name it had, and ⇧⌘Z
# deletes it again.
expect '^browser: toast shown Folder Deleted \[undo\]$' "Delete Folder didn't offer Undo"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Delete Folder"
expect "^browser: outline [0-9][0-9]*[-+]\\[$c\\]\$" "⌘Z didn't put the folder back around its tab: $(outline)"
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Delete Folder"
expect "^browser: outline $c\$" "⇧⌘Z didn't delete the folder again: $(outline)"

# A file dragged over the pinned tab gets a line above it, and let go, it opens there pinned and
# shows, titled by its name.
t=$(today)
../tools/window-screenshot.sh "$pid" "$out/drop-before.png" || fail "no window to capture"
# As Explorer would drag it on Windows, D:/a/..., which Chrome's file URL finds (#997).
send dragover 100 150 file "$(app_path "$(cd "$out" && pwd)/site/notes.txt")"
expect '^browser: drag over HostUIView taken$' "the list didn't take a file dragged over it"
expect '^browser: drop line in pinned 0$' "no line marked the file's place above the pinned tab"
../tools/window-screenshot.sh "$pid" "$out/drop-line.png" || fail "no window to capture"
send dragdrop 100 150
expect '^browser: drag dropped$' "the list didn't take the file let go over it"
expect '^browser: dropped 1 in pinned 0$' "the file didn't drop above the pinned tab"
expect_lists "[0-9][0-9]* $c" "$t" "after dropping a file"
e=$(pinned | cut -d' ' -f1)
expect "^browser: selected $e\$" "the dropped file's tab didn't show"
expect "^browser: loaded $e file:///.*/site/notes.txt\$" "the dropped file didn't load in its tab"
# A link let go under the Today tab, whose row is centred at 292 under the two pinned tabs', the
# divider and New Tab's, opens there.
send dragdrop 100 330 url "${site}notes.txt"
expect '^browser: dropped 1 in today 1$' "the link didn't drop under the Today tab"
expect_lists "$e $c" "$t [0-9][0-9]*" "after dropping a link"
d=$(today | cut -d' ' -f2)
expect "^browser: selected $d\$" "the dropped link's tab didn't show"
expect "^browser: loaded $d ${site}notes.txt\$" "the dropped link didn't load in its tab"
../tools/window-screenshot.sh "$pid" "$out/dropped.png" || fail "no window to capture"
# Text that isn't a web link isn't taken, and nothing opens.
send dragover 100 300 text "javascript:document.title='dropped'"
expect '^browser: drag over HostUIView refused$' "the list took a javascript: URL dragged as text"
left=$((step + 1))
send dragleave
# Other text, let go past the middle of the dropped link's row, centred at 333, goes in the gap
# under it, the nearest, and is searched for there with the default engine, its spaces run
# together.
send dragdrop 100 345 text "dogs  and   cats"
expect '^browser: dropped 1 in today 2$' "the dropped text wasn't taken under the dropped link"
[ "$(since "$left" | grep -c '^browser: dropped')" = 1 ] || fail "dragged text that isn't a web link opened"
expect_lists "$e $c" "$t $d [0-9][0-9]*" "after dropping text"
s=$(today | cut -d' ' -f3)
expect "^browser: selected $s\$" "the dropped text's search didn't show"
searching 'dogs+and+cats' \
  "the dropped text didn't start a search for it: $(grep "^browser: page $s " "$log" | tail -1)" "$s"
../tools/window-screenshot.sh "$pid" "$out/dropped-text.png" || fail "no window to capture"

# Under the pointer the Space's name row lights as a tab row does, with its chevron after the name
# (in the icon's slot with an icon) and its ••• at its end, and a press anywhere on it but the •••
# collapses the pinned rows, the chevron turning a quarter, and the name lights (Arc 1.171): they
# fold away, but for the tab shown's, as a closed folder's do, and ⌘1–⌘9 count Today's tabs first.
# The command bar's Expand Pinned Tabs brings them back, and View › Collapse Pinned Tabs folds them
# again.
# The middle of the chevron the app last drew for pinned rows open or shut ($1).
caret() {
  grep "^browser: pinned caret $1 at " "$log" | tail -1 |
    awk '{ printf "%d %d", $6 + $8 / 2, $7 + $9 / 2 }'
}
[ -n "$(caret open)" ] || fail "the Space with pinned tabs drew no chevron after its name"
away
send move $(caret open)
expect '^browser: sidebar hovered 1$' "the sidebar didn't see the pointer come over the chevron"
expect '^browser: space name hovered$' "the Space's name row didn't light under the pointer"
settled
../tools/window-screenshot.sh "$pid" "$out/pinned-caret.png" || fail "no window to capture"
# On the row past the name, not the chevron.
name_y=$(caret open | cut -d' ' -f2)
send move 150 "$name_y"
record pinned-turn 2
send click 150 "$name_y"
expect '^browser: command collapse-pinned via sidebar$' "a click on the Space's name row didn't run collapse-pinned"
expect "^browser: outline -\\[$e $c\\]\$" "the name row didn't collapse the pinned rows: $(outline)"
expect '^browser: pinned caret shut at ' "the chevron didn't turn for collapsed rows"
recorded
away
expect '^browser: space name left$' "the Space's name row stayed lit with the pointer away"
settled
../tools/window-screenshot.sh "$pid" "$out/pinned-collapsed.png" || fail "no window to capture"
switch "1 cmd" "$t"
bar_command expand-pinned "Expand Pinned Tabs" "expand pinned"
expect "^browser: outline $e $c\$" "Expand Pinned Tabs didn't bring the pinned rows back: $(outline)"
expect '^browser: pinned caret open at ' "the chevron didn't turn back for open rows"
settled
send menu Collapse Pinned Tabs
expect '^browser: command collapse-pinned via ui$' "View › Collapse Pinned Tabs didn't run collapse-pinned"
expect "^browser: outline -\\[$e $c\\]\$" "Collapse Pinned Tabs didn't collapse the pinned rows: $(outline)"

# Focus Sidebar, on F6 unless the user gives it another key, gives the sidebar the keyboard and rings
# the tab shown's row, the first ⌘1 counts. ↑ there stays, past the collapsed pinned rows, ↓ moves
# the ring down, and return shows the ringed tab with its page holding the keyboard. From the View
# menu, a key in the page clicked ends the ring, and escape gives the page the keyboard back
# without showing another tab.
bar_command focus-sidebar "Focus Sidebar" "focus sidebar"
expect "^browser: sidebar keyed $t\$" "Focus Sidebar didn't ring the tab shown's row"
expect "^browser: sidebar ring $t at " "the ring wasn't drawn round the tab shown's row"
in_browser AccessKitSubclassOfGPUIView "Focus Sidebar didn't give the sidebar the keyboard"
settled
../tools/window-screenshot.sh "$pid" "$out/sidebar-keyed.png" || fail "no window to capture"
up=$((step + 1))
send key up
send key down
expect "^browser: sidebar keyed $d\$" "↓ didn't move the ring to the next row"
[ "$(since "$up" | grep -c '^browser: sidebar keyed')" = 1 ] ||
  fail "↑ from the first row moved the ring, onto a collapsed row"
expect "^browser: sidebar ring $d at " "the ring wasn't drawn round the next row"
../tools/window-screenshot.sh "$pid" "$out/sidebar-keyed-down.png" || fail "no window to capture"
send key return
expect '^browser: command select via sidebar keys$' "return didn't show the ringed tab"
expect "^browser: selected $d\\( in .*\\)\\{0,1\\}\$" "return didn't show tab $d"
expect '^browser: sidebar ring hidden$' "the ring stayed after return"
in_browser RenderWidgetHostViewCocoa "return didn't give the page the keyboard"
send menu Focus Sidebar
expect '^browser: command focus-sidebar via ui$' "View › Focus Sidebar didn't run focus-sidebar"
expect "^browser: sidebar keyed $d\$" "View › Focus Sidebar didn't ring the tab shown's row"
send click $page_point
send key down
expect '^browser: sidebar keys left by page$' "a key in the page left the ring in the sidebar"
expect '^browser: sidebar ring hidden$' "the ring stayed with the page's keyboard"
send menu Focus Sidebar
expect "^browser: sidebar keyed $d\$" "Focus Sidebar didn't ring the row again"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't end the ring"
none_since '^browser: selected' || fail "escape showed another tab"
in_browser RenderWidgetHostViewCocoa "escape didn't give the page the keyboard"
# ⇧F10 opens the ringed row's menu, its first item highlighted, and escape closes it with the ring
# kept. ⌘W then closes the ringed row, the last, rather than the tab shown, and the ring goes to
# the row before it.
send menu Focus Sidebar
expect "^browser: sidebar keyed $d\$" "Focus Sidebar didn't ring the tab shown's row again"
send key f10 shift
expect "^browser: menu shown $(cased 'Copy Link, ')" "⇧F10 didn't open the ringed row's menu"
expect '^browser: menu highlights .* at ' "the row's menu from ⇧F10 didn't highlight its first item"
send key escape
expect '^browser: menu closed$' "escape didn't close the row's menu"
none_since '^browser: sidebar keys left' || fail "closing the row's menu ended the ring"
send key down
expect "^browser: sidebar keyed $s\$" "↓ didn't move the ring to the last row"
send key w cmd
expect '^browser: command close-tab via sidebar keys$' "⌘W didn't reach the ringed row"
expect "^browser: today $t $d\$" "⌘W didn't close the ringed row $s: Today is \"$(today)\""
expect "^browser: sidebar keyed $d\$" "the ring didn't go to the row before the closed one"
none_since '^browser: selected' || fail "⌘W on the ringed row showed another tab"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't end the ring after ⌘W"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't reach the UI"
exited

# Launched again, the pinned rows come back collapsed, with the chevron, which expands them.
log="$out/pinned-collapsed.log"
step=0
# It names the file the +'s Add File or Folder… adds, as it can't drive the system's open panel.
notes="$(cd "$out" && pwd)/Notes"
BROWSER_ADD_PATHS="$notes/A.txt" BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Space"
await '^browser: outline -\[[0-9]* [0-9]*\]$' || fail "the pinned rows didn't come back collapsed: $(outline)"
await '^browser: pinned caret shut at ' || fail "the collapsed rows came back without the chevron"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the restored pinned rows. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
send click $(caret shut)
expect '^browser: command expand-pinned via sidebar$' "a click on the chevron didn't run expand-pinned"
expect '^browser: outline [0-9]* [0-9]*$' "the chevron didn't expand the restored pinned rows: $(outline)"
settled
../tools/window-screenshot.sh "$pid" "$out/pinned-expanded.png" || fail "no window to capture"
# A folder from the desktop dropped above the pinned tabs becomes a folder named for it, open,
# holding its folder, closed, then a tab for each of its files by name, its hidden file left out,
# and none of them shows.
e=$(pinned | cut -d' ' -f1)
c=$(pinned | cut -d' ' -f2)
rm -rf "$notes"
mkdir -p "$notes/sub"
for file in b.txt A.txt .hidden sub/c.txt; do
  echo "$file" > "$notes/$file"
done
send dragdrop 100 150 file "$(app_path "$notes")"
expect '^browser: dropped 1 in pinned 0$' "the folder didn't drop above the pinned tabs"
expect '^browser: folder [0-9]* from disk 1 2 Notes$' \
  "the dropped folder didn't come in with its folder and two files, the hidden one left out"
expect '^browser: folder [0-9]* from disk 0 1 sub$' "the dropped folder's folder didn't come in"
n=$(grep '^browser: folder [0-9]* from disk 1 2 Notes$' "$log" | tail -1 | cut -d' ' -f3)
m=$(grep '^browser: folder [0-9]* from disk 0 1 sub$' "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: outline $n+\\[$m-\\[[0-9]*\\] [0-9]* [0-9]*\\] $e $c\$" \
  "the dropped folder isn't an open folder of its files above the pinned tabs: $(outline)"
none_since '^browser: selected' || fail "a file of the dropped folder showed"
settled
../tools/window-screenshot.sh "$pid" "$out/dropped-folder.png" || fail "no window to capture"
# The dropped file's row, under the folder's four, dragged onto the page hands the file to the
# system's drag, to upload it there, as a download's row does, and goes back to its place.
press=$((step + 1))
send press $(pinned_row 4)
send dragto 150 $(($(pinned_row 4 | cut -d' ' -f2) + 10))
send dragto 600 300
expect "^browser: tab $e file drag\$" \
  "a file's row dragged onto the page didn't start the system's drag"
send release 600 300
# No real button holds the system's drag on the Mac or Linux, so it may end before the release
# reaches it. On Windows the driver holds it.
for _ in $(seq 100); do
  sed -n "/^browser: drive $press: /,\$p" "$log" | grep -q '^browser: file drag ended ' && break
  sleep 0.1
done
sed -n "/^browser: drive $press: /,\$p" "$log" | grep -q '^browser: file drag ended ' ||
  fail "the file row's drag didn't end"
case "$(outline)" in
  "$n+["*"] $e $c") ;;
  *) fail "the file's row didn't go back to its place: $(outline)" ;;
esac
# The +'s menu ends with Add File or Folder… (Add Files… and Add Folder… off the Mac, whose
# panels pick one kind), whose file, named at launch, opens at the top of Today and shows. Today is
# read here, as the drag before may have dropped its file on the page, which opens it as a tab,
# and so whether the page shown offers New Boost… (boosts.sh checks that).
was=$(today)
send layout
height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
post click 204 $((height - 25))
add=$(plus_adds | cut -d, -f1)
plus="New Space…, New Folder…, New Split View, New Note, \\(New Boost…, \\)\\{0,1\\}$(plus_adds)"
expect "^browser: menu shown $(cased "$plus")\$" \
  "the +'s menu didn't end with $(plus_adds)"
../tools/window-screenshot.sh "$pid" "$out/plus-add.png" || fail "no window to capture"
send choose "$(cased "$add")"
expect "^browser: footer $add\$" "$add in the +'s menu didn't run"
expect '^browser: added 1 files 0 folders$' "$add didn't add the file named"
expect "^browser: today [0-9][0-9]* $was\$" "the added file didn't open at the top of Today"
added=$(today | cut -d' ' -f1)
expect "^browser: selected $added\$" "the added file's tab didn't show"
expect "^browser: loaded $added file:///.*/Notes/A.txt\$" "the added file didn't load in its tab"
exec 3>&-
quit
echo "ok: ⌘D pins and unpins from the page and the UI, and ⌘1–⌘9 count pinned tabs first. A middle-click on a row closes its tab as its ✕ or − does. Dragging a row moves its tab in the pinned tabs, pins it and unpins it, without showing it, and escape puts it back. ⌘W and − unload a pinned tab, shown or not, and keep its row, showing the top Today tab, else a pinned tab still loaded, else no page, and an unloaded pinned tab loads once shown. An unloaded pinned row's remove button, named by its tooltip, takes the tab out of the pinned tabs without loading it and not into Today, offering Undo in a toast, and ⌘Z pins it again at its place, while a second click on the − that unloaded it shows the tab. Away from its pinned URL, ⌘↩ from the page and the UI, and a click on its favicon, go back there. Pinned tabs come back pinned, unloaded and at their page, still away. New Folder makes a folder its field names, a drag right and left takes a tab in and out of it, a folder drags as a row, a click closes and opens it, and the command bar finds Collapse All Folders, which closes it, and Expand All Folders, which opens it. A tab shown two closed folders down is listed one under the outer one, whose glyph has dots, and stays listed as another shows until a click on the outer one closes it for good. Resting on it closed previews its tab, and typing the tab's title in the preview's search and ↩ show it, the folder kept closed. It comes back closed, a double-click renames it, its menu's Copy All Links is Copy All Links as Markdown with ⌥ held, which copies them, Change Icon… gives it an emoji and offers Remove Icon after, and Delete Folder leaves its tab in place. A link and a file dragged onto the list open as tabs where they're let go, Today or pinned, and show, text that isn't a link opens a search for it there, and a link that isn't a web link doesn't open. Delete Folder offers Undo, ⌘Z remakes the folder around its tab and ⇧⌘Z deletes it again. Settings' Export… writes the folder and its pinned tab as Markdown links. ⌥-click on a pinned tab's link opens a pinned split. Away from its pinned URL, Pinned URL › Replace Pinned URL with Current and Edit… change the pinned URL, which ⌘Z and ⇧⌘Z undo and redo, and ⌘-click on its favicon goes back after opening the page in a new tab. A press on the Space's name row, lit under the pointer with its chevron and •••, collapses the pinned rows, the chevron turning, which ⌘1 then skips, Expand Pinned Tabs and Collapse Pinned Tabs in the command bar and the View menu bring them back and fold them, and they come back collapsed. Focus Sidebar from the command bar and the View menu rings the tab shown's row, ↑ and ↓ move the ring past the collapsed rows, return shows its tab with the page holding the keyboard, and a key in the page and escape end it. A folder from the desktop dropped on the list becomes an open folder of its folder, closed, and its files' tabs, none shown, a file's row dragged onto the page starts the system's drag of its file, and the +'s Add File or Folder… opens a file at the top of Today. Windows: $out/pinned-close.png, $out/pinned-remove.png, $out/pinned-removed.png, $out/pinned-remove-undone.png, $out/pinned-split.png, $out/pinned-url-menu.png, $out/pinned-url-edit.png, $out/pinned-kept-page.png, $out/drag.png, $out/pinned.png, $out/pinned-unloaded.png, $out/pinned-away.png, $out/pinned-restored.png, $out/folder.png, $out/folder-nested.png, $out/folder-kept.png, $out/folder-closed-for-good.png, $out/folder-preview.png, $out/folder-preview-search.png, $out/folder-peek-motion.mp4, $out/folder-restored.png, $out/export.png, $out/folder-menu.png, $out/folder-menu-option.png, $out/folder-icon.png, $out/folder-motion.mp4, $out/drop-before.png, $out/drop-line.png, $out/dropped.png, $out/dropped-text.png, $out/command-bar-commands.png, $out/pinned-caret.png, $out/pinned-collapsed.png, $out/sidebar-keyed.png, $out/sidebar-keyed-down.png, $out/pinned-expanded.png, $out/dropped-folder.png, $out/plus-add.png"
