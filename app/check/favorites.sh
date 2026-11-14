# check.sh runs this at 160.
# A Today tab's menu is Arc's, whose Copy Link is Copy Link as Markdown with ⌥ held, and its row
# dragged onto the Space's name takes the tab out of Today into a tile above the Space's rows, its
# press having shown it. Resting on the tile shows its hover card, which other apps don't take for
# the window in front, and the footer's Library button
# and the header's sidebar button name themselves with their shortcuts' keys on chips, and Clear
# says what it does. A click on the tile shows it, and ⌘1–⌘9 count favorites first. Past its top, the list shows an
# edge under the tile, and showing a tab under its bottom scrolls to the tab's row. Scrolled out of
# the list, the shown tab's row has a marker at the edge it's past, which scrolls back to it. Away
# from its URL, a double-click on the tile goes back there. ⌥-click on a Today row with it shown
# joins that tab to a copy of it in Today, which ⌘Z closes. ⌘W unloads it and keeps the tile,
# showing the top Today tab. The tile's menu hangs the icon picker under the tile, whose pick is the
# tile's icon until Remove Icon, and duplicates it to the top of Today. Launched again, the app
# brings the favorite back, shown, with the icon it was given before the quit, and Remove from
# Favorites in its menu puts it at the top of Today. A row dragged onto the tiles turns into a tile, and a tile dragged into the list
# into a row. A split's row dragged onto the tiles is one favorite, shown focused where it was
# focused last, with its pages on its card, and Remove from Favorites puts it whole in Today. Today rows dragged onto the tiles become favorites where they're dropped, a tile
# dragged across the other goes before it, one dragged into the list goes there, and escape puts a
# dragged tile back. Tiles taller than 40 % of the window scroll with the list. ⌘, from the page and
# the UI opens Settings, where switches turn the warning before quitting and the haptics off and the
# menus of two pop-up buttons set how long Today tabs wait for the archive and who shares the
# favorites (Each Space, kept across the quit and its favorite with it, then Each Profile's Spaces
# again), and all of them stay after the quit, when ⌘Q quits without asking. ⌘W and the window's close button hide it, and More
# Settings… opens Chromium's settings in a tab. A Space keeps more than the 12 favorites it shows
# at most from a list saved before there was a most, and with 12 a row dragged over the tiles stays
# a row and a pinned tab's menu dims Add to Favorites.

# Favorites, from the served page with example.org above it.
serve
log="$out/favorites.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the favorites test page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: favorites. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The favorites the app last showed, left to right.
favorites() {
  grep '^browser: favorites' "$log" | tail -1 | sed 's/^browser: favorites *//'
}
# One favorite is one tile across the sidebar, 47 tall and 90 below the window's top, under the URL
# field, and its row of tiles takes the Space's rows 55 down, with the 8 under it.
tile="114 114"
tiled_row() {
  echo "100 $(($(row "$1" | cut -d' ' -f2) + 55))"
}
open_from_bar example.org
b=$tab
expect_tabs "$b $a" "$b" "after opening a tab"
# A Today tab's menu is Arc's, which has no Add to Favorites, and the bottom row's dims Archive Tabs
# Below. Dragged onto the Space's name, the row takes the tab out of Today into a tile, and its
# press shows the tab, as Arc's does.
post rightclick $(row 1)
expect "^browser: menu shown $(cased "Copy Link$share, Change Icon…, Rename…, Mute (disabled), New Boost…, Add Split View, Duplicate, Add to Folder › New Folder, Move to › [^,]* (checked), Move to › New Space…, Archive Tab, Archive Tabs Below (disabled)")\$" \
  "a right-click on the bottom Today row didn't show Arc's tab menu"
expect '^browser: menu drawn ' "the Today tab's menu didn't draw"
# With ⌥ held, its Copy Link is Copy Link as Markdown, as Arc's menu copies a link as Markdown.
send flags alt
expect "^browser: menu drawn with ⌥ held $(cased 'Copy Link as Markdown'), " \
  "⌥ didn't turn the tab's Copy Link into Copy Link as Markdown"
../tools/window-screenshot.sh "$pid" "$out/tab-menu-option.png" || fail "no window to capture"
send choose "$(cased 'Copy Link')"
expect "^browser: menu chose $(cased 'Copy Link as Markdown')\$" \
  "Copy Link with ⌥ held didn't choose Copy Link as Markdown"
expect "^browser: copied \[[^]]*\]($site)\$" \
  "Copy Link as Markdown didn't copy a Markdown link to the tab: \
$(grep '^browser: copied' "$log" | tail -1)"
send flags
send press $(row 1)
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "pressing the row to drag didn't show its tab"
send dragto 100 200
send dragto 114 112
expect '^browser: drag shape tile$' "a row dragged onto the Space's name didn't turn into a tile"
send release 114 112
released=$step
expect "^browser: dragged $a to favorites 0\$" "the row dragged onto the Space's name didn't go there"
expect "^browser: favorites $a\$" \
  "the row dragged onto the Space's name didn't make the tab a favorite: \"$(favorites)\""
expect "^browser: today $b\$" "the favorite is still in Today: Today is \"$(today)\""
# The tile has only an icon, so after a moment over it its hover card names the favorite, and it
# goes when the pointer leaves and when the tile is clicked. The pointer let go over the new tile,
# which the drag had hovered without a card, so its card comes once the tile has settled, which may
# be before the move onto it. It's the tab shown, so its card has no picture, and a favorite's only
# button takes it out of the favorites.
send move $tile
expect "^browser: hover card shown $a unfavorite none\$" \
  "resting on the tile the row was dropped on didn't show its card" "$released"
send move 100 400
expect '^browser: hover card hidden$' "the pointer leaving the tile didn't hide its card"
# The press showed the favorite, so the click on its tile below has a tab to change from.
switch "2 cmd" "$b"
send move $tile
expect "^browser: hover card shown $a unfavorite \(picture\|favicon\)\$" \
  "resting on the tile again didn't show its card"
expect '^browser: hover card window shown$' "the tile's card didn't show"
expect '^browser: hover card settled$' "the tile's card didn't come to rest"
# It starts at the tile's top, 90 down, right of the tile's middle, beside the rows, and the window
# the host put over the page is where the UI asked.
since_move() {
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep "$1" | tail -1
}
asked=$(since_move '^browser: hover card at ')
placed=$(since_move '^browser: hover card window at ')
echo "$asked $placed" | awk '
  { across = $5 - $14; down = $6 - $15; top = $6 - 90 }
  END { exit !(NR == 1 && NF == 15 && $5 > 114 && across > -1 && across < 1 \
    && down > -1 && down < 1 && top > -1 && top < 1) }' \
  || fail "the card isn't beside the tile: asked \"$asked\", placed \"$placed\""
