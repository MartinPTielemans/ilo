# Launched again after ⌘Q, and again after its window's close button, the app brings back the
# Spaces: their names, colours and icons, their tabs with each tab's history, the tab each showed
# and the Space it showed, and the sidebar's width. Only a tab shown loads; the others show their
# titles and favicons. A tab named with a double-click on its row, where escape cancels, keeps its
# name over its page's title, and Rename… in its menu with no name gives the title back. An
# emoji picked in the icon picker that Change Icon… hangs under its favicon takes the favicon's
# place until Remove Icon, and a Japanese conversion in the picker's search isn't a pick. A Space's
# icon, picked in its icon picker, comes back too. A tab closed before the quit stays closed,
# in the archive, from which the Library reopens it at its page. A page's downloads go in the
# check's folder, on the Mac with macOS's quarantine flag, a new one's icon flies
# into the Library button, none of those a relaunch reads does, and ⇧⌘J opens the Library on
# Downloads, which lists them. While one runs, the button keeps
# its name and resting on it shows the running one with the finished one. With one running, ⌘Q
# asks first: Continue Downloading shows the Library's Downloads, and Quit quits, cancelling it.
# After the relaunch, Remove from List and Clear List take them off the list and keep their files.
# A new download's row dragged hands its file to the system's drag, the Library closing until it's
# let go. Resting on the Library button shows the newest finished downloads, each with its picture
# or its type's icon, and the menu of one's row there has Move to Trash, which puts its file in the
# Trash. A SIGTERM, which macOS sends at logout and restart, keeps a tab opened just before it.
# Reports how long putting them back takes.
# With the auto-archive's 12 hours made 8 s, a Today tab no window has shown for that long goes in
# the archive, counting the time across a quit, and the tab shown stays. The first tabs it takes
# bring a note, until View Archive answers it, which opens the Library there, and never again. A
# tab put back unshown has no page until it's first shown, when its page loads scrolled where it
# was. A quit with a page full screen saves its window windowed.
# With Restore windows from last session off, a launch opens one window with no page, the Spaces
# and pinned tabs back and the Today tabs in the archive.

# The Spaces come back after a quit and after the window closes: their names, their tabs with each
# tab's history, the tab each showed and the Space the window showed. Back and Forward on a tab put
# back go through its saved history, and a window that quit full screen comes back so, unless a page
# took it there. Only a tab shown loads, and one that doesn't shows its title and its favicon from
# the profile's history, which keeps none for data: pages, so the first page is served. A tab the
# user closed stays closed.
serve
log="$out/restore.log"
step=0
rm -f "$saved" "$launch_frame"
# A failed run leaves its downloads in the profile's history, and the recent downloads count them.
history_db="$(dirname "$saved")/Default/History"
if command -v sqlite3 > /dev/null && [ -f "$history_db" ]; then
  sqlite3 "$history_db" "delete from downloads; delete from downloads_url_chains;
    delete from downloads_slices;" || { echo "FAIL: the downloads wouldn't clear" >&2; exit 1; }
fi
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the restore test page didn't draw"
first=$(space)
a=$(today)
tab=$a
await "^browser: favicon $a [1-9][0-9]* bytes\$" || fail "the served page's favicon didn't reach the UI"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$server"
  echo "skip: restoring Spaces. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The page's downloads go in the check's folder, and ⇧⌘J from the page opens the Library on
# Downloads, which lists them. The slow one runs until the quit.
in_browser RenderWidgetHostViewCocoa "after a click on the page, before \"d\" downloads"
send type d
# Run after store, the download didn't start on the Mac VM, and nothing said why.
for _ in $(seq 100); do
  logged '^browser: download [0-9]* done notes.txt$' && break
  sleep 0.1
done
logged '^browser: download [0-9]* done notes.txt$' ||
  fail "the page's download didn't finish. Its lines: \
$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -i 'download' | tr '\n' ';'). \
The folder: $(ls -A "$out/downloads" | tr '\n' ' ')"
[ -f "$out/downloads/notes.txt" ] || fail "the download isn't in the check's folder"
# On the Mac it carries macOS's quarantine flag, naming the app that downloaded it, so Gatekeeper
# checks it the first time it opens (#685). Chromium's quarantine service sets it.
if [ "$(uname)" = Darwin ]; then
  flag=$(xattr -p com.apple.quarantine "$out/downloads/notes.txt" 2>&1) ||
    fail "the download has no quarantine flag: $flag"
  [ -n "$(echo "$flag" | cut -d';' -f3)" ] || fail "the download's quarantine flag names no app: $flag"
fi
send type s
expect '^browser: download [0-9]* running slow.bin$' "the page's slow download didn't start"
# Its icon flies into the Library button.
d=$(grep -m1 '^browser: download [0-9]* running slow.bin$' "$log" | cut -d' ' -f3)
expect "^browser: download flying $d\$" "the new download's icon didn't fly"
../tools/window-screenshot.sh "$pid" "$out/download-flying.png" || fail "no window to capture"
expect "^browser: download landed $d\$" "the new download's icon didn't land"
# While it runs, the Library button keeps its name and keys, and resting on it shows the running
# download over the finished one.
check_layout "with a download running"
send move 24 $((height - 25))
library_keys=⇧⌘L
[ "$(uname)" = Darwin ] || library_keys=Ctrl+Shift+L
expect "^browser: tooltip shown View Library $library_keys\$" \
  "with a download running, the Library button didn't name itself View Library"
