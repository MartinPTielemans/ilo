# check.sh runs this at 240.
# Little ilo, Arc's Little Arc. ⌥⌘N opens a Little ilo window for the Space the window shows, under
# its bar, and ⌘L there opens the command bar inside it, whose URL loads in it. The bar's chevron
# opens its Spaces menu under it. ⌘O moves its page, live, to that Space in the main window and
# closes it. ⌥⌘-click on a link opens the link in one, not
# beside the page as ⌥-click does, and ⌘W closes that window, its page with it. An escape the page
# doesn't use closes one. A tab its page opens goes behind, to the top of its Space's Today, and
# outlives it, but for a pop-up its page asks for with window.open(), as a sign-in's: that opens
# as a Little ilo window of its own over it at the size it asked, whose field takes typing, and
# which goes once its page closes itself, the window under it taking the keyboard back (#1919).
# Closing one archives its tab, and ⇧⌘T reopens it in a new one, and one left unused
# past its time closes into the archive by itself. The first opens 900 × 640, and each after at the
# size the last one closed at. On the Mac one grows in, from its middle when a key opened it, else
# from under the pointer, and is recorded doing so. It opens over a full-screen window, and the
# Dock's menu opens one after its New Window, and hides and shows them all. One takes its Space's
# look as its theme turns light and dark, captured with a title too long for its bar. A page in one
# goes full screen in it, as a video's button takes it, and its escape leaves full screen without
# closing the window. Closed over
# another app's window, one leaves that window in front and its app active, with a stand-in for the
# app that the run opens itself. Turned off in Settings ›
# Features, ⌥⌘N opens nothing and ⌥⌘-click on a link opens it behind, also when ⌥⌘ and the
# press come as the pointer reaches the link and stay down past the link target's delay, its
# window waiting for the release (#2666).

serve
# The page ⌘O moves, live: a field holding text, and music, silence a second at a time, which the
# first key starts as a page's play needs one. Its title says the field's text, whether the music
# plays and the seconds it has played, so a reload, which would lose both, or a pause shows.
printf '%s' '<title>loading</title>
<textarea id=field style="position:absolute;inset:0;width:100%;height:100%;font:16px system-ui"></textarea>
<script>
const rate = 8000, n = rate * 10, wav = new DataView(new ArrayBuffer(44 + n));
const text = (at, s) => [...s].forEach((c, i) => wav.setUint8(at + i, c.charCodeAt(0)));
text(0, "RIFF"); wav.setUint32(4, 36 + n, true); text(8, "WAVEfmt ");
wav.setUint32(16, 16, true); wav.setUint16(20, 1, true); wav.setUint16(22, 1, true);
wav.setUint32(24, rate, true); wav.setUint32(28, rate, true); wav.setUint16(32, 1, true);
wav.setUint16(34, 8, true); text(36, "data"); wav.setUint32(40, n, true);
for (let i = 0; i < n; i++) wav.setUint8(44 + i, 128);
const music = new Audio(URL.createObjectURL(new Blob([wav], {type: "audio/wav"})));
music.loop = true;
let played = 0, last = 0;
const say = () => document.title =
  `live ${field.value} ${music.paused ? "paused" : "playing"} ${Math.floor(played)}`;
music.ontimeupdate = () => {
  const now = music.currentTime, before = Math.floor(played);
  played += now >= last ? now - last : now;
  last = now;
  Math.floor(played) == before || say();
};
music.onplaying = music.onpause = field.oninput = say;
field.addEventListener("keydown", () => music.play(), {once: true});
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
</script>' > "$out/site/little-live.html"
log="$out/little.log"
step=0
rm -f "$saved" "$launch_frame" "$(dirname "$saved")/little-size" "$(dirname "$saved")/little-in-menu-bar"
# With the motion GitHub's hosted Mac reduces, for the openings recorded below.
motion=
[ "$(uname)" = Darwin ] && motion=full
BROWSER_DRIVE=1 BROWSER_MOTION=$motion BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"
# The app can say which window opened after its page has loaded.
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
main=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
home=$(space)
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: little. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# The Little ilo window that opened last, from its log line.
opened() { grep '^browser: little window opened ' "$log" | tail -1 | cut -d' ' -f5; }
# grew <what> <from>: on the Mac with motion, the window grew in from a 40 × 28 rectangle at <from>
# once its UI drew on screen, its content at full size (Arc). Elsewhere, and under Reduce Motion, it
# didn't.
grew() {
  if [ "$(uname)" != Darwin ] || [ "$(last 'reduce motion')" = on ]; then
    none_since '^browser: little window grows from ' || fail "$1 grew in"
    return 0
  fi
  expect "^browser: little window grows from $2\$" \
    "$1 didn't grow in from $2: $(grep '^browser: little window grows' "$log" | tail -1)"
  expect '^browser: little window grown$' "$1 didn't finish growing in"
}

# ⌥⌘N opens an empty one for the Space shown, grown in from its middle as a key opened it, and
# ⌘L's URL loads in it.
record little-open 2
send key n cmd alt
expect '^browser: command new-little-window via ' "⌥⌘N didn't run New Little ilo Window"
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘N didn't open a Little ilo window for Space $home"
little=$(opened)
expect "^browser: window opened $little\$" "the Little ilo window didn't open"
expect "^browser: window $little frame -\{0,1\}[0-9]* -\{0,1\}[0-9]* 900 640\$" \
  "the first Little ilo window didn't open 900 × 640: $(grep "^browser: window $little frame" "$log")"