../tools/window-screenshot.sh "$pid" "$out/favorite-card.png" || fail "no window to capture"
# Another app asking which window is in front, as T3 Code's window capture does, gets the browser
# window, not the card over it or the command bar's shadow.
send front
expect '^browser: front driven, main driven, key driven, ax focused driven, ax main driven$' \
  "with the card showing, other apps take the wrong window: $(grep '^browser: front ' "$log" | tail -1)"
# A click on the tile shows the favorite, and ⌘1–⌘9 count favorites first.
send click $tile
expect '^browser: command select via favorite$' "a click on the tile didn't run"
expect '^browser: hover card hidden$' "a click on the tile didn't hide its card"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the tile didn't show the favorite"
../tools/window-screenshot.sh "$pid" "$out/favorites.png" || fail "no window to capture"
# The footer's Library button names itself beside it, with its shortcut's keys on chips (Arc), and
# the header's sidebar button under it; each goes when the pointer leaves.
check_layout "before the buttons' tooltips"
send move 24 $((height - 25))
# Off the Mac the keys are named as Chrome names them there.
library_keys=⇧⌘L sidebar_keys=⌘S
[ "$(uname)" = Darwin ] || { library_keys=Ctrl+Shift+L; sidebar_keys=Ctrl+S; }
expect "^browser: tooltip shown View Library $library_keys\$" \
  "resting on the Library button didn't show View Library and $library_keys"
expect '^browser: tooltip window shown$' "the Library button's tooltip didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/library-tooltip.png" || fail "no window to capture"
send move 100 400
expect '^browser: tooltip hidden$' "the pointer leaving the Library button didn't hide its tooltip"
send move 93 23
expect "^browser: tooltip shown Hide Sidebar $sidebar_keys\$" \
  "resting on the sidebar button didn't show Hide Sidebar and $sidebar_keys"
expect '^browser: tooltip window shown$' "the sidebar button's tooltip didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/header-tooltip.png" || fail "no window to capture"
send move 100 400
expect '^browser: tooltip hidden$' "the pointer leaving the sidebar button didn't hide its tooltip"
# Clear, at the end of the divider over Today, says what it does in Arc's words, under it.
send move 200 191
expect '^browser: clear hovered 1$' "Clear didn't see the pointer come over it"
expect '^browser: tooltip shown Clear unpinned tabs in this space$' \
  "resting on Clear didn't show its tooltip"
expect '^browser: tooltip window shown$' "Clear's tooltip didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/clear-tooltip.png" || fail "no window to capture"
send move 100 400
expect '^browser: tooltip hidden$' "the pointer leaving Clear didn't hide its tooltip"
send click $(tiled_row 0)
expect "^browser: selected $b\( in .*\)\{0,1\}\$" \
  "a click on the Today row under the tile didn't show its tab"
switch "1 cmd" "$a"
# Away from its URL, a double-click on the tile goes back there.
tab=$a
go_from_bar example.com
expect "^browser: page $a away 1\$" "the favorite at another page didn't read as away from its URL"
send doubleclick $tile
expect '^browser: command back-to-pinned-url via favorite$' \
  "a double-click on the tile didn't run Back to Favorite URL"
expect "^browser: loaded $a $site\$" "a double-click on the tile didn't load the favorite's URL"
expect "^browser: page $a away 0\$" "the favorite still read as away at its URL"
# ⌥-click on a Today row with the favorite shown joins that tab to a copy of the favorite at the
# top of Today, as no Space holds a favorite (Arc), and ⌘Z closes the copy and puts the tab back.
send click $(tiled_row 0) alt
expect '^browser: command split via sidebar$' "⌥-click on the Today row didn't reach the sidebar"
expect "^browser: splits [0-9][0-9]*+$b\$" \
  "the Today tab didn't join a copy of the favorite: $(grep '^browser: splits' "$log" | tail -1)"
[ "$(favorites)" = "$a" ] || fail "joining the favorite took its tile: \"$(favorites)\""
../tools/window-screenshot.sh "$pid" "$out/favorite-split.png" || fail "no window to capture"
send key z cmd
undone=$step
expect '^browser: splits $' "⌘Z didn't undo the split with the favorite's copy"
expect "^browser: today $b\$" "⌘Z didn't close the copy and put the tab back: \"$(today)\""
# The copy closing may have shown the favorite already.
send click $tile
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "the favorite isn't shown again" "$undone"
# ⌘W unloads a favorite and keeps its tile, showing the tab shown before it and ringing its row,
# and ⌘1 loads it again.
send click $page_point
send key w cmd
expect "^browser: selected $b in " "⌘W on a favorite didn't show the tab shown before it"
expect "^browser: sidebar ring $b at " "⌘W on a favorite didn't ring the row of the tab it showed"
expect "^browser: page $a unloaded 1\$" "⌘W on a favorite didn't unload it"
none_since "^browser: tab $a closed\$" || fail "⌘W closed a favorite"
[ "$(favorites)" = "$a" ] ||
  fail "⌘W on a favorite took its tile: the favorites are \"$(favorites)\""
../tools/window-screenshot.sh "$pid" "$out/favorite-unloaded.png" || fail "no window to capture"
switch "1 cmd" "$a"
expect "^browser: loaded $a $site\$" "the unloaded favorite didn't load once shown"
# Change Icon… in the tile's menu hangs the icon picker under the tile, its arrow at the tile's
# middle, and the emoji picked is the tile's icon, until Remove Icon takes it out (Arc).
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the tile didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for tab $a on emoji\$" \
  "Change Icon… on the tile didn't open the icon picker for the favorite"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker under the tile"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
# The arrow's tip is 13 in from the picker's left, 12 under the tile's middle at $tile.
at=$(grep '^browser: icon picker at' "$log" | tail -1 | cut -d' ' -f5-)
echo "$at $tile" | awk '{ x = $1 + 13 - $5; y = $2 - 12 - $6; exit !(x > -2 && x < 2 && y > -2 && y < 2) }' ||
  fail "the icon picker opened at $at, not under the tile's middle at $tile"
../tools/window-screenshot.sh "$pid" "$out/favorite-icon-picker.png" || fail "no window to capture"
# Under KWin it asks for the blur behind it that the Mac's material has, and stops as it goes.
[ "${BROWSER_XVFB:-}" != kwin ] || kwin_blurs ||
  fail "the icon picker's window didn't ask KWin to blur under it"
