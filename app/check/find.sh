# check.sh runs this at 120.
# ⌘F opens the find bar over the page with the keyboard, and what's typed right after it lands
# there. Typing finds in the page, with "n/m", once Japanese input has committed, and return,
# shift-return, ⌘G and ⇧⌘G and the bar's arrows go through the matches, from the bar and from the
# page. A text the page doesn't have shows NOT FOUND. A click
# on the page leaves the bar open, and the bar keeps its place through a resize and full screen.
# Each tab has its own find, which a new tab starts from the text found last; escape and the bar's
# close button close it, and so does another page. ⌘J from the page and Edit › Find › Jump to
# Selection scroll the page back to the match a find left selected. ⌘0 and ⌘- from the page and the
# View menu's Zoom In zoom the page, and the zoom HUD shows the percent under the find bar for a
# moment, and goes at once when another tab shows. ⌥⌘I from the page docks DevTools in the card,
# filling it, with the page on its left, through a resize, and F12 closes them and gives the page
# the keyboard. View › Developer's JavaScript Console opens them, where ⌘A, ⌘C and ⌘V are the
# console's, a tab without them shows its page across the card, and View Source opens the page's
# source in a tab under it. Help › Troubleshooting's Open Task Manager opens Chrome's, with the
# keyboard, and ⌘W there closes it, not the tab. Its Record a Trace records until the toast's Stop,
# which shows while it records, and the command bar offers to stop it meanwhile; Stop saves
# Chromium's trace where the save panel picks. On Windows that's a Save As of the browser window's,
# which closing the window cancels as it quits.
# Undocked, after a relaunch, ⌥⌘J opens DevTools in a window of Chrome's, where ⌘C copies
# the console's text to the run's own pasteboard and ⌘V pastes it, and ⌥⌘I from the page brings
# them to the front.

# Find in page, and zoom. The test page has three apples and one 日本語, and "far" well below
# them, and puts its zoom and how far it's scrolled in its title as they change.
page='data:text/html;charset=utf-8,<title>loading</title>
<p style="font:24px system-ui">apple banana apple cherry apple 日本語</p>
<p style="font:24px system-ui;margin-top:3000px">far</p>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let unzoomed = devicePixelRatio;
onclick = () => { unzoomed = devicePixelRatio; };
onresize = () => document.title = "zoom " + Math.round(100 * devicePixelRatio / unzoomed);
onscroll = () => document.title = "scrolled " + Math.round(scrollY);
</script>'
log="$out/find.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the find test page didn't draw"
first=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: find in page. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The find bar, a window of GPUI's over the browser window, has the keyboard.
in_find_bar() {
  [ "$key" = GPUIPanel ] && [ "$focus" = GPUIView ] ||
    fail "$1: the keyboard is in $key, on $focus"
}
# Waits for tab $1's find to be $2, as the core reports it: "closed", or "open", the selected
# match, "of", how many there are and the text.
expect_find() {
  expect "^browser: find $1 $2\$" "$3: $(grep "^browser: find $1 " "$log" | tail -1)"
}
# The find bar is 300 points wide and 32 high, 20 from the window's top right corner, one of the
# windows over the page, which in full screen can also be its toast. Sets $content to the window's
# width.
check_find_bar() {
  send layout
  children=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
    sed -n 's/^browser: layout child //p' | tr '\n' ';')
  content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
  case ";$children" in
  *";$((content - 320)),20 300x32;"*) ;;
  *) fail "$1: no find bar among the windows over the page (${children:-none}), $content wide" ;;
  esac
}

# On X11 through sway's Xwayland the find bar can't keep the X focus, which wlroots' window
# manager sends back to the browser window (#1646), so its steps, and the zoom HUD's under it, are
# skipped there, and the tabs they leave are made from the command bar: the second tab at
# example.com, and the first gone to example.org.
if sway_xwayland; then
  echo "skip: the find bar's keys, its matches and the zoom HUD under it: sway's Xwayland" \
    "sends child windows' X focus back (#1646)"
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type example.com
  send key return
  expect '^browser: loaded [0-9]* https://example.com/$' "the new tab didn't load"
  second=$(today | cut -d' ' -f1)
  send key 2 cmd
  expect "^browser: selected $first" "⌘2 didn't show the first tab"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type example.org
  send key return
  expect '^browser: loaded [0-9]* https://example.org/$' "the first tab didn't load example.org"
