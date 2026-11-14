# check.sh runs this at 140.
# View › Clear Cache and Refresh and the command bar's Clear Cookies and Refresh delete the cache
# and the cookies of the page's site, and load the page again. ⇧⌘C and ⌥⇧⌘C from the page and the UI
# copy the tab's URL and a Markdown link to it, to a pasteboard of the run's own, without its
# tracking parameters, and Share on ⇧⌘C's toast shares the page, where there's a share sheet. ⌥⌘V from the UI and the page and the command bar's Paste URL as New Tab open
# the link copied in a new tab, and a Markdown link nothing. The sidebar shows Clear and the URL
# field's button only while the pointer is over it, and the button copies the URL. ⇧⌘K from the
# page and the UI, and Clear on the divider, close the Space's Today tabs but the one shown, with no
# toast, and ⌘Z from the page and the UI
# brings them back in place, unloaded, unless the page's text field has the keyboard, when ⌘Z is its
# own. With only the tab shown left, Clear has an arrow, lights up under the pointer and closes that
# tab too, and ⌘Z shows it again. ⇧⌘T from the page and the UI reopens the tab closed last, where it
# was, with its history, muted if it was and with its icon, and after a Clear, ⌘Z brings back only
# the tabs ⇧⌘T didn't. A right-click on a row shows its tab's menu, Arc's for a Today tab, whose
# Duplicate, Archive Tabs Below and Unpin Tab work. ⌘W archives a Today tab. ⇧⌘L, Window › Open
# Library… and the footer's button open the Library, which pushes the page right at its width, and
# its back button closes it. A right-click on an archived tab shows its menu, whose Copy URL copies
# its URL and Delete from Archive takes it out. A ⌘-click on one reopens it at the top of Today with
# its history, and Clear Archive, from the Archive menu and a right-click on the Archived Tabs
# section, asks, then empties the archive, which ⌘Z brings back. A page's question about a
# permission shows under the URL field while its tab is shown, bringing out a hidden sidebar, and
# its ×, Block and Allow answer it. The URL field's Site Controls button and the command bar's Site
# Controls, from the page, open a panel under the field that lists the site's permissions as
# Chrome's page info does. A value changed there sticks, and its Reset permissions sets them back.
# Their Developer Mode row lists localhost on by itself, and turns it off and on for the tab, as the
# command bar's Turn off and Turn on Developer Mode for this site do, View › Developer's and ⌃D,
# from the page and the UI, which the page's text field and the command bar's keep as forward
# delete, and where ⌥← and ⌥⌦ go back a word and delete one. In it the URL field says the whole
# URL, the tab's row has its outline, and the field's DevTools button docks DevTools in the card
# and closes them.
# Escape, a click outside and the button close it. A page that asks for Widevine brings a note under
# the field, which ClearKey doesn't. From macOS 14 its button shows the page in Safari's engine, in
# the tab, and the profile keeps the site for it: the site's cookies, HttpOnly too, and its
# localStorage go there with it before the page's first request and script, another site goes back
# to Chromium, a link whose redirects pass through another site and come back stays, the site comes
# back to Safari's engine, keys, clicks and Japanese input work with its page focused, as they do in
# Chromium's, ⌘T to the site opens a new tab there, a private window's tab there has a session of
# its own, which the regular window and the next private window don't see, and the Site Controls
# list it and turn it off, which brings back to Chromium the cookies and the item its page stored.
# Before 14 the button opens the page in Safari, and the note goes once the tab is at another site.
# The pointer on a link shows its address in a plate in the card's bottom left corner, which goes
# when it leaves. From 14, a listed streaming service, a host of the run's server the dev build
# lists, opens in Safari's engine from its first load, in a private window too, the Site Controls
# show it on, and Off keeps it in Chromium after a reload, in a new tab, after another site and
# back and after a relaunch, where On brings it back. A profile made where one's folder was removed
# by hand has a Safari's engine store of its own, and the old one is deleted at the next launch.

# Clear, from the served page and two more, with the served page pinned for it to leave.
serve
# The run turns 127.0.0.1 on for Safari's engine and closes its note, which the profile keeps: a
# run before would leave the note closed here, and the runs after would open their pages there. It
# forgets them before its first launch and after its last.
forget_engine_sites

log="$out/clear.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the Clear test page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: Clear. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The pointer on a link reports its address, which shows in a plate in the card's bottom left
# corner after a moment, and the plate goes when the pointer leaves the link. The page's link is
# 200 by 40 points, 100 in from the card's top left, which is 228 and 10 in from the window's.
link_point="430 130"
send move $link_point
expect "^browser: link target http://127.0.0.1:$port/notes.txt\$" \
  "the pointer on the link reported no address"
expect '^browser: link target plate ' "the link's address didn't show in a plate"
expect '^browser: link target window shown$' "the plate's window didn't come on screen"
../tools/window-screenshot.sh "$pid" "$out/link-target.png" || fail "no window to capture"
send move $page_point
expect '^browser: link target none$' "the pointer leaving the link didn't report it"
expect '^browser: link target hidden$' "the plate stayed after the pointer left the link"
# A page Peek loads is a visit, as a tab's is (#2363): History lists it, the command bar suggests
# it, and Remove from History and Clear browsing data's Delete take it out. The link's page is one
# the run opens nowhere else. peek_notes: ⇧-click on the link opens it in Peek, whose page loads,
# and Esc closes it, giving the page back the keyboard.
notes="http://127.0.0.1:$port/notes.txt"
peek_notes() {
  send click $link_point shift
  expect "^browser: link preview asked from [1-9][0-9]*\$" \
    "⇧-click on the page's link didn't ask for it in Peek"
  expect "^browser: link preview [1-9][0-9]* $notes\$" \
    "⇧-click on the page's link didn't open $notes in Peek"
  peeked=$(grep "^browser: link preview [1-9][0-9]* $notes\$" "$log" | tail -1 | cut -d' ' -f4)
  expect '^browser: link preview settled$' "Peek of the page's link didn't settle"
  expect "^browser: loaded $peeked $notes\$" "the page Peek showed, $notes, didn't load"
  send key escape
  expect "^browser: tab $peeked closed\$" "Esc didn't close the page Peek showed"
  send click $page_point
}
peek_notes
# Clear Cache and Refresh, from the View menu, and Clear Cookies and Refresh, from the command bar,
# delete the page's site's cache and cookies and load the page again.
send key k
expect "^browser: title $a cookie [^ ]* cache 1\$" "k in the served page didn't open the cookie page"
send key c
expect "^browser: title $a cookie set\$" "c in the cookie page didn't set a cookie"
send key r cmd
expect "^browser: title $a cookie c cache 1\$" \
  "⌘R lost the cookie or fetched the cached script again: $(grep '^browser: title' "$log" | tail -1)"
send menu Clear Cache and Refresh
expect '^browser: command clear-cache-and-refresh via ui$' "Clear Cache and Refresh didn't run"
expect "^browser: title $a cookie c cache 2\$" \
  "Clear Cache and Refresh didn't fetch the script again, or lost the cookie: $(grep '^browser: title' "$log" | tail -1)"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type clear cookies
expect '^browser: command bar commands "clear cookies": Clear Cookies and Refresh$' \
  "the command bar didn't find Clear Cookies and Refresh"
send key return
expect '^browser: command clear-cookies-and-refresh via command bar$' \
  "Clear Cookies and Refresh didn't run from the command bar"
expect "^browser: title $a cookie none cache 2\$" \
  "Clear Cookies and Refresh didn't delete the cookie, or fetched the script again: $(grep '^browser: title' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/clear-site.png" || fail "no window to capture"
send key left cmd
expect "^browser: page $a back 0 forward 1 loading 0 $site\$" "⌘← didn't go back to the served page"
# Clear on the divider, which sits under the one pinned tab, the URL field's DevTools button in
# Developer Mode, Copy URL and Site Controls buttons, and the served page's text field, along the
# bottom of the 800 pt tall window.
clear_point="200 185"
devtools_point="150 64"
copy_point="178 64"
site_controls_point="206 64"
# The field is the page's bottom 60 pt, the page 10 above the window's foot, wherever the screen
# leaves that: Windows' runners have given the window 761 and 792 pt.
send layout
drawn_height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
case "$drawn_height" in
  [0-9]*) field_point="600 $((drawn_height - 40))" ;;
  *) fail "the driver didn't say how tall the window is: $drawn_height" ;;
esac
open_from_bar example.com
b=$tab
open_from_bar example.org
c=$tab
switch "3 cmd" "$a"
send key d cmd
expect_lists "$a" "$c $b" "after pinning the served page's tab"
switch "2 cmd" "$c"

# Share on ⇧⌘C's toast, where there's a share sheet, shares the tab's page as Site Controls' Share
# does; a driven run logs it and shows no sheet. The toast's window is at the page's top left, 228
# and 10 into the window, and Share is the plate's last chip, 31 wide and 14 in from its right. The
# window lets the pointer through until a move over its plate, so the first move has it take the
# pointer and the toast hears the second, as window.sh's new tab's toast does. The toast leaves
# after 2 s, so the pointer goes on Share at once, which keeps it past its time, and $2, if any, runs
# before the click, as the window's capture.
share_from_url_toast() {
  [ "$(uname)" != Linux ] || return 0
  local what=$1 then=${2:-} share first_move
  expect '^browser: toast plate [0-9]* [0-9]* [0-9]* [0-9]* in [0-9]* [0-9]*$' \
    "⇧⌘C's toast, $what, didn't tell where its plate is"
  set -- $(grep '^browser: toast plate' "$log" | tail -1 | sed 's/^browser: toast plate //; s/ in / /')
  share="$((228 + $1 + $3 - 29)) $((10 + $2 + $4 / 2))"
  send move $share
  first_move=$step
  send move $share
  expect '^browser: toast hovered true$' "the pointer at $share wasn't on ⇧⌘C's toast, $what" \
    "$first_move"
  [ -z "$then" ] || $then
  send click $share
  expect '^browser: toast button share$' "a click on Share at $share on ⇧⌘C's toast, $what, didn't take"
  expect '^browser: share https://example.org/$' "Share on ⇧⌘C's toast, $what, didn't share the page"
}
url_toast_capture() {
  ../tools/window-screenshot.sh "$pid" "$out/url-toast.png" || fail "no window to capture"
}

# ⇧⌘C and ⌥⇧⌘C copy the tab's URL and a Markdown link to it, from the page and the UI. A driven
# run copies to a pasteboard of its own, never the clipboard.
send key c cmd shift
expect '^browser: command copy-url via page$' "⇧⌘C with the page focused didn't reach the UI"
expect '^browser: copied https://example.org/$' "⇧⌘C didn't copy the tab's URL"
clipboard_holds https://example.org/ ||
  fail "⇧⌘C left the clipboard holding \"$(WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -n)\""
# The toast says so, 8 inside the card's top right corner.
expect "$url_toast" "⇧⌘C's toast didn't say it copied the URL, with Share where there's a share sheet"
expect '^browser: toast settled$' "⇧⌘C's toast didn't settle"
# Captured with the pointer on Share, or at once where there's none.
if [ "$(uname)" = Linux ]; then
  url_toast_capture
else
  share_from_url_toast "from the page" url_toast_capture
  expect '^browser: toast hidden$' "⇧⌘C's toast stayed after its Share"
fi
send click $sidebar_point
send key c cmd shift alt
expect '^browser: command copy-markdown-link via ui$' "⌥⇧⌘C with the UI focused didn't run"
expect '^browser: copied \[Example Domain](https://example.org/)$' \
  "⌥⇧⌘C didn't copy a Markdown link to the tab"
# Its toast comes where the last one went from under the pointer, which is on the sidebar now, so
# it isn't hovered and leaves on time (#2076).
expect '^browser: toast shown Copied URL as Markdown' "⌥⇧⌘C showed no toast"
expect '^browser: toast settled$' "⌥⇧⌘C's toast didn't settle"
none_since '^browser: toast hovered true$' ||
  fail "⌥⇧⌘C's toast, where ⇧⌘C's went from under the pointer, took the pointer on the sidebar as on it"
clipboard_holds '[Example Domain](https://example.org/)' ||
  fail "⌥⇧⌘C left the clipboard holding \"$(WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -n)\""

# ⌥⌘V, from the UI and the page, and the command bar's Paste URL as New Tab open the copied link in
# a new tab at the top of Today, as Arc's do. Text that isn't a web link, a Markdown link here,
# opens nothing, not a search.
rows=$(today)
send key v cmd alt
expect '^browser: command paste-url-as-tab via ui$' "⌥⌘V with the UI focused didn't reach the UI"
expect '^browser: pasted no URL$' "⌥⌘V with a Markdown link copied didn't open nothing"
none_since '^browser: loaded ' || fail "⌥⌘V opened a page for a Markdown link"
send key c cmd shift
expect '^browser: command copy-url via ui$' "⇧⌘C with the UI focused didn't reach the UI"
expect '^browser: copied https://example.org/$' "⇧⌘C from the UI didn't copy the tab's URL"
expect "$url_toast" "⇧⌘C from the UI showed no toast"
share_from_url_toast "from the UI"
bar_command paste-url-as-tab "Paste URL as New Tab" "paste url"
expect "^browser: today [0-9][0-9]* $rows\$" \
  "Paste URL as New Tab didn't open a tab at the top of Today: \"$(today)\""
p=$(today | cut -d' ' -f1)
expect "^browser: loaded $p https://example.org/\$" \
  "Paste URL as New Tab didn't open the link copied"
send click $page_point
send key v cmd alt
expect '^browser: command paste-url-as-tab via page$' \
  "⌥⌘V with the page focused didn't reach the UI"
expect "^browser: today [0-9][0-9]* $p $rows\$" \
  "⌥⌘V didn't open a tab at the top of Today: \"$(today)\""