send insert glowing star
send key return
expect "^browser: page $a icon 🌟\$" "the picker's pick didn't become the favorite's icon"
expect '^browser: icon picker hidden$' "picking an emoji didn't close the icon picker"
[ "${BROWSER_XVFB:-}" != kwin ] || ! kwin_blurs ||
  fail "a window still asked KWin to blur under it after the icon picker went"
expect "^browser: tile $a drawn with icon 🌟\$" "the tile didn't draw the favorite's new icon"
../tools/window-screenshot.sh "$pid" "$out/favorite-icon.png" || fail "no window to capture"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Remove Icon, Close Tab')\$" \
  "the favorite with an icon didn't offer Remove Icon"
send choose "$(cased 'Remove Icon')"
expect "^browser: page $a icon none\$" "Remove Icon didn't take the favorite's icon out"
expect "^browser: tile $a drawn with icon none\$" "the tile didn't draw its favicon again"
# The tile's menu. Duplicate Tab opens a copy at the top of Today and shows it.
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the tile didn't show the favorite's menu"
send choose "$(cased 'Duplicate Tab')"
expect '^browser: command duplicate-tab via tab menu$' "Duplicate Tab didn't run"
expect "^browser: today [0-9][0-9]* $b\$" \
  "Duplicate Tab didn't open a copy at the top of Today: Today is \"$(today)\""