expect '^browser: recent downloads shown 2$' \
  "with a download running, resting on the Library button didn't show it and the finished one"
../tools/window-screenshot.sh "$pid" "$out/recent-downloads-running.png" ||
  fail "no window to capture"
send move $page_point
expect '^browser: recent downloads hidden$' "the recent downloads stayed after the pointer left"
send key j cmd shift
expect '^browser: command view-downloads via page$' "⇧⌘J with the page focused didn't reach the UI"
expect '^browser: library section downloads$' "⇧⌘J didn't open the Library on Downloads"
expect '^browser: library shown$' "⇧⌘J didn't open the Library"
../tools/window-screenshot.sh "$pid" "$out/downloads.png" || fail "no window to capture"
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J again didn't close the Library"
# Tab a goes to two pages and back to the first, so it has pages to go forward to.
go_from_bar example.com
expect_page example.com 1 0 "after loading a page"
go_from_bar example.org
expect_page example.org 1 0 "after loading another"
history_key left back page
expect_page example.com 1 1 "after going back"
history_key left back page
expect "^browser: page $a back 0 forward 1 loading 0 $site\$" \
  "tab a didn't go back to the first page"
open_from_bar example.net
c=$tab
open_from_bar example.org
send key w cmd
expect_tabs "$c $a" "$c" "after ⌘W closed the tab for example.org"
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
send type reading list
send key return
expect "^browser: create space $second reading list\$" "return didn't name the Space"
send key t cmd
expect '^browser: command bar shown$' "⌘T in the new Space didn't open the command bar"
send type example.com
send key return
expect '^browser: today [0-9][0-9]*$' "the tab for example.com didn't open in the new Space"
e=$(today)
expect "^browser: loaded $e https://example.com/\$" "example.com didn't load in the new Space's tab"
post rightclick 100 112
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors, $profile, \
Export as Markdown…, Delete Space")\$" \
  "the Space's menu didn't offer its theme"
# The theme picker's pink swatch, and + for a second colour, which a relaunch brings back.
send choose "$(cased 'Edit Theme Color…')"
expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
send clickkey "$(swatch_x 1)" $swatch_y
expect "^browser: colored space $second [0-9a-f]*\$" "the pink swatch didn't colour the Space"
color=$(grep "^browser: colored space $second " "$log" | tail -1 | cut -d' ' -f5)
send clickkey 211.5 316.5
expect "^browser: themed space $second x=[-0-9.]* y=[-0-9.]* colors=2 " \
  "+ didn't give the Space a second colour"
theme=$(grep "^browser: themed space $second " "$log" | tail -1 | cut -d' ' -f5-)
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
../tools/window-screenshot.sh "$pid" "$out/space-color.png" || fail "no window to capture"
# Change Icon… in the Space's menu gives it an emoji from the icon picker's Emoji tab, found by a
# search and picked with ↩, which a relaunch brings back.
post rightclick 100 112
expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $themed, $profile, \
Export as Markdown…, Delete Space")\$" "the Space's menu didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for $second on icon\$" "Change Icon… didn't open the icon picker"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker"
# Emoji, the left half of Emoji | Icon at the picker's top.
send clickkey 92.5 39.5
send insert seedling
send key return
expect "^browser: iconed space $second 🌱\$" \
  "an emoji picked on the Emoji tab didn't become the Space's icon"
../tools/window-screenshot.sh "$pid" "$out/space-icon.png" || fail "no window to capture"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect_tabs "$c $a" "$c" "back in the first Space"
send drag 226 400 298 400
expect '^browser: sidebar width 300$' "dragging the sidebar's edge didn't make it 300 wide"
# A double-click on a tab's row turns its title into a field, all selected. Escape leaves the
# title, and return keeps the name typed, which the tab shows over its page's title.
send doubleclick $(row 0)
expect "^browser: rename tab $c\$" \
  "a double-click on the tab's row didn't turn its title into a field"
send type nope
send key escape
expect '^browser: rename cancelled$' "escape didn't cancel the tab's rename"
none_since "^browser: title $c nope" || fail "escape renamed the tab"
send doubleclick $(row 0)
expect "^browser: rename tab $c\$" "a second double-click on the row didn't rename the tab"
send type news
../tools/window-screenshot.sh "$pid" "$out/rename-tab.png" || fail "no window to capture"
send key return
expect "^browser: title $c news\$" "return didn't name the tab"
# Change Icon… in its menu hangs the icon picker under its favicon, on the Emoji tab, and the first
# emoji a search finds, which ↩ picks, is the tab's icon at once.
post rightclick $(row 0)
expect "^browser: menu shown $(cased '.*, Change Icon…, Rename…, ')" \
  "a right-click on the tab's row didn't offer Change Icon…"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for tab $c on emoji\$" \
  "Change Icon… didn't open the icon picker on the Emoji tab for the tab"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker under the tab"
../tools/window-screenshot.sh "$pid" "$out/tab-icon-picker.png" all || fail "no window to capture"
send insert fox
send key return
expect "^browser: page $c icon 🦊\$" "an emoji picked in the picker didn't become the tab's icon"
expect '^browser: icon picker hidden$' "picking an emoji didn't close the tab's icon picker"
../tools/window-screenshot.sh "$pid" "$out/tab-icon.png" || fail "no window to capture"
# With a download running, Quit in the alert ⌘Q shows asks again: on the Mac in Chrome's alert,
# elsewhere in the app's dialog in its words. Continue Downloading keeps the browser running and
# shows the Library's Downloads, and Quit quits, cancelling it.
case "$(uname)" in
  Darwin) downloads_question='^browser: modal shown Download is in progress, .*, Continue Downloading, Quit$' ;;
  *) downloads_question='^browser: dialog shown Download is in progress$' ;;