q=$(today | cut -d' ' -f1)
expect "^browser: loaded $q https://example.org/\$" "⌥⌘V didn't open the link copied"
../tools/window-screenshot.sh "$pid" "$out/pasted-url.png" || fail "no window to capture"
# The tab copied from is shown again, and a middle-click on each pasted tab's row closes it, the
# lower first, so the other stays put. The pinned tab's row above Today puts Today's top at row 1.
expect '^browser: rows settled$' "Today's rows didn't settle after ⌥⌘V"
send click $(row 3)
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "a click on $c's row didn't show it"
send middleclick $(row 2)
expect "^browser: tab $p closed\$" "a middle-click didn't close the tab Paste URL as New Tab opened"
send middleclick $(row 1)
expect "^browser: tab $q closed\$" "a middle-click didn't close the tab ⌥⌘V opened"
expect "^browser: today $rows\$" "the pasted tabs didn't go: \"$(today)\""

# Clear and the URL field's button fade in while the pointer is over the sidebar, and out when it
# leaves. The button copies the URL, and doesn't open the command bar as the rest of the field does.
away
send move $sidebar_point
expect '^browser: sidebar hovered 1$' "the sidebar didn't see the pointer come over it"
expect '^browser: sidebar tools shown$' "Clear and the URL field's button didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/sidebar-hovered.png" || fail "no window to capture"
send click $copy_point
expect '^browser: command copy-url via sidebar$' \
  "a click on the URL field's button didn't run Copy URL"
expect '^browser: copied https://example.org/$' "the URL field's button didn't copy the tab's URL"
none_since '^browser: command edit-url' ||
  fail "a click on the URL field's button opened the command bar"
# It says so with ⇧⌘C's toast, and with a checkmark in its glyph's place for 2 s, which draws
# itself in as the link leaves.
expect "$url_toast" "the URL field's button showed no toast"
expect '^browser: copy url draws check$' "the URL field's button didn't start drawing a checkmark"
expect '^browser: copy url shows check$' "the URL field's checkmark didn't finish drawing"
../tools/window-screenshot.sh "$pid" "$out/url-copied.png" || fail "no window to capture"
expect '^browser: copy url brings back link$' "the URL field's button kept its checkmark"
expect '^browser: copy url shows link$' "the URL field's link didn't come all the way back"
# A tab's row under the pointer has the row's hover fill, captured before its hover card's dwell.
# The pinned tab's row above Today puts the top Today tab at row 1.
send move $(row 1)
expect '^browser: hover tab [0-9]* close ' "the pointer on the top tab's row didn't hover it"
../tools/window-screenshot.sh "$pid" "$out/row-hovered.png" || fail "no window to capture"
send move $page_point
expect '^browser: sidebar hovered 0$' "the sidebar didn't see the pointer leave"
expect '^browser: sidebar tools hidden$' "Clear and the URL field's button didn't fade out"
../tools/window-screenshot.sh "$pid" "$out/sidebar-away.png" || fail "no window to capture"

# ⇧⌘K with the page focused closes the Today tabs but the one shown, here the middle of three,
# without showing or loading another.
open_from_bar example.net
x=$tab
switch "3 cmd" "$c"
send click $page_point
send key k cmd shift
expect '^browser: command clear via page$' "⇧⌘K with the page focused didn't reach the UI"
expect '^browser: cleared 2$' "Clear didn't close the two tabs not shown"
expect "^browser: tab $b closed\$" "Clear didn't close the bottom tab"
expect "^browser: tab $x closed\$" "Clear didn't close the top tab"
expect_lists "$a" "$c" "after Clear"
none_since '^browser: selected' || fail "Clear showed another tab"
none_since '^browser: loaded' || fail "Clear loaded a page"
none_since '^browser: toast' || fail "Clear showed a toast"
../tools/window-screenshot.sh "$pid" "$out/clear.png" || fail "no window to capture"

# ⌘Z with the page focused puts them back in place, unloaded, and the tab shown stays.
send key z cmd
expect '^browser: command undo via page$' "⌘Z with the page focused didn't undo Clear"
expect "^browser: today [0-9][0-9]* $c [0-9][0-9]*\$" \
  "Undo didn't put the tabs back around the one shown: Today is \"$(today)\""
b=$(today | cut -d' ' -f3)
expect "^browser: page $b back 0 forward 0 loading 0 https://example.com/\$" \
  "the bottom tab didn't come back at its page"
expect "^browser: page $b unloaded 1\$" "the tab Undo brought back came back loaded"
none_since '^browser: selected' || fail "Undo showed another tab"
none_since '^browser: loaded' || fail "Undo loaded a page"
# ⇧⌘Z closes them again, still without a toast, and ⌘Z brings them back once more.
send key z cmd shift
expect '^browser: command redo via page$' "⇧⌘Z with the page focused didn't redo Clear"
expect_lists "$a" "$c" "after redoing Clear"
none_since '^browser: toast' || fail "redoing Clear showed a toast"
send key z cmd
expect '^browser: command undo via page$' "⌘Z after redoing Clear didn't run"
expect "^browser: today [0-9][0-9]* $c [0-9][0-9]*\$" \
  "Undo after redoing Clear didn't put the tabs back around the one shown: Today is \"$(today)\""
b=$(today | cut -d' ' -f3)
expect "^browser: page $b unloaded 1\$" "the tab Undo brought back the second time came back loaded"

# The divider's Clear, under the pointer.
send move $clear_point
expect '^browser: sidebar hovered 1$' "the sidebar didn't see the pointer come over Clear"
expect '^browser: sidebar tools shown$' "Clear didn't fade in under the pointer"
send click $clear_point
expect '^browser: command clear via sidebar$' "a click on Clear didn't run it"
expect '^browser: cleared 2$' "Clear on the divider didn't close the two tabs not shown"
expect_lists "$a" "$c" "after Clear on the divider"

# With a Clear to undo, ⌘Z in the page's text field undoes typing there.
switch "1 cmd" "$a"
send click $field_point
send type ab
expect '^browser: title [0-9]* value ab$' "typing didn't reach the served page's text field"
send key z cmd
expect '^browser: title [0-9]* value a$' "⌘Z in the page's text field didn't undo typing there"
none_since '^browser: command undo' || fail "⌘Z in the page's text field undid Clear"

# ⇧⌘K with the UI focused and the pinned tab shown closes every Today tab, then finds none.
send click $sidebar_point
send key k cmd shift
expect '^browser: command clear via ui$' "⇧⌘K with the UI focused didn't run"
expect '^browser: cleared 1$' "⇧⌘K with the pinned tab shown didn't close the Today tab"
expect "^browser: tab $c closed\$" "⇧⌘K with the UI focused didn't close the Today tab"
expect_lists "$a" " *" "after Clear with the pinned tab shown"
send key k cmd shift
expect '^browser: command clear via ui$' "⇧⌘K without Today tabs didn't run"
none_since '^browser: cleared' || fail "Clear without Today tabs closed some"

# ⇧⌘T with the UI focused reopens the tab closed last and shows it.
send key t cmd shift
expect '^browser: command reopen-closed-tab via ui$' "⇧⌘T with the UI focused didn't reach the UI"
expect "^browser: today [0-9][0-9]*\$" "⇧⌘T didn't reopen the tab Clear closed: Today is \"$(today)\""
d=$(today)
expect "^browser: selected $d\$" "⇧⌘T didn't show the tab it reopened"
expect "^browser: loaded $d https://example.org/\$" "the reopened tab didn't load its page"

# With the one Today tab shown, Clear has an arrow before it, lights up under the pointer, and
# closes that tab too, showing the pinned one. ⌘Z brings it back, shown, with the arrow.
expect '^browser: clear arrow 1$' "Clear had no arrow with the one Today tab shown"
send move $clear_point
expect '^browser: clear hovered 1$' "Clear didn't see the pointer come over it"
../tools/window-screenshot.sh "$pid" "$out/clear-arrow.png" || fail "no window to capture"
send click $clear_point
expect '^browser: command clear via sidebar$' "a click on Clear with its arrow didn't run it"
expect '^browser: cleared 1$' "Clear with its arrow didn't close the last Today tab"
expect "^browser: tab $d closed\$" "Clear with its arrow didn't close the tab shown"
expect_lists "$a" " *" "after clearing the last Today tab"
expect "^browser: selected $a\$" "clearing the last Today tab didn't show the pinned tab"
expect '^browser: clear arrow 0$' "Clear's arrow stayed with no Today tabs"
send key z cmd
expect '^browser: command undo via ui$' "⌘Z after clearing the last Today tab didn't run"
expect "^browser: today [0-9][0-9]*\$" "⌘Z didn't bring back the last Today tab: Today is \"$(today)\""
d=$(today)
expect "^browser: selected $d\$" "⌘Z didn't show the last Today tab again"
expect "^browser: loaded $d https://example.org/\$" "the last Today tab didn't load once shown"
expect '^browser: clear arrow 1$' "Clear's arrow didn't come back with the last Today tab"
../tools/window-screenshot.sh "$pid" "$out/clear-arrow-undone.png" || fail "no window to capture"

# ⇧⌘T with the page focused puts a tab back where it was, with its history.
open_from_bar example.net
e=$tab
go_from_bar example.org
expect_page example.org 1 0 "after going on from example.net"
open_from_bar example.com
f=$tab
switch "3 cmd" "$e"
# Muted, it comes back muted.
bar_command toggle-mute "Mute/Unmute Tab" mute
expect "^browser: page $e audible 0 muted 1\$" "Mute/Unmute Tab didn't mute the middle tab"
# With an icon, it comes back with it.
post rightclick $(row 2)
expect "^browser: menu shown $(cased '.*, Change Icon…, Rename…, ')" \
  "a right-click on the middle Today row didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for tab $e on emoji\$" \
  "Change Icon… didn't open the icon picker for the middle tab"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker under the middle tab"
expect '^browser: icon picker drawn$' "the icon picker didn't draw itself on screen"
send insert glowing star
send key return
expect "^browser: page $e icon 🌟\$" "the picker's pick didn't become the middle tab's icon"
expect '^browser: icon picker hidden$' "picking an emoji didn't close the icon picker"
send click $page_point
send key w cmd
expect "^browser: tab $e closed\$" "⌘W didn't close the middle Today tab"
expect_lists "$a" "$f $d" "after closing the middle Today tab"
send click $page_point
send key t cmd shift
expect '^browser: command reopen-closed-tab via page$' "⇧⌘T with the page focused didn't reach the UI"
expect_lists "$a" "$f [0-9][0-9]* $d" "after ⇧⌘T, the tab isn't back between the two it was between"
e=$(today | cut -d' ' -f2)
tab=$e
expect "^browser: selected $e\$" "⇧⌘T didn't show the tab it reopened"
expect "^browser: page $e audible 0 muted 1\$" "⇧⌘T didn't keep the tab muted"
expect "^browser: page $e icon 🌟\$" "⇧⌘T didn't keep the tab's icon"
expect_page example.org 1 0 "after ⇧⌘T"
../tools/window-screenshot.sh "$pid" "$out/reopen.png" || fail "no window to capture"

# After Clear, ⇧⌘T reopens the tab it closed last, the top one, and ⌘Z with the UI focused brings
# back only the other.
send click $page_point
send key k cmd shift
expect '^browser: command clear via page$' "⇧⌘K with the page focused didn't reach the UI"
expect '^browser: cleared 2$' "Clear didn't close the two tabs not shown"
expect_lists "$a" "$e" "after clearing around the tab shown"
send click $sidebar_point
send key t cmd shift
expect '^browser: command reopen-closed-tab via ui$' "⇧⌘T after Clear didn't reach the UI"
expect "^browser: today [0-9][0-9]* $e\$" \
  "⇧⌘T after Clear didn't reopen the top tab: Today is \"$(today)\""
t=$(today | cut -d' ' -f1)
send key z cmd
expect '^browser: command undo via ui$' "⌘Z with the UI focused didn't run"
expect "^browser: today $t $e [0-9][0-9]*\$" \
  "Undo after ⇧⌘T didn't bring back just the bottom tab: Today is \"$(today)\""
none_since "^browser: selected" || fail "Undo after ⇧⌘T showed another tab"

# A right-click on a row shows its tab's menu, Arc's for a Today tab: Copy Link and Share, its
# look, Mute (Unmute while muted, dimmed while silent), its place, then Archive Tab and Archive Tabs
# Below. Duplicate opens a copy under it, with its history and icon, and shows it. Rows count from
# the one pinned tab. No check chooses Copy Link or Share.
d=$(today | cut -d' ' -f3)
t=$(today | cut -d' ' -f1)
post rightclick $(row 2)
expect "^browser: menu shown $(cased "Copy Link$share, Change Icon…, Remove Icon, Rename…, Unmute, New Boost…, Add Split View, Duplicate, Add to Folder › New Folder, Move to › [^,]* (checked), Move to › New Space…, Archive Tab, Archive Tabs Below")\$" "a right-click on a Today row didn't show Arc's tab menu"
../tools/window-screenshot.sh "$pid" "$out/tab-menu.png" || fail "no window to capture"
send choose Duplicate
expect '^browser: command duplicate-tab via tab menu$' "Duplicate didn't run"
expect_lists "$a" "$t $e [0-9][0-9]* $d" "after Duplicate"
tab=$(today | cut -d' ' -f3)
expect "^browser: selected $tab\$" "Duplicate didn't show the copy"
expect "^browser: page $tab icon 🌟\$" "Duplicate didn't copy the tab's icon"
expect_page example.org 1 0 "the copy Duplicate made"
# The window is titled by the tab shown, which the Window menu lists it by. The menu bar names its
# items as Arc's, by what they'd do now. Edit has Paste and Match Style, which no check chooses,
# then Paste URL as New Tab.
send title
expect '^browser: key window titled Example Domain$' "the window isn't titled by the tab shown"
maybe='\( (disabled)\)\{0,1\}'
send menubar Edit
expect "^browser: menu bar Edit: .*, Paste$maybe, Paste and Match Style$maybe, Paste URL as New Tab$maybe, Select All" \
  "the Edit menu has no Paste and Match Style and Paste URL as New Tab after Paste"