# Its middle, from its frame, which a 1x screen can round a point taller.
middle=$(grep "^browser: window $little frame " "$log" | tail -1 | awk '{ print $7 / 2, $8 / 2 }')
grew "the Little ilo window ⌥⌘N opened" "$middle"
recorded
send window 2
# The UI's size, which on Windows is the frame's less its borders, as the bar and the chevron go
# by the UI: 900 × 640 elsewhere.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\).*/\1/')
ui_width=${content%x*}
ui_height=${content#*x}
[ -n "$ui_width" ] && [ -n "$ui_height" ] || fail "the driver didn't give the Little ilo window's size"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
expect '^browser: command bar opens inside the window$' \
  "⌘L didn't open the command bar inside the Little ilo window"
# 500 wide, centred across the window's UI, its top a third of the way down (Arc): at 200 213.3 in a
# 900 × 640 one.
set -- $(grep '^browser: command bar at ' "$log" | tail -1 | cut -d' ' -f5-)
bar_left=$(((ui_width - 500) / 2))
bar_top=$((ui_height / 3))
[ "$1" = "$bar_left" ] && [ "${2%%.*}" = "$bar_top" ] && [ "$3" = 500 ] ||
  fail "⌘L's command bar in the Little ilo window is at $*, not at $bar_left $bar_top and 500 wide in its ${content} UI"
../tools/window-screenshot.sh "$pid" "$out/little-bar.png" all || fail "no window to capture"
send insert "${site}little-live.html"
send key return
expect "^browser: loaded [0-9]* ${site}little-live.html\$" \
  "the command bar's URL didn't load in the Little ilo window"
moved=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}little-live.html\$|\1|p" "$log" | tail -1)
expect "^browser: window $little shows $moved\$" "the Little ilo window didn't show its page"
expect "^browser: title $moved ready\$" "the Little ilo window's page didn't draw"
send click 250 300
send type ada
expect "^browser: title $moved live ada playing [1-9][0-9]*\$" \
  "the Little ilo window's page didn't take the typing and play its music"
../tools/window-screenshot.sh "$pid" "$out/little.png" all || fail "no window to capture"

# The bar's chevron, its plate ending 4 in from the window's right edge, opens the Spaces menu:
# the compact command bar 240 wide, its right edge 8 in from the window's as every overlay's, and
# its top 4 under the button, listing the user's Spaces: at 652 in a 900-wide UI. Escape closes it
# and leaves the page where it is.
send click $((ui_width - 18)) 23
expect "^browser: command bar lists spaces $home\$" "the chevron didn't list the user's Space $home"
expect '^browser: command bar shown$' "the chevron didn't open the Spaces menu"
set -- $(grep '^browser: command bar at ' "$log" | tail -1 | cut -d' ' -f5-)
menu_left=$((ui_width - 248))
[ "$1" = "$menu_left" ] && [ "$2" = 43.5 ] && [ "$3" = 240 ] ||
  fail "the Spaces menu is at $*, not at $menu_left 43.5 and 240 wide in its ${content} UI"
../tools/window-screenshot.sh "$pid" "$out/little-spaces.png" all || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the Spaces menu"
none_since "^browser: little window $little put " || fail "escape in the Spaces menu moved the page"

# ⌘O moves its page to its Space in the main window, which shows it, and closes it. The page goes
# live (Arc): its field keeps its text and its music plays on, with no reload, and it keeps its
# Developer Mode, which the run turns off for it here, as the site is local and starts in it.
bar_command toggle-developer-mode "Turn off Developer Mode for this site\$" "developer mode"
expect "^browser: page $moved developer mode 0\$" \
  "the command bar in the Little ilo window didn't turn Developer Mode off"
before=$(sed -n "s/^browser: title $moved live ada playing \([0-9]*\)\$/\1/p" "$log" | tail -1)
send key o cmd
expect '^browser: command open-in-main-window via ' "⌘O didn't run Open in Main Window"
expect "^browser: little window $little put $moved in window $main space $home\$" \
  "⌘O didn't move the page to the main window"
expect "^browser: window closed $little\$" "⌘O didn't close the Little ilo window"
expect "^browser: window $main shows $moved\$" "the main window doesn't show the page ⌘O moved"
expect "^browser: today $moved $a\$" "the moved page isn't at the top of Today: \"$(today)\""
# Three seconds more played than just before the move, the music playing all along.
for _ in $(seq 100); do
  now=$(sed -n "s/^browser: title $moved live ada playing \([0-9]*\)\$/\1/p" "$log" | tail -1)
  [ "${now:-0}" -ge $((before + 3)) ] && break
  sleep 0.1
done
[ "${now:-0}" -ge $((before + 3)) ] ||
  fail "the moved page's music didn't play on with its text kept: $(grep "^browser: title $moved " "$log" | tail -1)"
none_since "^browser: title $moved live ada paused " || fail "the move paused the page's music"
[ "$(grep -c "^browser: loaded $moved " "$log")" = 1 ] || fail "the move reloaded the page"
# Out of Developer Mode, the main window's URL field says the host alone and the row has no outline.
# Developer Mode is the site's, so the main window's own page on it left it too, and its field
# said the host already.
[ "$(grep '^browser: url field ' "$log" | tail -1)" = "browser: url field 127.0.0.1" ] ||
  fail "the moved page's URL field in the main window isn't the host, as out of Developer Mode: $(grep '^browser: url field ' "$log" | tail -1)"
none_since "^browser: page $moved developer mode 1\$" ||
  fail "the moved page went back into Developer Mode"
none_since "^browser: developer outline $moved at " || fail "the moved page's row has the Developer Mode outline"
send window 1
../tools/window-screenshot.sh "$pid" "$out/little-moved.png" || fail "no window to capture"
# The site's Developer Mode back on, as the steps after expect it.
bar_command toggle-developer-mode "Turn on Developer Mode for this site\$" "developer mode"
expect "^browser: page $moved developer mode 1\$" "the command bar didn't turn Developer Mode on again"
send key w cmd
expect "^browser: tab $moved closed\$" "⌘W didn't close the moved page in the main window"