esac
post key q cmd
expect '^browser: command quit via ' "⌘Q didn't reach the UI"
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask before quitting"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle"
post key return
expect "$downloads_question" "⌘Q with a download running didn't ask first"
send choose Continue Downloading
expect '^browser: command view-downloads via chrome$' \
  "Continue Downloading didn't show the downloads"
expect '^browser: library section downloads$' \
  "Continue Downloading didn't show the Library's Downloads"
../tools/window-screenshot.sh "$pid" "$out/quit-downloads.png" || fail "no window to capture"
# The window quits full screen, and the relaunch brings it back so.
send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen before the quit"
post key q cmd
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask before quitting"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle"
post key return
expect "$downloads_question" "⌘Q didn't ask again"
# The app's dialog takes a yes to Chrome's question once it has been on screen a moment.
[ "$(uname)" = Darwin ] ||
  expect '^browser: dialog takes input$' "the dialog asking about the download never took input"
# Quit is the driver's last command: the app doesn't report it done.
post choose Quit
exec 3>&-
exited

# Launched again, the app shows the first Space with the tab for example.net, and tab a under it
# at the served page, unloaded.
log="$out/restored.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
# The window that quit full screen comes back so, and leaves it for the rest of the run, whose
# points are the window's.
await '^browser: window entered full screen$' || fail "the window that quit full screen didn't come back full screen"
../tools/window-screenshot.sh "$pid" "$out/restored-fullscreen.png" || fail "no window to capture"
send fullscreen
expect '^browser: window left full screen$' "the restored window didn't leave full screen"
await '^browser: spaces [0-9]* [0-9]*$' || fail "the relaunch didn't bring back two Spaces"
first=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3)
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
grep -q "^browser: added space $first Space 1\$" "$log" || fail "the first Space came back without its name"
grep -q "^browser: added space $second reading list\$" "$log" || fail "the second Space came back without its name"
grep -q "^browser: colored space $second $color\$" "$log" ||
  fail "the second Space came back without its colour"
grep -qF "browser: themed space $second $theme" "$log" ||
  fail "the second Space came back without its theme"
grep -q "^browser: iconed space $second 🌱\$" "$log" ||
  fail "the second Space came back without its icon"
await "^browser: space $first\$" || fail "the relaunch didn't show the first Space"
# Without the tab closed before the quit.
await '^browser: today [0-9]* [0-9]*$' || fail "the first Space didn't come back with its two tabs"
c=$(today | cut -d' ' -f1)
a=$(today | cut -d' ' -f2)
await "^browser: selected $c\$" || fail "the first Space didn't show the tab it showed last"
await "^browser: loaded $c https://example.net/\$" || fail "the tab shown didn't load"
await "^browser: title $c news\$" || fail "the tab came back without its name"
grep -q "^browser: title $c Example Domain\$" "$log" &&
  fail "the tab's page's title took its name's place"
await "^browser: page $c icon 🦊\$" || fail "the tab came back without its icon"
await "^browser: page $a back 0 forward 1 loading 0 $site\$" ||
  fail "tab a didn't come back at its page with history: $(grep "^browser: page $a" "$log" | tail -1)"
await "^browser: title $a ready\$" || fail "tab a didn't come back with its title"
await "^browser: favicon $a [1-9][0-9]* bytes\$" || fail "tab a didn't come back with its favicon"
grep -q "^browser: loaded $a " "$log" && fail "tab a loaded without being shown"
grep -q "browser: tab $a without its page\$" "$log" || fail "tab a came back with a page, unshown"
await '^browser: page [0-9]* back 0 forward 0 loading 0 https://example.com/$' ||
  fail "the second Space's tab didn't come back at example.com"
e=$(grep -m1 '^browser: page [0-9]* back 0 forward 0 loading 0 https://example.com/$' "$log" | cut -d' ' -f3)
grep -q "^browser: loaded $e " "$log" && fail "the second Space's tab loaded without being shown"
../tools/window-screenshot.sh "$pid" "$out/restored.png" || fail "no window to capture"
await '^browser: archive 1 tabs$' || fail "the relaunch didn't bring back the archived tab"
await '^browser: download [0-9]* done notes.txt$' || fail "the relaunch didn't list the download"
await '^browser: download [0-9]* cancelled slow.bin$' ||
  fail "the download the quit cancelled didn't come back cancelled"
# Newest first, so the first is this run's.
s=$(grep -m1 '^browser: download [0-9]* cancelled slow.bin$' "$log" | cut -d' ' -f3)
check_layout "after the relaunch, with the sidebar it had" 300
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$server"
  echo "skip: restored history. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# Rename… in its menu, with the name taken out, gives the tab back its page's title.
post rightclick $(row 0)
menu=$(cased "Copy Link$share, Change Icon…, Remove Icon, Rename…, Mute (disabled), Add Split View, Duplicate, Add to Folder › New Folder, Move to › [^,]* (checked), Move to › reading list, Move to › New Space…, Archive Tab, Archive Tabs Below")
expect "^browser: menu shown $menu\$" \
  "a right-click on the renamed tab's row didn't offer Rename…"