else
  # What's typed right after ⌘F, with no wait, lands in the bar, though the page has ⌘F first: the
  # app holds the keys that come before the bar has the keyboard, and the bar can have it before the
  # first one comes, when there are none to hold (#905). All of it in the bar, which finds it, is
  # none of it in the page. On the Mac nothing that forks, such as uname, comes between ⌘F and the
  # keys, as the driver starts a command as its line comes.
  post key f cmd
  # The app can handle ⌘F before the driver reads the next line, so its lines are looked for from
  # ⌘F's step.
  shortcut=$step
  send type apple
  expect '^browser: command find via page$' "⌘F with the page focused didn't reach the UI" "$shortcut"
  expect '^browser: find bar shown$' "⌘F didn't open the find bar" "$shortcut"
  expect_find "$first" "open 1 of 3 apple" "typing right after ⌘F didn't find in the page"
  send state
  in_find_bar "after ⌘F"
  send key return
  expect_find "$first" "open 2 of 3 apple" "return didn't go to the next match"
  send key return shift
  expect_find "$first" "open 1 of 3 apple" "shift-return didn't go back a match"
  send key g cmd shift
  expect_find "$first" "open 3 of 3 apple" "⇧⌘G in the find bar didn't go back, round to the last"
  send key g cmd
  expect_find "$first" "open 1 of 3 apple" "⌘G in the find bar didn't go on, round to the first"
  check_find_bar "after ⌘F"
  ../tools/window-screenshot.sh "$pid" "$out/find.png" || fail "no window to capture"

  # The pointer resting on next names it and its key in the tooltip, hung off its glyph as the
  # header's buttons' are, and the click on it ends the tooltip. The bar is 20 in from the page's
  # corner, and the button 2 under its top and 28 tall, so its middle is at 36 and the tooltip's
  # top 7 under that, at 43.
  send move $((content - 320 + 252)) 36
  next_keys=⌘G
  [ "$(uname)" = Darwin ] || next_keys=Ctrl+G
  expect "^browser: tooltip shown Next $next_keys\$" \
    "resting on the find bar's next didn't show Next and $next_keys"
  expect '^browser: tooltip at [0-9.]* 43 ' "the find bar's tooltip didn't hang off its next's glyph"
  expect '^browser: tooltip window shown$' "the find bar's next's tooltip didn't fade in"
  ../tools/window-screenshot.sh "$pid" "$out/find-tooltip.png" || fail "no window to capture"

  # The bar's next and previous go through the matches. Arc's are 28 pt at 212 and 238 in the bar.
  send click $((content - 320 + 252)) 36
  expect '^browser: tooltip hidden$' "a click on the find bar's next didn't end its tooltip"
  expect_find "$first" "open 2 of 3 apple" \
    "a click on the find bar's next didn't go to the next match"
  send click $((content - 320 + 226)) 36
  expect_find "$first" "open 1 of 3 apple" "a click on the find bar's previous didn't go back a match"
  # A text the page doesn't have shows NOT FOUND in place of the count and the arrows.
  send type z
  expect '^browser: find bar drew "applez" NOT FOUND$' \
    "a text the page doesn't have didn't show NOT FOUND"
  ../tools/window-screenshot.sh "$pid" "$out/find-not-found.png" || fail "no window to capture"
  send key backspace
  expect_find "$first" "open [1-3] of 3 apple" "the find didn't find the text again"

  # A click on the page takes the keyboard and leaves the bar open, and ⌘G from the page goes on.
  send click $page_point
  in_browser RenderWidgetHostViewCocoa "after a click on the page under the find bar"
  send key g cmd
  expect '^browser: command find-next via page$' "⌘G with the page focused didn't reach the UI"
  expect_find "$first" "open [1-3] of 3 apple" "⌘G with the page focused didn't find"
  grep -q '^browser: find bar hidden$' "$log" && fail "a click on the page closed the find bar"

  # ⌘F gives the open bar the keyboard. Japanese input composes in it, and it finds once committed.
  send key f cmd
  expect '^browser: find bar drew "apple" ' "⌘F with the bar open didn't keep its text"
  send state
  in_find_bar "after ⌘F with the find bar open"
  send ime on
  send type nihongo
  [ "$marked" = 0,7 ] || fail "the find bar didn't take the composition: marked $marked"
  send key space
  [ "$marked" = 0,3 ] || fail "the find bar didn't take the conversion: marked $marked"
  send key return
  [ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
  send ime off
  expect_find "$first" "open 1 of 1 日本語" "the find bar didn't find the committed text"
  grep -q '^browser: find [0-9]* open [0-9-]* of [0-9-]* n' "$log" &&
    fail "the find bar searched for the composition"

  # A resize and full screen keep the bar by the window's top right corner.
  send resize 1000 700
  check_find_bar "after a resize"
  send fullscreen
  expect '^browser: window entered full screen$' "the window didn't go full screen"
  # Captured first, so a bar out of place shows where it went.
  ../tools/window-screenshot.sh "$pid" "$out/find-fullscreen.png" || fail "no window to capture"
  check_find_bar "in full screen"
  send fullscreen
  expect '^browser: window left full screen$' "the window didn't leave full screen"

  # Each tab has its own find. A new tab's starts from the text found last, and switching tabs shows
  # each one's in the bar, or none.
  send key t cmd
  expect '^browser: command bar shown$' "⌘T in the find bar didn't open the command bar"
  send type example.com
  send key return
  expect '^browser: find bar hidden$' "the find bar stayed over a new tab"
  expect '^browser: loaded [0-9]* https://example.com/$' "the new tab didn't load"
  second=$(today | cut -d' ' -f1)
  send key f cmd
  expect_find "$second" "open 0 of 0 日本語" "a new tab's find didn't start from the text found last"
  send type example
  expect_find "$second" "open 1 of [1-9][0-9]* example" "the new tab's find didn't find"
  send key 2 cmd
  expect "^browser: selected $first" "⌘2 in the find bar didn't show the first tab"
  expect '^browser: find bar drew "日本語" 1/1$' "the first tab didn't show its own find"
  # Escape closes the tab's find and gives the keyboard back to its page.
  send key escape
  expect_find "$first" "closed" "escape didn't close the find"
  expect '^browser: find bar hidden$' "escape didn't hide the find bar"
  send state
  in_browser RenderWidgetHostViewCocoa "after escape closed the find bar"
  send key 1 cmd
  expect '^browser: find bar shown$' "the second tab didn't show its find"
  expect '^browser: find bar drew "example" 1/' "the second tab didn't show its own find"
  send state
  [ "$key" = BrowserCoreNSWindow ] || fail "switching to a tab with its find open moved the keyboard to $key"
  send key 2 cmd
  expect '^browser: find bar hidden$' "the first tab, its find closed, showed the find bar"
  # ⌘F opens it again with the tab's last text, highlighting its match without selecting it.
  send key f cmd
  expect '^browser: find bar drew "日本語" 0/1$' "⌘F didn't open the find with the tab's last text"
  # The bar's close button closes it too, and gives the page the keyboard.
  check_find_bar "before its close button"
  send click $((content - 320 + 284)) 36
  expect_find "$first" "closed" "a click on the find bar's close button didn't close the find"
  expect '^browser: find bar hidden$' "a click on the find bar's close button didn't hide it"
  send state
  in_browser RenderWidgetHostViewCocoa "after a click on the find bar's close button"
  send key f cmd
  expect '^browser: find bar shown$' "⌘F after the close button didn't open the find bar again"

  # ⌘J from the page and Edit › Find › Jump to Selection scroll the page back to the match a find
  # left selected. ⌘↑ is the page's own, to its top, and off the Mac that's Home. It ends there, where
  # zooming doesn't scroll it.
  top="key up cmd"
  [ "$(uname)" = Darwin ] || top="key home"
  for _ in 1 2 3; do send key backspace; done
  send type far
  expect_find "$first" "open 1 of 1 far" "the find didn't find the text below the fold"
  expect '^browser: title [0-9]* scrolled [1-9][0-9]*$' "the find didn't scroll the page to the match"
  send key escape
  expect_find "$first" "closed" "escape didn't close the find"
  for jump in "key j cmd" "menu Jump to Selection"; do
    send $top
    expect '^browser: title [0-9]* scrolled 0$' "$top didn't scroll the page to its top"
    send $jump
    expect '^browser: command jump-to-selection via \(page\|ui\)$' "$jump didn't reach the UI"
    expect '^browser: title [0-9]* scrolled [1-9][0-9]*$' \
      "$jump didn't scroll the page to its selection"
  done
  send $top
  expect '^browser: title [0-9]* scrolled 0$' "$top didn't scroll the page to its top"
  send key f cmd
  expect '^browser: find bar shown$' "⌘F didn't open the find bar again"

  # ⌘0 and ⌘- from the page and the View menu's Zoom In zoom it, for its site, which the profile
  # keeps, so ⌘0 first. The HUD shows the zoom over the card's top right, 8 points under the find
  # bar, its pill 16 inside its window, which leaves room for the shadow. A tab shown in its place
  # takes it at once.
  send click $page_point
  send key 0 cmd
  expect '^browser: command actual-size via page$' "⌘0 with the page focused didn't reach the UI"
  expect "^browser: zoom $first 100%\$" "⌘0 didn't zoom the page to 100 %"
  # The page may have loaded zoomed, by a failed run, so it takes its baseline from a click at 100 %.
  send click $page_point
  send key - cmd
  expect '^browser: command zoom-out via page$' "⌘- with the page focused didn't reach the UI"
  expect "^browser: zoom $first 90%\$" "⌘- didn't zoom the page out to 90 %"
  expect '^browser: title [0-9]* zoom 90$' "⌘- didn't zoom the page out"
  expect '^browser: zoom hud shown$' "⌘- didn't show the zoom HUD"
  send layout
  # The HUD's window reaches 40 past its pill on every side, for the popover level's shadow, so the
  # pill sits 116 in from the content's right and 60 down.
  hud=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout child .* 178x122$' |
    sed 's/^browser: layout child //')
  [ "$hud" = "$((content - 156)),20 178x122" ] ||
    fail "the zoom HUD is at ${hud:-nowhere} in a window $content wide, under the find bar"
  ../tools/window-screenshot.sh "$pid" "$out/zoom.png" || fail "no window to capture"
  send menu "Zoom In"
  expect '^browser: command zoom-in via ui$' "the View menu's Zoom In didn't reach the UI"
  expect "^browser: zoom $first 100%\$" "the View menu's Zoom In didn't zoom the page in to 100 %"
  send menu "Zoom In"
  expect '^browser: title [0-9]* zoom 110$' "the View menu's Zoom In didn't zoom the page in"
  send key 1 cmd
  expect "^browser: selected $second" "⌘1 didn't show the second tab"
  # It shrinks out over 70 ms, as Arc's does on a tab switch, so it's gone well before its 5 s are up,
  # which would take it anyway.
  hud_gone() { sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: zoom hud hidden$'; }
  for _ in $(seq 20); do hud_gone && break; sleep 0.1; done
  hud_gone || fail "the zoom HUD stayed over the tab shown in place of the one zoomed"
  send key 2 cmd
  expect "^browser: selected $first" "⌘2 didn't show the first tab again"
  send key 0 cmd
  expect '^browser: title [0-9]* zoom 100$' "⌘0 didn't zoom the page back to 100 %"
  expect '^browser: zoom hud hidden$' "the zoom HUD didn't go after a moment"

  # Another page closes it.
  send key l cmd
  expect '^browser: command bar shown$' "⌘L in the find bar didn't open the command bar"
  send type example.org
  send key return
  expect_find "$first" "closed" "going to another page didn't close the find"
  expect '^browser: find bar hidden$' "going to another page didn't hide the find bar"
  expect '^browser: loaded [0-9]* https://example.org/$' "the first tab didn't load example.org"
fi

# DevTools dock in the card on the right. They fill the page's frame, and the page keeps the rest
# of it, from the frame's left, through a resize. Each tab has its own. Sets $views to the frames
# of the views shown, DevTools' first, once there are $1 of them. DevTools fill the frame at once,
# and the page moves aside once they've loaded, as in Arc, so with two it waits for that too.
# Windows' driver lists the pages the UI lays out, and DevTools are an aura window in the same
# Chromium window as the page, which Windows doesn't see, so there it doesn't check the views;
# devtools.png shows them. Linux's driver lists docked DevTools too (drive.rs).
sees_devtools=1
if [ "$(uname)" != Darwin ] && [ "$(uname)" != Linux ]; then
  sees_devtools=
  echo "skip: where DevTools dock beside the page: Windows' driver can't see aura windows."
fi
await_views() {
  [ -n "$sees_devtools" ] || return 0
  for _ in $(seq 50); do
    send layout
    layout=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content')
    views=$(echo "$layout" | grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' | cut -d' ' -f2-)
    [ "$(echo "$views" | wc -l)" -eq "$1" ] &&
      [ "$1" -eq 1 -o "$(echo "$views" | sed -n 1p)" != "$(echo "$views" | sed -n 2p)" ] &&
      return 0
  done
  fail "$2: the window shows $(echo "$views" | wc -l | tr -d ' ') page views, not $1"
}
# DevTools fill the card, which is 228 from the left and 10 from the other edges, and the page
# keeps its left part.
check_docked() {
  [ -n "$sees_devtools" ] || return 0
  content=$(echo "$layout" | sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
  width=${content%x*}
  height=${content#*x}
  devtools=$(echo "$views" | sed -n 1p)
  docked=$(echo "$views" | sed -n 2p)
  docked_width=$(echo "$docked" | sed 's/.* \([0-9]*\)x.*/\1/')
  [ "$devtools" = "228,10 $((width - 238))x$((height - 20))" ] ||
    fail "$1: DevTools are at $devtools in a $content window"
  [ "$docked" = "228,10 ${docked_width}x$((height - 20))" ] &&
    [ "$docked_width" -gt 0 ] && [ "$docked_width" -lt $((width - 238)) ] ||
    fail "$1: the page is at $docked, beside DevTools at $devtools"
}
send click $page_point
send key i cmd alt
expect '^browser: command devtools via page$' "⌥⌘I with the page focused didn't reach the UI"
await_views 2 "after ⌥⌘I"
check_docked "after ⌥⌘I"
../tools/window-screenshot.sh "$pid" "$out/devtools.png" || fail "no window to capture"
send resize 1100 700
await_views 2 "after a resize"
check_docked "after a resize"
# F12 closes them, as Dia's and Chrome's does, and the page has the keyboard.
send key f12
expect '^browser: command devtools via page$' "F12 with the page focused didn't reach the UI"
await_views 1 "after F12"
send state
in_browser RenderWidgetHostViewCocoa "after F12 closed DevTools"
send menu "JavaScript Console"
expect '^browser: command javascript-console via ui$' \
  "the View menu's JavaScript Console didn't reach the UI"
await_views 2 "after JavaScript Console"
# In DevTools' console, ⌘A, ⌘C and ⌘V are the console's: ⌘C copies its text to the run's own
# pasteboard, and ⌘V pastes it, which a second copy reads. The console's prompt takes the keyboard
# once DevTools have loaded, so the first copy is tried until it has. On a new profile Chromium's
# guard against pasted scams refuses the first paste until "allow pasting" is typed; where it's
# off already, ↩ runs those words as a script that fails, and the paste reads the same. $1 says
# where DevTools are, docked or undocked.
console_keys() {
  local word=$1-copy copied= pasted=
  for _ in $(seq 50); do
    send key a cmd
    send type "$word"
    send key a cmd
    send key c cmd
    holds "$word" && copied=1 && break
  done
  [ -n "$copied" ] || fail "⌘C in $1 DevTools didn't copy the console's text"
  send key v cmd
  send key a cmd
  send type "allow pasting"
  send key return
  send type pasted-
  send key v cmd
  # The console copies once its renderer has the keys, after the driver's done with them, so a
  # read straight after ⌘C can come before the copy: the second copy is tried until it holds the
  # paste too.
  for _ in $(seq 50); do
    send key a cmd
    send key c cmd
    holds "pasted-$word" && pasted=1 && break
  done
  [ -n "$pasted" ] || {
    ../tools/window-screenshot.sh "$pid" "$out/devtools-$1-paste.png" all || true
    fail "⌘V in $1 DevTools didn't paste in the console, see $out/devtools-$1-paste.png"
  }
}
# The run's pasteboard holds $1, as the driver reads GPUI's. On Wayland a client hears of the
# selection only while it has the keyboard, which DevTools' page has here. So there GPUI's reads
# nothing, and the session's clipboard is read instead (#1575).
holds() {
  if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    clipboard_holds "$1"
  else
    send pasteboard
    logged "^browser: pasteboard text $1\$"
  fi
}
keys=$((step + 1))
console_keys docked
# A command logs "command <name> via <where>"; the command bar's own lines aren't commands.
sed -n "/^browser: drive $keys: /,\$p" "$log" | grep -qE '^browser: command .* via ' &&
  fail "⌘A, ⌘C or ⌘V in docked DevTools ran a command of the app's"
send key 1 cmd
expect "^browser: selected $second" "⌘1 didn't show the second tab"
await_views 1 "on a tab without DevTools"
send key 2 cmd
expect "^browser: selected $first" "⌘2 didn't show the first tab again"
await_views 2 "back on the tab with DevTools"
check_docked "back on the tab with DevTools"
# View Source opens the page's source in a tab under it.
send menu "View Source"
expect '^browser: command view-source via ui$' "the View menu's View Source didn't reach the UI"
expect '^browser: page [0-9]* .* view-source:https://example.org/$' \
  "View Source didn't open the page's source"
source=$(grep '^browser: page [0-9]* .* view-source:' "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: today $second $first $source\$" "the source's tab isn't under the page's"
# Loaded before the Task Manager opens: Chromium makes the window key as a committing page's view
# replaces the focused one, which takes the keyboard back from a Task Manager that opened first.
expect "^browser: loaded $source " "the source's tab didn't load"
# Chrome's Task Manager, from the menu as Arc's, is a window of its own, which ⌘W closes as in
# Chrome, giving the browser window the keyboard back with its tabs. Off the Mac it's Shift+Esc
# from the page, as Chrome's there.
if [ "$(uname)" = Linux ]; then
  send key escape shift
  expect '^browser: command task-manager via page$' "Shift+Esc in the page didn't reach the UI"
else
  send menu "Open Task Manager"
  expect '^browser: command task-manager via ui$' "Open Task Manager didn't reach the UI"
fi
# Off the Mac the window manager gives it the keyboard a moment after it opens.
managed='^browser: key window titled Task Manager'
for _ in $(seq 100); do
  send title
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "$managed" && break
  sleep 0.1
done
# Captured first, so a Task Manager that didn't take the keyboard shows whether it opened.
../tools/window-screenshot.sh "$pid" "$out/task-manager.png" all || fail "no window to capture"
shown=1
if [ "$(uname)" = Linux ] &&
  ! sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "$managed"; then
  # In two of three runs on CI the core opened it and no window of it showed on the screen.
  shown=
  echo "skip: the Task Manager's window and ⌘W in it: it didn't show, likely openbox's" \
    "focus-stealing prevention keeping it under the browser window."
fi
if [ -n "$shown" ]; then
  expect "$managed" "Open Task Manager didn't show Chrome's"
  send key w cmd
  if [ "$(uname)" != Darwin ]; then
    # Off the Mac the Task Manager is Chromium's own window, which takes Ctrl+W itself, as
    # Chrome's, and the window manager gives the keyboard back once it's gone.
    for _ in $(seq 100); do
      send state
      [ "$key" = BrowserCoreNSWindow ] && break
      sleep 0.1
    done
  else
    expect '^browser: command close-tab via ui$' "⌘W in the Task Manager didn't reach the UI"
    send state
  fi
  in_browser RenderWidgetHostViewCocoa "after ⌘W closed the Task Manager"
  [ "$(today)" = "$second $first $source" ] || fail "⌘W in the Task Manager closed a tab"
fi
# Help › Troubleshooting's Record a Trace records until the toast's Stop, which stays while it
# records, and the command bar then offers to stop it in its place. Stop asks where to save it, and
# the file is Chromium's trace. The toast's Stop is about 38 in from its right edge, 18 from the
# window's, and 41 down.
send menu "Record a Trace"
expect '^browser: command record-trace via ui$' "Record a Trace didn't reach the UI"
expect '^browser: trace recording$' "Record a Trace didn't start a trace"
expect '^browser: toast shown Recording a trace \[stop\]$' "no toast said a trace was recording"
expect '^browser: toast settled$' "the trace's toast didn't settle"
../tools/window-screenshot.sh "$pid" "$out/trace.png" || fail "no window to capture"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type trace
expect '^browser: command bar commands "trace": .*Stop Recording Trace' \
  "the command bar didn't offer to stop the trace"
grep '^browser: command bar commands "trace": ' "$log" | grep -q 'Record a Trace' &&
  fail "the command bar offered to record a trace while one was recording"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send layout
width=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
send move $((width - 18 - 38)) 41
send click $((width - 18 - 38)) 41
expect '^browser: toast button stop$' "Stop wasn't pressed at $((width - 18 - 38)) 41"
expect '^browser: trace stopped$' "Stop didn't stop the trace"
await '^browser: trace recorded [1-9][0-9]* bytes$' || fail "the core didn't give the trace back"
expect '^browser: trace save panel shown$' "Stop didn't ask where to save the trace"
if [ "$(uname)" = Darwin ]; then
  # The app logs the absolute path it saved to.
  trace="$(cd "$out" && pwd)/trace.json"
  rm -f "$trace"
  send savepanel "$trace"
  grep -q "^browser: save panel offers .* Trace\.json in $HOME" "$log" ||
    fail "the save panel didn't offer the trace's name in Downloads or the home folder"
  expect '^browser: save panel answered$' "no save panel showed for the trace"
  expect "^browser: trace saved $trace\$" "the trace wasn't saved where it was picked"
  python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    trace = json.load(f)
assert trace["traceEvents"], "no events"
' "$out/trace.json" || fail "the saved trace isn't Chromium's"
elif windows; then
  # Windows' Save As is the browser window's, which it disables while the dialog shows, as Chrome's
  # is. Without an owner it left the window usable and kept ilo from quitting (#1647).
  for _ in $(seq 50); do
    python3 -I check/app-windows.py list "$(cat "/proc/$pid/winpid")" > "$out/trace-dialog.txt"
    grep -q '#32770 "Save As" visible' "$out/trace-dialog.txt" && break
    sleep 0.1
  done
  grep -q '#32770 "Save As" visible owned' "$out/trace-dialog.txt" ||
    fail "the trace's Save As isn't owned by a window: $(grep -v hidden "$out/trace-dialog.txt" | tr '\n' ';')"
  grep -q ' browser visible disabled' "$out/trace-dialog.txt" ||
    fail "the browser window takes input under the trace's Save As: $(grep -v hidden "$out/trace-dialog.txt" | tr '\n' ';')"
  echo "skip: saving the trace: the driver answers only the host's own Save As, not GPUI's."
else
  echo "skip: saving the trace: the headless session has no file chooser portal for the driver to answer."
fi
none_since '^browser: toast shown Recording a trace' || fail "the trace's toast came back after Stop"
if windows; then
  # The keyboard is the dialog's, so the browser window closes, as the taskbar's Close window
  # closes it, and the dialog goes with it as Cancel would.
  quit
else
  quit_by_key
  await '^browser: command quit via ' || fail "⌘Q didn't quit"
  exited
fi

# Undocked, as DevTools' Dock side menu leaves them for the next time, DevTools are a window of
# Chrome's, where ⌘C and ⌘V are the console's, and ⌥⌘I from the page raises them, as Chrome's and
# Arc's toggle does, rather than closing them. Sets where DevTools dock to $1.
dock() {
  python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    prefs = json.load(f)
prefs.setdefault("devtools", {}).setdefault("preferences", {})["currentDockState"] = sys.argv[2]
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(prefs, f)
' "$prefs" "\"$1\""
}
dock undocked
log="$out/find-undocked.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the find test page didn't draw after a relaunch"
send click $page_point
send key j cmd alt
expect '^browser: command javascript-console via page$' \
  "⌥⌘J with the page focused didn't reach the UI"
# The window takes the keyboard, and its title, a moment after the command.
send title DevTools
expect '^browser: key window titled DevTools' "⌥⌘J didn't open DevTools in a window of their own"
await_views 1 "with DevTools undocked"
console_keys undocked
# A command logs "command <name> via <where>"; the command bar's own lines aren't commands.
grep -E '^browser: command .* via ' "$log" | grep -qv ' javascript-console ' &&
  fail "⌘A, ⌘C or ⌘V in undocked DevTools ran a command of the app's"
../tools/window-screenshot.sh "$pid" "$out/devtools-undocked.png" || fail "no window to capture"
send window 1
send click $page_point
send key i cmd alt
expect '^browser: command devtools via page$' "⌥⌘I with the page focused didn't reach the UI"
send title DevTools
expect '^browser: key window titled DevTools' "⌥⌘I didn't bring undocked DevTools to the front"
send window 1
await_views 1 "after ⌥⌘I with DevTools undocked"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited
dock right
echo "ok: ⌘F opens the find bar from the page with the keyboard, what's typed right after it lands there, typing finds with \"n/m\", return, shift-return, ⌘G and ⇧⌘G and the bar's arrows go through the matches, from the bar and the page, a text the page doesn't have shows NOT FOUND, Japanese input finds once committed, the bar keeps its place through a resize and full screen, each tab keeps its own find, escape and the close button close it and another page closes it. The pointer resting on next names it and its key in a tooltip hung off its glyph, which the click ends. ⌘J and Jump to Selection scroll the page back to the match a find left selected. ⌘0, ⌘- and the View menu zoom the page, and the HUD shows the zoom for a moment, under the find bar, and goes when another tab shows. ⌥⌘I and the View menu dock DevTools in the card beside the page, through a resize and per tab, where ⌘A, ⌘C and ⌘V are the console's, F12 closes them and the page gets the keyboard back, and View Source opens a tab under the page. Open Task Manager shows Chrome's, which ⌘W closes, keeping the tab. Record a Trace records until the toast's Stop, and saves Chromium's trace where the save panel picks. Undocked DevTools open in a window of their own, where ⌘C copies and ⌘V pastes, and ⌥⌘I from the page brings them to the front. Windows: $out/find.png, $out/find-tooltip.png, $out/find-not-found.png, $out/find-fullscreen.png, $out/zoom.png, $out/devtools.png, $out/task-manager.png, $out/trace.png, $out/devtools-undocked.png"