c=$(today | cut -d' ' -f1)
expect "^browser: selected $c\$" "Duplicate Tab didn't show the copy"
expect "^browser: loaded $c $site\$" "the copy didn't load the favorite's page"
# The row under the copy slides down to make room, and until it's there the list is only as long as
# where the row is drawn, too short to scroll in the window below.
expect '^browser: rows settled$' "the row under the copy didn't settle"
# Past its top in a window too short for its rows, the list shows a line across the sidebar under
# the Space's name, which stays put, 2 over the rows' top, and back at the top it hides it. The
# knob comes at the sidebar's edge as the list scrolls, and fades once it has stopped (Arc 1.170),
# or with the system's scroll bars shown always, as the hosted runners have them, it's there while
# the rows overflow.
legacy=0
grep -q '^browser: scroll bars always$' "$log" && legacy=1
send layout
size=$(grep '^browser: layout content' "$log" | tail -1 |
  sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
send resize 1000 340
# The list scrolls as far as the last frame drawn has it.
expect '^browser: drawn at 1000x340$' "the window didn't draw at its new size"
# Rows hidden under the list's foot bring a line across the sidebar 1 under it (Arc).
expect '^browser: scroll foot edge shown$' "the rows under the list's foot didn't show its edge"
# The overlay knob holds 0.8 s after the last scroll, less than a still can take to be captured,
# so the scroll is recorded for a look at the knob (scroller.mp4).
record scroller 2
send scroll 100 200 10 5
expect '^browser: scroll edge shown$' "the list scrolled past its top didn't show its edge"
if [ $legacy = 1 ]; then
  grep -q '^browser: legacy scroller shown$' "$log" || fail "the rows overflowing didn't show the knob"
else
  expect '^browser: scroller shown$' "the list scrolled didn't show its knob"
fi
recorded
../tools/window-screenshot.sh "$pid" "$out/scroll-edge.png" || fail "no window to capture"
# Near each row's top, as Windows' capture is a row taller than the window.
edge=$(pixel "$out/scroll-edge.png" 0.1 $(awk 'BEGIN { print 187.2 / 340 }'))
gap=$(pixel "$out/scroll-edge.png" 0.1 $(awk 'BEGIN { print 185.2 / 340 }'))
# A light sidebar, as an opaque window's is, draws its edge darker.
brighter "$edge" "$gap" || brighter "$gap" "$edge" ||
  fail "the edge is $edge over the sidebar's $gap"
[ $legacy = 1 ] || expect '^browser: scroller hidden$' "the knob stayed once the list had stopped"
send scroll 100 200 -10 5
expect '^browser: scroll edge hidden$' "the list back at its top didn't hide its edge"
# There, showing the last tab glides the list the least that shows its row whole, recorded for
# its motion, and showing a tab whose row is in view leaves the list where it is.
record list-glide 2
switch "9 cmd" "$b"
expect "^browser: list scrolled to [1-9][0-9.]* for $b\$" \
  "showing a tab under the list's bottom didn't scroll to its row"
expect '^browser: scroll edge shown$' "the list scrolled to a row didn't show its edge"
# The last row in view takes the foot's line away, the room after it still under the foot.
expect '^browser: scroll foot edge hidden$' "the list scrolled to its last row kept its foot's edge"
recorded
../tools/window-screenshot.sh "$pid" "$out/list-scrolled.png" || fail "no window to capture"
switch "2 cmd" "$c"
none_since '^browser: list scrolled' || fail "showing a tab in view scrolled the list"
# Shown again in a window short enough that, with the list scrolled back to its top, the last tab's
# row is wholly under its bottom (at 340 its top still shows), a sliver of its plate marks it there,
# through the rest of the scroll; a click on that scrolls the list back to the row.
send resize 1000 300
expect '^browser: drawn at 1000x300$' "the window didn't draw at its new size"
switch "9 cmd" "$b"
expect "^browser: list scrolled to [1-9][0-9.]* for $b\$" \
  "showing a tab under the list's bottom didn't scroll to its row"
send scroll 100 200 -10 8
expect '^browser: shown tab marker bottom at ' \
  "the shown tab's row scrolled under the list's bottom didn't put its marker there"
../tools/window-screenshot.sh "$pid" "$out/shown-marker.png" || fail "no window to capture"
none_since '^browser: shown tab marker hidden$' || fail "the marker went before the click"
marker=$(grep '^browser: shown tab marker bottom at ' "$log" | tail -1 |
  awk '{ printf "%d %d", $7 + $9 / 2, $8 + $10 / 2 }')
send click $marker
expect "^browser: list scrolled to [1-9][0-9.]* for $b\$" \
  "a click on the marker didn't scroll the list to the shown tab's row"
expect '^browser: shown tab marker hidden$' "the marker stayed with the shown tab's row back in view"
# The page gets the keyboard for the ⌘, below before the window goes back to its size. On Wayland a
# click right after a resize can reach the UI: Chromium's surface takes clicks at the page's new
# place only once it has drawn at the new size, which can be after the UI has (#1577).
send click 600 150
in_browser RenderWidgetHostViewCocoa "a click on the page didn't give it the keyboard"
send resize $size
expect "^browser: drawn at ${size% *}x${size#* }\$" "the window didn't draw back at its size"
# ⌘, from the page opens Settings on General. Its switch turns the warning before quitting off;
# on Tabs & Sidebar, the menu of the profile's pop-up button sets how long its Today tabs wait for
# the archive, a click on a drawing of Share favorites (#2430) sets who shares them, and on the Mac, the haptics' switch turns them
# off; and ⌘W hides it.
send key , cmd
expect '^browser: command settings via page$' "⌘, from the page didn't run Settings"
expect '^browser: settings shown General$' "⌘, didn't show Settings on its General section"
settings_drew General
../tools/window-screenshot.sh "$pid" "$out/settings.png" || fail "no window to capture"
click_in_sight "switch warn before quitting"
expect '^browser: warn before quitting off$' "a click on Ask before quitting didn't turn it off"
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
post clickkey $(drawn "archive [0-9]*")
expect '^browser: menu shown 12 hours (default) (checked), 24 hours, 7 days, 30 days, Never$' \
  "a click on Archive Today tabs after didn't show the times, with 12 hours chosen"
send choose 24 hours
expect '^browser: profile [0-9]* archive tabs after 24 hours$' \
  "choosing 24 hours in Settings didn't set it"
in_sight "share favorites In Each Profile's Spaces" ||
  fail "Share favorites didn't show In Each Profile's Spaces chosen"
# Picked in the dark look, and in the light one below, to capture the row in each.
send menu "Appearance › Dark"
expect '^browser: appearance dark$' "View › Appearance › Dark didn't change the appearance"
in_sight "share favorites option In each Space" || fail "Settings didn't draw In Each Space's drawing"
send clickkey $at
expect "^browser: favorites scope In Each Space\$" "a click on In Each Space's drawing didn't set it"
expect "^browser: settings share favorites In Each Space at " "Share favorites didn't show In Each Space"
../tools/window-screenshot.sh "$pid" "$out/settings-share-favorites-dark.png" ||
  fail "no window to capture"
send menu "Appearance › Automatic"
expect '^browser: appearance automatic$' "View › Appearance › Automatic didn't change the appearance"
if [ "$(uname)" = Darwin ]; then
  # Under the card's edge where the screen is short, below Battery Saver's row: in_sight scrolls
  # it up.
  in_sight "switch haptics" || fail "Settings didn't draw the haptics' switch"
  send clickkey $at
  expect '^browser: haptics off$' "a click on the haptics switch didn't turn them off"
fi
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
none_since '^browser: command close-tab' || fail "⌘W in Settings closed a tab"
# The quit leaves the favorite shown, with an icon, which is kept with it as its name would be.
send click $tile
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the tile didn't show the favorite"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the tile before the quit didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect '^browser: icon picker shown$' "Change Icon… before the quit didn't show the icon picker"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send insert glowing star
send key return
expect "^browser: page $a icon 🌟\$" "the picker's pick before the quit didn't become the favorite's icon"
expect '^browser: icon picker hidden$' "picking an emoji before the quit didn't close the icon picker"
exec 3>&-
quit

# Launched again, the favorite comes back shown, with its icon, which Remove Icon takes out, and
# Remove from Favorites in its menu puts it at the top of Today.
log="$out/favorites-restored.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the favorites"
await '^browser: favorites [0-9][0-9]*$' || fail "the favorite didn't come back"
a=$(favorites)
await '^browser: today [0-9][0-9]* [0-9][0-9]*$' || fail "the Today tabs didn't come back"
left=$(today)
await "^browser: selected $a\$" || fail "the relaunch didn't show the favorite shown last"
await "^browser: loaded $a $site\$" || fail "the favorite didn't load"
await "^browser: tile $a drawn with icon 🌟\$" ||
  fail "the relaunch didn't bring back the favorite's icon: $(grep "^browser: tile $a drawn" "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/favorites-restored.png" || fail "no window to capture"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the restored favorite's menu. Browser wasn't the active app, so the page ignored"
  echo "  the click."
  exit 0
fi
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Remove Icon, Close Tab')\$" \
  "a right-click on the restored tile didn't offer Remove Icon for the icon it kept"
send choose "$(cased 'Remove Icon')"
expect "^browser: page $a icon none\$" "Remove Icon on the restored tile didn't take its icon out"
expect "^browser: tile $a drawn with icon none\$" "the restored tile didn't draw its favicon again"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the restored tile didn't show the favorite's menu"
send choose "$(cased 'Remove from Favorites')"
expect '^browser: command remove-from-favorites via tab menu$' "Remove from Favorites didn't run"
expect '^browser: favorites $' "Remove from Favorites left the tile"
expect "^browser: today $a $left\$" \
  "Remove from Favorites didn't put the tab at the top of Today: Today is \"$(today)\""
none_since '^browser: selected' || fail "Remove from Favorites showed another tab"
# The toast offers Undo, and ⌘Z puts the tile back where it was and the tab out of Today, without
# showing another tab; ⇧⌘Z removes it again.
expect '^browser: toast shown Removed from Favorites \[undo\]$' "Remove from Favorites didn't offer Undo"
# Only the toast's plate takes clicks while it offers Undo, and a click just under it, in its
# window's margin where its shadow falls, reaches the page. The window ends at the content's right
# edge, and the app logs where the plate is in it and how wide it is. This step has never run: the
# moves and the click are posted, as the driver's are, to the window the window server would pick,
# so it stands or falls with the toast's window letting the mouse through there.
expect '^browser: toast plate [0-9]* [0-9]* [0-9]* [0-9]* in [0-9]* [0-9]*$' \
  "the undo toast didn't tell where its plate is"
set -- $(grep '^browser: toast plate' "$log" | tail -1 | sed 's/^browser: toast plate //; s/ in / /')
send layout
toast_content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*\)x[0-9]*,.*/\1/')
under="$((toast_content - $5 + $1 + $3 / 2)) $(($2 + $4 + 12))"
send move $under
send click $under
expect '^browser: title [0-9]* corner$' "a click just under the undo toast's plate didn't reach the page"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Remove from Favorites"
expect "^browser: favorites $a\$" "⌘Z didn't put the tile back"
expect "^browser: today $left\$" "⌘Z left the tab in Today: Today is \"$(today)\""
none_since '^browser: selected' || fail "⌘Z after Remove from Favorites showed another tab"
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Remove from Favorites"
expect '^browser: favorites $' "⇧⌘Z left the tile"
expect "^browser: today $a $left\$" "⇧⌘Z didn't put the tab at the top of Today: Today is \"$(today)\""
# The rows slide down under the tab put back, and a press before they stop lands on the wrong one.
expect '^browser: rows settled$' "the rows didn't settle after ⇧⌘Z"
# Dragging moves tabs between the rows and the tiles. With no favorites, a Today row dragged over
# the Space's name is drawn as a tile there, and dropped it's the first favorite.
set -- $left
x=$1
y=$2
send press $(row 1)
send dragto 100 200
send dragto 114 112
expect '^browser: drag shape tile$' "a row dragged onto the favorites didn't turn into a tile"
expect '^browser: detent$' "a row dragged onto the favorites didn't click the trackpad"
../tools/window-screenshot.sh "$pid" "$out/favorite-drag.png" || fail "no window to capture"
none_since '^browser: dragged' ||
  fail "a row dragged onto the favorites dropped before the pointer let go"