send choose Rename…
expect "^browser: rename tab $c\$" "Rename… didn't turn the tab's title into a field"
send key backspace
send key return
expect "^browser: title $c Example Domain\$" \
  "taking the name out didn't give the tab back its page's title"
# A conversion in the picker's search is the input method's, ↩ included, and once committed it
# finds no emoji, so ↩ picks nothing. Escape closes the picker with the icon as it was.
post rightclick $(row 0)
expect "^browser: menu shown $menu\$" "a right-click on the tab's row didn't offer Remove Icon"
send choose "$(cased 'Change Icon…')"
expect "^browser: icon picker for tab $c on emoji\$" \
  "Change Icon… didn't open the icon picker again"
expect '^browser: icon picker shown$' "Change Icon… didn't show the icon picker again"
send ime on
send type nihongo
send key space
[ "$marked" != none ] || fail "the picker's search didn't take the conversion"
none_since "^browser: page $c icon " || fail "a composition became the tab's icon"
send key return
send ime off
send key return
none_since "^browser: page $c icon " || fail "↩ on a search that finds no emoji picked one"
send key escape
expect '^browser: icon picker dismissed$' "escape didn't close the tab's icon picker"
none_since "^browser: page $c icon " || fail "escape changed the tab's icon"
# Remove Icon gives it back its favicon.
post rightclick $(row 0)
expect "^browser: menu shown $menu\$" "a right-click on the tab's row didn't offer Remove Icon"
send choose "$(cased 'Remove Icon')"
expect "^browser: page $c icon none\$" "Remove Icon didn't give the tab back its favicon"
send click $page_point
# The tab closed before the quit reopens from the Library at the top of Today, at its page. ⌘W
# archives it again.
send key l cmd shift
expect '^browser: command view-library via page$' "⇧⌘L with the page focused didn't reach the UI"
expect '^browser: library shown$' "⇧⌘L didn't open the Library"
send click 300 111
expect '^browser: restore archived [0-9][0-9]*$' "a click on the archived tab didn't restore it"
expect "^browser: today [0-9][0-9]* $c $a\$" \
  "the archived tab didn't come back at the top of Today: Today is \"$(today)\""
r=$(today | cut -d' ' -f1)
expect "^browser: selected $r\$" "the tab from the archive isn't shown"
expect "^browser: loaded $r https://example.org/\$" "the tab from the archive didn't load its page"
expect '^browser: archive 0 tabs$' "the tab reopened stayed in the archive"
expect '^browser: library hidden$' "reopening a tab didn't close the Library"
send click $page_point
send key w cmd
expect_tabs "$c $a" "$c" "after closing the tab from the archive"
# The downloads the relaunch read didn't fly.
grep -q '^browser: download flying ' "$log" && fail "a download the relaunch read flew in"
# Resting on the footer's Library button shows the newest finished downloads over the tab list,
# here notes.txt alone, as the cancelled one isn't finished. They stay as the pointer comes onto
# them, and go once it's on neither.
send move 24 $((height - 25))
expect '^browser: recent downloads shown 1$' \
  "resting on the Library button didn't show the recent download"
# notes.txt's row shows the system's icon for its type, as Finder's or the file manager's.
expect '^browser: download [0-9]* preview icon 52x52$' "notes.txt's row didn't show its type's icon"
send move 24 $((height - 45))
send move 120 $((height - 78))
none_since '^browser: recent downloads hidden$' ||
  fail "the recent downloads went as the pointer came onto them"
../tools/window-screenshot.sh "$pid" "$out/recent-downloads.png" || fail "no window to capture"
send move $page_point
expect '^browser: recent downloads hidden$' "the recent downloads stayed after the pointer left"
# A right-click on the button picks what the rest lists, all of it at first. With Other Files off,
# notes.txt isn't listed, so a rest shows nothing, and back on it's listed again.
all=$(cased 'Images (checked), Videos (checked), Other Files (checked), Captures (checked)')
post rightclick 24 $((height - 25))
expect "^browser: menu shown $all\$" "a right-click on the Library button didn't show what it lists"
../tools/window-screenshot.sh "$pid" "$out/library-button-menu.png" || fail "no window to capture"
send choose "$(cased 'Other Files')"
expect '^browser: hover kinds images, videos, captures$' "Other Files didn't turn off"
send move $page_point
send move 24 $((height - 25))
expect '^browser: recent downloads none$' "with Other Files off, a rest still had notes.txt to show"
none_since '^browser: recent downloads shown' || fail "with Other Files off, notes.txt still showed"
send move $page_point
post rightclick 24 $((height - 25))
expect "^browser: menu shown $(cased 'Images (checked), Videos (checked), Other Files, ')" \
  "the Library button's menu didn't untick Other Files"