send menubar View
expect "^browser: menu bar View: \(Appearance › [^,]*, \)*Hide Sidebar$maybe, " "View doesn't name ⌘S Hide Sidebar"
send menubar Tabs
expect "^browser: menu bar Tabs: .*, Pin Tab$maybe, .*, Reveal Tab in Sidebar$maybe, Focus Sidebar, Clear Today" \
  "Tabs doesn't name ⌘D Pin Tab for a Today tab, or lacks Reveal Tab in Sidebar, Focus Sidebar or Clear Today"
send menubar File
expect "^browser: menu bar File: .*, Open Command Bar$maybe, New Profile (disabled), Close Window" \
  "File has no New Profile after Open Command Bar, dimmed while the Space has tabs"
send menubar Spaces
expect "^browser: menu bar Spaces: New Space…$maybe, Edit Theme…$maybe, Rename Space$maybe, Change Profile › Default (checked)$maybe, Change Profile › New Profile (disabled), Next Space" \
  "Spaces has no Edit Theme… after New Space…, or Change Profile after Rename Space, with the Space's profile checked"
send menubar Archive
expect "^browser: menu bar Archive: .*, View Archive…$maybe, " "Archive has no View Archive…"
send menubar Window
expect "^browser: menu bar Window: .*, Open Library…$maybe, View Downloads…" \
  "Window has no Open Library… and View Downloads…"
# Help has Arc's help, the shortcuts and Report a Problem…, and Troubleshooting's Copy Build Info
# copies this copy's version, the system's and the chip, read from the computer.
if [ "$(uname)" = Darwin ]; then
  send menubar Help
  expect "^browser: menu bar Help: Welcome to [^,]*, Help Center$maybe, What's New$maybe, Essential Keyboard Shortcuts$maybe, Report a Problem…$maybe, \(Restore Earlier Sidebar.*, \)\?Troubleshooting › Copy Build Info$maybe, Troubleshooting › Open Task Manager" \
    "Help lacks Help Center, What's New, the shortcuts, Report a Problem… or Troubleshooting"
  post openmenu Help
  expect '^browser: menu tracked Welcome to ' "the Help menu didn't open"
  ../tools/window-screenshot.sh "$pid" "$out/help-menu.png" || fail "no window to capture"
  post dismiss
  send menu "Troubleshooting › Copy Build Info"
  expect '^browser: command copy-build-info via ui$' "Copy Build Info didn't run"
  info=$(grep -A3 '^browser: copied [^ ]* [0-9][0-9.]*' "$log" | tail -4)
  echo "$info" | grep -q '^macOS [0-9][0-9]*\.[0-9]' || fail "Copy Build Info has no macOS version: $info"
  echo "$info" | grep -q '^Apple ' || fail "Copy Build Info has no chip: $info"
  echo "ok: Help lists Help Center, What's New, the shortcuts, Report a Problem… and" \
    "Troubleshooting, $out/help-menu.png. Copy Build Info copied: $(echo "$info" | sed 's/^browser: copied //' | tr '\n' ';')"
else
  echo "skip: Help's items and Troubleshooting › Copy Build Info: what it reads, the macOS version and the Apple chip, is the Mac's, and off the Mac it isn't checked yet."
fi
# Archive Tabs Below shows the tab right-clicked first, so none below it loads on the way out.
post rightclick $(row 1)
expect "^browser: menu shown $(cased "Copy Link$share, Change Icon…, Rename…, Mute (disabled), New Boost…, Add Split View, Duplicate, Add to Folder › New Folder, Move to › [^,]* (checked), Move to › New Space…, Archive Tab, Archive Tabs Below")\$" "a right-click on the top Today row didn't show Arc's tab menu"
send choose "$(cased 'Archive Tabs Below')"
expect '^browser: command close-tabs-below via tab menu$' "Archive Tabs Below didn't run"
expect_lists "$a" "$t" "after Archive Tabs Below"
expect "^browser: tab $e closed\$" "Archive Tabs Below didn't close the tab under the one right-clicked"
expect "^browser: tab $d closed\$" "Archive Tabs Below didn't close the bottom tab"
expect "^browser: selected $t\$" "Archive Tabs Below didn't show the tab right-clicked"
[ "$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: selected ')" = 1 ] ||
  fail "Archive Tabs Below showed a tab on the way out"
# It says so with an Undo, and one ⌘Z brings back the three tabs under the one right-clicked, the
# copy Duplicate made among them, in place, and shows the copy again, as it was shown when they
# closed; ⇧⌘Z closes them again.
expect '^browser: toast shown Tabs Closed \[undo\]$' "Archive Tabs Below didn't offer Undo"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Archive Tabs Below"
expect "^browser: today $t [0-9][0-9]* [0-9][0-9]* [0-9][0-9]*\$" \
  "⌘Z didn't bring back the three tabs under the one right-clicked: Today is \"$(today)\""
copy=$(today | cut -d' ' -f3)
expect "^browser: selected $copy\( in .*\)\{0,1\}\$" \
  "⌘Z after Archive Tabs Below didn't show the copy that was shown when they closed"
[ "$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: selected ')" = 1 ] ||
  fail "⌘Z after Archive Tabs Below showed the tabs it brought back on the way"
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Archive Tabs Below"
expect_lists "$a" "$t" "after redoing Archive Tabs Below"
# ⇧⌘T brings them back one at a time, each at its place.
send key t cmd shift
expect "^browser: today $t [0-9][0-9]*\$" "⇧⌘T after Archive Tabs Below didn't reopen the tab under the one right-clicked: Today is \"$(today)\""
# A pinned tab's menu unpins it, and goes back to its pinned URL only when it's away from it.
post rightclick $(pinned_row 0)
expect "^browser: menu shown $(cased 'Unpin Tab, Add to Favorites, Back to Pinned URL (disabled), Pinned URL › Replace Pinned URL with Current (disabled), Pinned URL › Edit…, Rename Tab, Change Icon…, Duplicate Tab, Add to Folder › New Folder, Move to Space (disabled), Close Tab')\$" "a right-click on a pinned row didn't show its tab's menu"
send choose "$(cased 'Unpin Tab')"
expect '^browser: command toggle-pin via tab menu$' "Unpin Tab didn't run"
expect "^browser: today $a $t [0-9][0-9]*\$" "Unpin Tab didn't move the tab to the top of Today: Today is \"$(today)\""