# ⌥⌘-click on the page's link opens it in a Little ilo window, not beside the page.
send layout
frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
x=${frame%%,*}
y=${frame#*,}
# The pointer rests on the link until its bubble shows, then ⌥⌘ go down and it clicks. On Windows'
# real input, with the keys down before the pointer came onto the link, the page opened nothing
# about half the time (#2666).
rest_on_link() {
  send move $((x + 20)) $((y + 20))
  expect '^browser: link target window shown$' "the pointer resting on the page's link showed no bubble"
}
send click $page_point
rest_on_link
# Grown in from under the pointer, which the driver's clicks leave where it was.
record little-click-open 2
send click $((x + 20)) $((y + 20)) alt cmd
expect "^browser: link ${site}notes.txt in little window [0-9]* from $main profile [0-9]*\$" \
  "⌥⌘-click on a link didn't open it in a Little ilo window"
little=$(opened)
grew "the Little ilo window a ⌥⌘-click opened" "-\{0,1\}[0-9.]* -\{0,1\}[0-9.]*"
recorded
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the link didn't load in the Little ilo window"
none_since "^browser: splits $a+" || fail "⌥⌘-click on a link opened it beside the page too"
# The driver numbers windows in the order they were made, the closed one too.
send window 3
../tools/window-screenshot.sh "$pid" "$out/little-link.png" all || fail "no window to capture"
# ⌘W closes the window, its page with it, and the next opens at the size it had.
send resize 760 520
# The size it's kept at is the window's frame, which on Windows takes in the borders, not the page.
send order
closed_size=$(grep '^browser: frame window 3 ' "$log" | tail -1 | cut -d' ' -f7-)
[ -n "$closed_size" ] || fail "the driver didn't give the Little ilo window's frame"
send key w cmd
expect '^browser: command close-tab via ' "⌘W didn't run in the Little ilo window"
expect "^browser: window closed $little\$" "⌘W didn't close the Little ilo window"
# The main window was behind it, and has the keyboard, as the window manager may hand it on late.
# The driver needs a window that's open, and driving it mustn't bring it forward.
send window 1 behind
for _ in $(seq 20); do
  send order
  case "$(grep '^browser: order ' "$log" | tail -1)" in *", key window 1") break ;; esac
  sleep 0.1
done
case "$(grep '^browser: order ' "$log" | tail -1)" in
  *", key window 1") ;;
  *) fail "closing the Little ilo window didn't give the main window behind it the keyboard: $(grep '^browser: order ' "$log" | tail -1)" ;;
esac
send window 1

# A ⌘-click on its page's link opens the link behind, at the top of its Space's Today, and an
# escape the page doesn't use closes one.
send key n cmd alt
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘N didn't open a second Little ilo window"
little=$(opened)
expect "^browser: window $little frame -\{0,1\}[0-9]* -\{0,1\}[0-9]* $closed_size\$" \
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

# The Dock's menu, the Mac's alone: ilo's New Window and New Incognito Window, File's, in place of
# the engine's, which opened Chrome's windows, then New Little ilo Window, which opens one for the
# Space shown, and while one is open Hide All Little ilo Windows hides it, which Show All Little
# ilo Windows brings back in front.
if [ "$(uname)" = Darwin ]; then
  dock_menu() { grep '^browser: dock menu ' "$log" | tail -1 | cut -d' ' -f4-; }
  # A window ⌘N's command opens may open its command bar, which escape closes.
  dock_window() {
    send dock "$1"
    expect "^browser: command $2 via dock\$" "the Dock's $1 didn't run File's"
    expect "^browser: $3 opened [0-9]" "the Dock's $1 didn't open a window"
    docked=$(sed -n "s/^browser: $3 opened \([0-9]*\).*/\1/p" "$log" | tail -1)
    for _ in $(seq 30); do
      logged '^browser: command bar shown$' && { send key escape; break; }
      sleep 0.1
    done
    send window "$(grep -c '^browser: window opened ' "$log")"
    send key w cmd shift
    expect "^browser: window closed $docked\$" "⇧⌘W didn't close the window the Dock's $1 opened"
    send window 1
  }
  send dock
  expect '^browser: dock menu \(.*, \)\{0,1\}New Window, New Incognito Window, New Little ilo Window\(, .*\)\{0,1\}$' \
    "the Dock menu doesn't have ilo's New Window, New Incognito Window and New Little ilo Window: \"$(dock_menu)\""
  [ "$(dock_menu | grep -o 'New Window' | wc -l | tr -d ' ')" = 1 ] ||
    fail "the Dock menu kept the engine's New Window beside ilo's: \"$(dock_menu)\""
  dock_window "New Window" new-window "window"
  dock_window "New Incognito Window" new-private-window "private window"
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
  # The driver numbers windows in the order they were made, the closed ones too.
  send window "$(grep -c '^browser: window opened ' "$log")"
  ../tools/window-screenshot.sh "$pid" "$out/little-dock.png" all || fail "no window to capture"
  # Brought back from another desktop: the main window in full screen is a desktop of its own,
  # which the Little ilo window left on the first isn't on, and Show All brings it over the full
  # screen, rather than the user back to the desktop it was left on.
  little_window=$(grep -c '^browser: window opened ' "$log")
  # Waits until the desktop shown has the browser windows $1 and not those of $2, by number.
  desktop_shows() {
    for _ in $(seq 50); do
      send desktop
      shown="$(grep '^browser: desktop shown' "$log" | tail -1 | sed 's/^browser: desktop shown//') "
      missing=
      for each in $1; do case "$shown" in *" window $each "*) ;; *) missing=1 ;; esac; done
      for each in $2; do case "$shown" in *" window $each "*) missing=1 ;; esac; done
      [ -z "$missing" ] && return 0
      sleep 0.1
    done
    return 1
  }
  send window 1
  send fullscreen
  expect '^browser: window entered full screen$' "the main window didn't go full screen"
  desktop_shows 1 "$little_window" ||
    fail "the full-screen desktop didn't show the main window alone:$shown"
  send dock Hide All Little ilo Windows
  expect "^browser: dock hid little windows $little\$" "Hide All didn't hide it over full screen"
  send dock Show All Little ilo Windows
  expect "^browser: dock showed little windows $little\$" "Show All didn't show it over full screen"
  desktop_shows "1 $little_window" ||
    fail "Show All didn't bring the Little ilo window over the full-screen window:$shown"
  send window "$little_window"
  ../tools/window-screenshot.sh "$pid" "$out/little-brought-back.png" all || fail "no window to capture"
  send window 1
  send fullscreen
  expect '^browser: window left full screen$' "the main window didn't leave full screen"
  send window "$little_window"
  send key w cmd
  expect "^browser: window closed $little\$" "⌘W didn't close the Dock's Little ilo window"
  send window 1