send choose "$(cased 'Other Files')"
expect '^browser: hover kinds images, videos, other files, captures$' "Other Files didn't turn on"
# A finished download's menu, with Chrome's word off the Mac for the file manager there.
file_manager=Finder
[ "$(uname)" = Darwin ] || file_manager=Folder
finished_menu=$(cased "Open, Show in $file_manager, Move to Trash, Remove from List, Clear List")
# Remove from List in the cancelled download's menu takes it off the list, and Clear List in the
# finished one's takes off every download that isn't running and keeps their files.
send key j cmd shift
expect '^browser: library section downloads$' "⇧⌘J didn't open the Library on Downloads"
expect '^browser: library shown$' "⇧⌘J didn't open the Library after the relaunch"
listed=$(grep '^browser: downloads ' "$log" | tail -1 | cut -d' ' -f3)
post rightclick 300 111
expect "^browser: menu shown $(cased 'Remove from List, Clear List')\$" \
  "a right-click on the cancelled download's row didn't show its menu"
send choose "$(cased 'Remove from List')"
expect "^browser: download $s Remove via download menu\$" "Remove from List didn't reach it"
expect "^browser: downloads $((listed - 1))\$" "Remove from List didn't take the download off"
post rightclick 300 111
expect "^browser: menu shown $finished_menu\$" \
  "a right-click on the finished download's row didn't show its menu"
send choose "$(cased 'Clear List')"
expect '^browser: command clear-downloads via download menu$' "Clear List didn't run"
expect '^browser: downloads 0$' "Clear List didn't empty the list"
[ -f "$out/downloads/notes.txt" ] || fail "Clear List deleted the download's file"
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J didn't close the Library"
switch "2 cmd" "$a"
expect "^browser: loaded $a $site\$" "tab a didn't load once shown"
# A finished download's row dragged hands its file to the system's drag and closes the Library,
# which comes back once it's let go where nothing takes it. Move to Trash in its menu then puts the
# file in the Trash and takes it off the list.
send click $page_point
send type q
trashed='^browser: download [0-9]* done trash.txt$'
expect "$trashed" "the page's second download didn't finish"
t=$(grep "$trashed" "$log" | tail -1 | cut -d' ' -f3)
send key j cmd shift
expect '^browser: library shown$' "⇧⌘J didn't open the Library on the second download"
press=$((step + 1))
send press 300 111
send dragto 300 140
expect "^browser: download $t drag\$" "dragging the download's row didn't start the system's drag"
# Off the Mac a user holds the drag until the Library has gone, as the release waits for it there.
# On the Mac the release lands as the Library closes, where nothing takes the file, not on the page.
[ "$(uname)" = Darwin ] ||
  expect '^browser: library hidden$' "the Library didn't close for the download's drag"
send release 300 140
# No real button holds the system's drag, so it may end before the release reaches it.
dragged() {
  for _ in $(seq 100); do
    sed -n "/^browser: drive $press: /,\$p" "$log" | sed -n "/$1/,\$p" | grep -q "$2" && return 0
    sleep 0.1
  done
  return 1
}
dragged "download $t drag" '^browser: library hidden$' ||
  fail "the Library didn't close for the download's drag"
dragged "download $t drag" '^browser: file drag ended nowhere$' ||
  fail "the download's drag didn't end where nothing took it"
dragged 'file drag ended' '^browser: library shown$' ||
  fail "the Library didn't come back after the download's drag"
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J didn't close the Library after the download's drag"
# Its row in the recent downloads shows the Library row's menu, whose Move to Trash takes the rows
# away with the last of them.
send move 24 $((height - 25))
expect '^browser: recent downloads shown 1$' \
  "resting on the Library button didn't show the second download"
post rightclick 120 $((height - 78))
expect "^browser: menu shown $finished_menu\$" \
  "a right-click on the recent download's row didn't show its menu"
send choose "$(cased 'Move to Trash')"
expect "^browser: download $t Trash via download menu\$" "Move to Trash didn't reach the download"
expect '^browser: downloads 0$' "Move to Trash didn't take the download off the list"
expect '^browser: recent downloads hidden$' "the recent downloads stayed with none left"
[ -e "$out/downloads/trash.txt" ] && fail "Move to Trash left the file in the check's folder"
# Media tiles the video downloaded, which plays in its tile without sound while the tile shows,
# and stops once the Library closes. Only the Mac's platform plays one.
send click $page_point
send type v
expect '^browser: download [0-9]* done clip.mp4$' "the page's video didn't download"
# The menu bar keeps its titles' case on every system, unlike the sidebar's menus.
send menu 'View Media…'
expect '^browser: library section media$' "View Media… didn't open the Library on Media"
expect '^browser: library shown$' "View Media… didn't open the Library"
if [ "$(uname)" = Darwin ]; then
  expect '^browser: media video playing clip.mp4$' "the video's tile didn't play it"
  ../tools/window-screenshot.sh "$pid" "$out/media-video.png" || fail "no window to capture"
fi
send menu 'View Media…'
expect '^browser: library hidden$' "View Media… didn't close the Library"
if [ "$(uname)" = Darwin ]; then
  expect '^browser: media video stopped clip.mp4$' "the video played on with the Library closed"
fi
send click $page_point
tab=$a
history_key right forward page
expect_page example.com 1 1 "going forward in tab a's restored history"
send key 2 ctrl
expect "^browser: space $second in " "⌃2 didn't show the second Space"
expect_tabs "$e" "$e" "in the second Space"
expect "^browser: loaded $e https://example.com/\$" "the second Space's tab didn't load once shown"
# The window's close button, which ends the app too: the driver doesn't report it done.
echo "click 20 24" >&3
exec 3>&-
await '^browser: window closed ' || fail "a click on the close button didn't close the window"
exited