# ⌘W archives the tab shown. ⇧⌘L opens the Library, which pushes the page right at its width, and
# a click on the archived tab reopens it at the top of Today with its history, shows it and closes
# the Library.
shown=$(grep '^browser: selected' "$log" | tail -1 | cut -d' ' -f3)
history=$(grep "^browser: page $shown back " "$log" | tail -1 | sed 's/^browser: page [0-9]* //')
url=${history##* }
send click $sidebar_point
send key w cmd
expect "^browser: tab $shown closed\$" "⌘W didn't close the tab shown"
expect '^browser: archive [1-9][0-9]* tabs$' "⌘W didn't archive the tab"
expect "^browser: today [0-9 ]*\$" "⌘W didn't take the tab out of Today"
left=$(today)
send layout
layout=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout')
content=$(echo "$layout" | sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
docked=$(echo "$layout" | sed -n 's/.*WebContentsViewCocoa \([0-9,]* [0-9x]*\).*/\1/p')
width=${content%x*}
height=${content#*x}
send key l cmd shift
expect '^browser: command view-library via ui$' "⇧⌘L with the UI focused didn't reach the UI"
expect '^browser: library shown$' "⇧⌘L didn't open the Library"
send layout
view=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed -n 's/.*WebContentsViewCocoa \([0-9,]* [0-9x]*\).*/\1/p')
[ "$view" = "417,10 ${docked#* }" ] ||
  fail "the Library didn't push the page to 417 at its width: it's at $view, docked at $docked"
../tools/window-screenshot.sh "$pid" "$out/library.png" || fail "no window to capture"
# History lists the pages this run visited among its closed tabs (#2337). The pointer on a row
# shows its trash, and its title truncates before it; a click on a visited page's row opens the
# page in Peek, as a closed tab's does, and Esc closes it, leaving the Library.
send move 300 111
expect '^browser: history pointed closed [0-9][0-9]*$' \
  "the pointer on History's first row, the tab ⌘W closed, didn't point at it"
../tools/window-screenshot.sh "$pid" "$out/history-hover-closed.png" || fail "no window to capture"
# The run's closed tabs come first, newest first, so its visited pages can start low on the list:
# each row the window shows, down to its foot.
visited_y=
for y in 167 223 279 335 391 447 503 559 615 671 727; do
  send move 300 $y
  expect '^browser: history pointed \(closed\|visited\) [0-9][0-9]*$' \
    "the pointer on History's row at y $y didn't point at it"
  if sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: history pointed visited '; then
    visited_y=$y
    break
  fi
done
[ -n "$visited_y" ] || fail "History listed no visited page in the rows the window shows; they were: $(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: history pointed ' | cut -d' ' -f4- | tr '\n' ' ')"
../tools/window-screenshot.sh "$pid" "$out/history-hover-visited.png" || fail "no window to capture"
# Remove from History, in a visited page's menu, takes the row's day of it out of the profile's
# history through the core, and History lists one page fewer once the core says so; ⌘Z puts it
# back, which the core doesn't say, so History asks again and lists it (#2337). Before Peek, whose
# visit would put the page back above the closed tabs, so the rows stay where they were.
listed=$(grep '^browser: history lists [0-9]* pages$' "$log" | tail -1 | cut -d' ' -f4)
[ -n "$listed" ] || fail "History never said how many pages it lists"
post rightclick 300 $visited_y
expect "^browser: menu shown $(cased 'Open in New Tab, Copy URL, Remove from History')\$" \
  "a right-click on a visited page's row didn't show its menu"
send choose "$(cased 'Remove from History')"
expect '^browser: history removed a page$' "Remove from History didn't remove the visited page"
expect "^browser: history lists $((listed - 1)) pages\$" \
  "History didn't list $((listed - 1)) pages, one fewer than $listed, once the core took the page out"
send key z cmd
expect "^browser: history lists $listed pages\$" \
  "⌘Z didn't bring the removed page back to History's $listed pages"
send click 300 $visited_y
expect '^browser: history page in preview$' "a click on a visited page's row didn't open it in Peek"
none_since '^browser: history open$' || fail "a click on a visited page's row opened it at once"
expect '^browser: link preview [1-9][0-9]* https\{0,1\}://' "Peek of the visited page didn't open at a page"
peek=$(grep '^browser: link preview [1-9][0-9]* http' "$log" | tail -1 | cut -d' ' -f4)
expect '^browser: link preview settled$' "Peek of the visited page didn't settle"
../tools/window-screenshot.sh "$pid" "$out/history-peek-visited.png" || fail "no window to capture"
send key escape
expect '^browser: link preview closing (' "Esc didn't close Peek of the visited page"
expect "^browser: tab $peek closed\$" "Esc didn't close the page Peek showed"
none_since '^browser: library hidden$' || fail "closing Peek of a visited page closed the Library"
# A right-click on the archived tab shows its menu, whose Copy URL copies its URL.
post rightclick 300 111
expect "^browser: menu shown $(cased 'Reopen Tab, Copy URL, Delete from Archive')\$" \
  "a right-click on the archived tab didn't show its menu"
../tools/window-screenshot.sh "$pid" "$out/archived-menu.png" || fail "no window to capture"
send choose Copy URL
expect "^browser: copied $url\$" "Copy URL in the archived tab's menu didn't copy its URL"
# ⌘-click, Ctrl off the Mac, restores it at once, with its history, where a click previews it.
send click 300 111 cmd
expect '^browser: restore archived [0-9][0-9]*$' "a ⌘-click on the archived tab didn't restore it"
expect "^browser: today [0-9][0-9]* $left\$" \
  "the archived tab didn't come back at the top of Today: Today is \"$(today)\""
r=$(today | cut -d' ' -f1)
expect "^browser: selected $r\$" "the tab from the archive isn't shown"
expect "^browser: loaded $r $url\$" "the tab from the archive didn't load its page"
expect "^browser: page $r $history\$" "the tab from the archive came back without its history"
expect '^browser: library hidden$' "reopening a tab didn't close the Library"
send layout
view=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed -n 's/.*WebContentsViewCocoa \([0-9,]* [0-9x]*\).*/\1/p')
[ "$view" = "$docked" ] || fail "the page is at $view after the Library, not back at $docked"
# The Window menu's Open Library… opens it too, and the Archive menu's Clear Archive asks first,
# and return in the dialog empties it. The Library's back button closes it, and the footer's
# button opens it.
send menu Open Library…
expect '^browser: command view-library via ui$' "Open Library… didn't run"
expect '^browser: library shown$' "Open Library… didn't open the Library"
# Delete from Archive in a tab's menu takes it out, and the Library stays open. Peek's visit made
# its page History's newest, so a closed tab can list under it: the first row that points at one.
# The pointer starts off the rows, as the ⌘-click left it on the first.
archived=$(grep '^browser: archive [0-9]* tabs$' "$log" | tail -1 | cut -d' ' -f3)
send move 300 71
closed_y=
for y in 111 167 223 279 335 391 447 503 559 615 671 727; do
  send move 300 $y
  expect '^browser: history pointed \(closed\|visited\) [0-9][0-9]*$' \
    "the pointer on History's row at y $y didn't point at it"
  if sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: history pointed closed '; then
    closed_y=$y
    break
  fi
done
[ -n "$closed_y" ] || fail "History listed no closed tab in the rows the window shows; they were: $(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: history pointed ' | cut -d' ' -f4- | tr '\n' ' ')"
post rightclick 300 $closed_y
expect "^browser: menu shown $(cased 'Reopen Tab, Copy URL, Delete from Archive')\$" \
  "a right-click on an archived tab didn't show its menu"
send choose "$(cased 'Delete from Archive')"
expect '^browser: remove archived [0-9][0-9]*$' "Delete from Archive didn't run"
expect "^browser: archive $((archived - 1)) tabs\$" "Delete from Archive didn't take the tab out"
none_since '^browser: library hidden$' || fail "Delete from Archive closed the Library"
# A right-click on the Archived Tabs section offers Clear Archive, as Arc's does, which asks as
# the menu's does, and ⌘Z brings the tabs back. It's the fourth of six, with Agents (#1080) and
# Boosts (#695) last, 100 apart and centred 5 above the column's middle.
archive_section="56 $((height / 2 + 45))"
post rightclick $archive_section
expect "^browser: menu shown $(cased 'Clear Archive')\$" \
  "a right-click on the Archived Tabs section didn't offer Clear Archive"
../tools/window-screenshot.sh "$pid" "$out/archive-section-menu.png" || fail "no window to capture"
send choose "$(cased 'Clear Archive')"
expect '^browser: command clear-archive via library$' "Clear Archive from the section didn't run"
expect '^browser: dialog shown Clear the archive?$' "Clear Archive from the section didn't ask first"
send key return
expect '^browser: archive 0 tabs$' "Clear Archive from the section didn't empty the archive"
send key z cmd
expect "^browser: archive $((archived - 1)) tabs\$" "⌘Z didn't bring back the tabs Clear Archive took"
send menu Clear Archive
expect '^browser: dialog shown Clear the archive?$' "Clear Archive didn't ask first"
send key return
expect '^browser: archive 0 tabs$' "Clear Archive didn't empty the archive"
../tools/window-screenshot.sh "$pid" "$out/library-empty.png" || fail "no window to capture"
# With nothing in the archive, the section's Clear Archive has nothing to do.
post rightclick $archive_section
expect "^browser: menu shown $(cased 'Clear Archive (disabled)')\$" \
  "the section's Clear Archive wasn't dimmed with the archive empty"
send dismiss
# With the archive empty, History's search finds the page Peek showed at the start as its one
# page, which Copy URL shows is that page, and Remove from History takes it out. Emptying the
# search lists them all again.
send click 224 27
send type notes
expect '^browser: history lists 1 pages$' \
  "History's search for notes didn't list one page, the one Peek showed: $(grep '^browser: history lists' "$log" | tail -1)"
send move 300 71
send move 300 111
expect '^browser: history pointed visited [0-9][0-9]*$' \
  "the pointer on the search's first row didn't point at a visited page"
post rightclick 300 111
expect "^browser: menu shown $(cased 'Open in New Tab, Copy URL, Remove from History')\$" \
  "a right-click on the found page's row didn't show its menu"
send choose Copy URL
expect "^browser: copied $notes\$" \
  "the found page isn't the one Peek showed: its Copy URL copied $(grep '^browser: copied' "$log" | tail -1 | cut -d' ' -f3)"
post rightclick 300 111
expect "^browser: menu shown $(cased 'Open in New Tab, Copy URL, Remove from History')\$" \
  "a right-click on the page Peek showed didn't show its menu again"
send choose "$(cased 'Remove from History')"
expect '^browser: history removed a page$' "Remove from History didn't remove the page Peek showed"
expect '^browser: history lists 0 pages$' \
  "History's search still listed the page Peek showed once it was removed: $(grep '^browser: history lists' "$log" | tail -1)"
send click 224 27
send key a cmd
send key backspace
expect '^browser: history lists [1-9][0-9]* pages$' "emptying History's search didn't list its pages again"
send click 24 $((height - 24))
expect '^browser: command view-library via library$' "the Library's back button didn't run"
expect '^browser: library hidden$' "the back button didn't close the Library"
send click 24 $((height - 25))
expect '^browser: command view-library via sidebar$' "the footer's Library button didn't run"
expect '^browser: library shown$' "the footer's Library button didn't open the Library"
send key l cmd shift
expect '^browser: library hidden$' "⇧⌘L didn't close the Library"
# A copied URL leaves out the parameters that only tell a site where a click came from. On the
# check's own site: the tabs run types example.org, which this URL in history would complete to.
# Mailchimp's mc_cid, which uBlock Origin's lists leave in the address, as they do utm_ parameters,
# while they take fbclid out before the page loads (the adblock run).
go_from_bar "${site#http://}?utm_source=check&id=1&mc_cid=2"
expect "^browser: loaded $r ${site}.utm_source=check&id=1&mc_cid=2\$" \
  "the tab didn't go to the URL with tracking parameters"
send key c cmd shift
expect "^browser: copied ${site}.id=1\$" \
  "⇧⌘C copied the URL with its tracking parameters: $(grep '^browser: copied' "$log" | tail -1)"

# A page's question about a permission shows under the URL field, as Chrome's hangs under its
# address bar. Another tab takes it down, and coming back brings it again. With the sidebar hidden,
# a question brings it out and keeps it out until it's answered. × dismisses it, and the site may
# ask again; Block refuses, and the site's next ask is refused without one; Allow grants.
# asks <site> <what>: the page asks, and the card is drawn at $qx $qy, $qw by $qh inside its edge
# and has been on screen long enough for Block and Allow to work.
asks() {
  expect "^browser: page $r asks $1 to Manage windows on all your displays\$" "$2"
  expect "^browser: question $r at " "the page's question wasn't drawn"
  expect "^browser: question $r takes input\$" "the page's question never took input"
  send layout
  read -r qx qy qw qh <<EOF
$(grep "^browser: question $r at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
}
send click $page_point
send key a
asks "${site%/}" "the page's question didn't come"
../tools/window-screenshot.sh "$pid" "$out/question.png" || fail "no window to capture"
open_from_bar example.net
expect "^browser: page $r asks nothing\$" "showing another tab didn't take the page's question down"
switch "$(($(pinned | wc -w) + 2)) cmd" "$r"
asks "${site%/}" "showing the asking tab again didn't bring its question back"
send click $((qx + qw - 18)) $((qy + 18))
expect "^browser: page $r asks nothing\$" "× didn't take the question down"
expect "^browser: title $r refused NotAllowedError\$" "× didn't refuse the page"
send key s cmd
expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar"
send click $page_point
send key a
expect '^browser: reveal shown$' "the page's question didn't bring the hidden sidebar out"
asks "${site%/}" "the site couldn't ask again after ×"
away=$((step + 1))
send move 600 400
../tools/window-screenshot.sh "$pid" "$out/question-reveal.png" || fail "no window to capture"
# The pointer comes over the panel to Block, as a hand's does: on Linux the panel hears the pointer
# leave only once it has had it moving over it.
send move $((qx + qw / 4)) $((qy + qh - 26))
send click $((qx + qw / 4)) $((qy + qh - 26))
expect "^browser: page $r asks nothing\$" "Block didn't take the question down"
expect "^browser: title $r refused NotAllowedError\$" "Block didn't refuse the page"
none_since '^browser: reveal hidden$' "$away" || fail "the pointer leaving sent the question away"
send move 600 400
expect '^browser: reveal hidden$' "the pointer leaving didn't send the sidebar away once answered"
# A key that does nothing reports who has the keyboard, which the hidden panel must not.
send flags
in_browser RenderWidgetHostViewCocoa "the hidden sidebar's panel kept the keyboard"
send key s cmd
expect '^browser: sidebar shown$' "⌘S didn't bring the sidebar back"
send click $page_point
send key a
expect "^browser: title $r refused NotAllowedError\$" "the blocked site's next ask wasn't refused"
none_since "^browser: page $r asks http" || fail "the blocked site asked again"
# The page by its file's name, which no other run's history would complete it past.
go_from_bar "localhost:$port/index.html"
expect "^browser: loaded $r http://localhost:$port/index.html\$" "localhost didn't load in the tab"
send click $page_point
send key a
asks "http://localhost:$port" "another site's question didn't come"
away
send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
expect "^browser: page $r asks nothing\$" "Allow didn't take the question down"
expect "^browser: title $r granted\$" "Allow didn't grant the page"
# The click on Allow tells the sidebar the pointer is over it, uncovered as the question closes.
expect '^browser: sidebar tools shown$' \
  "Clear and the URL field's buttons didn't fade in under the pointer once the question closed"

# The Site Controls list what differs for the site of Chrome's page info, here the permission Allow
# gave. The panel hangs from the button at 206 64, and logs where it drew each part, which the
# clicks below read (click_placed).
controls="^browser: site controls http://localhost:$port: "
# What the Site Controls reported last, one a line, in their order: ad blocking, Developer Mode,
# then the permissions.
listed() {
  grep "$controls" "$log" | tail -1 | sed 's/^[^;]*; //' | tr ',' '\n' | sed 's/^ //'
}
send move $page_point
expect '^browser: sidebar tools hidden$' "Clear and the URL field's buttons didn't fade out"
send move $sidebar_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
send click $site_controls_point
expect '^browser: command site-controls via sidebar$' "a click on Site Controls didn't run it"
expect "${controls}[^;]*; .*Window management Allow" \
  "the Site Controls didn't list Window management as allowed"
expect '^browser: site controls shown$' "the Site Controls didn't show"
expect '^browser: site controls settled$' "the Site Controls didn't finish opening"
none_since '^browser: command edit-url' || fail "a click on Site Controls opened the command bar"
../tools/window-screenshot.sh "$pid" "$out/site-controls.png" || fail "no window to capture"
# Its value's menu offers Chrome's three, and Block blocks the site at once.
click_placed post "line Window management"
expect '^browser: menu shown Allow (checked), Block, Ask$' \
  "the value of Window management didn't offer Allow, Block and Ask"
send choose Block
expect '^browser: site controls set Window management Block$' "Block in the menu didn't run"
expect "${controls}[^;]*; .*Window management Block" \
  "the Site Controls didn't show Window management as blocked"
# Developer Mode is on by itself for localhost, and its switch shows while it's on (#2334): a click
# turns it off for the site, which the tab draws, and the switch leaves. "…" then has Turn On
# Developer Mode, which turns it on again from the command bar's command and closes the panel.
listed | grep -qx 'Developer Mode On' ||
  fail "the Site Controls didn't list localhost in Developer Mode"
grep -q "^browser: page $r developer mode 1\$" "$log" ||
  fail "the localhost tab wasn't in Developer Mode"
click_placed send "switch Developer Mode"
expect '^browser: site controls set developer mode off$' \
  "a click on Developer Mode didn't turn it off"
expect "${controls}[^;]*; .*Developer Mode Off" "the Site Controls didn't show Developer Mode off"
expect "^browser: page $r developer mode 0\$" "the tab didn't leave Developer Mode"
expect '^browser: site controls more at ' "the Site Controls didn't move their footer up"
click_placed send more
expect "^browser: menu shown .*, $(cased 'Turn On Developer Mode'), " \
  "out of Developer Mode the … menu didn't offer to turn it on"
send choose "$(cased 'Turn On Developer Mode')"
expect "^browser: site controls run $(cased 'Turn On Developer Mode')\$" \
  "Turn On Developer Mode didn't run"
expect "^browser: page $r developer mode 1\$" "the tab didn't go back to Developer Mode"
expect '^browser: site controls hidden$' "the Site Controls didn't close for Turn On Developer Mode"
away
send move $sidebar_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
send click $site_controls_point
expect '^browser: site controls settled$' "the Site Controls didn't open again in Developer Mode"
logged '^browser: site controls switch Developer Mode at ' ||
  fail "back in Developer Mode the Site Controls didn't show its switch"
# http has no certificate to show, so the lock doesn't take the click.
lock=$((step + 1))
click_placed send lock
# Share closes them and hangs the system's share menu from its tile. Linux has no share menu, so no
# Share tile, and escape closes them there instead.
if [ "$(uname)" = Darwin ]; then
  click_placed send "tile share"
  expect '^browser: site controls run share$' "a click on Share didn't run it"
  expect "^browser: share http://localhost:$port/index.html\$" "Share didn't share the page"
  expect '^browser: site controls hidden$' "the Site Controls didn't leave for Share"
else
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
fi
none_since '^browser: site controls run certificate' "$lock" || fail "the lock ran without a certificate"
# File › Share… shares the page shown too. A driven run shows no sheet, which would wait on a
# choice, so its line is the proof. Linux has no share sheet.
if [ "$(uname)" = Darwin ] || windows; then
  send menu 'Share…'
  expect "^browser: share http://localhost:$port/index.html\$" "File › Share… didn't share the page"
else
  echo "skip: File › Share…: Linux has no share sheet."
fi
# Hover still works once they've closed: the pointer out to the page hides the URL field's buttons,
# and back over the sidebar shows them. While they were open the buttons stayed, as the pointer
# over the sidebar keeps them.
away
send move $site_controls_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't show under the pointer again"
send click $site_controls_point
expect '^browser: site controls settled$' "the Site Controls didn't open after Share"
# The button again closes them.
send click $site_controls_point
expect '^browser: site controls closing (toggle)$' "Site Controls' second click didn't close them"
expect '^browser: site controls hidden$' "the Site Controls didn't leave"
# The command bar offers what would change the page's Developer Mode, and turns it off and on.
# Out of it the URL field says the host alone and the tab's row loses its outline, and in it the
# whole URL and the outline come back.
bar_command toggle-developer-mode "Turn off Developer Mode for this site\$" "developer mode"
expect "^browser: page $r developer mode 0\$" "the command bar didn't turn Developer Mode off"
expect '^browser: url field localhost$' "out of Developer Mode the URL field didn't say the host"
expect '^browser: developer outline none$' "out of Developer Mode the tab kept its outline"
bar_command toggle-developer-mode "Turn on Developer Mode for this site\$" "developer mode"
expect "^browser: page $r developer mode 1\$" "the command bar didn't turn Developer Mode on"
expect "^browser: url field http://localhost:$port/index.html\$" \
  "in Developer Mode the URL field didn't say the whole URL"
expect "^browser: developer outline $r at " "in Developer Mode the tab drew no outline"
# In Developer Mode the URL field has DevTools' button before Copy URL, under the pointer as those
# are: a click docks DevTools in the card, as ⌥⌘I does, and another closes them.
# Windows' driver lists the pages the UI lays out, and docked DevTools are an aura window in
# Chromium's, which Windows doesn't see, so there it doesn't count them, as find.sh. The Mac's sees
# them, and Linux's lists them beside their page (drive.rs).
case "$(uname)" in
  Darwin | Linux) ;;
  *) echo "skip: where DevTools' button docks DevTools: Windows' driver can't see aura windows." ;;
