# Little ilo, Arc's Little Arc. ⌥⌘N opens a Little ilo window for the Space the window shows, under
# its bar, and ⌘L there opens the command bar, whose URL loads in it. ⌘O moves its page, live, to
# that Space in the main window and closes it. ⌥⌘-click on a link opens the link in one, not
# beside the page as ⌥-click does, and ⌘W closes that window, its page with it. An escape the page
# doesn't use closes one. A tab its page opens goes behind, to the top of its Space's Today, and
# outlives it. Closing one archives its tab, and ⇧⌘T reopens it in a new one. The first opens 900 × 640, and each after at the size the last one closed at. On the
# Mac one opens over a full-screen window, and the Dock's menu opens one after its New Window, and
# hides and shows them all.

serve
log="$out/little.log"
step=0
rm -f "$saved" "$launch_frame" "$(dirname "$saved")/little-size" "$(dirname "$saved")/little-in-menu-bar"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"
main=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
home=$(space)
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$server"
  echo "skip: little. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# The Little ilo window that opened last, from its log line.
opened() { grep '^browser: little window opened ' "$log" | tail -1 | cut -d' ' -f5; }

# ⌥⌘N opens an empty one for the Space shown, and ⌘L's URL loads in it.
send key n cmd alt
expect '^browser: command new-little-window via ' "⌥⌘N didn't run New Little ilo Window"
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘N didn't open a Little ilo window for Space $home"
little=$(opened)
expect "^browser: window opened $little\$" "the Little ilo window didn't open"
expect "^browser: window $little frame -\{0,1\}[0-9]* -\{0,1\}[0-9]* 900 640\$" \
  "the first Little ilo window didn't open 900 × 640: $(grep "^browser: window $little frame" "$log")"
send window 2
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
send insert "${site}notes.txt"
send key return
expect "^browser: loaded [0-9]* ${site}notes.txt\$" \
  "the command bar's URL didn't load in the Little ilo window"
moved=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}notes.txt\$|\1|p" "$log" | tail -1)
expect "^browser: window $little shows $moved\$" "the Little ilo window didn't show its page"
../tools/window-screenshot.sh "$pid" "$out/little.png" all || fail "no window to capture"

# ⌘O moves its page to its Space in the main window, which shows it, and closes it.
send key o cmd
expect '^browser: command open-in-main-window via ' "⌘O didn't run Open in Main Window"
expect "^browser: little window $little put $moved in window $main space $home\$" \
  "⌘O didn't move the page to the main window"
expect "^browser: window closed $little\$" "⌘O didn't close the Little ilo window"
expect "^browser: window $main shows $moved\$" "the main window doesn't show the page ⌘O moved"
expect "^browser: today $moved $a\$" "the moved page isn't at the top of Today: \"$(today)\""
send window 1
../tools/window-screenshot.sh "$pid" "$out/little-moved.png" || fail "no window to capture"
send key w cmd
expect "^browser: tab $moved closed\$" "⌘W didn't close the moved page in the main window"