# Closing the window kept them too, and the second Space shows.
log="$out/restored-again.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the launch after closing the window didn't restore the Spaces"
await '^browser: spaces [0-9]* [0-9]*$' || fail "closing the window didn't keep both Spaces"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
await "^browser: space $second\$" || fail "the launch after closing the window didn't show the second Space"
await '^browser: today [0-9]*$' || fail "the second Space didn't come back with its tab"
await "^browser: loaded $(today) https://example.com/\$" || fail "the second Space's tab didn't load"
await '^browser: page [0-9]* back 1 forward 1 loading 0 https://example.com/$' ||
  fail "tab a didn't keep the page it went forward to"
# The window was left full screen before the close, so it doesn't come back so.
grep -q '^browser: window entered full screen$' "$log" && fail "the window came back full screen after it quit windowed"
# Tab a is unloaded in the middle of its history, with Back and Forward on. Shown, and with its
# page perhaps still loading, ⌘← goes back to the page before and ⌘→ forward over the pages after.
a=$(grep -m1 '^browser: page [0-9]* back 1 forward 1 loading 0 https://example.com/$' "$log" | cut -d' ' -f3)
grep -q "^browser: page $a unloaded 1\$" "$log" || fail "tab a didn't come back unloaded"
grep -q "^browser: loaded $a " "$log" && fail "tab a loaded without being shown"
send key 1 ctrl
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "⌃1 didn't show tab a"
send key left cmd
expect "^browser: command back via " "⌘← on the restored tab didn't reach the UI"
expect "^browser: page $a back 0 forward 1 loading 0 $site\$" \
  "Back on the restored tab didn't go to the page before: $(grep "^browser: page $a" "$log" | tail -1)"
expect "^browser: loaded $a $site\$" "Back on the restored tab didn't load the page before"
send key right cmd
expect "^browser: command forward via " "⌘→ on the restored tab didn't reach the UI"
expect "^browser: page $a back 1 forward 1 loading 0 https://example.com/\$" \
  "Forward didn't come back to the page it left"
send key right cmd
expect "^browser: page $a back 1 forward 0 loading 0 https://example.org/\$" \
  "Forward didn't reach the last page of the restored history"
# The Space shown at the quit is the one the SIGTERM launch below opens its tab in, and the count it
# checks is the second Space's tab and the new one.
send key 2 ctrl
expect "^browser: space $second in " "⌃2 didn't show the second Space again"
exec 3>&-
quit

# A SIGTERM ends the session at once, before the save the new tab scheduled, which the core writes
# as Chrome exits.
log="$out/session-ending.log"
BROWSER_URL="$site" "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await '^browser: restored in ' || fail "the launch before the SIGTERM didn't restore the Spaces"
await "^browser: loaded [0-9]* $site\$" || fail "the page opened over the Spaces didn't load"
kill -TERM "$pid"
exited
[ "$(python3 -I -c 'import json, sys
print(*[len(space["today"]) for space in json.load(open(sys.argv[1]))["spaces"]])' "$saved")" = "2 2" ] ||
  fail "the tab opened before the SIGTERM wasn't kept"
restore_ms=$(sed -n 's/^browser: restored in \([0-9.]*\) ms$/\1/p' "$out/restored.log" "$out/restored-again.log" | tr '\n' ' ')

# Auto-archive, 8 s instead of 12 hours. Tab a, then b, opens over the Spaces, and a isn't seen
# after b shows. Past a's 8 s, the next launch archives a at once and b 8 s after c shows.
log="$out/archive-a.log"
rm -f "$saved" "$launch_frame"
BROWSER_URL="${site}?a" "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await "^browser: loaded [0-9]* ${site}?a\$" || fail "tab a didn't load"
quit
log="$out/archive-b.log"
BROWSER_URL="${site}?b" "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await "^browser: loaded [0-9]* ${site}?b\$" || fail "tab b didn't load over tab a"
seen=$(date +%s)
quit
wait=$((seen + 9 - $(date +%s)))
[ "$wait" -le 0 ] || sleep "$wait"
log="$out/archive-c.log"
step=0
start=$(date +%s)
BROWSER_DRIVE=1 BROWSER_URL="${site}?c" "$browser" --archive-tabs-after=8 < "$out/drive" \
  > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: archive 1 tabs$' || fail "tab a, unseen for 8 s across the quit, wasn't archived"
[ $(($(date +%s) - start)) -lt 6 ] || fail "tab a was archived 8 s after the launch, not at once"
await "^browser: loaded [0-9]* ${site}?c\$" || fail "tab c didn't load"
c=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}?c\$|\1|p" "$log" | head -1)
await '^browser: archive 2 tabs$' || fail "tab b, unseen for 8 s after c showed, wasn't archived"
[ $(($(date +%s) - start)) -ge 8 ] || fail "tab b was archived before it was unseen for 8 s"
await "^browser: today $c\$" || fail "Today is \"$(today)\" after the auto-archive, not tab c alone"
# The first tabs auto-archive takes bring a note, once, which stays until it's answered: the
# toast at the top right, the interval in its words being the user's, not the run's 8 s. View
# Archive, the chip before its ×, opens the Library on Archived Tabs, and the note never comes
# back.
note='Tabs unseen for 12 hours went to the archive\.'
grep -c "^browser: toast shown $note \[view archive, dismiss\]\$" "$log" | grep -qx 1 ||
  fail "the auto-archive's note didn't show once: $(grep '^browser: toast shown' "$log")"