esac
page_views() {
  case "$(uname)" in Darwin | Linux) ;; *) return 0 ;; esac
  for _ in $(seq 50); do
    send layout
    views=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
      grep -o 'WebContentsViewCocoa' | wc -l | tr -d ' ')
    [ "$views" -eq "$1" ] && return 0
  done
  fail "$2: the window shows $views page views, not $1"
}
away
send move $devtools_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in in Developer Mode"
send click $devtools_point
expect '^browser: command devtools via sidebar$' "a click on DevTools' button didn't run DevTools"
page_views 2 "after a click on DevTools' button"
../tools/window-screenshot.sh "$pid" "$out/developer-devtools.png" || fail "no window to capture"
send click $devtools_point
expect '^browser: command devtools via sidebar$' "a second click on DevTools' button didn't run"
page_views 1 "after a second click on DevTools' button"
# So does View › Developer's last item, named by what it would do.
developer="Developer › JavaScript Console$maybe, Developer › Turn"
send menubar View
expect "^browser: menu bar View: .*, $developer off Developer Mode for this site$maybe, " \
  "View › Developer doesn't end with Turn off Developer Mode for this site"
send menu Developer › Turn off Developer Mode for this site
expect '^browser: command toggle-developer-mode via ui$' "View's Turn off Developer Mode didn't run"
expect "^browser: page $r developer mode 0\$" "View › Developer didn't turn Developer Mode off"
send menubar View
expect "^browser: menu bar View: .*, $developer on Developer Mode for this site$maybe, " \
  "View › Developer didn't name Turn on Developer Mode for this site once it was off"
send menu Developer › Turn on Developer Mode for this site
expect "^browser: page $r developer mode 1\$" "View › Developer didn't turn Developer Mode on"
# Off the Mac ⌃D is pressed as Alt+D, Chrome's for the address bar, so Developer Mode has no key
# there (commands.rs), and fields don't delete forward on it.
if [ "$(uname)" != Darwin ]; then
  echo "skip: ⌃D for Developer Mode and forward delete: off the Mac it's Alt+D, the address bar's."
else
  # So does ⌃D, which the page gets first: in its text field it deletes forward, as in any Mac
  # field, and so it does in the command bar's, and with the page's keyboard elsewhere, or the
  # UI's, it turns Developer Mode off and on.
  send click $field_point
  send key a cmd
  send type ab
  expect "^browser: title $r value ab\$" "typing didn't reach the localhost page's text field"
  send key left
  send key d ctrl
  expect "^browser: title $r value a\$" "⌃D in the page's text field didn't delete forward"
  none_since '^browser: command toggle-developer-mode ' ||
    fail "⌃D in the page's text field toggled Developer Mode"
  send click $page_point
  send key d ctrl
  expect '^browser: command toggle-developer-mode via page$' \
    "⌃D with the page focused didn't reach the UI"
  expect "^browser: page $r developer mode 0\$" "⌃D from the page didn't turn Developer Mode off"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type developer
  expect '^browser: command bar commands "developer": ' "the command bar didn't find developer"
  send key left
  send key d ctrl
  expect '^browser: command bar commands "develope": ' "⌃D in the command bar didn't delete forward"
  none_since '^browser: command toggle-developer-mode ' ||
    fail "⌃D in the command bar's field toggled Developer Mode"
  # ⌥← goes back a word and ⌥⌦ deletes the word after the caret, as in any Mac field, though
  # the input method has ⌥← first and only gives it back.
  send key space
  send type mode
  expect '^browser: command bar commands "develope mode": ' "typing didn't reach the command bar"
  send key left alt
  send key delete alt
  expect '^browser: command bar commands "develope ": ' \
    "⌥← and ⌥⌦ in the command bar didn't delete the word before the caret"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  # The sidebar's empty space under its rows.
  send click 120 650
  send key d ctrl
  expect '^browser: command toggle-developer-mode via ui$' "⌃D with the UI focused didn't run"
  expect "^browser: page $r developer mode 1\$" "⌃D from the UI didn't turn Developer Mode on"
fi
# The page asks and is refused without a question.
send click $page_point
send key a
expect "^browser: title $r refused NotAllowedError\$" \
  "the site blocked in Site Controls wasn't refused"
none_since "^browser: page $r asks http" || fail "the site blocked in Site Controls asked"
# From the page, the command bar's Site Controls open them, which kept the change, and Escape closes
# them.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type site controls
expect '^browser: command bar commands "site controls": Site Controls$' \
  "the command bar didn't find Site Controls"
send key return
expect '^browser: command site-controls via command bar$' "Site Controls didn't run from the bar"
expect "${controls}[^;]*; .*Window management Block" "the Site Controls didn't keep Block"
expect '^browser: site controls settled$' "the Site Controls didn't open from the command bar"
../tools/window-screenshot.sh "$pid" "$out/site-controls-blocked.png" ||
  fail "no window to capture"
send key escape
expect '^browser: site controls closing (escape)$' "escape didn't close the Site Controls"
expect '^browser: site controls hidden$' "the Site Controls didn't leave after escape"
none_since '^browser: site controls takes the keyboard back from its window$' ||
  fail "the Site Controls took the keyboard back from the browser window as they closed"
# Reset permissions sets the site back to asking, and a click outside closes them. The button
# shows only under the pointer, as it was last over the page.
away
send move $sidebar_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
send click $site_controls_point
expect '^browser: site controls settled$' "the Site Controls didn't open again"
click_placed send "line Reset [a-z]*"
expect '^browser: site controls reset$' "Reset permissions didn't run"
expect "${controls}" "the Site Controls didn't list the permissions again after the reset"
send click $page_point
expect '^browser: site controls closing (dismissed)$' \
  "a click outside didn't close the Site Controls"
expect '^browser: site controls hidden$' "the Site Controls didn't leave after a click outside"
send key a
asks "http://localhost:$port" "the site reset in Site Controls didn't ask again"
send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
expect "^browser: title $r granted\$" "Allow didn't grant the page after the reset"
# The footer's … menu is Arc's, with New Boost… and View Boosts… (#695), and All Site Settings… opens
# Chrome's for the site.
away
send move $sidebar_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
send click $site_controls_point
expect '^browser: site controls settled$' "the Site Controls didn't open for their … menu"
click_placed send more
more_menu=$(cased 'Clear Cache, Clear Cookies, New Boost…, View Boosts…, Manage Extensions…, Add Extension…, All Site Settings…')
expect "^browser: menu shown $more_menu\$" "the … menu didn't list Arc's items"
send choose "$(cased 'All Site Settings…')"
expect "^browser: site controls run $(cased 'All Site Settings…')\$" "All Site Settings… didn't run"
site_settings="chrome://settings/content/siteDetails?site=http%3A%2F%2Flocalhost%3A$port"
expect "^browser: loaded [0-9]* $site_settings\$" "All Site Settings… didn't open them"

# A page that asks for a key system Chromium can't play, such as Widevine, brings a note under the
# URL field. ClearKey plays, so a page asking for it brings none. The tab is the one shown, which
# All Site Settings… opened.
tab=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
go_from_bar "127.0.0.1:$port/clearkey.html"
expect "^browser: loaded $tab http://127.0.0.1:$port/clearkey.html\$" "the ClearKey page didn't load"
expect "^browser: title $tab clearkey asked\$" "the ClearKey page's request wasn't answered"
# The driver's round trip comes after the report the page's request would have made.
send layout
grep -q "^browser: page $tab needs " "$log" && fail "a page asking for ClearKey brought the note"
# The session page stores in Chromium a cookie from its server, HttpOnly, one from its script and
# an item, which go with the tab to Safari's engine below.
go_from_bar "127.0.0.1:$port/session?set"
expect "^browser: title $tab session chromium sees n item\$" \
  "the session page didn't store its cookie and item in Chromium"