send release 114 112
expect "^browser: dragged $x to favorites 0\$" \
  "a Today row dragged onto the favorites didn't go there"
none_since '^browser: detent$' || fail "letting go on the favorites clicked the trackpad"
expect "^browser: favorites $x\$" \
  "the row dropped on the favorites isn't one: the favorites are \"$(favorites)\""
expect "^browser: today $a $y\$" "the row dropped on the favorites is still in Today: \"$(today)\""
# Dropped right of the tile, a row is the second favorite, and the two share the tiles' row.
send drag $(tiled_row 0) 180 112
expect "^browser: dragged $a to favorites 1\$" "a row dragged right of the tile didn't go there"
expect "^browser: favorites $x $a\$" \
  "the second row dropped on the favorites isn't the second: the favorites are \"$(favorites)\""
expect '^browser: tiles settled$' "the tiles didn't settle after the drop"
# A tile dragged left of the other goes before it.
send drag 168 110 40 110
expect "^browser: dragged $a to favorites 0\$" \
  "a tile dragged left of the other didn't go before it"
expect "^browser: favorites $a $x\$" "the tiles didn't swap: the favorites are \"$(favorites)\""
expect '^browser: tiles settled$' "the tiles didn't settle after the swap"
# A tile dragged into the list goes there, unpinned in Today.
send drag 168 110 100 240
expect "^browser: dragged $x to today 0\$" "a tile dragged into Today didn't go there"
expect "^browser: today $x $y\$" "the tile dropped in Today isn't at its top: Today is \"$(today)\""
expect "^browser: favorites $a\$" "the tile dropped in Today is still a favorite"
none_since '^browser: selected' || fail "dragging a tile showed it"
# Escape puts a dragged tile back.
send press 114 110
expect "^browser: pressed $a\$" "the press on a tile didn't reach the UI"
none_since '^browser: detent$' || fail "pressing a tile clicked the trackpad"
send dragto 100 200
expect '^browser: drag shape row$' "a tile dragged into the list didn't turn into a row"
expect '^browser: detent$' "a tile dragged into the list didn't click the trackpad"
../tools/window-screenshot.sh "$pid" "$out/favorite-drag-out.png" || fail "no window to capture"
send dragto 100 300
# The put-back can end within the escape's step, before the release.
escaped=$((step + 1))
send key escape
expect '^browser: command stop via ' "escape during a tile's drag didn't reach the UI"
let_go=$((step + 1))
send release 100 300
[ "$(favorites)" = "$a" ] || fail "a tile's drag escape put back moved it: \"$(favorites)\""
# A press hits the frame drawn, so the next drag waits for the put-back's.
expect '^browser: drag put back$' "Escape's put-back didn't finish" "$escaped"
# A split's row dragged onto the tiles is one favorite, a split, its tile showing each page's
# favicon (#689, ours: Arc's tile isn't recorded). It shows focused where it was focused last: a
# click in its right page focuses that one, and ⌘2 from another tab shows the split focused there.
# Resting on it, its card has its pages. Separate All Tabs on its tile leaves its first page the
# favorite and its other at the top of Today, and ⌘Z makes it the favorite split again (#1776).
# Remove from Favorites puts it whole at the top of Today, where Separate All Tabs leaves the rows
# as they were for the steps after.
send click $(tiled_row 0)
expect "^browser: selected $x\( in .*\)\{0,1\}\$" "a click on the first Today row didn't show it"
none_since '^browser: dragged' "$let_go" || fail "a tile's drag escape put back dropped it"
send click $(tiled_row 1) alt
expect "^browser: splits $x+$y\$" "⌥-click on the second Today row didn't split it with the first"
# The ⌥-click focused the right pane, which has the row's width, so the drag presses the left one's
# favicon, folded at the row's start (#2340), and the split shows focused there.
send drag 24 $(tiled_row 0 | cut -d' ' -f2) 180 112
expect "^browser: selected $x\( in .*\)\{0,1\}\$" "a press on the split's left segment didn't focus it"
expect "^browser: dragged $x to favorites 1\$" "the split's row dragged onto the tiles didn't go there"
expect "^browser: favorites $a $x\$" "the split isn't one favorite: the favorites are \"$(favorites)\""
expect "^browser: favorite splits $x+$y\$" "the split dragged onto the tiles isn't a favorite split"
expect '^browser: today $' "the split's tabs are still in Today: \"$(today)\""
expect '^browser: tiles settled$' "the tiles didn't settle after the split's drop"
../tools/window-screenshot.sh "$pid" "$out/favorite-split-tile.png" || fail "no window to capture"
# Its right page, three quarters across the card the sidebar leaves.
width=$(grep '^browser: drawn at' "$log" | tail -1 | sed 's/^browser: drawn at \([0-9]*\).*/\1/')
# The pointer goes there first, as a real one does: the Mac's click alone doesn't tell GPUI the
# pointer left the tile, and resting on it again below would then be no new hover.
send move $((228 + (width - 238) * 3 / 4)) 400
send click $((228 + (width - 238) * 3 / 4)) 400
expect "^browser: selected $y\( in .*\)\{0,1\}\$" "a click in the split's right page didn't focus it"
switch "1 cmd" "$a"
switch "2 cmd" "$y"
# It keeps that pane across a Space switch and a relaunch (#1398). A second Space, made and left
# with ⌃1, gives the split back focused there, and deleted it leaves the first Space as it was.
first=$(grep '^browser: spaces ' "$log" | tail -1 | cut -d' ' -f3)
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
send type Elsewhere
send key return
expect "^browser: create space $second Elsewhere\$" "return didn't name the second Space"
expect '^browser: space form gone$' "the form didn't leave after Create"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect "^browser: selected $y\( in .*\)\{0,1\}\$" \
  "⌃1 back didn't show the favorite split focused on its right page"