await '^browser: toast settled$' || fail "the auto-archive's note didn't settle"
../tools/window-screenshot.sh "$pid" "$out/archive-note.png" || fail "no window to capture"
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
# The × chip is 31 wide, 13.5 in from the toast's edge, 18 in from the window's, and View
# Archive's about 98, 8 before it. Both centred 41 down.
view_archive="$((${content%x*} - 18 - 13 - 31 - 8 - 49)) 41"
send move $view_archive
send click $view_archive
expect '^browser: toast button view archive$' "View Archive wasn't pressed at $view_archive"
expect '^browser: command view-archive via archive note$' "View Archive didn't run"
expect '^browser: library shown$' "View Archive didn't open the Library"
expect '^browser: toast hidden$' "the note stayed after View Archive"
exec 3>&-
quit
grep -q '"archive_noted": *true' "$saved" || fail "the Spaces didn't keep that the note was answered"
# Tab d opens over c, which the next 8 s archive, with no note.
log="$out/archive-d.log"
BROWSER_URL="${site}?d" "$browser" --archive-tabs-after=8 < /dev/null > "$log" 2>&1 &
pid=$!
await '^browser: archive 3 tabs$' || fail "tab c, unseen for 8 s after d showed, wasn't archived"
grep -q '^browser: toast shown Tabs unseen' "$log" && fail "the answered note came back"
quit

# A tab put back that no window shows has no page, as the core logs, until it's first shown, when
# its page loads scrolled where it was.
long='data:text/html,<title>top</title><div style="height:5000px"></div>
<script>onscroll = () => document.title = "scrolled " + Math.round(scrollY);
onkeydown = (e) => e.key == "f" && document.documentElement.requestFullscreen();</script>'
log="$out/unloaded-a.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$long" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: today [0-9][0-9]*$' || fail "the long page's tab isn't in Today"
l=$(today)
send scroll $page_point 40 10
expect "^browser: title $l scrolled [1-9][0-9]*\$" "the long page didn't scroll"
scrolled=$(date +%s)
open_from_bar example.com
# Chrome sends a page's scroll to the browser a second after it changes, for the quit to save.
wait=$((scrolled + 2 - $(date +%s)))
[ "$wait" -le 0 ] || sleep "$wait"
exec 3>&-
quit
log="$out/unloaded-b.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the launch after scrolling didn't restore the Spaces"
await '^browser: today [0-9]* [0-9]*$' || fail "the Space didn't come back with its two tabs"
shown=$(today | cut -d' ' -f1)
l=$(today | cut -d' ' -f2)
await "^browser: loaded $shown https://example.com/\$" || fail "the tab shown didn't load"
grep -q "browser: tab $l without its page\$" "$log" || fail "the tab not shown came back with a page"
grep -q "browser: tab $l has its page\$" "$log" && fail "the tab not shown got its page"
switch "2 cmd" "$l"
expect "browser: tab $l has its page\$" "the tab put back didn't get its page once shown"
expect "^browser: title $l scrolled [1-9][0-9]*\$" "the tab put back loaded its page at the top"
# The page takes its scroll back before it has loaded, and a click while it loads misses it.
expect "^browser: page $l back [0-9]* forward [0-9]* loading 0 " "the tab put back didn't finish loading"
../tools/window-screenshot.sh "$pid" "$out/unloaded-scrolled.png" || fail "no window to capture"
# A quit while a page is full screen, as a video is, keeps the window windowed: the page took it
# full screen, and it would come back so with no video in it (#383).
send click $page_point
send key f
expect '^browser: page entered full screen$' "the long page didn't go full screen"
expect '^browser: window entered full screen$' "the window didn't go full screen with the page"
exec 3>&-
quit
grep -q '"fullscreen": *true' "$saved" &&
  fail "the window was saved full screen for the page that was full screen at the quit"