go_from_bar "127.0.0.1:$port/drm.html"
expect "^browser: loaded $tab http://127.0.0.1:$port/drm.html\$" "the Widevine page didn't load"
expect "^browser: page $tab needs com.widevine.alpha\$" "the page asking for Widevine brought no note"
expect "^browser: drm note $tab at " "the note wasn't drawn"
send layout
read -r nx ny nw nh <<EOF
$(grep "^browser: drm note $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
../tools/window-screenshot.sh "$pid" "$out/drm-note.png" || fail "no window to capture"
# × in its corner, under the pointer, closes the note for the site, here localhost, the profile
# keeping it closed: not after a reload, nor in another tab, nor after another site and back. The
# core no longer reports it, read from the line the log had before each page asked.
none_after() { ! tail -n "+$(($1 + 1))" "$log" | grep -q "$2"; }
go_from_bar "localhost:$port/drm.html"
expect "^browser: page $tab needs com.widevine.alpha\$" \
  "localhost's page asking for Widevine brought no note"
# 127.0.0.1's note goes as the tab leaves its site, but when localhost's page asks before the next
# frame, the note stays drawn where it was, which logs no line. So it's the last line about it.
send layout
drawn=
for _ in $(seq 20); do
  if grep -E "^browser: drm note ($tab at |hidden\$)" "$log" | tail -1 | grep -q ' at '; then
    drawn=1
    break
  fi
  sleep 0.1
done
[ -n "$drawn" ] || fail "localhost's note wasn't drawn"
read -r nx ny nw nh <<EOF
$(grep "^browser: drm note $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
send move $((nx + nw / 2)) $((ny + nh / 2))
send click $((nx + nw - 18)) $((ny + 18))
expect "^browser: drm note $tab closed\$" "× didn't close the note"
expect '^browser: drm note hidden$' "the note stayed after ×"
asked() {
  expect "^browser: loaded $tab http://localhost:$port/drm.html\$" "localhost's page didn't load $1"
  expect "^browser: title $tab drm asked\$" "localhost's page didn't ask for Widevine $1"
  # The driver's round trip comes after the note the page's request would have brought.
  send layout
  none_after "$mark" "^browser: page $tab needs com" || fail "the closed note was reported again $1"
  none_after "$mark" "^browser: drm note $tab at " || fail "the closed note was drawn again $1"
}
mark=$(wc -l < "$log")
go_from_bar "localhost:$port/drm.html"
asked "on a reload"
closed=$tab
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
mark=$(wc -l < "$log")
send type "localhost:$port/drm.html"
send key return
expect '^browser: selected [1-9]' "⌘T's page showed no tab"
tab=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
[ "$tab" != "$closed" ] || fail "⌘T's page opened in the tab it was typed over"
asked "in another tab"
go_from_bar "127.0.0.1:$port/index.html"
expect "^browser: loaded $tab http://127.0.0.1:$port/index.html\$" "another site didn't load"
mark=$(wc -l < "$log")
go_from_bar "localhost:$port/drm.html"
asked "after another site and back"
# Another site's note still shows, and its button is next.
go_from_bar "127.0.0.1:$port/drm.html"
expect "^browser: page $tab needs com.widevine.alpha\$" "closing localhost's note closed 127.0.0.1's"
expect "^browser: drm note $tab at " "127.0.0.1's note wasn't drawn"
send layout
read -r nx ny nw nh <<EOF
$(grep "^browser: drm note $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
# WebKit loads with the first page in it, or earlier only to clear a store a page made in an
# earlier run, as Clear Cookies and Refresh above does once this run has played. Read before the
# click, whose page can say it loaded WebKit before the next line runs.
early=$(grep '^browser: system engine loaded for ' "$log" | grep -v ' for clearing a site' || true)
# On Linux the note's button downloads Google's Widevine module from Google, which the widevine run
# checks, so it isn't pressed here. On Windows it opens the default browser, which the runner's
# isn't ilo, and a driven run only logs it.
[ "$(uname)" = Linux ] || send click $((nx + nw / 2)) $((ny + nh - 26))
# Safari's engine keeps a store for each profile from macOS 14; before it, the note offers Safari.
if [ "$(uname)" = Linux ]; then
  drm_done="nothing, as on Linux it offers Google's Widevine module, the widevine run's"
  echo "skip: the note's button: on Linux it downloads Google's Widevine module (the widevine run)."
elif windows; then
  drm_done="the default browser"
  expect "^browser: open in default browser http://127.0.0.1:$port/drm.html\$" \
    "the note's button didn't open the page in the default browser"
  none_since "^browser: page $tab needs nothing\$" || fail "the button took the note away"
elif [ "$(sw_vers -productVersion | cut -d. -f1)" -ge 14 ]; then
  drm_done="Safari's engine"
  [ -z "$early" ] || fail "WebKit loaded before its first page: $early"
  # Play in This Tab shows the page in Safari's engine, in the tab, and the note goes.
  expect "^browser: system engine $tab on\$" "the note's button didn't show the page in Safari's engine"
  expect "^browser: page $tab needs nothing\$" "the note stayed in Safari's engine"
  grep -q '^browser: system engine loaded for ' "$log" || fail "Safari's engine's page didn't say WebKit loaded"
  expect "^browser: loaded $tab http://127.0.0.1:$port/drm.html\$" "Safari's engine didn't load the page"
  expect_late "^browser: title $tab drm asked\$" "Safari's engine didn't report the page's title"
  # The site's session came first: the page's first request carried both cookies, and its script
  # found the item.
  copied="^browser: copied [0-9]* cookies and [0-9]* storage items for 127.0.0.1 into the system's engine\$"
  expect "$copied" "the switch didn't copy the site's session into Safari's engine"
  cookies=$(grep "$copied" "$log" | tail -1 | cut -d' ' -f3)
  items=$(grep "$copied" "$log" | tail -1 | cut -d' ' -f6)
  [ "$cookies" -ge 2 ] && [ "$items" -ge 1 ] ||
    fail "the switch copied $cookies cookies and $items storage items, not the session page's"
  # The cookies are logged sorted, and Safari's engine may hold its own from a run before: the
  # store is named after the profile's path, which a check's wiped profile comes back at (#2396).
  served "^GET safari 127.0.0.1:$port/drm.html cookie .*h=$port;.* n=$port" \
    "Safari's engine's first request didn't carry the site's cookies from Chromium" "^GET safari "
  served "^GET safari 127.0.0.1:$port/seen?item=chromium " \
    "Safari's engine's page didn't find the item from Chromium as its script ran"
  # Chromium's page let go for the switch isn't a renderer lost, which a crash card would show.
  grep -q "^.*browser: tab $tab page gone" "$log" && fail "the switch to Safari's engine read as the page gone"
  ../tools/window-screenshot.sh "$pid" "$out/drm-engine.png" || fail "no window to capture"
  # The site's other pages stay there; another site goes back to Chromium, in the same tab.
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine safari\$" "the site's next page left Safari's engine"
  go_from_bar "localhost:$port/engine.html"
  expect "^browser: tab $tab back to Chromium for http://localhost:$port/engine.html\$" \
    "another site didn't take the tab back to Chromium"
  expect "^browser: title $tab engine chromium\$" "another site stayed in Safari's engine"
  # The profile kept the site, which opens in Safari's engine with no note.
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine safari\$" "the site kept for Safari's engine opened in Chromium"
  # A link on the site whose redirects pass through another site and come back, as a sign-in's
  # through an identity site do, stays in Safari's engine. One whose redirects end at the other site
  # goes back to Chromium, as a link there does.
  go_from_bar "127.0.0.1:$port/hop.html"
  expect "^browser: title $tab hop safari\$" "the site's hop page left Safari's engine"
  send click $page_point
  expect "^browser: title $tab engine safari\$" \
    "a link's redirects through another site didn't come back in Safari's engine"
  none_since "^browser: tab $tab back to Chromium" ||
    fail "a link's redirects through another site took the tab back to Chromium"
  served "^GET safari localhost:$port/bounce" "the redirects didn't pass through the other site in Safari's engine"
  go_from_bar "127.0.0.1:$port/hop.html?away"
  expect "^browser: title $tab hop safari\$" "the site's hop page left Safari's engine"
  send click $page_point
  expect "^browser: tab $tab back to Chromium for http://localhost:$port/engine.html\$" \
    "a link whose redirects ended at another site didn't take the tab back to Chromium"
  expect "^browser: title $tab engine chromium\$" "a link's redirects to another site stayed in Safari's engine"
  # Chromium's view shows the page it kept, with its title, until the handback's load replaces it,
  # which would count a click from zero again.
  expect "^browser: loaded $tab http://localhost:$port/engine.html\$" "Chromium didn't load the handed-back page"
  # The command bar closes at a click outside it before the click is dispatched, so the click lands
  # on the page as it would with the bar closed (AGENTS.md › Over the page). Chromium's page counts
  # the click that closed the bar and the next, as Safari's engine's does below.
  send click $page_point
  expect "^browser: title $tab pressed 1\$" "a click didn't reach Chromium's page"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L over Chromium's page didn't open the command bar"
  send click $page_point
  expect '^browser: command bar hidden$' "a click on Chromium's page didn't close the command bar"
  expect "^browser: title $tab pressed 2\$" \
    "the click that closed the bar didn't reach Chromium's page: $(grep "^browser: title $tab " "$log" | tail -1)"
  in_browser RenderWidgetHostViewCocoa "after a click on Chromium's page closed the command bar"
  send click $page_point
  expect "^browser: title $tab pressed 3\$" \
    "the click after the bar closed didn't reach Chromium's page: $(grep "^browser: title $tab " "$log" | tail -1)"
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine safari\$" "the site kept for Safari's engine opened in Chromium"
  # Keys with Safari's engine's page focused, as keys.sh has them with Chromium's. A click gives the
  # page the keyboard, which the note's button left with the UI, and the reserved shortcuts still
  # reach the UI.
  send click $page_point
  expect "^browser: title $tab pressed 1\$" "a click didn't reach Safari's engine's page"
  send state
  in_browser WKWebView "after a click on Safari's engine's page"
  for shortcut in "t new-tab" "l edit-url"; do
    page_shortcut $shortcut
    expect '^browser: command bar shown$' "$shortcut over Safari's engine didn't open the command bar"
    send key escape
    expect '^browser: command bar hidden$' "escape didn't close the command bar over Safari's engine"
    in_browser WKWebView "after the command bar over Safari's engine closed"
  done
  # A click in the page closes the bar before it's dispatched, so the page counts it and the next,
  # as Chromium's does above.
  send key l cmd
  expect '^browser: command bar shown$' "⌘L over Safari's engine didn't open the command bar"
  send click $page_point
  expect '^browser: command bar hidden$' "a click on Safari's engine's page didn't close the command bar"
  expect "^browser: title $tab pressed 2\$" \
    "the click that closed the bar didn't reach Safari's engine's page: $(grep "^browser: title $tab " "$log" | tail -1)"
  in_browser WKWebView "after a click on Safari's engine's page closed the command bar"
  send click $page_point
  expect "^browser: title $tab pressed 3\$" \
    "the click after the bar closed didn't reach Safari's engine's page: $(grep "^browser: title $tab " "$log" | tail -1)"
  # In the page's text field, ⌘A and ⌘C stay with the page.
  send type abc
  expect "^browser: title $tab value abc\$" "typing didn't reach the text field in Safari's engine"
  for shortcut in "a select" "c copy"; do
    letter=${shortcut% *} did=${shortcut#* }
    send key "$letter" cmd
    expect "^browser: title $tab $did abc\$" \
      "⌘$(echo "$letter" | tr a-z A-Z) in Safari's engine's text field didn't $did its text"
    none_since '^browser: command [a-z-]* via page$' ||
      fail "⌘$(echo "$letter" | tr a-z A-Z) in Safari's engine's text field reached the UI"
  done
  # Japanese input composes in the field and commits there, replacing what ⌘A selected.
  send ime on
  send type nihongo
  send key space
  send key return
  expect "^browser: title $tab committed 日本語\$" \
    "a Japanese composition didn't commit into Safari's engine's text field"
  send ime off
  send type x
  expect "^browser: title $tab value 日本語x\$" \
    "return in Safari's engine's text field did more than commit: $(grep "^browser: title $tab " "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/engine-ime.png" || fail "no window to capture"
  # ⌘W from the page closes the tab, and the site kept opens in Safari's engine in the next one shown.
  page_shortcut w close-tab
  expect '^browser: selected [1-9]' "⌘W over Safari's engine showed no tab"
  closed=$tab
  tab=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
  [ "$tab" != "$closed" ] || fail "⌘W over Safari's engine left its tab shown"
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine safari\$" "the site kept for Safari's engine opened in Chromium in another tab"
  # Safari's engine's page stores a cookie from its server, HttpOnly, one from its script and an
  # item. Its script sees Chromium's cookie and item, and neither HttpOnly cookie.
  go_from_bar "127.0.0.1:$port/session?webkit"
  expect "^browser: title $tab session safari sees.* n w item back\$" \
    "Safari's engine's session page didn't see Chromium's cookie and item, or keep its own"
  grep "^browser: title $tab session safari sees " "$log" | tail -1 | grep -qwE "h|hw" &&
    fail "an HttpOnly cookie reached the script in Safari's engine"
  # The tab leaves the page that sets them for one that only reads them, so Off below hands it back
  # to a page in Chromium that can see only what the copy brought.
  go_from_bar "127.0.0.1:$port/session"
  expect "^browser: loaded $tab http://127.0.0.1:$port/session\$" "the plain session page didn't load"
  expect "^browser: title $tab session safari sees.* n w item back\$" \
    "Safari's engine's plain session page didn't see what the one before stored"
  reader=$tab
  # ⌘T to the site opens a new tab in Safari's engine, though Chrome starts a new tab's first load
  # before the core has the tab. The new tab is the one shown from here.
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send insert "127.0.0.1:$port/engine.html"
  send key return
  expect "^browser: loaded [0-9]* http://127.0.0.1:$port/engine.html\$" "⌘T to the site kept for Safari's engine loaded nothing"
  opened=$(sed -n "s|^browser: loaded \([0-9]*\) http://127.0.0.1:$port/engine.html\$|\1|p" "$log" | tail -1)
  [ "$opened" != "$tab" ] || fail "⌘T to the site loaded it in the tab shown"
  expect "^browser: title $opened engine safari\$" \
    "⌘T to the site kept for Safari's engine opened it in Chromium: $(grep "^browser: title $opened " "$log" | tail -1)"
  expect "$copied" "the new tab didn't take the site's session into Safari's engine"
  tab=$opened
  # A private window's tab at the site opens in Safari's engine too, in a session of its own that
  # starts from the private window's and ends with it: the regular window doesn't see what its page
  # stored, and nor does the next private window. The page names what it finds, never a value.
  # Opens the session page at query $1 from ⌘T's bar in the key window, the private window $2, and
  # sets $private_tab to its tab.
  private_session_page() {
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
    send insert "127.0.0.1:$port/session$1"
    send key return
    expect "^browser: loaded [0-9]* http://127.0.0.1:$port/session$1\$" \
      "the private window didn't load the session page"
    private_tab=$(sed -n "s|^browser: loaded \([0-9]*\) http://127.0.0.1:$port/session$1\$|\1|p" \
      "$log" | tail -1)
    expect "^browser: window $2 shows $private_tab\$" "the private window didn't show the session page"
  }
  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
  private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
  new_window_bar
  send window 2
  private_session_page "?private" "$private"
  expect "^browser: copied 0 cookies and 0 storage items for 127.0.0.1 into the system's engine\$" \
    "the private tab didn't start Safari's engine from the private session, which has none of the site's"
  expect "^browser: title $private_tab session safari sees p mine\$" \
    "the private window's page didn't store its cookie and item in Safari's engine: $(grep "^browser: title $private_tab " "$log" | tail -1)"
  send window 1
  go_from_bar "127.0.0.1:$port/session"
  expect "^browser: title $tab session safari sees " "the regular window's session page didn't load in Safari's engine"
  grep "^browser: title $tab session safari sees " "$log" | tail -1 | grep -qwE "p|mine" &&
    fail "the regular window's Safari's engine saw what the private window's page stored"
  send window 2
  send key w cmd shift
  expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
  # await, which says so if the app has exited. Its line can only be this close's: none came before.
  await '^browser: private session ended [0-9]*$' ||
    fail "the private window's closing didn't end its session"
  send window 1
  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window again"
  private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
  new_window_bar
  # The driver numbers windows in the order they were made, the closed one too.
  send window 3
  private_session_page "" "$private"
  expect "^browser: title $private_tab session safari sees none\$" \
    "the next private window's Safari's engine saw the ended session's values, or the regular window's: $(grep "^browser: title $private_tab " "$log" | tail -1)"
  send key w cmd shift
  expect "^browser: window closed $private\$" "⇧⌘W didn't close the next private window"
  send window 1
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine safari\$" "the site's engine page left Safari's engine"
  # The Site Controls show its switch, and Off forgets it, brings its session back to
  # Chromium and shows the page there.
  away
  send move $sidebar_point
  expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
  send click $site_controls_point
  expect "^browser: site controls http://127.0.0.1:$port: .*; Safari's engine on\$" \
    "the Site Controls didn't list Safari's engine"
  expect '^browser: site controls settled$' "the Site Controls didn't open in Safari's engine"
  ../tools/window-screenshot.sh "$pid" "$out/site-controls-engine.png" || fail "no window to capture"
  click_placed post "switch Safari's engine"
  expect "^browser: menu shown On (checked), Off\$" "Safari's engine's switch didn't offer On and Off"
  send choose Off
  expect "^browser: site controls set Safari's engine Off\$" "Off in the menu didn't run"
  expect "^browser: copied [0-9]* cookies and [0-9]* storage items for 127.0.0.1 back to Chromium\$" \
    "Off didn't copy the site's session back to Chromium"
  expect "^browser: copied [1-9][0-9]* cookies and [1-9][0-9]* storage items for 127.0.0.1 back to Chromium\$" \
    "Off copied none of the site's cookies or items back to Chromium: $(grep '^browser: system store has ' "$log" | tail -1)"
  # The tab handed back at the plain session page loads it in Chromium, which sets nothing, so what
  # its script sees came back with the copy: Safari's engine's page's cookie and item.
  expect "^browser: title $reader session chromium sees.* w item back\$" \
    "Chromium didn't get the cookie and the item Safari's engine's page stored: $(grep "^browser: title $reader " "$log" | tail -1)"
  served "^GET chromium 127.0.0.1:$port/session cookie .*hw=$port.*w=$port" \
    "Chromium's request didn't carry the HttpOnly cookie Safari's engine's server set"
  expect "^browser: title $tab engine chromium\$" "Off didn't show the page in Chromium"
  # Chromium's page, taking the pane, may have closed them already.
  if none_since '^browser: site controls hidden$'; then
    send key escape
    expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  fi
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine chromium\$" "the site turned off went to Safari's engine again"
  # Chromium has what Safari's engine's page stored: its request carries both of that page's
  # cookies, and its script sees the one it may and the item.
  go_from_bar "127.0.0.1:$port/session"
  expect "^browser: title $tab session chromium sees.* w item back\$" \
    "Chromium didn't get the cookie and the item Safari's engine's page stored"
  grep "^browser: title $tab session chromium sees " "$log" | tail -1 | grep -qwE "h|hw" &&
    fail "an HttpOnly cookie reached the script in Chromium"
  served "^GET chromium 127.0.0.1:$port/session cookie .*hw=$port.*w=$port" \
    "Chromium's request didn't carry the HttpOnly cookie Safari's engine's server set"
  # Chromium setting the session itself would hide a copy that brought nothing.
  grep -q "^GET chromium 127.0.0.1:$port/session?webkit " "$out/site.log" &&
    fail "Chromium loaded the page that sets Safari's engine's session, so the copy back went untested"
  # Back at the site's engine page, still in Chromium, for the Site Controls.
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine chromium\$" "the site turned off went to Safari's engine again"
  # Off keeps the site offered, as a closed note's is: the Site Controls still list Safari's
  # engine, off, and On shows the page there again.
  away
  send move $sidebar_point
  expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
  send click $site_controls_point
  expect "^browser: site controls http://127.0.0.1:$port: .*; Safari's engine off\$" \
    "the Site Controls didn't offer Safari's engine for the site turned off"
  expect '^browser: site controls settled$' "the Site Controls didn't open at the site turned off"
  click_placed post "switch Safari's engine"
  expect "^browser: menu shown On, Off (checked)\$" "Safari's engine's switch didn't offer On after Off"
  send choose On
  expect "^browser: site controls set Safari's engine On\$" "On in the menu didn't run"
  expect "^browser: system engine $tab on\$" "On didn't turn Safari's engine on for the site turned off"
  expect "^browser: title $tab engine safari\$" "On didn't show the site turned off in Safari's engine"
  # Safari's engine's page, taking the pane, may have closed them already.
  if none_since '^browser: site controls hidden$'; then
    send key escape
    expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  fi
  go_from_bar "localhost:$port/index.html"
  expect "^browser: loaded $tab http://localhost:$port/index.html\$" "localhost didn't load again"
else
  drm_done="Safari"
  expect "^browser: open in safari http://127.0.0.1:$port/drm.html\$" \
    "the note's button didn't open the page in Safari"
  none_since "^browser: page $tab needs nothing\$" || fail "the button took the note away"
  go_from_bar "localhost:$port/index.html"
  expect "^browser: loaded $tab http://localhost:$port/index.html\$" "localhost didn't load again"
  expect "^browser: page $tab needs nothing\$" "the note stayed at another site"
fi
# The way back from the closed note, at localhost, which stayed closed.
mark=$(wc -l < "$log")
go_from_bar "localhost:$port/drm.html"
asked "after Safari's engine's steps"
# The way back: the Site Controls offer Safari's engine for the site, where the note's button would
# show the page in it, and On does.
away
send move $sidebar_point
expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
send click $site_controls_point
expect "$controls" "the Site Controls didn't list the closed site"
expect '^browser: site controls settled$' "the Site Controls didn't open at the closed site"
if [ "$drm_done" = "Safari's engine" ]; then
  grep "$controls" "$log" | tail -1 | grep -q "; Safari's engine off\$" ||
    fail "the Site Controls didn't offer Safari's engine for the closed site: $(grep "$controls" "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/site-controls-offered.png" || fail "no window to capture"
  click_placed post "switch Safari's engine"
  expect "^browser: menu shown On, Off (checked)\$" "Safari's engine's switch didn't offer On"
  send choose On
  expect "^browser: site controls set Safari's engine On\$" "On in the menu didn't run"
  expect "^browser: system engine $tab on\$" "On didn't show the page in Safari's engine"
  expect_late "^browser: title $tab drm asked\$" "Safari's engine didn't load the closed site's page"
else
  grep "$controls" "$log" | tail -1 | grep -q "Safari's engine" &&
    fail "the Site Controls offered Safari's engine where the note's button opens Safari"
fi
# Safari's engine's page, taking the pane, may have closed them already.
if none_since '^browser: site controls hidden$'; then
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
fi
# Rested on a link whose address is longer than half the card, the plate shows it cut, then after
# 1.5 s whole, wider, in the same corner, as it fits in the card.
long_path="notes.txt?$(printf 'a%.0s' $(seq 110))"
printf '%s' "<title>long link</title><a href=\"${site}$long_path\" style=\"position:fixed;left:100px;top:100px;width:200px;height:40px\"></a>" \
  > "$out/site/long-link.html"
# From the file, in a new tab: on the Mac the run has given the served sites to Safari's engine,
# which reports no link targets.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
# Windows' path, D:\a\…, is file:///D:/a/….
long_file=$(native_path "$PWD/$out/site/long-link.html" | sed 's|\\|/|g')
case "$long_file" in /*) ;; *) long_file="/$long_file" ;; esac
send type "file://$long_file"
send key return
expect '^browser: command bar hidden$' "going to the long link's page didn't close the command bar"
expect '^browser: title [0-9]* long link$' "the page with the long link didn't load"
send move $link_point
expect "^browser: link target ${site}$long_path\$" "the pointer on the long link reported no address"
expect '^browser: link target plate ' "the long link's address didn't show in a plate"
cut=$(sed -n 's/^browser: link target plate //p' "$log" | tail -1)
case "$cut" in *…*) ;; *) fail "the long link's first plate wasn't cut: $cut" ;; esac
expect '^browser: link target whole ' "resting on the long link didn't show its whole address"
whole=$(sed -n 's/^browser: link target whole //p' "$log" | tail -1)
[ "${whole% *}" = "${site}$long_path" ] || fail "the whole address shown was ${whole% *}"
../tools/window-screenshot.sh "$pid" "$out/link-target-whole.png" || fail "no window to capture"
send move $page_point
expect '^browser: link target hidden$' "the whole address stayed after the pointer left the link"
exec 3>&-
quit
# In a launch of its own, with a DevTools client that says when Clear browsing data's dialog has the
# keyboard (cdp.py), as a DevTools pipe for the whole run keeps the Widevine note above from coming.
# The history is the profile's, kept from the launch before.
log="$out/clear-delete.log"
cdp_log="$out/clear-cdp.log"
step=0
scratch=$(mktemp -d)
start_with_cdp "$site" "$profile_dir" ""
await_load
# Clear browsing data's Delete, with Browsing history ticked as it is at first, empties the
# history and the browser stays up, though Chrome tells the BrowserView, which a window of the
# core's hasn't got (#1821). The page typed before is history, which the command bar suggests with
# its remove button until then. Cancel has the dialog's keyboard, and Delete is after it.
visited="localhost:$port/drm"
# history_suggested [typed] [page]: ⌘T's bar, given typed, suggests page from history.
history_suggested() {
  local typed=${1:-$visited} page=${2:-localhost:$port/drm.html}
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$typed"
  expect_late "^browser: command bar suggestions \"$typed\" in " \
    "the command bar suggested nothing for $typed"
  suggested=$(grep "^browser: command bar suggestions \"$typed\" in " "$log" | tail -1)
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  echo "$suggested" | grep -q "$page[^,]* removable"
}
history_suggested || fail "the command bar didn't suggest $visited from history: $suggested"
# The page Peek shows is history the command bar suggests, as a tab's page is.
peek_notes
notes_suggested() { history_suggested "127.0.0.1:$port/notes" "$notes"; }
notes_suggested ||
  fail "the command bar didn't suggest the page Peek showed from history, as it does a tab's: $suggested"
# The dialog takes the keyboard a moment after its page loads, which a key sent before then misses:
# the run waits for its Cancel to have it, as cdp.py sees it.
focused() {
  asked=$(grep -c '^cdp: focus ' "$cdp_log" || true)
  echo "focus chrome://settings/" >&5
  for _ in $(seq 50); do
    [ "$(grep -c '^cdp: focus ' "$cdp_log" || true)" -gt "$asked" ] && break
    sleep 0.1
  done
  grep '^cdp: focus ' "$cdp_log" | tail -1 | cut -d' ' -f3-
}
open_clear_browsing_data() {
  bar_command clear-browsing-data "Clear Browsing Data" "clear browsing"
  expect_late '^browser: loaded [0-9]* chrome://settings/clearBrowserData$' \
    "Clear Browsing Data didn't open Clear browsing data: $(grep '^browser: loaded' "$log" | tail -1)"
  for _ in $(seq 100); do
    [ "$(focused)" = Cancel ] && break
    sleep 0.1
  done
  if [ "$(focused)" != Cancel ]; then
    # Which window has the keyboard, and whether ilo is the active app, for the VM's failure (#2396).
    send front
    fail "Clear browsing data's dialog didn't give its Cancel the keyboard: $(focused); active $active, $(grep '^browser: front ' "$log" | tail -1)"
  fi
}
# Cancel closes the dialog, deleting nothing, and the browser stays up: from Chromium 157 it tells
# the BrowserView too, as Delete does.
open_clear_browsing_data
send key space
history_suggested ||
  fail "after Cancel in Clear browsing data, the command bar didn't suggest $visited: $suggested"
open_clear_browsing_data
send key tab
send key space
for _ in $(seq 20); do
  history_suggested || break
done
history_suggested && fail "Delete in Clear browsing data left $visited in the history: $suggested"
notes_suggested && fail "Delete in Clear browsing data left the page Peek showed in the history: $suggested"
../tools/window-screenshot.sh "$pid" "$out/clear-browsing-data-deleted.png" || fail "no window to capture"
send key w cmd
exec 3>&- 5>&-
quit
rm -rf "$scratch"
# A listed streaming service opens in Safari's engine from its first load, here 127.0.0.1, which
# the dev build lists for the run (main.rs). The run above kept it, and turned its note off, so the
# profile forgets both first, or the site kept would open there without the list. The app launches
# at its page, a new tab's first load, as a link from another app's would be.
listed_prefs="$profile_dir/Default/Preferences"
forget_engine_sites
listed_launch() {
  log="$out/clear-listed-$1.log"
  step=0
  rm -f "$saved"
  BROWSER_DRIVE=1 BROWSER_STREAMING_TEST_HOST=127.0.0.1 BROWSER_URL="http://127.0.0.1:$port/engine.html" \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: title [0-9]* engine ' || fail "the listed host's page didn't load $2"
  tab=$(sed -n 's/^browser: title \([0-9]*\) engine .*/\1/p' "$log" | head -1)
}
# Opens the Site Controls of the tab shown, which list $1 for Safari's engine, and chooses $2 in its
# switch's menu, which checks $3.
listed_controls() {
  away
  send move $sidebar_point
  expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
  send click $site_controls_point
  expect "^browser: site controls http://127.0.0.1:$port: .*; Safari's engine $1\$" \
    "the Site Controls didn't list the listed host's Safari's engine $1: $(grep '^browser: site controls http' "$log" | tail -1)"
  expect '^browser: site controls settled$' "the Site Controls didn't open at the listed host"
  ../tools/window-screenshot.sh "$pid" "$out/site-controls-listed-$1.png" || fail "no window to capture"
  click_placed post "switch Safari's engine"
  expect "^browser: menu shown $3\$" "Safari's engine's switch didn't offer On and Off"
  send choose "$2"
  expect "^browser: site controls set Safari's engine $2\$" "$2 in the menu didn't run"
}
listed_closed() {
  # The page, taking the pane, may have closed them already.
  if none_since '^browser: site controls hidden$'; then
    send key escape
    expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  fi
}
if [ "$drm_done" = "Safari's engine" ]; then
  listed_launch on "on its first load"
  # No page of Chromium's came first: its script would have named it.
  grep -q '^browser: title [0-9]* engine chromium$' "$log" &&
    fail "the listed host's first page loaded in Chromium before Safari's engine"
  # await, as no command has been sent to read from.
  await "^browser: title $tab engine safari\$" ||
    fail "the listed host's first page didn't open in Safari's engine: $(grep "^browser: title $tab " "$log" | tail -1)"
  grep -q '^browser: system engine loaded for ' "$log" || fail "Safari's engine's page didn't say WebKit loaded"
  ../tools/window-screenshot.sh "$pid" "$out/listed.png" || fail "no window to capture"
  # A private window's tab at the listed host opens there too, in the private window's own store.
  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
  private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
  new_window_bar
  send window 2
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
  send insert "127.0.0.1:$port/engine.html?private"
  send key return
  expect "^browser: loaded [0-9]* http://127.0.0.1:$port/engine.html?private\$" \
    "the private window didn't load the listed host's page"
  private_tab=$(sed -n "s|^browser: loaded \([0-9]*\) http://127.0.0.1:$port/engine.html?private\$|\1|p" \
    "$log" | tail -1)
  expect "^browser: title $private_tab engine safari\$" \
    "the listed host opened in Chromium in a private window: $(grep "^browser: title $private_tab " "$log" | tail -1)"
  send key w cmd shift
  expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
  send window 1
  # The Site Controls show it on, the reason it's there, and Off shows the page in Chromium, which it
  # stays in after a reload, in a new tab, and after another site and back.
  listed_controls on Off "On (checked), Off"
  expect "^browser: title $tab engine chromium\$" "Off didn't show the listed host's page in Chromium"
  listed_closed
  send key r cmd
  expect "^browser: loaded $tab http://127.0.0.1:$port/engine.html\$" "⌘R didn't reload the listed host's page"
  expect "^browser: title $tab engine chromium\$" "the listed host turned off went to Safari's engine on a reload"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send insert "127.0.0.1:$port/engine.html?new"
  send key return
  expect "^browser: loaded [0-9]* http://127.0.0.1:$port/engine.html?new\$" "⌘T to the listed host loaded nothing"
  opened=$(sed -n "s|^browser: loaded \([0-9]*\) http://127.0.0.1:$port/engine.html?new\$|\1|p" "$log" | tail -1)
  [ "$opened" != "$tab" ] || fail "⌘T to the listed host loaded it in the tab shown"
  expect "^browser: title $opened engine chromium\$" "the listed host turned off went to Safari's engine in a new tab"
  tab=$opened
  go_from_bar "localhost:$port/index.html"
  expect "^browser: loaded $tab http://localhost:$port/index.html\$" "localhost didn't load"
  go_from_bar "127.0.0.1:$port/engine.html"
  expect "^browser: title $tab engine chromium\$" \
    "the listed host turned off went to Safari's engine after another site and back"
  exec 3>&-
  quit
  # The profile keeps it off across launches, and the Site Controls offer it there, whose On brings
  # it back.
  listed_launch off "after a relaunch"
  await "^browser: title $tab engine chromium\$" ||
    fail "the listed host turned off went to Safari's engine after a relaunch"
  listed_controls off On "On, Off (checked)"
  expect "^browser: system engine $tab on\$" "On didn't show the listed host's page in Safari's engine"
  expect "^browser: title $tab engine safari\$" "On didn't load the listed host's page in Safari's engine"
  listed_closed
  go_from_bar "127.0.0.1:$port/engine.html?again"
  expect "^browser: title $tab engine safari\$" "the listed host turned on again left Safari's engine"
  exec 3>&-
  quit
  # On took back the off, and never kept the listed host as the user's own.
  python3 -I -c '
import json, sys
engine = json.load(open(sys.argv[1], encoding="utf-8")).get("browser_core", {}).get("system_engine", {})
for kind in ("sites", "listed_off"):
    if "127.0.0.1" in engine.get(kind, []):
        sys.exit(f"the profile has the listed host in its {kind}")
' "$listed_prefs" || fail "On didn't leave the listed host as listed"
  # A profile folder removed by hand takes its Safari's engine store's name with it (#2657): a
  # profile made at its path has a store of its own, and the next launch deletes the old one. In a
  # data folder of the run's own, as the user's Default is the one the steps above share.
  store_dir="$PWD/$out/clear-store"
  rm -rf "$store_dir"
  mkdir -p "$store_dir/Default"
  # Launches the app in it at the session page with the query $1, as the run $2.
  store_launch() {
    log="$out/clear-store-$2.log"
    step=0
    BROWSER_DRIVE=1 BROWSER_PROFILE_DIR="$store_dir" BROWSER_STREAMING_TEST_HOST=127.0.0.1 \
      BROWSER_URL="http://127.0.0.1:$port/session$1" "$browser" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await_load
  }
  store_launch '?webkit' before
  await '^browser: title [0-9]* session safari sees w back$' ||
    fail "the session page didn't keep its cookie and item in Safari's engine: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  exec 3>&-
  quit
  rm -rf "$store_dir/Default"
  store_launch '' after
  await '^browser: title [0-9]* session safari sees none$' ||
    fail "a profile made where one was removed by hand saw its Safari's engine session: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  await '^browser: system store deleted$' ||
    fail "the launch after a profile's folder was removed by hand didn't delete its store: $(grep '^browser: system store' "$log" | tail -1)"
  exec 3>&-
  quit
  rm -rf "$store_dir"
  # A store a version before #2664 gave a profile that's gone is deleted once, by its path (#2701).
  # Default names the stores the old ids, as an old version would have, so the app makes them
  # itself. With no symlinks in the path, as the old ids are of the path the app holds.
  store_dir="$(cd "$out" && pwd -P)/clear-old-store"
  rm -rf "$store_dir"
  mkdir -p "$store_dir/Default"
  # The old id of the profile path $1 in the data folder: StoreIdFor's before #2664.
  old_id() {
    python3 -I -c '
import hashlib, sys, uuid
digest = bytearray(hashlib.sha1(b"ilo system store " + sys.argv[1].encode()).digest())
digest[6] = (digest[6] & 0x0f) | 0x50
digest[8] = (digest[8] & 0x3f) | 0x80
print(uuid.UUID(bytes=bytes(digest[:16])))
' "$store_dir/$1"
  }
  # Has Default name the store $1, or none for "", and takes the stores the launch before listed
  # off Local State's list, as a folder from before #2664 has none. With $2 "unswept", also as the
  # folder of an old version's: unmarked, with Profile 1 counted.
  old_store_prefs() {
    python3 -I -c '
import json, os, sys
folder, store, unswept = sys.argv[1], sys.argv[2], sys.argv[3] == "unswept"
def edit(path, change):
    prefs = json.load(open(path, encoding="utf-8")) if os.path.exists(path) else {}
    change(prefs)
    json.dump(prefs, open(path, "w", encoding="utf-8"))
def named(prefs):
    engine = prefs.setdefault("browser_core", {}).setdefault("system_engine", {})
    if store:
        engine["store"] = store
    else:
        engine.pop("store", None)
def listed(state):
    engine = state.setdefault("browser_core", {}).setdefault("system_engine", {})
    engine.pop("given_stores", None)
    if unswept:
        engine.pop("old_stores_swept", None)
        state.setdefault("profile", {})["profiles_created"] = 2
edit(os.path.join(folder, "Default", "Preferences"), named)
if os.path.exists(os.path.join(folder, "Local State")):
    edit(os.path.join(folder, "Local State"), listed)
' "$store_dir" "$1" "${2:-}"
  }
  # Whether Local State marks the folder swept.
  old_stores_marked() {
    python3 -I -c '
import json, sys
state = json.load(open(sys.argv[1], encoding="utf-8"))
sys.exit(0 if state.get("browser_core", {}).get("system_engine", {}).get("old_stores_swept") else 1)
' "$store_dir/Local State"
  }
  gone_store=$(old_id "Profile 1")
  kept_store=$(old_id Default)
  # A folder's first launch makes no old store, so it's marked without a sweep: here Default's
  # page fills the store of Profile 1's path, which has no folder.
  old_store_prefs "$gone_store"
  store_launch '?webkit' old-first
  await '^browser: title [0-9]* session safari sees w back$' ||
    fail "the session page didn't keep its cookie and item in the store of Profile 1's path: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  exec 3>&-
  quit
  ! grep -q '^browser: old system stores swept$' "$log" ||
    fail "a data folder's first launch swept old stores, which a folder set aside and put back would lose"
  old_stores_marked || fail "a data folder's first launch didn't mark it as having no old stores"
  old_store_prefs "$kept_store"
  store_launch '?webkit' old-kept
  await '^browser: title [0-9]* session safari sees w back$' ||
    fail "the session page didn't keep its cookie and item in the store of Default's path: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  exec 3>&-
  quit
  # As an old version leaves the folder: no store named, none listed, unmarked, Profile 1 counted.
  old_store_prefs '' unswept
  store_launch '' old-swept
  await '^browser: old system stores swept$' ||
    fail "the launch after an old version's didn't sweep its stores: $(grep 'system store' "$log" | tail -3)"
  deleted=$(grep -c '^browser: system store deleted$' "$log" || true)
  [ "$deleted" = 1 ] ||
    fail "the sweep deleted $deleted stores, not the one of Profile 1's path alone: $(grep '^browser: system store' "$log" | tail -3)"
  exec 3>&-
  quit
  old_stores_marked || fail "the sweep didn't mark the folder swept"
  # The store of Default's path stayed, its folder being there, and Profile 1's went.
  old_store_prefs "$kept_store"
  store_launch '' old-kept-after
  await '^browser: title [0-9]* session safari sees w back$' ||
    fail "the sweep deleted the store of Default's path, whose folder is there: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  exec 3>&-
  quit
  old_store_prefs "$gone_store"
  store_launch '' old-gone-after
  await '^browser: title [0-9]* session safari sees none$' ||
    fail "the store of Profile 1's path, which has no folder, kept its session after the sweep: $(grep '^browser: title [0-9]* session' "$log" | tail -1)"
  exec 3>&-
  quit
  rm -rf "$store_dir"
fi
forget_engine_sites
echo "ok: the pointer on a link shows its address in a plate that goes when the pointer leaves it, and a long one's whole after 1.5 s, Clear Cache and Refresh from the View menu fetches the page's script again, keeping its cookie, and Clear Cookies and Refresh from the command bar deletes its cookie, keeping the cached script. Clear browsing data's Cancel keeps the history and Delete, with Browsing history ticked, empties the history the command bar suggests from, and the browser stays up after each. A page ⇧-click opens in Peek is a visit, as a tab's is: History's search finds it, Remove from History takes it out, the command bar suggests it, and Clear browsing data's Delete takes it out. ⇧⌘C and ⌥⇧⌘C from the page and the UI copy the tab's URL and a Markdown link to it, without its tracking parameters, and ⇧⌘C's toast says Copied Current URL, with the capture toast's Share where there's a share sheet, which shares the page from either. ⌥⌘V from the UI and the page and the command bar's Paste URL as New Tab open the link copied in a new tab at the top of Today, and a Markdown link copied nothing. The sidebar shows Clear and the URL field's button only while the pointer is over it, and the button copies the URL. ⇧⌘K from the page and the UI, and Clear on the divider, close the Today tabs but the one shown, with no toast, and ⌘Z from the page and the UI brings them back in place, unloaded, but not from the page's text field, where it undoes typing. With only the tab shown left, Clear has an arrow before it and closes that tab too, and ⌘Z brings it back, shown. ⇧⌘T from the page and the UI reopens the tab closed last, where it was, with its history, muted if it was and with its icon, and ⌘Z brings back only the tabs ⇧⌘T didn't. A right-click on a row shows its tab's menu, Arc's for a Today tab: Duplicate copies the tab with its history and icon under it, Archive Tabs Below closes the Today tabs under it without showing them, and Unpin Tab unpins. ⌘W archives a Today tab, and ⇧⌘L, Window › Open Library… and the footer's button open the Library, which pushes the page right at its width; History lists the pages visited among the closed tabs, a row under the pointer shows its trash, and a click on a visited page's row opens it in Peek, which Esc closes, leaving the Library, and its menu's Remove from History takes it out, ⌘Z bringing it back; a click on an archived tab reopens it at the top of Today with its history and closes the Library, a right-click on one shows its menu, whose Copy URL copies its URL and Delete from Archive takes it out; and Clear Archive, from a right-click on the Archived Tabs section and the Archive menu, asks, then empties it, ⌘Z bringing its tabs back, and the section's is dimmed once it's empty. A page's question about a permission shows under the URL field while its tab is shown, and brings a hidden sidebar out until it's answered: × dismisses it, Block refuses the site now and later, and Allow grants. The URL field's Site Controls button and the command bar's Site Controls list the site's permissions as Chrome does; Block there sticks and refuses the site, Reset permissions has it ask again, Share shares the page, as File › Share… does where there's a share sheet, the lock does nothing without a certificate, the … menu lists Arc's items and opens the site's settings, and the button, escape and a click outside close them. A page that asks for Widevine brings a note under the URL field, while one that asks for ClearKey brings none. Its button used $drm_done: Safari's engine shows the page in the tab with the site's cookies, HttpOnly too, and localStorage from Chromium there before its first request and script, keeps the site for it until the Site Controls turn it off, which brings the cookies and the item its page stored back to Chromium, gives another site back to Chromium, keeps a link whose redirects pass through another site and come back, and gives back one whose redirects end there, opens a new tab from ⌘T at the site there, gives a private window's tab there a session of its own, which the regular window doesn't see and which ends with the window, and with its page focused ⌘T, ⌘L and ⌘W reach the UI, the click that closes the command bar reaches the page, as it does Chromium's, and in its text field ⌘A and ⌘C stay with the page and a Japanese composition commits; Safari opens the page there, leaving the note until the tab is at another site. The note's × closes it for the site, which leaves it closed after a reload, in another tab and after another site and back, while another site's note still shows; from macOS 14 the Site Controls offer Safari's engine for the site instead, and On shows the page there, as they do for a site turned off. From macOS 14 a listed streaming service opens in Safari's engine on its first load, without Chromium's first, in a private window too; the Site Controls show it on, and Off shows it in Chromium after a reload, in a new tab, after another site and back and after a relaunch, where On brings it back without keeping it as the user's, and a profile made where one's folder was removed by hand starts with an empty session there, the old store deleted, as is once a store an old version gave a profile that's gone, keeping one whose profile's folder is there, but never on a data folder's first launch. ⇧⌘Z redoes Clear, and Archive Tabs Below offers Undo, which brings back its tabs in place and shows the one that was shown, and ⇧⌘Z closes them again. Windows: $out/link-target.png, $out/link-target-whole.png, $out/clear-site.png, $out/url-toast.png, $out/pasted-url.png, $out/sidebar-hovered.png, $out/row-hovered.png, $out/sidebar-away.png, $out/clear.png, $out/clear-arrow.png, $out/clear-arrow-undone.png, $out/reopen.png, $out/tab-menu.png, $out/library.png, $out/history-hover-closed.png, $out/history-hover-visited.png, $out/history-peek-visited.png, $out/archived-menu.png, $out/archive-section-menu.png, $out/library-empty.png, $out/question.png, $out/question-reveal.png, $out/site-controls.png, $out/site-controls-blocked.png, $out/drm-note.png, and from macOS 14 $out/drm-engine.png, $out/engine-ime.png, $out/site-controls-engine.png, $out/site-controls-offered.png, $out/listed.png, $out/site-controls-listed-on.png and $out/site-controls-listed-off.png"