../tools/window-screenshot.sh "$pid" "$out/favorite-split-space.png" || fail "no window to capture"
send key 2 ctrl
expect "^browser: space $second in " "⌃2 didn't show the second Space"
# The second Space has no favorites, so its name is where the first's was before its tiles.
post rightclick 100 112
expect "^browser: menu shown $(cased 'Change Space Icon…'), " \
  "a right-click on the second Space's name didn't show its menu"
post choose "$(cased 'Delete Space')"
expect "^browser: spaces $first\$" "Delete didn't delete the second Space"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "deleting the second Space didn't show the first"
exec 3>&-
quit

# Launched again, the favorite split comes back shown, focused on its right page, with new ids.
log="$out/favorites-split.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch with the favorite split shown didn't restore"
await '^browser: favorite splits [0-9][0-9]*+[0-9][0-9]*$' ||
  fail "the relaunch didn't bring the favorite split back a split"
set -- $(grep '^browser: favorite splits' "$log" | tail -1 | sed 's/^browser: favorite splits //; s/+/ /')
x=$1
y=$2
await "^browser: favorites [0-9][0-9]* $x\$" ||
  fail "the relaunch didn't keep the favorite split second: \"$(favorites)\""
a=$(favorites | cut -d' ' -f1)
await "^browser: selected $y\$" ||
  fail "the relaunch didn't show the favorite split focused on its right page: $(grep '^browser: selected' "$log" | tail -1)"
await "^browser: loaded $y " || fail "the favorite split's right page didn't load"
../tools/window-screenshot.sh "$pid" "$out/favorite-split-restored.png" || fail "no window to capture"
send move $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the restored favorite split's card and menu. Browser wasn't the active app, so the"
  echo "  page ignored the clicks."
  exit 0
fi
switch "1 cmd" "$a"
# Two tiles across the sidebar: the split's is the second, 118 in and 102 wide.
send move 169 113
expect "^browser: hover card shown $x unfavorite \(picture\|favicon\)\$" \
  "resting on the split's tile didn't show its card"
expect '^browser: hover card settled$' "the split's card didn't settle"
../tools/window-screenshot.sh "$pid" "$out/favorite-split-card.png" || fail "no window to capture"
send move $page_point
post rightclick 169 113
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Separate All Tabs, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the split's tile didn't show its menu"
send choose "$(cased 'Separate All Tabs')"
expect '^browser: favorite splits $' "Separate All Tabs on the split's tile left it a split"
expect "^browser: today $y\$" \
  "Separate All Tabs on the split's tile didn't put its other page in Today: \"$(today)\""
[ "$(favorites)" = "$a $x" ] ||
  fail "Separate All Tabs on the split's tile didn't leave its first page the favorite: \"$(favorites)\""
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Separate All Tabs on the tile"
expect "^browser: favorite splits $x+$y\$" "⌘Z didn't make the favorite split again"
expect '^browser: today $' "⌘Z left a page of the favorite split in Today: \"$(today)\""
[ "$(favorites)" = "$a $x" ] || fail "⌘Z didn't put the favorite split back second: \"$(favorites)\""
send move $page_point
post rightclick 169 113
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Separate All Tabs, Remove from Favorites, Change Icon…, Close Tab')\$" \
  "a right-click on the split's tile made again didn't show its menu"
