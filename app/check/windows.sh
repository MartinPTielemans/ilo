# ⌘N opens another window on the same Spaces, showing the key window's Space with no page, whose
# card shows its one line, and a click on the card with ⌘T's bar open only closes the bar. Settings
# opened from it and closed by its button gives it the keyboard back. The new window never shows
# another window's page: a Space it switches to and back shows the first tab no window shows.
# The Spaces menu acts on the window in front. A click on a row whose page another window shows
# moves the page there live, with no second load, and the window it left keeps its rows and draws a
# picture of the page in its card, which a click on takes the page back from, live. A window that
# switches away from the page gives it back to the one with its picture, live, and keeps the
# keyboard. ⇧⌘W closes a window and gives its tabs to the one that was in front before it, live,
# which shows its page again; in the last window it quits. ⇧↩ in the command bar opens its page in
# a new window, as ⌘N makes one, and a Switch to Tab row for that window's page brings the window
# forward rather than taking the page. With the first window in full screen, other apps take the
# second for the window in front once the user is in it. From macOS 14, a page in Safari's engine,
# which the Site Controls turn on for a page that asks for Widevine, moves to another window the
# same way, and the window it left draws WebKit's picture of it, which the core gives from a task of
# its own (#364: an answer inside the call that asked aborted the browser). A heavy page and a light
# one move between two windows every way a page moves, and the run says how long each stage of a
# move takes.

# The page counts in its title from its first frame, so a page that reloaded would count from 1
# again. "o" in it opens a tab under it, as a link to a new tab does. Its beforeunload handler,
# which never asks, has Chrome run the pages' handlers as the last window closes, which once
# archived the page's tab and left the window open (#870).
page='data:text/html,<title>loading</title>
<script>
onbeforeunload = () => {};
let count = 0;
requestAnimationFrame(() => setTimeout(() =>
  setInterval(() => document.title = "count " + ++count, 200)));
onkeydown = e => { if (e.key == "o") window.open(); };
</script>'
# The drm page, which Safari's engine shows below. The run turns 127.0.0.1 on for it, which the
# profile keeps, so it forgets it before its launch and after its quit.
serve
forget_engine_sites
log="$out/windows.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* count 1$' || fail "the counting page didn't draw"
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
a=$(today)
# The page's count, last logged.
count() {
  grep "^browser: title $a count " "$log" | tail -1 | cut -d' ' -f5
}
# Waits for the page to count three more, as a page that's live and not loaded again does.
counting() {
  from=$(count)
  await "^browser: title $a count $((from + 3))\$" || fail "$1: the page stopped counting at $from"
  [ "$(grep -c "^browser: loaded $a " "$log")" = 1 ] || fail "$1: the page loaded again"
}
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: windows. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
send type o
expect "^browser: today $a [0-9][0-9]*\$" "o in the page didn't open a tab under it"
b=$(today | cut -d' ' -f2)
expect "^browser: window $first shows $b\$" "the tab the page opened didn't show"
send click $(row 0)
expect "^browser: window $first shows $a\$" "a click on the counting page's row didn't show it"

empties=$(grep -c '^browser: empty card shown$' "$log" || true)
send key n cmd
expect '^browser: command new-window via ' "⌘N didn't run New Window"
expect '^browser: window opened [0-9]' "⌘N didn't open a window"
second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
[ "$second" != "$first" ] || fail "⌘N's window has the first one's id"
expect "^browser: window $second shows 0\$" "the new window showed a page"
none_since "^browser: window $first shows" || fail "⌘N changed what the first window shows"
shown_first=$(grep -c "^browser: window $first shows" "$log")
send window 2
../tools/window-screenshot.sh "$pid" "$out/windows-new.png" all || fail "no window to capture"
# The new window's card shows its mark and line, and its URL field its placeholder, once the fade
# has ended, as the shell says in the log. The first window shows a page, so the line is the new
# window's.
for _ in $(seq 100); do
  [ "$(grep -c '^browser: empty card shown$' "$log")" -gt "$empties" ] && break
  sleep 0.1
done
[ "$(grep -c '^browser: empty card shown$' "$log")" -gt "$empties" ] ||
  fail "the new window's empty card didn't show its line"