fi

# The menu bar's button, and on Windows the notification area's icon: none until Settings ›
# Features turns it on, a click on it opens a Little ilo window for the Space shown, and turned off
# it's gone. Windows' driver can't reach Explorer's area or its flyout, so its click is the message
# Explorer sends the icon's window for one.
if [ "$(uname)" = Darwin ] || windows; then
  place="menu bar"
  ! windows || place="notification area"
  send statusitem
  expect '^browser: status item none$' "the menu bar has Little ilo's button before it was turned on"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Features$'; then
    send clickkey $(drawn "toolbar Features")
    expect '^browser: settings tab Features$' "a click on Features didn't show its section"
    settings_drew Features
  fi
  menu_bar_switch=$(drawn "switch little in the menu bar") ||
    fail "Settings didn't draw Show in the $place's switch"
  send clickkey $menu_bar_switch
  expect '^browser: little in the menu bar on$' "a click on the menu bar switch didn't turn it on"
  send statusitem
  expect '^browser: status item shown$' "turning the switch on didn't put the button in the $place"
  if [ "$(uname)" = Darwin ]; then
    send statusitem capture "$out/little-menu-bar.png"
    expect '^browser: status item captured ' "the menu bar's button didn't draw itself to capture"
  elif [ -n "${GITHUB_ACTIONS:-}" ]; then
    # Windows' icon is Explorer's to draw, where no capture of ours can take it, so a check VM's
    # run copies the notification area off its screen, which shows every icon (check-windows.yml),
    # as it shows ilo's for a user who's dragged it out of the overflow. Explorer places it a
    # moment after it's added.
    tries=0
    until send statusitem place
      placed=$(grep '^browser: status item ' "$log" | tail -1)
      [ "${placed#browser: status item at }" != "$placed" ] || [ $tries -ge 50 ]; do
      tries=$((tries + 1))
    done
    # The icon's left, top, right and bottom, then the area's.
    set -- $(echo "${placed#browser: status item at }" | sed 's/[^-0-9 ]//g')
    [ "${placed#browser: status item at }" != "$placed" ] && [ $# = 8 ] ||
      fail "Explorer didn't place the notification area's icon: $placed"
    { [ "$1" -ge "$5" ] && [ "$2" -ge "$6" ] && [ "$3" -le "$7" ] && [ "$4" -le "$8" ]; } ||
      fail "Explorer drew the icon outside the notification area, in its overflow: $placed"
    screen_png "$5" "$6" "$7" "$8" "$out/little-notification-area.png" ||
      fail "PowerShell didn't copy the notification area off the screen"
  fi
  send statusitem click
  expect "^browser: $place opened little window [0-9]*\$" "the $place's button didn't open a Little ilo window"
  little=$(opened)
  expect "^browser: little window opened $little profile [0-9]* home $home\$" \
    "the $place's Little ilo window isn't for Space $home"
  # A new window goes in front of the others, so it logs no "key window"; ⌘W closing it shows it's key.
  send key w cmd
  expect "^browser: window closed $little\$" "the $place's Little ilo window didn't come to the front: ⌘W didn't close it"
  send menu Settings…
  expect '^browser: settings shown Features$' "Settings… didn't show Features again"
  settings_drew Features
  send clickkey $menu_bar_switch
  expect '^browser: little in the menu bar off$' "a click on the menu bar switch didn't turn it off"
  send statusitem
  expect '^browser: status item none$' "turning the switch off left the button in the $place"
  settings_close
  post clickkey $close
  expect '^browser: settings hidden$' "the close button didn't hide Settings"
  send window 1
fi

# The taskbar's jump list, Windows' alone: ilo writes its task, New Little ilo Window, which runs
# the browser with --new-little-window, as its first window opens, and that command opens a Little
# ilo window for the Space shown. The driver can't open Explorer's jump list, so the run reads the task ilo wrote
# and runs its command, as Windows runs it. A launch the browser hands on reaches the core, so that
# part needs an engine built with core/main_delegate.cc as it is here.
if windows; then
  await '^browser: jump list task New Little [^ ]* Window: --new-little-window$' ||
    fail "the jump list didn't get New Little ilo Window: $(grep '^browser: jump list' "$log" | tail -1)"
  if engine_has ../core/main_delegate.cc; then
    send window 1
    ("$browser" --new-little-window < /dev/null) >> "$out/little-launch.log" 2>&1 ||
      fail "the jump list's command didn't reach the browser"
    expect_late '^browser: command line opened little window [0-9]*$' \
      "the jump list's command didn't open a Little ilo window"
    little=$(opened)
    expect "^browser: little window opened $little profile [0-9]* home $home\$" \
      "the jump list's Little ilo window isn't for Space $home"
    send window "$(grep -c '^browser: window opened ' "$log")"
    send key w cmd
    expect "^browser: window closed $little\$" "⌘W didn't close the jump list's Little ilo window"
    send window 1
  else
    echo "skip: the jump list's command opening Little ilo: the engine's core predates the switch (core/main_delegate.cc), until an engine is built with it."
  fi
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
expect "^browser: menu shown $(cased 'All (checked), Little ilo, Tasks')\$" \
  "a click on the archive's Filter didn't show its menu"
send choose "$(cased 'Little ilo')"
expect '^browser: archive filter little$' "choosing Little ilo didn't filter the archive"
../tools/window-screenshot.sh "$pid" "$out/little-archive.png" || fail "no window to capture"
send key l cmd shift
expect '^browser: library hidden$' "⇧⌘L didn't close the Library"

# A Little ilo window takes its Space's look as it changes, light and then dark, and its bar ends a
# title too long for it in an ellipsis: the autofocus page puts what's typed in its title.
send window 1
send key n cmd alt
expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
  "⌥⌘N didn't open a Little ilo window for Space $home"
look=$(opened)
expect "^browser: window opened $look\$" "the Little ilo window didn't open"
# The driver numbers windows in the order they were made, the closed ones too.
look_window=$(grep -c '^browser: window opened ' "$log")
send window "$look_window"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
send insert "${site}autofocus.html"
send key return
expect "^browser: title [0-9]* ready\$" "the autofocus page didn't draw in the Little ilo window"
long="a title as long as this one, which runs on past the end of the bar however wide the window is, ends in an ellipsis rather than in the middle of a word"
# Its text field is at the page's top left, under the bar, and has the keyboard once clicked.
send click 40 70
send insert "$long"
expect "^browser: title [0-9]* value $long\$" "the autofocus page didn't take the typed title"
# Its home's theme: the theme picker's blue swatch, then the sun, and later the moon, each 13
# right of its appearance-x token, past the picker's arrow, as cards.sh clicks the sun.
look_theme() {
  send window 1
  post rightclick 100 112
  expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors")" \
    "the Space's menu didn't offer its theme"
  send choose "$(cased 'Edit Theme Color…')"
  expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
  expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
}
look_theme
send clickkey "$(swatch_x 7)" $swatch_y
expect '^browser: colored space [0-9]* [0-9a-f]*$' "the blue swatch didn't colour the Space"
send clickkey 191.3 49
expect "^browser: little window for $home looks .*appearance=light" \
  "the Little ilo window didn't take its Space's light theme"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
send window "$look_window"
../tools/window-screenshot.sh "$pid" "$out/little-light.png" all || fail "no window to capture"
look_theme
send clickkey 231.7 49
expect "^browser: little window for $home looks .*appearance=dark" \
  "the Little ilo window didn't take its Space's dark theme"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
send window "$look_window"
../tools/window-screenshot.sh "$pid" "$out/little-dark.png" all || fail "no window to capture"

# Closing a Little ilo window by ⌘W or by an unused escape leaves the window behind it in front, as
# closing any app's window does, and when that's another app's, that app active, with ilo's main
# window where it was, behind it, where the main window went in front of it on the Mac (#1689).
# The other app is a stand-in of the run's own over the main window: on the Mac check/cover.m, on
# Windows app-windows.py's cover, over the window's lower half and behind the Little ilo window, as
# the driver never brings a window in front of another app's there, and on Linux ImageMagick's
# display, which the window manager puts in front with the keyboard as it opens. The Little ilo
# window then comes back in front of it, as the user going back to it does. Openbox then gives the keyboard to
# the window that had it last, the stand-in's.
send window 1
send order
main_frame=$(grep '^browser: frame window 1 ' "$log" | tail -1 | cut -d' ' -f5-)
if [ "$(uname)" = Darwin ]; then
  clang -fobjc-arc -framework Cocoa -o "$out/cover" check/cover.m 2> "$out/cover-build.log" ||
    fail "check/cover.m didn't build: $(cat "$out/cover-build.log")"
fi
# The windows the driver last listed over the window driven.
stack() { grep '^browser: stack ' "$log" | tail -1 | cut -c16-; }
# Whether the window in front of window 1, the main window, is the stand-in's, leaving it where it is.
covered() {
  send window 1 behind
  send stack
  case "$(stack)" in "other "*cover*) return 0 ;; esac
  return 1
}
# Waits for the stand-in's window to be over the main window, behind the Little ilo window or in
# front of it, or fails with $1.
await_cover() {
  for _ in $(seq 50); do
    send window 1 behind
    send stack
    case "$(stack)" in *"other cover"*) return 0 ;; esac
    sleep 0.1
  done
  fail "$1: $(stack)"
}
# Waits for the driver to say ilo is the active app ($1 1) or isn't (0), or fails with $2.
await_active() {
  for _ in $(seq 50); do
    send state
    [ "$active" = "$1" ] && return 0
    sleep 0.1
  done
  fail "$2"
}
# Opens the stand-in over the main window.
cover_main() {
  rm -f "$out/cover-front"
  case "$(uname)" in
    Darwin)
      # shellcheck disable=SC2086 # the frame's four numbers
      "$out/cover" "$pid" $main_frame < /dev/null > "$out/cover.log" 2>&1 &
      helper=$!
      ;;
    Linux)
      # shellcheck disable=SC2086 # the frame's four numbers
      set -- $main_frame
      # On Wayland through sway's Xwayland, as another app's X11 window, which sway lists too.
      DISPLAY=${DISPLAY:-${BROWSER_XWAYLAND:-}} \
        display -title cover -geometry "$3x$4+$1+$2" -size "$3x$4" xc:'#406080' \
        > "$out/cover.log" 2>&1 &
      helper=$!
      ;;
    *)
      python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" behind \
        "$out/cover-front" > "$out/cover.log" 2>&1 &
      helper=$!
      ;;
  esac
}
# Ends the stand-in, which gives the main window the keyboard back, and drives the main window.
uncover() {
  case "$(uname)" in
    Darwin | Linux) kill "$helper" 2> /dev/null || true ;;
    *) touch "$out/cover-front" ;;
  esac
  # Waited for no longer than 5 s: the Mac's runner has no timeout to end a run that hangs.
  for _ in $(seq 50); do
    kill -0 "$helper" 2> /dev/null || break
    sleep 0.1
  done
  kill -9 "$helper" 2> /dev/null || true
  wait "$helper" 2> /dev/null || true
  helper=
  await_active 1 "ilo didn't become the active app again once the other app's window went"
  send window 1
}
# Closes a Little ilo window over the stand-in's window by $1, ⌘W or escape, named $2 in its
# captures.
give_way() {
  send key n cmd alt
  expect "^browser: little window opened [0-9]* profile [0-9]* home $home\$" \
    "⌥⌘N didn't open a Little ilo window over the other app's ($1)"
  little=$(opened)
  expect "^browser: window opened $little\$" "the Little ilo window didn't open ($1)"
  number=$(grep -c '^browser: window opened ' "$log")
  send window "$number"
  if [ "$2" = escape ]; then
    send key l cmd
    expect '^browser: command bar shown$' "⌘L didn't open the Little ilo window's command bar ($1)"
    send insert "$site"
    send key return
    expect "^browser: window $little shows [0-9]*\$" "the Little ilo window didn't show its page ($1)"
    page=$(sed -n "s|^browser: window $little shows \([0-9]*\)\$|\1|p" "$log" | tail -1)
    expect "^browser: title $page ready\$" "the Little ilo window's page didn't draw ($1)"
  fi
  # The stand-in opens once the Little ilo window has, which takes the foreground as it opens, and
  # the Little ilo window then comes back in front of it. On Windows the stand-in opens behind it,
  # and driving it would put the app's other windows right behind it, over the stand-in.
  cover_main
  await_cover "the other app's window didn't open in front of the main window ($1)"
  case "$(uname)" in Darwin | Linux) send window "$number" ;; esac
  send window "$number" behind
  send stack
  [ "$(stack)" = driven ] ||
    fail "the Little ilo window isn't in front of the other app's ($1): $(stack)"
  send window 1 behind
  send stack
  case "$(stack)" in
    "window $number, other "*cover*) ;;
    *) fail "the other app's window isn't between the Little ilo window and the main window ($1): $(stack)" ;;
  esac
  send window "$number" behind
  ../tools/window-screenshot.sh "$pid" "$out/little-over-other-app.png" all ||
    fail "no window to capture"
  if [ "$2" = escape ]; then
    send click 450 340
    send key escape
    expect '^browser: little window closed by escape$' "an unused escape didn't close Little ilo over the other app's window"
  else
    send key w cmd
  fi
  expect "^browser: window closed $little\$" "$1 didn't close the Little ilo window over the other app's"
  # Linux's window manager gives the keyboard on by itself.
  [ "$(uname)" = Linux ] || expect '^browser: little window gave way to cover$' \
    "the Little ilo window didn't give way to the other app's window behind it ($1)"
  covered || fail "closing the Little ilo window by $1 put a window of ilo's in front of the other app's: $(stack)"
  await_active 0 "closing the Little ilo window by $1 left ilo the active app, not the other app"
  ../tools/window-screenshot.sh "$pid" "$out/little-gave-way-$2.png" all || fail "no window to capture"
  uncover
}
give_way ⌘W w
give_way escape escape