# ⌥⌘-click on the page's link opens it in a Little ilo window, not beside the page.
send layout
frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
x=${frame%%,*}
y=${frame#*,}
send click $page_point
send click $((x + 20)) $((y + 20)) alt cmd
expect "^browser: link ${site}notes.txt in little window [0-9]* from $main profile [0-9]*\$" \
  "⌥⌘-click on a link didn't open it in a Little ilo window"
little=$(opened)
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the link didn't load in the Little ilo window"
none_since "^browser: splits $a+" || fail "⌥⌘-click on a link opened it beside the page too"
# The driver numbers windows in the order they were made, the closed one too.
send window 3
../tools/window-screenshot.sh "$pid" "$out/little-link.png" all || fail "no window to capture"
# ⌘W closes the window, its page with it, and the next opens at the size it had.
send resize 760 520
send key w cmd
expect '^browser: command close-tab via ' "⌘W didn't run in the Little ilo window"
expect "^browser: window closed $little\$" "⌘W didn't close the Little ilo window"
send window 1

# A ⌘-click on its page's link opens the link behind, at the top of its Space's Today, and an
# escape the page doesn't use closes one.
send key n cmd alt
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘N didn't open a second Little ilo window"
little=$(opened)
expect "^browser: window $little frame -\{0,1\}[0-9]* -\{0,1\}[0-9]* 760 520\$" \
  "the Little ilo window didn't open at the last one's size: $(grep "^browser: window $little frame" "$log")"
send window 4
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the second Little ilo window's command bar"
send insert "$site"
send key return
expect "^browser: window $little shows [0-9]*\$" "the second Little ilo window didn't show its page"
page=$(sed -n "s|^browser: window $little shows \([0-9]*\)\$|\1|p" "$log" | tail -1)
# Chromium drops a click on a page that hasn't drawn yet.
expect "^browser: title $page ready\$" "the second Little ilo window's page didn't draw"
send layout
# The layout's frames are from the content's bottom-left corner, as the Mac's views are, so the
# page's top is the content's height less its bottom and height: under the bar, not at its foot.
layout=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout')
content=$(echo "$layout" | grep -o 'content [0-9]*x[0-9]*' | cut -d' ' -f2)
frame=$(echo "$layout" | grep -o 'WebContentsViewCocoa [0-9,]* [0-9]*x[0-9]*' | head -1)
origin=$(echo "$frame" | cut -d' ' -f2)
size=$(echo "$frame" | cut -d' ' -f3)
x=${origin%%,*}
top=$((${content#*x} - ${origin#*,} - ${size#*x}))
send click $((x + 20)) $((top + 20)) cmd
expect "^browser: tab [0-9]* opened by $page in the background\$" \
  "a ⌘-click on the Little ilo window's link didn't open a tab"
behind=$(grep "^browser: tab [0-9]* opened by $page" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: little window $little put $behind in space $home\$" \
  "the tab the Little ilo window's page opened didn't go to its Space"
expect "^browser: today $behind $a\$" \
  "the tab opened behind isn't at the top of Today: \"$(today)\""
none_since "^browser: window $little shows $behind\$" ||
  fail "the Little ilo window showed the tab its page opened behind"
send click 450 340
send key escape
expect '^browser: little window closed by escape$' "an unused escape didn't close Little ilo"
expect "^browser: window closed $little\$" "the escape didn't close the Little ilo window"
none_since "^browser: tab $behind closed\$" ||
  fail "the tab the Little ilo window's page opened behind closed with the window"
send window 1

# Over the main window in full screen, on the Mac, ⌥⌘N's Little ilo window opens on its desktop,
# over it, rather than on another.
if [ "$(uname)" = Darwin ]; then
  send fullscreen
  expect '^browser: window entered full screen$' "the main window didn't go full screen"
  send key n cmd alt
  expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
    "⌥⌘N in full screen didn't open a Little ilo window"
  little=$(opened)
  expect '^browser: little window over full screen$' \
    "the Little ilo window didn't open over the full-screen window"
  send window 5
  ../tools/window-screenshot.sh "$pid" "$out/little-full-screen.png" all || fail "no window to capture"
  send key w cmd
  expect "^browser: window closed $little\$" "⌘W didn't close the Little ilo window over full screen"
  send window 1
  send fullscreen
  expect '^browser: window left full screen$' "the main window didn't leave full screen"
fi

# The Dock's menu, the Mac's alone: New Little ilo Window after the engine's New Window opens one
# for the Space shown, and while one is open Hide All Little ilo Windows hides it, which Show All
# Little ilo Windows brings back in front.
if [ "$(uname)" = Darwin ]; then
  dock_menu() { grep '^browser: dock menu ' "$log" | tail -1 | cut -d' ' -f4-; }
  send dock
  expect '^browser: dock menu \(.*, \)\{0,1\}New Window, New Little ilo Window\(, .*\)\{0,1\}$' \
    "the Dock menu has no New Little ilo Window after New Window: \"$(dock_menu)\""
  case "$(dock_menu)" in *"All Little ilo Windows"*) fail "the Dock menu offers Show or Hide All with none open" ;; esac
  send dock New Little ilo Window
  expect '^browser: dock opened little window [0-9]*$' "the Dock's New Little ilo Window didn't open one"
  little=$(opened)
  expect "^browser: little window opened $little profile [0-9]* home $home\$" \
    "the Dock's Little ilo window isn't for Space $home"
  send dock
  expect '^browser: dock menu .*New Little ilo Window, Hide All Little ilo Windows' \
    "the Dock menu doesn't offer Hide All with one open: \"$(dock_menu)\""
  send dock Hide All Little ilo Windows
  expect "^browser: dock hid little windows $little\$" "Hide All Little ilo Windows didn't hide it"
  send dock
  expect '^browser: dock menu .*New Little ilo Window, Show All Little ilo Windows' \
    "the Dock menu doesn't offer Show All with all hidden: \"$(dock_menu)\""
  send dock Show All Little ilo Windows
  expect "^browser: dock showed little windows $little\$" "Show All Little ilo Windows didn't show it"
  expect "^browser: key window $little\$" "Show All didn't put the Little ilo window in front"
  send window 6
  ../tools/window-screenshot.sh "$pid" "$out/little-dock.png" all || fail "no window to capture"
  send key w cmd
  expect "^browser: window closed $little\$" "⌘W didn't close the Dock's Little ilo window"
  send window 1
fi

# The menu bar's button, the Mac's alone: none until Settings › General turns it on, a click on it
# opens a Little ilo window for the Space shown, and turned off it's gone.
if [ "$(uname)" = Darwin ]; then
  send statusitem
  expect '^browser: status item none$' "the menu bar has Little ilo's button before it was turned on"
  send menu Settings…
  expect '^browser: settings shown General$' "Settings… didn't show General"
  settings_drew General
  menu_bar_switch=$(drawn "switch little in the menu bar") ||
    fail "Settings didn't draw Show Little ilo in the menu bar's switch"
  send clickkey $menu_bar_switch
  expect '^browser: little in the menu bar on$' "a click on the menu bar switch didn't turn it on"
  send statusitem
  expect '^browser: status item shown$' "turning the switch on didn't put the button in the menu bar"
  send statusitem click
  expect '^browser: menu bar opened little window [0-9]*$' "the menu bar's button didn't open a Little ilo window"
  little=$(opened)
  expect "^browser: little window opened $little profile [0-9]* home $home\$" \
    "the menu bar's Little ilo window isn't for Space $home"
  # A new window goes in front of the others, so it logs no "key window"; ⌘W closing it shows it's key.
  send key w cmd
  expect "^browser: window closed $little\$" "the menu bar's Little ilo window didn't come to the front: ⌘W didn't close it"
  send menu Settings…
  expect '^browser: settings shown General$' "Settings… didn't show General again"
  settings_drew General
  send clickkey $menu_bar_switch
  expect '^browser: little in the menu bar off$' "a click on the menu bar switch didn't turn it off"
  send statusitem
  expect '^browser: status item none$' "turning the switch off left the button in the menu bar"
  settings_close
  post clickkey $close
  expect '^browser: settings hidden$' "the close button didn't hide Settings"
  send window 1
fi

# ⌥⌘-click on a Today tab's row moves the tab, its page live, into a Little ilo window for its
# Space, and the main window shows another.
send window 1
# Shown first, so its page has loaded and can move live: a tab opened behind loads only when shown.
send click $(row 0)
expect '^browser: selected [0-9][0-9]*' "a click on the first Today row didn't show its tab"
shown=$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: loaded $shown " "the first Today tab's page didn't load once shown"
# Read from the sidebar's own line: each Little ilo window logs its own Today, so the last Today
# line may be one of theirs.
send click $(row 0) alt cmd
expect '^browser: command little via sidebar [0-9][0-9]*$' "⌥⌘-click on a Today tab's row didn't reach the sidebar"
taken=$(grep '^browser: command little via sidebar ' "$log" | tail -1 | cut -d' ' -f6)
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘-click on a Today tab's row didn't open a Little ilo window for Space $home"
little=$(opened)
expect "^browser: little window $little took $taken from window $main\$" \
  "the Little ilo window didn't take the tab: $(grep "^browser: little window $little " "$log" | tail -1)"
expect "^browser: window $little shows $taken\$" "the Little ilo window doesn't show the tab's page"
none_since "^browser: loaded $taken " || fail "the tab's page loaded again instead of moving live"
none_since "^browser: splits [0-9]" || fail "⌥⌘-click on a Today tab's row opened a split view too"

# ⌘W closes it into the archive, as Arc's Little Arc does, and ⇧⌘T in the main window reopens it
# in a new Little ilo window for its Space.
# New, so it logs no "key window"; ⌘W closing it shows it's key.
send key w cmd
expect "^browser: window closed $little\$" "the Little ilo window holding the tab didn't come to the front: ⌘W didn't close it"
expect '^browser: archive [1-9][0-9]* tabs$' "closing the Little ilo window didn't archive its tab"
send window 1
send key t cmd shift
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⇧⌘T didn't open a Little ilo window for Space $home"
little=$(opened)
expect "^browser: little window $little reopened [0-9]* as [0-9]*\$" \
  "⇧⌘T didn't reopen the Little ilo window's tab in it: $(grep "^browser: little window $little " "$log" | tail -1)"
send key w cmd
expect "^browser: window closed $little\$" "the reopened Little ilo window didn't come to the front: ⌘W didn't close it"
send window 1

# The Library's Archived Tabs then has a Filter button, whose Little ilo lists only its tabs.
send key l cmd shift
expect '^browser: library shown$' "⇧⌘L didn't open the Library"
post click 372 28
expect "^browser: menu shown $(cased 'All Tabs (checked), Little ilo')\$" \
  "a click on the archive's Filter didn't show its menu"
send choose "$(cased 'Little ilo')"
expect '^browser: archive filter little$' "choosing Little ilo didn't filter the archive"
../tools/window-screenshot.sh "$pid" "$out/little-archive.png" || fail "no window to capture"
send key l cmd shift
expect '^browser: library hidden$' "⇧⌘L didn't close the Library"

exec 3>&-
quit
kill "$server"
echo "ok: ⌥⌘N opens a Little ilo window for the Space shown, ⌘L loads a URL in it, ⌘O moves its page to the main window and closes it, ⌥⌘-click on a link opens it in one and not beside the page, a tab a Little ilo window's page opens goes to its Space's Today and outlives it, ⌘W closes one, and so does an unused escape, each opens at the size the last closed at, the first 900 × 640, and on the Mac one opens over the main window in full screen, the Dock's menu opens one and hides and shows them all, and the menu bar's button opens one once Settings turns it on. ⌥⌘-click on a Today tab's row moves the tab into one, live. Closing one archives its tab, ⇧⌘T reopens it in a new one, and the Library's Filter lists Little ilo's alone. Windows: $out/little.png $out/little-moved.png $out/little-link.png $out/little-archive.png, and on the Mac $out/little-full-screen.png $out/little-dock.png"