../tools/window-screenshot.sh "$pid" "$out/empty-window.png" all || fail "no window to capture"
# A click on the card with ⌘T's bar open only closes it: the host closes the bar at the press, and
# the release mustn't open it again. Another driver step after the bar's report is in gives the
# release time to have run.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the new window"
expect '^browser: new tab plate 1$' "⌘T didn't give New Tab the selected plate"
opened=$(grep -c '^browser: command new-tab via card$' "$log" || true)
# Below the bar, which reaches y 410 and covers $page_point.
send click 600 600
expect '^browser: command bar hidden$' "a click on the card didn't close the command bar"
expect '^browser: new tab plate 0$' "a click on the card left New Tab's plate"
send window 2
[ "$(grep -c '^browser: command new-tab via card$' "$log" || true)" = "$opened" ] ||
  fail "the click that closed the command bar opened it again"

# Settings opened from the second window and closed by its close button gives the keyboard back to
# that window, which AppKit makes key from inside the close (#819: the shell heard it there, inside
# GPUI's update, and the app aborted).
send key , cmd
expect '^browser: settings shown ' "⌘, in the second window didn't show Settings"
settings_drew
# The driver's done for a click that closes its window can be lost with the window, as in
# favorites.sh, so Settings going is waited for.
settings_close
post clickkey $close
expect '^browser: settings hidden$' "the close button didn't hide Settings"
# The app is still there to answer, which fails with "the app exited" if the close aborted it.
send front
grep '^browser: front ' "$log" | tail -1 | grep -q ' key driven,' ||
  fail "after Settings closed, the second window isn't key: $(grep '^browser: front ' "$log" | tail -1)"

# A Space the new window makes and leaves: back in the first Space it shows the first tab no other
# window shows, the one the page opened, and the first window keeps the page.
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run in the new window"
expect "^browser: window $second shows 0\$" "the new Space showed a page"
send type reading
send key return
expect '^browser: create space [0-9]* reading$' "return didn't name the new window's Space"
send key 1 ctrl
expect "^browser: window $second shows $b\$" \
  "back in the first Space, the new window didn't show the tab no window shows"
none_since "^browser: window $second shows $a\$" || fail "the new window took the first one's page"
[ "$(grep -c "^browser: window $first shows" "$log")" = "$shown_first" ] ||
  fail "the new window's Space switches changed what the first window shows"
counting "with two windows"

# With the first window in full screen, and the command bar's shadow left over it, the second is
# the one another app takes for the window in front once the user is in it, as T3 Code's window
# capture does: the first at the normal level, front to back.
send window 1
send fullscreen
expect '^browser: window entered full screen$' "the first window didn't go full screen"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in full screen"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar in full screen"
# The driven window is the first on screen once its Space has slid in.
in_front() {
  for _ in $(seq 50); do
    send front
    grep '^browser: front ' "$log" | tail -1 | grep -q '^browser: front driven,' && return 0
    sleep 0.1
  done
  return 1
}
send window 2
in_front || true
expect '^browser: front driven, main driven, key driven, ax focused driven, ax main driven$' \
  "with the first window in full screen, other apps take the wrong window: $(grep '^browser: front ' "$log" | tail -1)"
# Bringing the first window forward slides to its Space. Left from the desktop before that slide
# ends, full screen ends at once and macOS slides none, or there and back after AppKit says it has
# left, so a click can come in the middle (#889). Left from its own Space, it slides back once, and
# until then the first window still covers the second and clicks go nowhere. That Space's first
# window is full screen's toolbar, so only the slide's end says it's in. Elsewhere full screen is no
# Space of its own.
send window 1
if [ "$(uname)" = Darwin ]; then
  expect '^browser: desktop space changed$' "the first window's full-screen Space didn't come forward"
fi
send fullscreen
left=$step
expect '^browser: window left full screen$' "the first window didn't leave full screen"
if [ "$(uname)" = Darwin ]; then
  expect '^browser: desktop space changed$' \
    "the desktop's Space didn't come back after full screen" "$left"
fi
send window 2
in_front ||
  fail "after full screen, the second window didn't come forward: $(grep '^browser: front ' "$log" | tail -1)"

# A click on the page's row in the new window moves the page there, live.
send click $(row 0)
expect "^browser: window $second shows $a\$" \
  "a click on the page's row didn't show it in the new window"
counting "in the new window"
[ "$(today)" = "$a $b" ] || fail "the move changed Today to \"$(today)\""
../tools/window-screenshot.sh "$pid" "$out/windows-moved.png" all || fail "no window to capture"