exec 3>&-
quit

# A Little ilo window left unused closes into the archive by itself, as Arc's Little Arc does after
# 6 hours: here after the run's 4 s, counted from when the main window takes over as the active
# one, and not while it's the active window itself.
log="$out/little-unused.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" --archive-little-after=4 < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
send key n cmd alt
expect "^browser: little window opened [0-9]* profile [0-9]* home [0-9]*\$" \
  "⌥⌘N didn't open a Little ilo window"
unused=$(opened)
expect "^browser: window opened $unused\$" "the Little ilo window didn't open"
send window 2
# With the app in the background no window is the active one, so its clock would start at once.
if [ "$active" = 1 ]; then
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
  send insert "${site}notes.txt"
  send key return
  expect "^browser: loaded [0-9]* ${site}notes.txt\$" \
    "the command bar's URL didn't load in the Little ilo window"
  archived=$(grep -c '^browser: archive [0-9]* tabs$' "$log")
  # Past its time while it's the active window, it stays.
  sleep 5
  grep -q "^browser: window closed $unused\$" "$log" &&
    fail "the Little ilo window closed while it was the active window"
  send window 1
  left=$(date +%s)
  await "^browser: window closed $unused\$" ||
    fail "the Little ilo window, unused for its 4 s, didn't close"
  [ $(($(date +%s) - left)) -ge 3 ] ||
    fail "the Little ilo window closed sooner than 4 s after the main window took over"
  for _ in $(seq 100); do
    [ "$(grep -c '^browser: archive [0-9]* tabs$' "$log")" -gt "$archived" ] && break
    sleep 0.1
  done
  [ "$(grep -c '^browser: archive [0-9]* tabs$' "$log")" -gt "$archived" ] ||
    fail "the Little ilo window that closed unused didn't archive its tab"