# Help › Restore Earlier Sidebar brings back the profile's sidebar from a backup, which the core
# takes as it saves the Spaces, in the profile's directory. The sidebar it replaces becomes the
# newest backup, which brings it back in turn. Without the run's earlier backups, the first one
# taken is this launch's sidebar.
backups="$(dirname "$saved")/Default/Sidebar Backups"
rm -rf "$backups"
log="$out/sidebar-backups.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the launch for sidebar backups didn't restore the Spaces"
await '^browser: sidebar backups of Default: 1$' || fail "saving the Spaces took no backup"
ls "$backups"/*.json > /dev/null 2>&1 || fail "the backup isn't in the profile's directory"
await '^browser: newest sidebar backup ' || fail "Help wasn't given the backup"
backed_up=$(today | wc -w | tr -d " ")
# A page of the check's own site, so not open_from_bar, which types a bare host.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site}notes.txt"
send key return
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the tab opened after the backup didn't load ${site}notes.txt"
expect "^browser: today [0-9 ]*\$" "the tab opened after the backup isn't in Today"
await_today() {
  for _ in $(seq 100); do
    [ "$(today | wc -w | tr -d " ")" = "$1" ] && return 0
    sleep 0.1
  done
  fail "$2: Today is \"$(today)\""
}
await_today $((backed_up + 1)) "the tab opened after the backup isn't in Today"
# By the newest's label, as the menu names it. Off the Mac there's no menu bar, so the way in is
# the command bar's Restore Earlier Sidebar…, which lists the backups in ilo's menu, newest first.
restore_newest() {
  label=$(grep '^browser: newest sidebar backup ' "$log" | tail -1 | cut -d' ' -f6-)
  if [ "$(uname)" = Darwin ]; then
    send menu "Restore Earlier Sidebar › $label"
  else
    bar_command restore-earlier-sidebar "Restore Earlier Sidebar…" "restore earlier"
    expect "^browser: menu shown $label" "$1's list didn't start with the newest backup, $label"
    send choose "$label"
  fi
  expect "^browser: dialog shown Restore the sidebar from " "$1 didn't ask first"
  expect '^browser: dialog settled$' "$1's dialog didn't settle"
  send key return
  expect '^browser: dialog confirmed$' "return didn't choose Restore"
  expect '^browser: sidebar of Default restored, [1-9][0-9]* Spaces$' "$1 didn't restore the sidebar"
  expect "^browser: sidebar backups of Default: $2\$" "$1 didn't keep the sidebar it replaced"
}
restore_newest "Restore Earlier Sidebar" 2
await_today "$backed_up" "the backup's sidebar came back with the tab opened after it"
../tools/window-screenshot.sh "$pid" "$out/sidebar-restored.png" || fail "no window to capture"
# The sidebar replaced is the newest backup, so restoring it undoes the restore.
restore_newest "Restoring the sidebar replaced" 3
await_today $((backed_up + 1)) "restoring the sidebar replaced didn't bring back the tab"
exec 3>&-
quit
# With Restore windows from last session turned off in Settings › Advanced, a launch opens one
# window showing no page, as ⌘N's does: the Space comes back with its pinned tab, and its Today tab
# goes in the archive. The second window doesn't come back (#684).
log="$out/fresh-a.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}?pinned" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
p=$(today)
send key d cmd
expect "^browser: pinned $p\$" "⌘D didn't pin the tab"
open_from_bar example.com
send key n cmd
expect '^browser: window opened [0-9]*$' "⌘N didn't open a second window"
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Advanced")
expect '^browser: settings tab Advanced$' "a click on Advanced didn't show its tab"
settings_drew Advanced
# Where Settings drew the switch, as the Mini Player's row above it moves it.
at=$(drawn "switch restore windows") || fail "Restore windows from last session has no switch"
send clickkey $at
expect '^browser: restore windows off$' \
  "a click on Restore windows from last session didn't turn it off"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
exec 3>&-
quit
grep -q '"restore_windows": *false' "$saved" || fail "Restore windows off wasn't saved"
log="$out/fresh-b.log"
BROWSER_DRIVE=1 "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await '^browser: restored in ' || fail "a launch with Restore windows off didn't bring the Spaces back"
await "^browser: pinned $p\$" || fail "the pinned tab didn't come back with Restore windows off"
await '^browser: archive 1 tabs$' || fail "the Today tab didn't go in the archive with Restore windows off"
# A window says it's open after its Space's lines, on Linux after the archive's too, so the count
# below waits for it (#1038).
await '^browser: window opened ' || fail "no window opened with Restore windows off"
../tools/window-screenshot.sh "$pid" "$out/fresh-window.png" || fail "no window to capture"
[ -z "$(today)" ] || fail "Today is \"$(today)\" with Restore windows off, not empty"
grep -q '^browser: loaded ' "$log" && fail "a page loaded at a launch with Restore windows off"
[ "$(grep -c '^browser: window opened ' "$log")" = 1 ] ||
  fail "the second window came back with Restore windows off"
quit
echo "ok: after ⌘Q and after closing the window, the Spaces come back with their names, themes, icons, tabs, history and the tab each showed, in the Space shown last, with the sidebar's width; only tabs shown load, the others show their titles and favicons, a tab renamed with a double-click keeps its name over its page's title until Rename… takes it out, Change Icon… hangs the icon picker under its favicon, and the emoji picked there keeps the favicon's place until Remove Icon, and a closed tab stays closed, in the archive, from which it reopens at its page. Back and Forward on a restored tab walk its saved history, and a window that quit full screen comes back so, unless a page took it there. A page's downloads are listed in the Library's Downloads, which ⇧⌘J opens, and a new one's icon flies into the Library button, though none of those a relaunch reads does. With one running, ⌘Q asks first, Continue Downloading shows Downloads and Quit cancels it, and after the relaunch Remove from List and Clear List take them off and keep their files. Resting on the Library button shows the newest finished downloads, until the pointer leaves them, and its right-click menu picks the kinds it lists, and Media plays a video downloaded in its tile while the Library shows it, and a row's menu is the Library row's. A SIGTERM keeps a tab opened just before it. Today tabs no window shows go in the archive after the interval, counted across a quit, and the tab shown stays; the first ones bring a note, until View Archive answers it, opening Archived Tabs, and none after that ($out/archive-note.png). A tab put back unshown has no page until it's shown, when it loads scrolled where it was. With Restore windows from last session off, a launch opens one window with no page, the pinned tab back and the Today tab in the archive. Help › Restore Earlier Sidebar asks, then brings back a backup's sidebar, and the sidebar it replaced is the newest backup, which undoes it. Windows: $out/sidebar-restored.png, $out/fresh-window.png"
echo "  Restore, 3 tabs in 2 Spaces: ${restore_ms}ms. Windows: $out/restored.png, $out/tab-icon-picker.png, $out/unloaded-scrolled.png,"
echo "  $out/recent-downloads.png, $out/library-button-menu.png, $out/media-video.png"