# The first window, which the page left, asks the core for a picture of it once the move is over,
# and draws it in its card once it has stepped back.
expect "^browser: window $first page elsewhere $a\$" "the first window wasn't told its page went"
expect "^browser: window $first pictures tab $a\$" "the first window got no picture of its page"
expect "^browser: window $first picture rests\$" "the first window's picture didn't step back"
# Brought forward, it takes a new picture, and still leaves the page where it is.
send window 1
expect "^browser: window $first pictures tab $a\$" "the first window took no new picture as it came forward"
../tools/window-screenshot.sh "$pid" "$out/windows-pictured.png" || fail "no window to capture"

# A click on the picture takes the page back, live, from the new window, which draws its picture.
send click $page_point
expect '^browser: command select via picture$' "a click on the picture didn't take the page"
expect "^browser: window $first shows $a\$" "a click on the picture didn't show the page"
counting "back in the first window from its picture"
expect "^browser: window $second pictures tab $a\$" "the new window got no picture of the page"
expect "^browser: window $second picture rests\$" "the new window's picture didn't step back"
send window 2
expect "^browser: window $second pictures tab $a\$" "the new window took no new picture as it came forward"
../tools/window-screenshot.sh "$pid" "$out/windows-pictured-back.png" || fail "no window to capture"

# The row of the tab the new window has selected takes the page again, as before.
send click $(row 0)
expect "^browser: window $second shows $a\$" "a click on the row didn't take the page from its picture"
counting "in the new window again"

# The new window switching to the other tab lets go of the page, which goes back to the first,
# live, in place of its picture. The new window keeps the keyboard where the click left it.
keyed=$(grep -c "^browser: key window $first\$" "$log" || true)
send click $(row 1)
expect "^browser: window $second shows $b\$" "a click on the other tab's row didn't show it"
expect "^browser: window $first shows $a\$" \
  "the page the new window left didn't go to the first, which had its picture"
expect "^browser: window $first page elsewhere 0\$" \
  "the first window wasn't told it shows its page again"
counting "back in the first window after the new one left it"
send front
front=$(grep '^browser: front ' "$log" | tail -1)
echo "$front" | grep -q ' key driven,' || fail "the page going back took the keyboard: $front"
# The row's click gave the new window's page the keyboard, in a task of its own.
[ "$focus" = RenderWidgetHostViewCocoa ] ||
  fail "the page going back moved the new window's keyboard from its page to $focus"
[ "$(grep -c "^browser: key window $first\$" "$log")" = "$keyed" ] ||
  fail "the page going back made the first window key"
../tools/window-screenshot.sh "$pid" "$out/windows-handed-back.png" all || fail "no window to capture"

# The new window takes the page once more, for ⇧⌘W to close it while it shows the page.
send click $(row 0)
expect "^browser: window $second shows $a\$" "a click on the row didn't take the page back"
expect "^browser: window $first page elsewhere $a\$" \
  "the first window wasn't told its page went again"
counting "in the new window once more"

# ⇧⌘W closes the new window. The first keeps every row and shows its page again, live.
send key w cmd shift
expect '^browser: command close-window via ' "⇧⌘W didn't run Close Window"
expect "^browser: window closed $second\$" "⇧⌘W didn't close the window"
expect "^browser: window $first shows $a\$" "the first window didn't get its page back"
expect "^browser: window $first page elsewhere 0\$" \
  "the first window wasn't told it shows its page again after the other closed"
counting "back in the first window"
grep -q '^browser: tab [0-9]* closed$' "$log" && fail "closing a window closed a tab"
[ "$(today)" = "$a $b" ] || fail "closing the window changed Today to \"$(today)\""
send window 1
check_layout "back in the first window"
../tools/window-screenshot.sh "$pid" "$out/windows-closed.png" || fail "no window to capture"