send choose "$(cased 'Remove from Favorites')"
expect "^browser: favorites $a\$" "Remove from Favorites left the split a favorite: \"$(favorites)\""
expect "^browser: today $x $y\$" "Remove from Favorites didn't put the split at the top of Today"
expect "^browser: splits $x+$y\$" "the split removed from the favorites isn't a split in Today"
send key 2 cmd
expect "^browser: selected \($x\|$y\) in " "⌘2 didn't show the split put back in Today"
bar_command separate-split "Separate All Tabs" "separate all"
expect '^browser: splits $' "Separate All Tabs didn't end the split"
expect "^browser: today $x $y\$" "Separate All Tabs didn't leave the rows as they were: \"$(today)\""
# Three tiles in the sidebar at its narrowest take two rows, 110 points, more than 40 % of a window
# 220 tall, so they scroll with the list, under the URL field (Ardia).
send drag $(tiled_row 0) 200 112
expect "^browser: dragged $x to favorites 1\$" "a row dragged right of the tile didn't go there"
expect '^browser: tiles settled$' "the tiles didn't settle after the second drop"
# The tiles can settle before the drop, and the rows close up over the row that left after it, so
# the next press waits for them, or it lands on that row's old place.
expect '^browser: rows settled$' "the rows didn't close up after the second drop"
send drag $(tiled_row 0) 200 112
expect "^browser: dragged $y to favorites 2\$" "a row dragged right of the tiles didn't go last"
expect "^browser: favorites $a $x $y\$" "the tiles aren't three: the favorites are \"$(favorites)\""
expect '^browser: tiles settled$' "the tiles didn't settle after the third drop"
# The drag's press showed $y, so the click on its tile below has a tab to change from.
switch "1 cmd" "$a"
send drag 225 400 120 400
expect '^browser: sidebar width 164$' "dragging the sidebar's edge didn't narrow it to 164"
send layout
size=$(grep '^browser: layout content' "$log" | tail -1 |
  sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
send resize 1000 220
expect '^browser: tiles scroll with the list$' \
  "two rows of tiles in a window 220 tall didn't scroll with the list"
../tools/window-screenshot.sh "$pid" "$out/tiles-tall.png" || fail "no window to capture"
# Scrolled 40, the tiles went up 40 with the rows, and the first tile's place holds the third's.
send scroll 100 150 20 2
expect '^browser: scroll edge shown$' "the list scrolled with the tiles didn't show its edge"
../tools/window-screenshot.sh "$pid" "$out/tiles-scrolled.png" || fail "no window to capture"
send click 40 117
expect '^browser: command select via favorite$' "a click on the scrolled tiles didn't reach one"
expect "^browser: selected $y\( in .*\)\{0,1\}\$" \
  "the third tile isn't 40 up with the list: a click there didn't show its favorite"
send scroll 100 150 -20 2
expect '^browser: scroll edge hidden$' "the list back at its top didn't hide its edge"
send resize $size
expect '^browser: tiles stay put$' "the tiles in a window back at its height still scroll"
send doubleclick 161 400
expect '^browser: sidebar width 228$' \
  "a double-click on the sidebar's edge didn't put its width back"
# Three tiles across the sidebar with titles of very different widths, long, medium and short as
# the owner's are: each one's card goes beside the rows at its tile's top, not over the tiles,
# where the UI asked, going along them and back, and while one shows the next swaps in at once. In
# a window too narrow for it there, it keeps 8 from the window's right edge.
long="Creating A World That Doesn't Exist - YouTube"
medium="Elon Musk on X: a post / X"
short="Proton Mail: Sign-in"
for page in long medium short; do
  eval "printf '<title>%s</title>' \"\$$page\"" > "$out/site/$page.html"
done
for titled in "1 $a long" "2 $x medium" "3 $y short"; do
  set -- $titled
  switch "$1 cmd" "$2"
  tab=$2
  go_from_bar "${site#http://}$3.html"
  eval "title=\$$3"
  expect "^browser: title $2 $title\$" "the $3 page didn't give its title to tab $2"
done
# The width the window was last drawn at.
drawn_width() {
  grep '^browser: drawn at' "$log" | tail -1 | sed 's/^browser: drawn at \([0-9.]*\)x.*/\1/'
}
# Waits for the card shown to be at rest, whether it came in this step or swapped in for another
# tile's, which came before: a capture before then has it partway in.
card_settled() {
  for _ in $(seq 100); do
    [ "$(grep -E '^browser: hover card (settled|hidden)$' "$log" | tail -1)" = 'browser: hover card settled' ] &&
      return 0
    sleep 0.1
  done
  fail "$1"
}
# Resting on the tile whose middle is at x $1 shows tab $2's card at the tile's top, 90 down,
# inside the window, placed where the UI asked. Sets $left to where it starts.
beside() {
  send move $1 110
  expect "^browser: hover card shown $2 " "resting on the tile at $1 didn't show tab $2's card"
  expect '^browser: hover card window at ' "tab $2's card wasn't put over the window"
  asked=$(since_move '^browser: hover card at ')
  placed=$(since_move '^browser: hover card window at ')
  echo "$asked $placed" | awk -v window=$(drawn_width) '
    { across = $5 - $14; down = $6 - $15; top = $6 - 90; inside = window - 8 - ($5 + $7) }
    END { exit !(NR == 1 && NF == 15 && across > -1 && across < 1 && down > -1 && down < 1 \
      && top > -1 && top < 1 && inside > -1) }' \
    || fail "tab $2's card isn't beside its tile at $1: asked \"$asked\", placed \"$placed\", window $(drawn_width)"
  left=$(echo "$asked" | cut -d' ' -f5)
}
beside 41 "$a"
first=$left
beside 114 "$x"
[ "$left" = "$first" ] || fail "the second tile's card starts at $left, not beside the rows at $first"
beside 187 "$y"
[ "$left" = "$first" ] || fail "the third tile's card starts at $left, not beside the rows at $first"
card_settled "the tiles' card didn't come to rest"
../tools/window-screenshot.sh "$pid" "$out/favorite-cards.png" || fail "no window to capture"
beside 114 "$x"
beside 41 "$a"
# The window may not go so narrow.
send resize 320 ${size#* }
for _ in $(seq 30); do
  [ "$(drawn_width | cut -d. -f1)" -le 322 ] && break
  sleep 0.1
done
if [ "$(drawn_width | cut -d. -f1)" -le 322 ]; then
  beside 187 "$y"
  card_settled "the card in the narrow window didn't come to rest"
  ../tools/window-screenshot.sh "$pid" "$out/favorite-card-narrow.png" || fail "no window to capture"
else
  echo "note: the window didn't narrow to 320, so the card in a narrow window wasn't checked."
fi
send resize $size
expect "^browser: drawn at ${size% *}x${size#* }\$" "the window didn't draw back at its size"
# Settings kept the time, the haptics and the warning across the quit. ⌘, from the UI opens it,
# its close button hides it, and Chromium settings at the column's foot opens them in a tab.
send click $sidebar_point
send key , cmd
expect '^browser: command settings via ui$' "⌘, from the UI didn't run Settings"
expect '^browser: settings shown General$' "⌘, from the UI didn't show Settings"
settings_drew General
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
post clickkey $(drawn "archive [0-9]*")
expect '^browser: menu shown 12 hours (default), 24 hours (checked), 7 days, 30 days, Never$' \
  "Settings didn't keep 24 hours across the quit"
send choose "12 hours (default)"
expect '^browser: profile [0-9]* archive tabs after 12 hours (default)$' \
  "choosing 12 hours didn't set it"
in_sight "share favorites In Each Space" || fail "Settings didn't keep Each Space across the quit"
send menu "Appearance › Light"
expect '^browser: appearance light$' "View › Appearance › Light didn't change the appearance"
in_sight "share favorites option In each profile's Spaces" ||
  fail "Settings didn't draw In Each Profile's Spaces' drawing"
send clickkey $at
expect "^browser: favorites scope In Each Profile's Spaces\$" \
  "a click on In Each Profile's Spaces' drawing didn't set it"
expect "^browser: settings share favorites In Each Profile's Spaces at " \
  "Share favorites didn't show In Each Profile's Spaces"
../tools/window-screenshot.sh "$pid" "$out/settings-share-favorites-light.png" ||
  fail "no window to capture"
send menu "Appearance › Automatic"
expect '^browser: appearance automatic$' "View › Appearance › Automatic didn't change the appearance"
if [ "$(uname)" = Darwin ]; then
  in_sight "switch haptics" || fail "Settings didn't draw the haptics' switch"
  send clickkey $at
  expect '^browser: haptics on$' "Settings didn't keep the haptics off across the quit"
fi
# The driver's done for a click that closes its window can be lost with the window, as it always
# was in a VM, so the click isn't waited for; Settings going is.
settings_close
post clickkey $close
expect '^browser: settings hidden$' "the close button didn't hide Settings"
send menu Settings…
expect '^browser: settings shown Tabs & Sidebar$' \
  "Settings… didn't show Settings on the section it showed"
settings_drew 'Tabs & Sidebar'
send clickkey $(drawn "chromium settings")
expect '^browser: more settings$' "a click on Chromium settings didn't run it"
expect '^browser: settings hidden$' "Chromium settings didn't hide Settings"
expect '^browser: loaded [0-9]* chrome://settings/$' \
  "Chromium settings didn't open them"
# The warning stayed off, so ⌘Q quits without asking. It's the driver's last command.
echo "key q cmd" >&3
exec 3>&-
await '^browser: command quit via ' || fail "⌘Q didn't reach the UI"
exited
grep -q '^browser: dialog shown Quit' "$log" && fail "⌘Q asked first with the warning off"

# A list longer than the 12 a Space shows at most, as one saved before there was a most, comes back
# whole. With one taken out, the 12 left refuse a 13th: a pinned tab's menu dims Add to Favorites,
# and its row dragged over the tiles stays a row. Each copy is a favorite of its own, at a pinned
# URL of its own.
python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    session = json.load(f)
kept = session["favorites"]
copies = []
for i in range(13):
    copy = {k: v for k, v in kept[i % len(kept)].items() if k not in ("id", "position", "site")}
    copy["pinned_url"] = f"{sys.argv[2]}favorite-{i}.html"
    copies.append(copy)
session["favorites"] = copies
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(session, f)
' "$saved" "$site" || fail "the saved favorites couldn't be made 13"
log="$out/favorites-most.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the 13 favorites"
await '^browser: favorites\( [0-9][0-9]*\)\{13\}$' ||
  fail "the 13 favorites saved didn't all come back: \"$(favorites)\""
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: the most favorites. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The rows are drawn by the time the driver's click is done.
../tools/window-screenshot.sh "$pid" "$out/favorites-most.png" || fail "no window to capture"
# Four to a row across the sidebar, the first tile's middle at 32 114.
r=$(favorites | cut -d' ' -f1)
post rightclick 32 114
expect "^browser: menu shown $(cased 'Back to Favorite URL, ')" "a right-click on the first tile didn't show its menu"
send choose "$(cased 'Remove from Favorites')"
expect '^browser: favorites\( [0-9][0-9]*\)\{12\}$' "Remove from Favorites didn't leave 12 favorites"
expect "^browser: today $r\( \|\$\)" "Remove from Favorites didn't put the tab at the top of Today: \"$(today)\""
expect '^browser: tiles settled$' "the 12 tiles didn't settle"
expect '^browser: rows settled$' "the rows didn't settle under the 12 tiles"
twelve=$(favorites)
# Three rows of tiles take the Space's rows 165 down.
send click 100 $(($(row 0 | cut -d' ' -f2) + 165))
expect "^browser: selected $r\( in .*\)\{0,1\}\$" "a click on Today's top row didn't show the tab taken out"
send key d cmd
expect "^browser: pinned $r\$" "⌘D didn't pin the tab taken out of the favorites"
expect '^browser: rows settled$' "the rows didn't settle after ⌘D"
pinned_full="100 $(($(pinned_row 0 | cut -d' ' -f2) + 165))"
post rightclick $pinned_full
expect "^browser: menu shown $(cased 'Unpin Tab, Add to Favorites (disabled), ')" \
  "with 12 favorites, a pinned tab's menu didn't dim Add to Favorites"
../tools/window-screenshot.sh "$pid" "$out/favorites-full-menu.png" || fail "no window to capture"
send dismiss
send press $pinned_full
send dragto 100 200
over=$((step + 1))
send dragto 114 112
send release 114 112
# Over the tiles it's still over its own place, so it goes back there, which moves nothing in the
# browser and logs only the put-back.
expect "^browser: \(dragged $r to \(pinned\|today\) \|drag put back\$\)" \
  "the row dragged over the 12 tiles didn't drop back in the list"
none_since "^browser: dragged $r to favorites" || fail "with 12 favorites, a row dropped on the tiles became the 13th"
none_since '^browser: drag shape tile$' "$over" ||
  fail "with 12 favorites, a row dragged over the tiles turned into a tile"
[ "$(favorites)" = "$twelve" ] || fail "with 12 favorites, the drag changed them: \"$(favorites)\""
exec 3>&-
quit
echo "ok: a Today tab's menu is Arc's, whose Copy Link with ⌥ held copies a Markdown link to the tab, and its row dragged onto the Space's name makes the tab a tile above the Space's rows, which shows its hover card beside the rows, with its page's picture once another tab is shown, which other apps don't take for the window in front, while the Library and sidebar buttons name themselves and their keys in a tooltip, which a click shows and ⌘1–⌘9 count first, with an edge under them while the list is scrolled, which scrolls to the row of a tab shown under its bottom, and marks the row of the tab shown scrolled out of it at that edge, a click on the mark scrolling back to it. A double-click on the tile takes it back to its URL, and ⌘W unloads it and keeps the tile. Its menu duplicates it into Today, and after a quit the favorite comes back, shown, and Remove from Favorites puts it at the top of Today. Rows dragged onto the tiles turn into tiles, and tiles dragged into the list into rows. Rows dragged onto the tiles become favorites where they're dropped, tiles reorder and go into Today, and Escape puts a dragged tile back. A split's row dragged onto the tiles is one favorite, a click in its right page focuses it and ⌘2 shows it focused there, as do a Space switch and a relaunch, its card has its pages, and Remove from Favorites puts it whole at the top of Today. Tiles taller than 40 % of the window scroll with the list, under the URL field. ⌘, from the page and the UI opens Settings, whose switches set the warning before quitting and the haptics and whose pop-ups set the archive's time and who shares the favorites (Each Space, with the favorite kept in it across the quit, and back), all kept across a quit, when ⌘Q quits without asking; ⌘W and its close button hide it, and Chromium settings at its column.s foot opens them. Three tiles with a long, a medium and a short title each show their card beside the rows at their own top, along them and back, placed where the UI asked, and inside a narrow window. Remove from Favorites offers Undo, and ⌘Z puts the tile back with the tab out of Today, and ⇧⌘Z removes it again. Change Icon… on the tile hangs the icon picker under it, its pick is the tile's icon and Remove Icon takes it out. 13 favorites saved come back whole, and with 12 a pinned tab's menu dims Add to Favorites and its row dragged over the tiles stays a row. Windows: $out/tab-menu-option.png, $out/favorite-icon-picker.png, $out/favorite-icon.png, $out/favorites-most.png, $out/favorites-full-menu.png, $out/favorite-card.png, $out/favorite-cards.png, $out/favorite-card-narrow.png, $out/library-tooltip.png, $out/header-tooltip.png, $out/favorites.png, $out/scroll-edge.png, $out/list-scrolled.png, $out/shown-marker.png, $out/favorite-unloaded.png, $out/favorites-restored.png, $out/favorite-drag.png, $out/favorite-drag-out.png, $out/favorite-split-tile.png, $out/favorite-split-space.png, $out/favorite-split-restored.png, $out/favorite-split-card.png, $out/tiles-tall.png, $out/tiles-scrolled.png, $out/settings.png"