else
  echo "skip: the unused Little ilo window. Browser wasn't the active app."
fi
# Little ilo off in Settings › Features (#1636): ⌥⌘N opens nothing, and ⌥⌘-click on a link opens
# it behind, as ⌘-click does. On again after.
send window 1
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Features")
settings_drew Features
click_in_sight "switch little windows"
expect '^browser: little windows off$' "a click on Little ilo's switch didn't turn it off"
../tools/window-screenshot.sh "$pid" "$out/little-off.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send insert "$site"
send key return
expect '^browser: title [0-9]* ready$' "the test page didn't draw again"
off_from=$((step + 1))
send key n cmd alt
send layout
frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
x=${frame%%,*}
y=${frame#*,}
send click $page_point
rest_on_link
send click $((x + 20)) $((y + 20)) alt cmd
expect '^browser: tab [0-9]* opened by [0-9]* in the background$' \
  "with Little ilo off, ⌥⌘-click on a link didn't open it behind"
# ⌥⌘ and the press as the pointer comes onto a link, held until the plate is due, eight times
# (#2666). Its window waits for the release, then shows: on Windows a window showing makes a move
# at the pointer, which with the button down Blink takes as a drag, so an Alt-press on a link was a
# selection and lost its click. Within 400 ms of a plate going the next shows at once, as the
# pointer arrives and before the press, so the page says when 600 ms have passed since the pointer
# left its link, and each press waits for that. Only real input holds the button where the plate
# can ask.
printf '%s' '<title>loading</title>
<body style="margin:0;height:100vh">
<a id=l href="notes.txt" style="position:fixed;left:0;top:0;width:40px;height:40px"></a>
<script>
let entered = 0, rest;
l.addEventListener("mouseenter", () => {
  entered++;
  clearTimeout(rest);
  document.title = "on " + entered;
});
l.addEventListener("mouseleave", () => {
  clearTimeout(rest);
  rest = setTimeout(() => document.title = "rested " + entered, 600);
});
requestAnimationFrame(() => setTimeout(() => document.title = "held ready"));
</script>' > "$out/site/little-held.html"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar for the held press's page"
send insert "${site}little-held.html"
send key return
expect '^browser: title [0-9]* held ready$' "the held press's page didn't draw"
# A click on the page first, as before the hovers above: with Windows' real input, a move onto a
# link in a page loaded from the command bar reached neither the page nor the link target. Real
# input sets the cursor, and two sets in a row make one move, so the page says when the pointer
# reached its link before it leaves.
send click $page_point
send move $((x + 20)) $((y + 20))
await '^browser: title [0-9]* on 1$' ||
  fail "the held press's page didn't say the pointer reached its link"
send move $page_point
await '^browser: title [0-9]* rested 1$' ||
  fail "the held press's page didn't say the pointer had left its link 600 ms before"
for try in 1 2 3 4 5 6 7 8; do
  send press $((x + 20)) $((y + 20)) alt cmd
  # Elsewhere the driver posts the press to the window, which neither moves the pointer onto the
  # link nor holds a button the plate can ask about.
  real=0
  logged '^browser: drive: real press' && real=1
  [ $real = 0 ] || expect '^browser: link target waits for the release$' \
    "press $try: the plate didn't come due while ⌥⌘ and the button were down on the link"
  send release $((x + 20)) $((y + 20)) alt cmd
  expect '^browser: tab [0-9]* opened by [0-9]* in the background$' \
    "press $try: ⌥⌘ and the press as the pointer came onto a link, held past the plate's delay, opened nothing behind"
  [ $real = 0 ] || expect '^browser: link target window shown$' \
    "press $try: the plate didn't show at the release with the pointer still on the link"
  send move $page_point
  await "^browser: title [0-9]* rested $((try + 1))\$" ||
    fail "press $try: the held press's page didn't say the pointer had left its link 600 ms before"
done
! sed -n "/^browser: drive $off_from: /,\$p" "$log" | grep -q '^browser: little window opened ' ||
  fail "with Little ilo off, ⌥⌘N or ⌥⌘-click opened a Little ilo window"
send menu Settings…
expect '^browser: settings shown Features$' "Settings… didn't show Features again"
settings_drew Features
click_in_sight "switch little windows"
expect '^browser: little windows on$' "a click on Little ilo's switch didn't turn it on again"
send key w cmd
exec 3>&-
quit

# The steps below run without the 4 s: a Little ilo window whose pop-up has the keyboard isn't
# the active window, so it would close under its pop-up if the sign-in took longer.
log="$out/little-page.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"

# A page in a Little ilo window goes full screen there, as a video's full-screen button takes it,
# not into the main window: its window takes the page full screen, and the page's escape brings it
# back without closing the window, as Esc closes one only with no full screen to leave (#693).
printf '<title>loading</title><p style="font:16px system-ui">A video</p>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
addEventListener("keydown", e => e.key == "f" && document.documentElement.requestFullscreen());
document.addEventListener("fullscreenchange",
  () => document.title = "fullscreen " + (document.fullscreenElement ? 1 : 0));
</script>' > "$out/site/little-video.html"
# A sign-in pop-up its page opens with window.open() opens as a Little ilo window of its own, over
# it, at the size it asked for, under its bar (#1919): its field takes the typing, and once it
# closes itself, as a sign-in's does when it's done, its window goes and the Little ilo window
# that opened it has the keyboard back. The page asks 480 by 560.
printf '<title>loading</title><p style="font:16px system-ui">Continue with Example ID</p>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
addEventListener("click", () => open("little-signin.html", "signin", "width=480,height=560"));
</script>' > "$out/site/little-opener.html"
printf '<title>loading</title>
<input id="email" autofocus style="position:absolute;inset:0;width:100%%;font:16px system-ui">
<script>requestAnimationFrame(() => setTimeout(() => document.title = "signin ready"));
email.addEventListener("input", () => document.title = "typed " + email.value);
email.addEventListener("keydown", e => e.key == "Enter" && setTimeout(() => close(), 300));
</script>' > "$out/site/little-signin.html"
if [ "$active" = 1 ]; then
  send window 1
  send key n cmd alt
  expect "^browser: little window opened [0-9]* profile [0-9]* home [0-9]*\$" \
    "⌥⌘N didn't open a Little ilo window for the video"
  video_window=$(opened)
  expect "^browser: window opened $video_window\$" "the video's Little ilo window didn't open"
  send window "$(grep -c '^browser: window opened ' "$log")"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
  send insert "${site}little-video.html"
  send key return
  expect '^browser: title [0-9]* ready$' "the video's page didn't draw"
  video=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
  send click $page_point
  send key f
  expect "^browser: title $video fullscreen 1\$" "the page in Little ilo didn't go full screen"
  expect '^browser: page entered full screen$' "the app didn't hear the page went full screen"
  expect '^browser: window entered full screen$' "the Little ilo window didn't take the page full screen"
  ../tools/window-screenshot.sh "$pid" "$out/little-page-fullscreen.png" all || fail "no window to capture"
  grep -q "^browser: little window $video_window put $video " "$log" &&
    fail "the page left its Little ilo window to go full screen"
  send key escape
  expect "^browser: title $video fullscreen 0\$" "escape didn't take the page out of full screen"
  expect '^browser: window left full screen$' "the Little ilo window didn't leave full screen"
  none_since "^browser: window closed $video_window\$" ||
    fail "the escape that left full screen closed the Little ilo window too"
  send key w cmd
  expect "^browser: window closed $video_window\$" "⌘W didn't close the video's Little ilo window"
  # The sign-in pop-up, in a Little ilo window of its own.
  send window 1
  send key n cmd alt
  expect "^browser: little window opened [0-9]* profile [0-9]* home [0-9]*\$" \
    "⌥⌘N didn't open a Little ilo window for the sign-in"
  opener=$(opened)
  expect "^browser: window opened $opener\$" "the sign-in's Little ilo window didn't open"
  send window "$(grep -c '^browser: window opened ' "$log")"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Little ilo window"
  send insert "${site}little-opener.html"
  send key return
  expect '^browser: title [0-9]* ready$' "the sign-in's opener didn't draw"
  send click $page_point
  expect "^browser: tab [0-9]* popup of [0-9]* in window $opener asks 480 560\$" \
    "the opener's window.open() didn't tell the app of a pop-up of 480 by 560"
  popup_tab=$(sed -n 's/^browser: tab \([0-9]*\) popup of .*/\1/p' "$log" | tail -1)
  expect "^browser: little window $opener popup $popup_tab in window [0-9]* at " \
    "the pop-up didn't open in a window of its own: $(grep "^browser: little window $opener " "$log" | tail -1)"
  popped=$(sed -n "s/^browser: little window $opener popup $popup_tab in window \([0-9]*\) at \(.*\)\$/\1 \2/p" "$log" | tail -1)
  set -- $popped
  popup_window=$1
  [ "$4 $5" = "500 616" ] || fail "the pop-up's window is $4 by $5, not its page's 480 by 560 under its bar"
  expect "^browser: title $popup_tab signin ready\$" "the sign-in pop-up didn't draw"
  ../tools/window-screenshot.sh "$pid" "$out/little-popup.png" all || fail "no window to capture"
  send window "$(grep -c '^browser: window opened ' "$log")"
  # The driver's window gives the window the keyboard, which its UI takes on X11 as GPUI does a
  # window's first focus, so a click in the page gives the field its autofocus asked for.
  send click 250 300
  send type ada
  expect "^browser: title $popup_tab typed ada\$" "the pop-up's field didn't take the typing"
  send key return
  expect "^browser: window closed $popup_window\$" "the pop-up's window didn't close with its page"
  expect "^browser: little window $opener has the keyboard back from $popup_window\$" \
    "the Little ilo window didn't get the keyboard back from its pop-up"
  # The pop-up's window can close before the Little ilo window takes the keyboard back, which then
  # is first among the windows already and logs no key window, so its header saying it's the one
  # the user is in again counts too.
  expect "^browser: \(key window $opener\|window $opener main true\)\$" \
    "the Little ilo window isn't the key window again"
else
  echo "skip: a page's full screen in Little ilo and the Little ilo window's sign-in pop-up. Browser wasn't the active app."
fi
exec 3>&-
quit

# The launcher's action, Linux's: the desktop entry's New Little ilo Window runs the browser with
# --new-little-window (#693), which opens a Little ilo window for the Space shown, launching the
# browser if it isn't running, else handed on to the one that is, which needs an engine built with
# core/main_delegate.cc as it is here. The run takes the action's command from the entry itself.
if [ "$(uname)" = Linux ]; then
  entry=platform/linux/flatpak/app.ilo.browser.desktop
  grep -q '^Actions=.*new-little-window;' "$entry" || fail "the desktop entry lists no new-little-window action"
  action=$(sed -n '/^\[Desktop Action new-little-window\]$/,/^\[/p' "$entry")
  echo "$action" | grep -qx 'Name=New Little ilo Window' || fail "the action isn't called New Little ilo Window"
  command=$(echo "$action" | sed -n 's/^Exec=//p')
  [ "${command%% *}" = ilo ] || fail "the action doesn't run ilo: $command"
  # As the launcher runs it, with the check's browser for the Flatpak's ilo.
  arguments=${command#* }
  log="$out/little-action.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" $arguments < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: command line opened little window [0-9]*$' ||
    fail "the launcher's action, launching the browser, didn't open a Little ilo window"
  launched=$(sed -n 's/^browser: command line opened little window //p' "$log" | tail -1)
  await "^browser: little window opened $launched profile [0-9]* home [0-9]*\$" ||
    fail "the action's Little ilo window didn't open"
  if engine_has ../core/main_delegate.cc; then
    "$browser" $arguments >> "$out/little-launch.log" 2>&1 || fail "the action's command didn't reach the browser"
    # The first launch's line is in the log already, so it waits for a second.
    for _ in $(seq 100); do
      [ "$(grep -c '^browser: command line opened little window ' "$log")" = 2 ] && break
      sleep 0.1
    done
    [ "$(grep -c '^browser: command line opened little window ' "$log")" = 2 ] ||
      fail "the launcher's action, with the browser running, didn't open a second Little ilo window"
  else
    echo "skip: the launcher's action handed to the browser running: the engine's core predates the switch (core/main_delegate.cc), until an engine is built with it."
  fi
  exec 3>&-
  quit
fi
echo "ok: ⌥⌘N opens a Little ilo window for the Space shown, ⌘L opens the command bar inside it and loads a URL in it, its chevron opens the Spaces menu under it, ⌘O moves its page to the main window live, its text, its music playing and its Developer Mode kept, and closes it, ⌥⌘-click on a link opens it in one and not beside the page, a tab a Little ilo window's page opens goes to its Space's Today and outlives it, ⌘W closes one, and so does an unused escape, each opens at the size the last closed at, the first 900 × 640, and on the Mac one grows in, from its middle for ⌥⌘N and from the pointer for a click, and opens over the main window in full screen, the Dock's menu opens one and hides and shows them all, Show All bringing one left on another desktop over the main window in full screen, and the menu bar's button opens one once Settings turns it on. ⌥⌘-click on a Today tab's row moves the tab into one, live. Closing one archives its tab, ⇧⌘T reopens it in a new one, and the Library's Filter lists Little ilo's alone. One left unused past its time, which the run makes 4 s, closes into the archive, and one that's the active window doesn't. One takes its Space's look as it changes, light and dark, and its bar ends a long title in an ellipsis. Closed by ⌘W or an unused escape over another app's window, one leaves that window in front and its app active, the main window behind it. A page in one goes full screen there, and its escape brings it back without closing the window. On Linux the desktop entry's New Little ilo Window action opens one, launching the browser. Windows: $out/little-bar.png $out/little.png $out/little-spaces.png $out/little-moved.png $out/little-link.png $out/little-archive.png $out/little-light.png $out/little-dark.png $out/little-over-other-app.png $out/little-gave-way-w.png $out/little-gave-way-escape.png $out/little-page-fullscreen.png, on a Windows check VM $out/little-notification-area.png, and on the Mac $out/little-full-screen.png $out/little-dock.png $out/little-brought-back.png, recorded in $out/little-open.mp4 $out/little-click-open.mp4 A sign-in pop-up a page in one opens comes up as a Little ilo window of its own over it, at the size it asked, and gives the keyboard back as it closes."