# ⇧↩ in the command bar opens example.com in a new window, and the first keeps its page.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type example.com
send key return shift
# The bar sends what it suggested first, http:// while the https:// probe is out; the page upgrades.
expect '^browser: command bar window https*://example.com/' "⇧↩ didn't open example.com in a window"
expect '^browser: window opened [0-9]' "⇧↩ didn't open a window"
third=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: window $third shows [1-9][0-9]*\$" "⇧↩'s window didn't show a tab"
c=$(grep "^browser: window $third shows [1-9]" "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: loaded $c https://example.com/\$" "example.com didn't load in ⇧↩'s window"
none_since "^browser: window $first shows" || fail "⇧↩ changed what the first window shows"
../tools/window-screenshot.sh "$pid" "$out/windows-shift-return.png" all ||
  fail "no window to capture"

# From the first window, the Switch to Tab row for that page brings its window forward, and the
# first window keeps its own page. "example" is the start of the page's title and its URL.
send window 1
send key t cmd
expect '^browser: command bar shown$' "⌘T in the first window didn't open the command bar"
send type example
expect '^browser: command bar completes "example" with "\.' \
  "the command bar didn't complete \"example\""
send key backspace
suggested="^browser: command bar suggestions \"example\" in .* ms: .*https://example.com/ tab $c"
expect "$suggested" "the command bar didn't suggest the tab ⇧↩'s window shows"
for _ in 1 2 3 4 5 6 7 8; do
  send key down
  grep '^browser: command bar selected' "$log" | tail -1 | grep -q ' https://example.com/$' && break
done
send key return
expect "^browser: command bar switch to tab $c in window $third\$" \
  "↩ on the tab's row didn't go to the window that shows it"
expect "^browser: key window $third\$" "Switch to Tab didn't make the window showing the tab key"
none_since "^browser: window $first shows" ||
  fail "Switch to Tab changed what the first window shows"
counting "after Switch to Tab"
../tools/window-screenshot.sh "$pid" "$out/windows-switch.png" all || fail "no window to capture"

# ⇧↩'s window closes, so the next run doesn't bring back two.
send window 3
send key w cmd shift
expect "^browser: window closed $third\$" "⇧⌘W didn't close ⇧↩'s window"
send window 1

# A page in Safari's engine moves to a new window as Chromium's does, and the window it left draws
# WebKit's picture of it. The drm page asks for Widevine, so its Site Controls offer Safari's
# engine, whose row is their first, at y 262 (clear.sh), and On shows the page there.
webkit_moved=
if [ "$(sw_vers -productVersion | cut -d. -f1)" -ge 14 ]; then
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for the drm page"
  send type "127.0.0.1:$port/drm.html"
  send key return
  expect "^browser: today [0-9][0-9]* $before\$" "the drm page's tab didn't open at the top of Today"
  d=$(today | cut -d' ' -f1)
  expect "^browser: page $d needs com.widevine.alpha\$" "the drm page didn't ask for Widevine"
  bar_command site-controls "Site Controls" "site controls"
  expect "^browser: site controls http://127.0.0.1:$port: .*; Safari's engine off\$" \
    "the Site Controls didn't offer Safari's engine for the drm page: $(grep '^browser: site controls http' "$log" | tail -1)"
  expect '^browser: site controls settled$' "the Site Controls didn't open at the drm page"
  post click 207 262
  expect "^browser: menu shown On, Off (checked)\$" "Safari's engine's row didn't offer On"
  send choose On
  expect "^browser: site controls set Safari's engine On\$" "On in the menu didn't run"
  expect "^browser: system engine $d on\$" "On didn't show the drm page in Safari's engine"
  expect "^browser: loaded $d http://127.0.0.1:$port/drm.html\$" "Safari's engine didn't load the drm page"
  expect_late "^browser: title $d drm asked\$" "Safari's engine didn't answer the drm page's request"
  # Safari's engine's page, taking the pane, may have closed them already.
  if none_since '^browser: site controls hidden$'; then
    send key escape
    expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  fi
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open a window for Safari's engine's page"
  fourth=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  expect "^browser: window $fourth shows 0\$" "the window for Safari's engine's page showed a page"
  send window 4
  send click $(row 0)
  expect "^browser: window $fourth shows $d\$" \
    "a click on Safari's engine's page's row didn't show it in the new window"
  expect "^browser: window $first page elsewhere $d\$" \
    "the first window wasn't told Safari's engine's page went"
  expect "^browser: tab $d pictured [0-9]*: [1-9][0-9]*x[1-9][0-9]*\$" \
    "Safari's engine gave no picture of its page: $(grep "^browser: tab $d pictured" "$log" | tail -1)"
  expect "^browser: window $first pictures tab $d\$" \
    "the first window got no picture of Safari's engine's page"
  expect "^browser: window $first picture rests\$" \
    "the first window's picture of Safari's engine's page didn't step back"
  ! grep -q 'panicked' "$log" || fail "the move of Safari's engine's page panicked"
  ../tools/window-screenshot.sh "$pid" "$out/windows-webkit-pictured.png" all ||
    fail "no window to capture"
  # ⇧⌘W gives it back to the first window, live in Safari's engine.
  send key w cmd shift
  expect "^browser: window closed $fourth\$" "⇧⌘W didn't close the window with Safari's engine's page"
  expect "^browser: window $first shows $d\$" "the first window didn't get Safari's engine's page back"
  expect "^browser: window $first page elsewhere 0\$" \
    "the first window wasn't told it shows Safari's engine's page again"
  send window 1
  webkit_moved=", a page in Safari's engine, turned on in the Site Controls, moves to another window and back, and the window it left draws WebKit's picture of it"
fi

# A heavy page shaped like a video grid and a light one move between two windows, twice with the
# second window smaller than the first and twice at the first's size, each time four ways: a click
# on its row in the window without it, when no window has a picture of it; a click on the picture
# of it; a click on its row in the window with its picture; and the window showing it switching to
# another tab, which gives it back. The core and the app log each stage of a move as "browser:
# handover <stage> +<ms> ms (<ids>)", in ms since the click. Each move's go in handover.txt, and
# the run ends with each stage's median for each way, in handover-summary.txt.
serve
# Opens $1 of the check's site from the first window's command bar, and sets $tab to its tab.
open_site() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
  send insert "$site$1"
  send key return
  expect "^browser: loaded [0-9]* $site$1\$" "$1 didn't load"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) $site$1\$|\1|p" "$log" | tail -1)
  expect "^browser: window $first shows $tab\$" "the first window didn't show $1"
}
open_site grid.html
heavy=$tab
expect "^browser: title $heavy grid\$" "the heavy page didn't draw"
open_site welcome.html
light=$tab
# The point of the Today row of tab $1, in either window, as both list the same.
row_of() {
  index=0
  for each in $(today); do
    [ "$each" = "$1" ] && row $index && return
    index=$((index + 1))
  done
}
# Marks where the next move's lines start in the log.
mark() {
  marked_at=$(wc -l < "$log")
}
# Writes the handover lines since the mark to handover.txt as "<$1>|<stage>|<ms>", the first of
# each stage, leaving out those of a tab other than $2, such as the one the window switched to.
record_handover() {
  tail -n +$((marked_at + 1)) "$log" |
    sed -n 's/^browser: handover \(.*\) +\([0-9.]*\) ms (\(.*\))$/\1|\2|\3/p' |
    awk -F'|' -v way="$1" -v tab="$2" '
      $3 ~ /tab [0-9]+/ && $3 !~ ("tab " tab "([,)]|$)") { next }
      !seen[$1]++ { print way "|" $1 "|" $2 }' >> "$out/handover.txt"
}
# The core's mark of the page's first frame in the window that took it. A page laid out at another
# size can draw without Chromium ever answering the core's request for that mark, so a missing one
# goes unmeasured and is noted, rather than failing the move: the window has shown the page.
drew() {
  for _ in $(seq 30); do
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "^browser: handover taker's page drew a frame .*tab $1," &&
      return 0
    sleep 0.1
  done
  echo "note: $2: no first-frame mark"
}
# Moves tab $1 four ways and back, labelled $2.
moves() {
  # The second window takes it by its row. No window has a picture of it yet.
  send window $driven_fourth
  mark
  send click $(row_of $1)
  expect "^browser: window $fourth shows $1\$" "$2: a click on its row in the second window didn't take it"
  drew "$1" "$2: the page didn't draw in the second window"
  expect "^browser: window $first picture rests\$" "$2: the first window's picture of the page didn't come"
  expect "^browser: window $first pictures tab $1\$" "$2: the first window's fresh picture didn't come"
  record_handover "$2, row, no picture" "$1"
  # The first window takes it back by a click on its picture, once coming forward has taken it again.
  send window 1
  expect "^browser: window $first pictures tab $1\$" "$2: the first window took no new picture as it came forward"
  mark
  send click $page_point
  expect "^browser: window $first shows $1\$" "$2: a click on the picture didn't take the page back"
  drew "$1" "$2: the page didn't draw back in the first window"
  expect "^browser: window $fourth picture rests\$" "$2: the second window's picture of the page didn't come"
  expect "^browser: window $fourth pictures tab $1\$" "$2: the second window's fresh picture didn't come"
  record_handover "$2, picture" "$1"
  # The second window takes it again by its row, from the window it just gave its picture to.
  send window $driven_fourth
  expect "^browser: window $fourth pictures tab $1\$" "$2: the second window took no new picture as it came forward"
  mark
  send click $(row_of $1)
  expect "^browser: window $fourth shows $1\$" "$2: a click on its row with its picture didn't take it"
  drew "$1" "$2: the page didn't draw in the second window again"
  expect "^browser: window $first picture rests\$" "$2: the first window's picture of the page didn't come again"
  expect "^browser: window $first pictures tab $1\$" "$2: the first window's fresh picture didn't come"
  record_handover "$2, row" "$1"
  # The second window shows another tab, which gives the page back to the first.
  mark
  send click $(row_of $b)
  expect "^browser: window $fourth shows $b\$" "$2: a click on another row didn't show its tab"
  expect "^browser: window $first shows $1\$" "$2: the page didn't go back to the first window"
  drew "$1" "$2: the page didn't draw back in the first window"
  record_handover "$2, let go" "$1"
}
send layout
first_size=$(grep '^browser: layout content ' "$log" | tail -1 | sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\).*/\1 \2/')
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a window for the moves"
fourth=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
# The driver counts windows in the order they opened, closed ones too.
driven_fourth=$(grep -c '^browser: window opened ' "$log")
rm -f "$out/handover.txt"
send window $driven_fourth
send resize 1000 700
for round in 1 2; do
  send window 1
  send click $(row_of $heavy)
  expect "^browser: window $first shows $heavy\$" "the first window didn't show the heavy page"
  moves "$heavy" "heavy, other size"
  send window 1
  send click $(row_of $light)
  expect "^browser: window $first shows $light\$" "the first window didn't show the light page"
  moves "$light" "light, other size"
done
send window $driven_fourth
send resize $first_size
for round in 1 2; do
  send window 1
  send click $(row_of $heavy)
  expect "^browser: window $first shows $heavy\$" "the first window didn't show the heavy page"
  moves "$heavy" "heavy, same size"
  send window 1
  send click $(row_of $light)
  expect "^browser: window $first shows $light\$" "the first window didn't show the light page"
  moves "$light" "light, same size"
done
awk -F'|' '
  { key = $1 "|" $2; if (!(key in n)) order[++keys] = key; v[key, ++n[key]] = $3 }
  END {
    for (k = 1; k <= keys; k++) {
      key = order[k]; m = n[key]
      for (i = 1; i <= m; i++) s[i] = v[key, i]
      for (i = 2; i <= m; i++) for (j = i; j > 1 && s[j - 1] > s[j]; j--) { t = s[j]; s[j] = s[j - 1]; s[j - 1] = t }
      split(key, part, "|")
      printf "%s: %s, median %.1f ms of %d\n", part[1], part[2], s[int((m + 1) / 2)], m
    }
  }' "$out/handover.txt" > "$out/handover-summary.txt"
cat "$out/handover-summary.txt"
send window $driven_fourth
send key w cmd shift
expect "^browser: window closed $fourth\$" "⇧⌘W didn't close the window the pages moved to"
send window 1

# ⇧⌘W in the last window quits. It's the driver's last command: the app doesn't report it done.
post key w cmd shift
exec 3>&-
await "^browser: window closed $first\$" || fail "⇧⌘W didn't close the last window"
exited
forget_engine_sites
echo "ok: ⌘N opens a window on the same Spaces with no page, whose card shows its one line, and a click on it with the command bar open only closes the bar, which never takes another window's page on a Space switch, is the one other apps take for the window in front with the first in full screen, the Spaces menu acts on it, a click on a row moves its page there live and the window it left draws a picture of the page, a click on which takes the page back, a window switching away from the page gives it back to the one with its picture and keeps the keyboard, ⇧⌘W gives the closed window's tabs back to the other, live, and quits from the last, ⇧↩ in the command bar opens a new window, and Switch to Tab brings forward the window showing the tab${webkit_moved:-}. Each stage of a page's move between windows: $out/handover-summary.txt. Windows: $out/windows-new.png, $out/empty-window.png, $out/windows-moved.png, $out/windows-pictured.png, $out/windows-pictured-back.png, $out/windows-handed-back.png, $out/windows-closed.png, $out/windows-shift-return.png, $out/windows-switch.png${webkit_moved:+, $out/windows-webkit-pictured.png}"
