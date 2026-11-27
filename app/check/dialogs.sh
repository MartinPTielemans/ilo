# check.sh runs this at 320.
# A page's own dialogs ask with the browser's dialog over the tab's card, in Chrome's words, each
# captured while it's up: alert(), confirm() and prompt() from the served page's keys. ↩ answers
# OK, with what was typed in a prompt, and Esc Cancel, and the page puts the answer in its title.
# The confirm's Cancel and OK are captured with the pointer on each. An alert goes with its tab
# and comes back with it. In a split view it covers its own pane, and a click or a key to the page
# beside rests it there, the page beside keeping the keys, Japanese input and a resize; a click on
# the resting card brings it live, and one on its OK answers it, with the page beside on another
# site and on the same one (#3231, #3461). Leaving a page whose beforeunload handler asks says "Leave
# site?": Esc stays, and on X11 ⌘L the moment it has closed opens the command bar (#1711), and ↩ leaves. A site's HTTP sign-in asks for a username and password: Esc shows the page
# the site sends without them, and ↩ signs in, with the password in no log. A link to another app
# asks first, and ↩ there opens nothing. The pointer on and off a link 20 times shows and hides the
# link target each time, with the command bar and a tab's menu opened and closed between, and the
# app lives through it with no panic (#1351). On Windows, with another app's window in front of
# ilo's, the link target and the hover card show under it, and the link target over ilo's window
# again once it's gone (#1419). An alert that comes while another app has the foreground goes over
# ilo's window but under the other app's, its taskbar button asking for attention, and has the
# keyboard once ilo's window comes to the front (#1365). Back from another app in front, with the
# Library's search beside the page that has the keyboard, an alert shows at once (#3758). Alt+F4 in a page's dialog cancels it as Esc
# does, and the next question asks again (#1362). On Windows and the Mac, with a second
# window beside the first and the first in front, the pointer resting on a row of the second
# shows its card over the second, and on a row of the first over the first (#1432).
# Closing a pinned tab from its row, by its − or a middle-click, or a Favorite by its tile's Close Tab,
# while its page's alert is up unloads it, and ilo goes on. A page out of sight whose alert waits
# for its tab is let go by the idle check once it looks 2 hours unseen, and ilo goes on (#3549).
# An alert that comes while the user types in the command bar waits, every key going to the bar,
# and shows once the bar closes. A key the page answers only after its alert, once the command bar
# has opened, never reaches the bar (#3791).

serve
log="$out/dialogs.log"
step=0
rm -f "$saved"
BROWSER_LOG=keys BROWSER_DRIVE=1 BROWSER_URL="${site}dialogs.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the dialogs test page didn't draw"
page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: page dialogs. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The page's key $1 asks with its $2 (Alert, Confirm or Prompt), which settles over the card and is
# captured as dialog-$3.png.
ask() {
  send key "$1"
  expect "^browser: page $page dialog $2 .* says\$" "\"$1\" in the page didn't ask with its $2"
  expect '^browser: dialog settled$' "the page's $2 didn't settle"
  ../tools/window-screenshot.sh "$pid" "$out/dialog-$3.png" || fail "no window to capture"
}
# The key $1 in the dialog answers it $2 (accept or cancel), and the page's title becomes $3.
answer() {
  send key "$1"
  expect "^browser: page $page dialog answered $2\$" "$1 didn't answer the page's dialog $2"
  expect '^browser: dialog hidden$' "the page's dialog didn't leave once answered"
  expect "^browser: title $page $3\$" "the page didn't hear \"$3\" from its dialog"
}

ask a Alert alert
answer return accept alerted
ask c Confirm confirm
answer escape cancel "confirm false"
ask c Confirm confirm-again
# Under the pointer Cancel, a text button, brightens, and OK, a filled one, darkens, each captured,
# and the pointer goes back where it was, off them, before ↩ answers.
for button in cancel confirm; do
  expect "^browser: dialog $button at " "the confirm's $button button didn't say where it's drawn"
done
button_at() {
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: dialog $1 at " | tail -1 |
    sed 's/.* at //'
}
cancel_at=$(button_at cancel)
confirm_at=$(button_at confirm)
send move $cancel_at
expect '^browser: dialog hover cancel$' "the pointer on Cancel didn't hover it"
../tools/window-screenshot.sh "$pid" "$out/dialog-cancel-hovered.png" || fail "no window to capture"
send move $confirm_at
expect '^browser: dialog hover confirm$' "the pointer on OK didn't hover it"
../tools/window-screenshot.sh "$pid" "$out/dialog-confirm-hovered.png" || fail "no window to capture"
# Held down, it goes a step darker again (#707), and let go of off it, it doesn't answer.
send press $confirm_at
expect '^browser: dialog pressed confirm$' "pressing OK didn't draw it pressed"
../tools/window-screenshot.sh "$pid" "$out/dialog-confirm-pressed.png" || fail "no window to capture"
send dragto $page_point
expect '^browser: dialog hover none$' "the pointer leaving OK left it hovered"
send release $page_point
expect '^browser: dialog pressed none$' "letting go off OK left it pressed"
answer return accept "confirm true"
# The prompt's field holds the page's default, selected, so typing takes its place.
ask p Prompt prompt
send type grace
answer return accept "prompt grace"
# Asked again once answered, the prompt is drawn again: on X11 a managed one shown again has to
# come out of its wait for its place (#2800).
ask p Prompt prompt-again
send type ada
answer return accept "prompt ada"

# On X11, another app's window raised over ilo's while the page's alert shows: the window manager
# stacks the alert under that window with ilo's, where KWin drew an override-redirect one over every
# window it manages (#2790). Back in ilo's window, the alert has the keys again for ↩. While the
# alert holds the input focus ilo hears each button press on the display, and only counts them
# (#2943); with another app active it hears none.
raised_over=
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ] && ! sway_xwayland; then
  heard() { grep '^browser: x11 hears ' "$log" | tail -1 | cut -d' ' -f4-; }
  ask a Alert alert-raised-over
  for _ in $(seq 50); do [ "$(heard)" = "button presses" ] && break; sleep 0.1; done
  [ "$(heard)" = "button presses" ] ||
    fail "the alert holding the input focus, ilo didn't hear the presses on the display: $(heard)"
  rm -f "$out/other-in"
  mkfifo "$out/other-in"
  : > "$out/other-window.log" # an earlier run's window stays named in it until emptied (#4040)
  python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
  other_app=$!
  exec 7> "$out/other-in"
  for _ in $(seq 50); do grep -q '^[0-9]' "$out/other-window.log" && break; sleep 0.1; done
  cover=$(head -1 "$out/other-window.log")
  xdotool windowactivate --sync "$cover" 2> /dev/null ||
    fail "another app's window didn't open and take the keyboard: $(cat "$out/other-window.log")"
  # The manager restacks as it activates, so its list is read until it settles, a second at most.
  under=
  for _ in $(seq 10); do
    stacking=$(python3 -I check/x11-stacking.py "$cover" "$pid" 2>&1) && under=1 && break
    sleep 0.1
  done
  [ -n "$under" ] || fail "the alert isn't under another app's window raised over ilo's: $stacking"
  for _ in $(seq 50); do [ "$(heard)" = "no button presses" ] && break; sleep 0.1; done
  [ "$(heard)" = "no button presses" ] ||
    fail "with another app active, ilo still hears every button press on the display: $(heard)"
  # The manager's stacking, bottom first, for whoever reads the run.
  printf '%s\n' "$stacking" > "$out/alert-raised-over-stacking.json"
  exec 7>&-
  wait "$other_app" 2> /dev/null || true
  send window 1
  answer return accept alerted
  # A tiling manager, as i3, tiles the other app's window beside ilo's, so the alert is beside it.
  # Elsewhere the other app's window overlaps ilo's.
  case $stacking in
    *'"beside"'*) raised_over="beside the other app's window a tiling manager put beside" ;;
    *) raised_over="under another app's window raised over" ;;
  esac
fi

# A page's dialog waits while the user types in a field of ilo's: the page alerts as the driver
# types in the command bar, every key lands in the bar, and the alert shows once the bar closes.
send click $page_point
rm -f "$out/site/held.go"
send key l
expect "^browser: title $page waiting\$" "\"l\" in the page didn't start waiting for /held"
served "127.0.0.1:$port/held" "the page didn't ask for /held"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type wait
expect '^browser: command bar rows for "wait" in ' "what was typed didn't reach the command bar"
touch "$out/site/held.go"
expect "^browser: page $page dialog Alert .* says\$" "the page didn't alert once /held came"
expect "^browser: page $page dialog [0-9]* waits for the field\$" \
  "the page's alert didn't wait while the command bar had the keyboard"
waited=$step
send type ing
expect '^browser: command bar rows for "waiting" in ' "keys typed as the alert came didn't all reach the command bar"
sed -n "/^browser: drive $waited: /,\$p" "$log" | grep -q '^browser: dialog shown' &&
  fail "the page's alert showed while the command bar had the keyboard"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"
expect '^browser: dialog shown .* says$' "the alert didn't show once the command bar closed"
expect '^browser: dialog settled$' "the alert that waited didn't settle"
answer return accept "alerted later"

# A key the page didn't keep comes back to the app once the page's handler returns, which can be
# after the alert it asked was answered: it never reaches the command bar a shortcut after it
# opens, where on the Mac it was typed (#3791). The page's "h" alerts, then waits for /held, so
# ⌘L after it, which the page gets first, holds the keys typed after it until the page answers it,
# and the page's "h" comes back while they're held.
send click $page_point
rm -f "$out/site/held.go"
send key h
expect "^browser: page $page dialog Alert .* says\$" "\"h\" in the page didn't alert"
expect '^browser: dialog settled$' "the page's alert from \"h\" didn't settle"
send key return
expect "^browser: page $page dialog answered accept\$" "↩ didn't answer the page's alert from \"h\""
expect '^browser: dialog hidden$' "the page's alert from \"h\" didn't leave once answered"
served "127.0.0.1:$port/held" "the page didn't ask for /held once its alert was answered"
send key l cmd
expect "^browser: \[keys\] [a-z]*-l before the page: page, holding the keys after it\$" \
  "⌘L with the page busy didn't go to the page first, holding the keys after it"
touch "$out/site/held.go"
expect "^browser: title $page held back\$" "the page's \"h\" handler didn't end"
expect "^browser: \[keys\] a character the page didn't keep: " \
  "the page's \"h\" didn't come back to the app once its handler returned"
expect '^browser: command bar shown$' "⌘L didn't open the command bar once the page answered it"
send type x
expect '^browser: command bar rows for "x" in ' \
  "the page's \"h\" reached the command bar, or what was typed didn't"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar"

# Another tab takes the alert down, and its own brings it back.
open_from_bar "http://localhost:$port/one.html"
other=$tab
send click $(row 1)
expect "^browser: selected $page\( in .*\)\{0,1\}\$" "a click on the dialogs page's row didn't show it"
send click $page_point
ask a Alert alert-back
send click $(row 0)
expect "^browser: selected $other\( in .*\)\{0,1\}\$" "a click on another tab's row didn't show it"
expect '^browser: dialog withdrawn$' "the alert stayed up over another tab"
send click $(row 1)
expect "^browser: selected $page\( in .*\)\{0,1\}\$" "a click on the dialogs page's row didn't show it"
expect '^browser: dialog shown .* says$' "the alert didn't come back with its tab"
expect '^browser: dialog settled$' "the alert didn't settle again"
send key return
expect "^browser: page $page dialog answered accept\$" "↩ didn't answer the alert that came back"
# Gone first: on X11 Add Split View, from the menu while the answered alert was leaving, ran nothing.
expect '^browser: dialog hidden$' "the alert that came back didn't leave once answered"

# In a split view an alert covers its own pane alone, as Chrome's do, where it covered both (#3231).
# A click on the page beside it lands there, focusing that pane, and the alert rests where it is,
# quiet and without the keyboard (#3461): the typing after reaches that page, Japanese input
# composes there, and a resize keeps the keys there. A click on the resting card's body focuses
# its pane and brings it live, answering nothing. ⌃⇧2 and ⌃⇧1 rest it and bring it back from the
# keys, ⌘T from the page beside opens the command bar and leaves it resting, and a click on the
# resting OK answers it. Then ⌘W from the page beside, the alert asked again and resting, closes
# that page alone. split_rests runs it with the page beside at $1, named $2 in its captures: here
# another site's, in a renderer process of its own, and further on, with $3, the one the dialogs
# page opens itself, which shares its renderer process and so is blocked while the alert asks: the
# page gets none of its keys until then, but ilo's shortcuts work from it (#3770).
printf '%s' '<title>beside</title>
<input id=field style="position:fixed;inset:0;font-size:40px" oninput="document.title = `beside has ${field.value}`">
<script>
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "beside ready"));
</script>' > "$out/site/beside.html"
# The pages' frames in the window, left first, once both are laid out, in $panes.
read_panes() {
  for _ in $(seq 50); do
    send layout
    panes=$(since | grep -m1 '^browser: layout' | grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' |
      sed 's/^WebContentsViewCocoa //')
    [ "$(echo "$panes" | wc -l)" -eq 2 ] && break
    sleep 0.1
  done
  [ "$(echo "$panes" | wc -l)" -eq 2 ] ||
    fail "the layout didn't show two pages beside each other: $(since | grep -m1 '^browser: layout')"
}
# A point 100 in from the top left of pane $1.
in_pane() {
  pane=$(echo "$panes" | sed -n "$1p")
  pane_y=${pane#*,}
  echo "$((${pane%%,*} + 100)) $((${pane_y%% *} + 100))"
}
# Where the dialog last drew OK, once it has drawn it since command $1.
confirm_since() {
  expect '^browser: dialog confirm at ' "the alert didn't say where it draws OK" "$1"
  grep '^browser: dialog confirm at ' "$log" | tail -1 | sed 's/.* at //'
}
# Fails with $1 if the alert went from its pane since command $2.
stayed() {
  since "$2" | grep -q -e '^browser: dialog hidden$' -e '^browser: dialog withdrawn$' && fail "$1"
  true
}
# The window in the tone $1, light or dark, from View › Appearance, which waits while a dialog
# has the keyboard, so it's chosen with the alert at rest, and the alert's card drawn in it.
tone() {
  local light=false
  [ "$1" = light ] && light=true
  if ! grep '^browser: window light ' "$log" | tail -1 | grep -q " $light\$"; then
    send menu "Appearance › $([ "$1" = light ] && echo Light || echo Dark)"
    toned=$step
    expect "^browser: window light $light\$" "View › Appearance didn't make the window $1"
    # The card is a window of its own, which draws the tone a frame or more later.
    expect "^browser: dialog drawn light $light\$" "the alert didn't draw in the window's $1 tone" "$toned"
  fi
}
# The window captured as $out/dialog-$1.png.
capture() {
  ../tools/window-screenshot.sh "$pid" "$out/dialog-$1.png" || fail "no window to capture"
}
# The driver's scripted input method composes in a page on the Mac, and under Wayland as sway's
# input method; on Windows it's Microsoft's Japanese IME, where the runner has it. On X11 the Linux
# driver can't reach a page's (keys.sh says the same).
ime="ime on"
japanese=
if windows; then
  powershell.exe -NoProfile -NonInteractive -Command "(Get-WinUserLanguageList).InputMethodTips" \
    2> /dev/null | grep -qi 03B5835F-F03C-411B-9CE2-AA23E1171E36 && japanese=1 && ime="ime ja"
fi
# The sidebar row of tab $1, before any split.
row_of() {
  n=0
  for id in $(today); do
    [ "$id" = "$1" ] && echo $n && return
    n=$((n + 1))
  done
}
split_rests() {
  shared=${3:-}
  if [ -z "$shared" ]; then
    send menu Add Split View
    expect '^browser: command add-split via ' "Add Split View didn't run ($2)"
    expect '^browser: command bar shown$' "Add Split View didn't open the command bar ($2)"
    send type "$1"
    send key return
  else
    # A page's open() keeps the page it opens in its own browsing instance, so on its site in its
    # renderer process, which two tabs typed in the bar never share. ⌥-click on its row joins it
    # to the page shown.
    # The two windows' steps left this one half the screen wide, where an X press at the page's
    # usual point misses it, so it's near the size the two-site split had, and clicked in its
    # middle. Not that size, or the resize the rest goes through below would change nothing.
    send resize 1180 760
    for _ in $(seq 50); do
      send layout
      alone=$(since | grep -m1 '^browser: layout' | grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' |
        sed 's/^WebContentsViewCocoa //' | head -1)
      [ -n "$alone" ] && break
      sleep 0.1
    done
    [ -n "$alone" ] || fail "the layout didn't show the dialogs page ($2)"
    alone_y=${alone#*,}
    alone_size=${alone##* }
    send click $((${alone%%,*} + ${alone_size%%x*} / 2)) $((${alone_y%% *} + ${alone_size##*x} / 2))
    [ "$focus" = RenderWidgetHostViewCocoa ] ||
      fail "a click in the dialogs page didn't give it the keyboard ($2): $key, $focus"
    send key w
    expect "^browser: tab [0-9]* opened by $page in the foreground\$" "\"w\" in the dialogs page didn't open beside.html ($2)"
    beside=$(grep "^browser: tab [0-9]* opened by $page " "$log" | tail -1 | cut -d' ' -f3)
    expect "^browser: title $beside beside ready\$" "beside.html, opened by the dialogs page, didn't draw ($2)"
    send click $(row "$(row_of "$page")")
    expect "^browser: selected $page\( in .*\)\{0,1\}\$" "a click on the dialogs page's row didn't show it ($2)"
    send click $(row "$(row_of "$beside")") alt
  fi
  expect "^browser: splits .*$page+[0-9][0-9]*" \
    "beside.html didn't open beside the dialogs page ($2): $(grep '^browser: splits' "$log" | tail -1)"
  beside=$(grep '^browser: splits' "$log" | tail -1 | grep -o "$page+[0-9]*" | cut -d+ -f2)
  [ -n "$shared" ] || expect "^browser: title $beside beside ready\$" "beside.html didn't draw in the split ($2)"
  read_panes
  send click $(in_pane 1)
  expect "^browser: selected $page\( in .*\)\{0,1\}\$" \
    "a click on the dialogs page's pane didn't focus it ($2)"
  ask a Alert "alert-split-$2"
  send click $(in_pane 2)
  clicked=$step
  expect "^browser: selected $beside\( in .*\)\{0,1\}\$" \
    "a click on the page beside the alert, in a $2 split, didn't focus its pane: $(since "$clicked" | grep -e '^browser: drive' -e '^browser: selected' -e '^browser: dialog' | tail -6 | tr '\n' ' ')"
  [ "$(uname)" != Darwin ] ||
    expect '^browser: a press in the pane beside takes the keyboard from the dialog$' \
      "the click on the page beside the alert didn't take the keyboard from it ($2)" "$clicked"
  expect '^browser: dialog at rest$' "the alert didn't rest with its pane no longer focused ($2)"
  send key x
  send key y
  [ -n "$shared" ] ||
    expect "^browser: title $beside beside has xy\$" \
      "typing after a click on the page beside the alert, in a $2 split, didn't reach that page: the keyboard is in $key, on $focus: $(since "$clicked" | grep -e '^browser: drive' -e "^browser: title $beside " -e '^browser: dialog' | tail -6 | tr '\n' ' ')"
  # What the user sees of a page waiting on its alert while the page beside has the keys, in each
  # tone, as the review of its look asks.
  for each in light dark; do
    tone $each
    capture "alert-split-$2-rest-$each"
  done
  # Japanese input composes in the page beside, and its commit is that page's.
  typed=xy
  if [ -n "$shared" ]; then
    echo "skip: Japanese input in the page beside the resting alert ($2): that page shares the alert's renderer process, blocked while it asks, so nothing it's typed shows until then."
  elif [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "skip: Japanese input in the page beside the resting alert ($2): on X11 the Linux driver types plain keys in a page; check/ime.sh composes in pages with a real input method."
  elif windows && [ -z "$japanese" ]; then
    echo "skip: Japanese input in the page beside the resting alert ($2): the runner has no Japanese IME, and the Windows driver's own composes in the UI's fields alone."
  else
    send $ime
    send type nihongo
    send key space
    # The page shows the conversion before return commits it, as keys.sh reads it; the z typed
    # after the resize says the commit was the page's and did nothing more.
    expect "^browser: title $beside beside has xy日本語\$" \
      "Japanese input in the page beside the resting alert didn't reach it ($2): marked $marked, the keyboard in $key, on $focus"
    send key return
    send ime off
    typed=xy日本語
  fi
  # Through a resize the keys stay with the page beside, the alert at rest over its pane.
  send resize 1100 720
  resized=$step
  read_panes
  [ -n "$shared" ] || {
    send key z
    expect "^browser: title $beside beside has ${typed}z\$" \
      "after a resize the page beside the resting alert lost the keys ($2): the keyboard is in $key, on $focus"
  }
  stayed "the alert left its pane at rest ($2)" "$clicked"
  # A click on the resting card's body, above OK, focuses its pane and brings it live, unanswered.
  confirm=$(confirm_since "$resized")
  pane=$(echo "$panes" | sed -n 1p)
  pane_w=${pane##* }
  send click $((${pane%%,*} + ${pane_w%%x*} / 2)) $((${confirm#* } - 50))
  body=$step
  expect "^browser: selected $page\( in .*\)\{0,1\}\$" \
    "a click on the resting alert's card didn't focus its pane ($2)"
  expect '^browser: dialog live$' "a click on the resting alert's card didn't bring it live ($2)"
  since "$body" | grep -q "^browser: page $page dialog answered" &&
    fail "a click on the resting alert's card answered it ($2)"
  # From the keys: ⌃⇧2 rests it, the page beside taking the keys, and ⌃⇧1 brings it live.
  send key 2 ctrl shift
  at=$step
  expect "^browser: selected $beside\( in .*\)\{0,1\}\$" "⌃⇧2 from the alert didn't focus the pane beside ($2)"
  expect '^browser: dialog at rest$' "⌃⇧2 from the alert didn't rest it ($2)"
  [ -n "$shared" ] || {
    send key q
    expect "^browser: title $beside beside has ${typed}zq\$" \
      "after ⌃⇧2 from the alert the page beside didn't have the keys ($2): the keyboard is in $key, on $focus"
  }
  # ⌘T from the page beside opens the command bar, and Escape closes it, the alert resting still.
  send key t cmd
  expect '^browser: command bar shown$' \
    "⌘T from the page beside the resting alert didn't open the command bar ($2): $(since | grep -e '^browser: drive' -e '^browser: key' -e '^browser: dialog' | tail -6 | tr '\n' ' ')"
  send key escape
  expect '^browser: command bar hidden$' "Escape didn't close the command bar opened beside the resting alert ($2)"
  none_since '^browser: dialog live$' "$at" ||
    fail "the command bar opened from the page beside brought the resting alert live ($2)"
  stayed "the alert left its pane while the command bar was open beside it ($2)" "$at"
  send key 1 ctrl shift
  expect "^browser: selected $page\( in .*\)\{0,1\}\$" \
    "⌃⇧1 from the page beside the resting alert didn't focus the alert's pane ($2): $(since | grep -e '^browser: drive' -e '^browser: key' -e '^browser: selected' | tail -6 | tr '\n' ' ')"
  expect '^browser: dialog live$' "⌃⇧1 from the page beside didn't bring the alert live ($2)"
  capture "alert-split-$2-live-dark"
  # Live in the light tone, chosen while it rests.
  send key 2 ctrl shift
  at=$step
  expect '^browser: dialog at rest$' "⌃⇧2 from the alert didn't rest it again ($2)"
  tone light
  send key 1 ctrl shift
  expect '^browser: dialog live$' "⌃⇧1 from the page beside didn't bring the alert live again ($2)"
  capture "alert-split-$2-live-light"
  send key 2 ctrl shift
  rested=$step
  expect '^browser: dialog at rest$' "⌃⇧2 from the alert didn't rest it once more ($2)"
  # A click on the resting OK answers it, as a button in a background window does on the Mac.
  send click $(confirm_since "$rested")
  expect "^browser: page $page dialog answered accept\$" "a click on the resting alert's OK didn't answer it ($2)"
  expect '^browser: dialog hidden$' "the alert answered at rest didn't leave ($2)"
  # Sharing the alert's process, the page beside heard nothing while it was asked: Chromium drops
  # the keys of a page whose renderer a dialog stopped, as Chrome's, after ilo's shortcuts (#3770).
  if [ -n "$shared" ] &&
    since "$clicked" | sed "/^browser: page $page dialog answered/q" | grep -q "^browser: title $beside beside has"; then
    fail "the page beside took keys while the alert asked, so it doesn't share the alert's renderer process ($2)"
  fi
  # ⌘W from the page beside, the alert asked again and resting, closes that page alone, leaving the
  # alert's tab shown and its alert there, unanswered, live once its pane is the only one. The
  # click on the resting OK focused the alert's pane, so this click in it gives its page the keys.
  send click $(in_pane 1)
  ask a Alert "alert-split-$2-again"
  send click $(in_pane 2)
  closing=$step
  expect "^browser: selected $beside\( in .*\)\{0,1\}\$" "a click on the page beside didn't focus it ($2)"
  expect '^browser: dialog at rest$' "the alert asked again didn't rest with the page beside focused ($2)"
  send key w cmd
  closed=$step
  expect "^browser: tab $beside closed\$" \
    "⌘W from the page beside the resting alert didn't close that page ($2): $(since | grep -e '^browser: drive' -e '^browser: key' -e '^browser: tab' | tail -6 | tr '\n' ' ')"
  expect "^browser: selected $page\( in .*\)\{0,1\}\$" \
    "closing the page beside it didn't leave the dialogs page shown ($2): $(since | grep -e '^browser: selected' -e '^browser: splits' | tail -3 | tr '\n' ' ')"
  none_since "^browser: tab $page closed\$" "$closing" || fail "⌘W from the page beside closed the alert's tab too ($2)"
  none_since "^browser: page $page dialog answered" "$closing" || fail "⌘W from the page beside answered the alert ($2)"
  stayed "closing the page beside took the alert from its tab ($2)" "$closing"
  expect '^browser: dialog live$' "with the page beside closed, the alert didn't come live in its tab ($2)" "$closed"
  send key return
  expect "^browser: page $page dialog answered accept\$" "↩ didn't answer the alert left in its tab ($2)"
  expect '^browser: dialog hidden$' "the alert left in its tab didn't leave once answered ($2)"
}
split_rests "http://localhost:$port/beside.html" two-site

# ↩ doesn't open the app, so a key the page timed can't.
send click $page_point
send key o
expect "^browser: page $page dialog OpenApp Open .*?\$" "a link to Maps didn't ask to open it"
expect '^browser: dialog settled$' "Open Maps? didn't settle"
../tools/window-screenshot.sh "$pid" "$out/dialog-open-app.png" || fail "no window to capture"
send key return
expect "^browser: page $page dialog answered cancel\$" "↩ didn't cancel opening Maps"
expect '^browser: dialog hidden$' "Open Maps? didn't leave once answered"

send click $page_point
send key u
expect "^browser: title $page armed\$" "the page didn't ask to be asked before it's left"
# Going from the page asks first, with Chrome's title.
leave() {
  go_from_bar "http://localhost:$port/two.html"
  expect "^browser: page $page dialog Leave Leave site?\$" "going from the page didn't ask first"
  expect '^browser: dialog settled$' "Leave site? didn't settle"
}
leave
../tools/window-screenshot.sh "$pid" "$out/dialog-leave.png" || fail "no window to capture"
send key escape
expect "^browser: page $page dialog answered cancel\$" "Esc didn't stay on the page"
# A key typed as the dialog closes is the page's: ⌘L, sent the moment the X input focus is back in
# the browser window, opens the command bar, five times over, as the page takes the keyboard as its
# window becomes active (#1711). X11's driver alone can send a key then.
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  for _ in 1 2 3 4 5; do
    leave
    post key escape
    send keyonreturn l cmd
    expect "^browser: page $page dialog answered cancel\$" "Esc didn't stay on the page" $((step - 1))
    expect '^browser: command bar shown$' "⌘L as the dialog closed reached no page (#1711)"
    send key escape
    expect '^browser: command bar hidden$' "Esc didn't close the command bar"
  done
fi
leave
send key return
expect "^browser: page $page dialog answered accept\$" "↩ didn't leave the page"
expect "^browser: loaded $page http://localhost:$port/two.html\$" "the tab didn't go to two.html once left"

# Each visit asks again, as a sign-in that was cancelled isn't asked for the page it shows.
sign_in() {
  go_from_bar "${site}sign-in?$1"
  expect "^browser: page $page dialog SignIn Sign in\$" "the site's sign-in didn't ask"
  expect '^browser: dialog settled$' "the sign-in didn't settle"
}
sign_in cancel
../tools/window-screenshot.sh "$pid" "$out/dialog-sign-in.png" || fail "no window to capture"
send key escape
expect "^browser: page $page dialog answered cancel\$" "Esc didn't cancel the sign-in"
expect "^browser: title $page signed out\$" "a cancelled sign-in didn't show what the site sent"
sign_in again
send type ada
send key tab
send type zq7lovelace
send key return
expect "^browser: page $page dialog answered accept\$" "↩ didn't sign in"
expect "^browser: title $page signed in ada\$" "the site didn't take the username and password"
# The driver's own lines say what it typed.
grep -v '^browser: drive ' "$log" | grep -q zq7lovelace && fail "the password reached the log"

# The pointer on and off the served page's link, over and over, shows and hides the link target's
# window, with the command bar and a tab's menu opened and closed between. Each is a window the UI
# moves and shows from inside its own update, which on Windows aborted the app when the window
# landed on another monitor (#1351). One screen has no other monitor to land on, so this checks the
# app outlives them all with no panic.
go_from_bar "$site"
expect "^browser: title $page ready\$" "the served page didn't load for its link"
# 200 by 40 points, 100 in from the card's top left, which is 228 and 8 in from the window's.
link_point="430 130"
for round in $(seq 20); do
  send move $link_point
  expect '^browser: link target window shown$' "round $round: the pointer on the link showed no plate"
  send move $page_point
  expect '^browser: link target hidden$' "round $round: the plate stayed after the pointer left"
  case $round in
  *5)
    send key t cmd
    expect '^browser: command bar shown$' "round $round: ⌘T didn't open the command bar"
    send key escape
    expect '^browser: command bar hidden$' "round $round: Esc didn't close the command bar"
    ;;
  *0)
    post rightclick $(row 0)
    expect '^browser: menu window at ' "round $round: a right-click on a row didn't open its menu"
    send dismiss
    expect '^browser: menu window hidden$' "round $round: the row's menu stayed up"
    ;;
  esac
done
kill -0 "$pid" 2>/dev/null || gone
! grep -q 'panicked' "$log" || fail "the app panicked as the link target, the bar or a menu showed"

# On Windows, another app's window in front of ilo's lower half, where the link target's corner
# is: the plate a link shows then, and the card a row shows, go under that window and over ilo's,
# by Windows' own z-order, front first (#1419), with the foreground in ilo's window and then in a
# window away from both. They went to the front of every app's window, which Windows allows unless
# the window in front has the foreground. Once it's gone the plate is over ilo's window again.
covered=
# Fails unless the top row of the picture $1, $2 across, behind the window buttons, is the sidebar's
# colour further down: not Windows' caption, which the frame painted under the UI as a PrintWindow
# of it repainted it, the launch picture's as the app went to the background or a capture's (#1262).
no_caption() {
  local top below
  top=$(pixel "$1" "$2" 0.008)
  below=$(pixel "$1" "$2" 0.8)
  ! brighter "$top" "$below" ||
    fail "Windows' caption showed behind the window buttons $3, $top over the sidebar's $below: $1"
}
# On X11, whether the driver's "onscreen" says the X focus is on $1 (overlay, window or none)
# within 5 s.
keys_on() {
  for _ in $(seq 50); do
    send onscreen
    grep '^browser: onscreen ' "$log" | tail -1 | grep -q "keyboard $1\$" && return 0
    sleep 0.1
  done
  return 1
}
# The window and plate on top of the stack the driver last listed match $1, or fail with $2.
stacked() {
  send stack
  stack=$(grep '^browser: stack ' "$log" | tail -1 | cut -c16-)
  case $stack in $1) ;; *) fail "$2: $stack" ;; esac
}
# Another app's window over ilo's, with the foreground in $1 (ilo or away).
under_cover() {
  python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" "$1" \
    > "$out/cover-$1.log" 2>&1 &
  helper=$!
  for _ in $(seq 100); do
    grep -q '^cover ' "$out/cover-$1.log" || ! kill -0 "$helper" 2>/dev/null && break
    sleep 0.1
  done
  grep -q '^cover ' "$out/cover-$1.log" ||
    fail "another app's window didn't open over ilo's: $(cat "$out/cover-$1.log")"
  local foreground
  foreground=$(sed -n 's/^cover .*, foreground //p' "$out/cover-$1.log")
  stacked "*other cover, *driven" "the other app's window isn't in front of ilo's ($1)"
  send move $link_point
  expect '^browser: link target window shown$' \
    "the pointer on the link showed no plate under the other app ($1)"
  ../tools/window-screenshot.sh "$pid" "$out/link-target-covered-$1.png" ||
    fail "no window to capture"
  stacked "*other cover, *link target, *driven" \
    "the link target isn't under the other app's window in front of ilo (foreground $foreground)"
  send move $page_point
  expect '^browser: link target hidden$' \
    "the plate under the other app stayed after the pointer left ($1)"
  send move $(row 0)
  expect '^browser: hover card window shown$' \
    "resting on the tab's row showed no card under the other app ($1)"
  stacked "*other cover, *hover card, *driven" \
    "the hover card isn't under the other app's window in front of ilo (foreground $foreground)"
  send move $page_point
  expect '^browser: hover card hidden$' "leaving the rows didn't hide the card under the other app"
  covered="$covered${covered:+, }the foreground in $foreground with ilo active $active"
  # By its Windows id, as Git Bash's kill may not end a native process.
  taskkill //F //PID "$(cat "/proc/$helper/winpid")" > /dev/null 2>&1 || true
  helper=
  for _ in $(seq 50); do
    send stack
    grep '^browser: stack ' "$log" | tail -1 | grep -q 'other cover' || break
    sleep 0.1
  done
}
if windows; then
  under_cover ilo
  under_cover away
  send move $link_point
  expect '^browser: link target window shown$' \
    "the pointer on the link showed no plate once the other app went"
  stacked "*link target, *driven" "the link target isn't over ilo's window once the other app went"
  case $stack in *"other cover"*) fail "the other app's window outlived its process: $stack" ;; esac
  ../tools/window-screenshot.sh "$pid" "$out/link-target-uncovered.png" ||
    fail "no window to capture"
  send move $page_point
  expect '^browser: link target hidden$' \
    "the plate stayed after the pointer left, the other app gone"
fi

# On Windows, a page that alerts while another app has the foreground: the alert goes over its
# own browser window, under the other app's window in front of it, and the taskbar button asks for
# attention, where it went in front of every app's window (#1365). Brought to the front, as a click
# on its taskbar button does, ilo's window gives the keyboard to the alert.
background_dialog=
if windows; then
  go_from_bar "${site}dialogs.html"
  expect "^browser: title $page ready\$" "the dialogs page didn't load again"
  send click $page_point
  rm -f "$out/site/held.go" "$out/front"
  send key l
  expect "^browser: title $page waiting\$" "\"l\" in the page didn't start waiting for /held"
  served "127.0.0.1:$port/held" "the page didn't ask for /held"
  python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" away "$out/front" \
    > "$out/cover-asking.log" 2>&1 &
  helper=$!
  for _ in $(seq 100); do
    grep -q '^cover ' "$out/cover-asking.log" || ! kill -0 "$helper" 2>/dev/null && break
    sleep 0.1
  done
  grep -q '^cover ' "$out/cover-asking.log" ||
    fail "another app's window didn't open over ilo's: $(cat "$out/cover-asking.log")"
  foreground=$(sed -n 's/^cover .*, foreground //p' "$out/cover-asking.log")
  touch "$out/site/held.go"
  expect "^browser: page $page dialog Alert .* says\$" "the page didn't alert once /held came"
  expect '^browser: dialog settled$' "the page's alert didn't settle with another app in front"
  ../tools/window-screenshot.sh "$pid" "$out/dialog-background.png" || fail "no window to capture"
  # The launch picture the app took as it went to the background, of the sidebar alone.
  launch_picture=$(ls -t "$profile_dir"/launch-picture-*.png 2>/dev/null | head -1)
  [ -n "$launch_picture" ] || fail "going to the background, the app kept no launch picture"
  cp "$launch_picture" "$out/dialog-launch-picture.png"
  [ "$foreground" != away ] ||
    expect '^browser: dialog shown in the background, its window asking for attention$' \
      "the alert shown in the background didn't have its window ask for attention"
  stacked "*other cover, *dialog, *driven" \
    "the page's alert isn't under the other app's window in front of ilo (foreground $foreground)"
  if [ "$foreground" = away ]; then
    touch "$out/front"
    expect '^browser: dialog takes the keyboard back from its window$' \
      "the alert didn't take the keyboard as ilo's window came to the front"
    send state
    [ "$key" = GPUIPanel ] || fail "the alert hasn't the keyboard once ilo came to the front: $key"
    background_dialog="the foreground away"
  else
    background_dialog="the foreground in $foreground, so not in the background"
  fi
  # Brought to the front, the other app ends by itself.
  winpid=$(cat "/proc/$helper/winpid" 2>/dev/null) && taskkill //F //PID "$winpid" > /dev/null 2>&1 ||
    true
  helper=
  answer return accept "alerted later"
fi

# On Windows, the page the user clicked into keeps the keyboard through another app in front, with
# the Library's search beside it still showing its cursor: once ilo is back, the page's alert shows
# at once, where it waited for the field until a click in the page (#3758).
came_back=
if windows && [ "$background_dialog" = "the foreground away" ]; then
  send key l cmd shift
  expect '^browser: command view-library via ' "Ctrl+Shift+L didn't open the Library"
  expect '^browser: library shown$' "Ctrl+Shift+L didn't show the Library"
  # The page beside the Library, which is 644 wide.
  send click 744 400
  [ "$focus" = RenderWidgetHostViewCocoa ] ||
    fail "a click on the page beside the Library didn't give it the keyboard: $focus"
  rm -f "$out/site/held.go" "$out/front"
  send key l
  expect "^browser: title $page waiting\$" "\"l\" in the page didn't start waiting for /held"
  served "127.0.0.1:$port/held" "the page didn't ask for /held"
  python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" away "$out/front" \
    > "$out/cover-came-back.log" 2>&1 &
  helper=$!
  for _ in $(seq 100); do
    grep -q '^cover ' "$out/cover-came-back.log" || ! kill -0 "$helper" 2>/dev/null && break
    sleep 0.1
  done
  grep -q '^cover .*, foreground away$' "$out/cover-came-back.log" ||
    fail "another app didn't come in front of ilo's window with the foreground: $(cat "$out/cover-came-back.log")"
  touch "$out/front"
  for _ in $(seq 50); do
    send state
    [ "$active" = 1 ] && break
    sleep 0.1
  done
  [ "$active" = 1 ] || fail "ilo's window didn't come back to the front"
  [ "$focus" = RenderWidgetHostViewCocoa ] ||
    fail "the page didn't have the keyboard again with ilo back: $focus"
  winpid=$(cat "/proc/$helper/winpid" 2>/dev/null) && taskkill //F //PID "$winpid" > /dev/null 2>&1 ||
    true
  helper=
  touch "$out/site/held.go"
  expect "^browser: page $page dialog Alert .* says\$" "the page didn't alert once /held came"
  expect '^browser: dialog shown .* says$' \
    "the page's alert waited for the Library's search, which the page had the keyboard from: $(grep "^browser: page $page dialog [0-9]* waits" "$log" | tail -1)"
  none_since "^browser: page $page dialog [0-9]* waits for the field\$" ||
    fail "the page's alert waited for the Library's search, which the page had the keyboard from"
  expect '^browser: dialog settled$' "the alert didn't settle with ilo back"
  answer return accept "alerted later"
  send key l cmd shift
  expect '^browser: library hidden$' "Ctrl+Shift+L didn't close the Library"
  send click $page_point
  came_back=1
fi

# On Windows Alt+F4 in a page's dialog cancels it as Esc does, where Windows destroyed the dialog's
# window, which the app shows again for the next question (#1362).
closed_dialog=
if windows; then
  send key c
  expect "^browser: page $page dialog Confirm .* says\$" "\"c\" in the page didn't ask with its Confirm"
  expect '^browser: dialog settled$' "the page's Confirm didn't settle"
  post key f4 ctrl
  await "^browser: drive $step done" || fail "the driver didn't finish Alt+F4 in the dialog"
  expect "^browser: page $page dialog answered cancel\$" "Alt+F4 didn't cancel the page's dialog"
  expect '^browser: dialog hidden$' "the page's dialog didn't leave on Alt+F4"
  expect "^browser: title $page confirm false\$" "the page didn't hear Cancel from Alt+F4"
  ask c Confirm after-close
  # A copy of the screen, with nothing in front: a caption painted under the UI at the background
  # step would still show, as no activation change since paints over it.
  no_caption "$out/dialog-after-close.png" 0.15 "once the dialog came back"
  # And in the pictures of the background step, PrintWindow's, the capture's and the launch picture.
  [ -z "$background_dialog" ] || {
    no_caption "$out/dialog-background.png" 0.15 "with the alert in the background"
    no_caption "$out/dialog-launch-picture.png" 0.4 "in the launch picture"
  }
  answer return accept "confirm true"
  closed_dialog=1
fi

# Two windows side by side, the first in front with the keyboard: the pointer resting on a row of
# the second, behind it, which still hears the pointer, shows that row's card over the second, not
# over the first, the browser window last in front, where every overlay showed (#1432). Read from
# the OS's windows: on Windows the card is on the stack over the second, on the Mac a child window
# of it, and on X11 an overlay inside its content. A row of the first shows its card over the first
# again. Under Wayland and on sway's Xwayland the compositor places the windows.
two_windows=
if windows || [ "$(uname)" = Darwin ] ||
  { [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ] && ! sway_xwayland; }; then
  first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
  second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  new_window_bar
  # The driver numbers windows in the order they opened.
  driven_second=$(grep -c '^browser: window opened ' "$log")
  # The Windows runner's screen is 1920 by 1080 (check-windows.yml), and Xvfb's 1280 by 800
  # (check.sh).
  screen="0 0 1920 1080"
  [ "$(uname)" != Linux ] || screen="0 0 1280 800"
  if [ "$(uname)" = Darwin ]; then
    send screen
    screen=$(grep '^browser: screen at ' "$log" | tail -1 | cut -d' ' -f4-)
  fi
  read -r screen_x screen_y screen_width screen_height <<SCREEN
$screen
SCREEN
  wide=$(((screen_width - 60) / 2))
  [ "$wide" -le 900 ] || wide=900
  tall=$((screen_height - 120))
  [ "$tall" -le 700 ] || tall=700
  send window "$driven_second"
  send place $((screen_x + 40 + wide)) $((screen_y + 60)) $wide $tall
  send window 1
  expect "^browser: key window $first\$" "the first window didn't come in front of the second"
  send place $((screen_x + 20)) $((screen_y + 60)) $wide $tall
  # The driven window's children on the Mac, which the card's window becomes one of, in $children.
  # On X11 the driver lists every overlay shown, from the driven window's content, so there it
  # counts those inside it.
  children() {
    send layout
    children=$(sed -n "/^browser: drive $step: /,\$p" "$log" | awk '
      /^browser: layout content / { split($4, size, "[x,]"); width = size[1]; height = size[2] }
      /^browser: layout child / {
        split($4, at, ","); split($5, size, "x")
        if (linux == "" || (at[1] >= 0 && at[2] >= 0 && at[1] + size[1] <= width &&
          at[2] + size[2] <= height)) count++
      }
      END { print count + 0 }' linux="$([ "$(uname)" != Linux ] || echo 1)")
  }
  send window "$driven_second" behind
  windows || children
  before=${children:-0}
  send move $(row 0)
  expect '^browser: hover card window shown$' \
    "resting on a row of the second window, behind the first, showed no card"
  send order
  order=$(grep '^browser: order ' "$log" | tail -1 | cut -c16-)
  case $order in
  *"window 1 window $driven_second, key window 1") ;;
  *) fail "the second window didn't stay behind the first as its row showed a card: $order" ;;
  esac
  if windows; then
    stacked "*hover card, *driven" "the card for a row of the window behind isn't over that window"
  else
    children
    [ "$children" -gt "$before" ] ||
      fail "the card for a row of the window behind isn't a child window of that window"
  fi
  ../tools/window-screenshot.sh "$pid" "$out/hover-card-window-behind.png" all ||
    fail "no window to capture"
  send move $page_point
  expect '^browser: hover card hidden$' "leaving the second window's rows didn't hide its card"
  # The reverse: the first's row, in front, shows its card over the first.
  send window 1
  windows || children
  before=${children:-0}
  send move $(row 0)
  expect '^browser: hover card window shown$' "resting on a row of the first window showed no card"
  if windows; then
    stacked "*hover card, *driven" "the card for a row of the window in front isn't over it"
  else
    children
    [ "$children" -gt "$before" ] ||
      fail "the card for a row of the window in front isn't a child window of it"
  fi
  send move $page_point
  expect '^browser: hover card hidden$' "leaving the first window's rows didn't hide its card"
  two_windows=1
else
  echo "skip: the hover card over the window behind another, under Wayland or on sway's Xwayland: the compositor places the windows."
fi
# The same steps with the page beside opened by the dialogs page, whose renderer process it shares,
# so its page is blocked while the alert asks, and the core focuses its pane for the press (#3454).
# After the steps that need no split, which this one's failing would hide.
send window 1
go_from_bar "${site}dialogs.html"
expect '^browser: title [0-9]* ready$' "the dialogs page didn't load again for the same-site split"
page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
split_rests "${site}beside.html" same-site shared
# On sway's Xwayland the prompt takes the keys back once its window has sway's focus again after
# another app's window had it: back from that window's workspace, and as that window, beside the
# browser's, quits. wlroots gives the X focus to the browser's window, not to the prompt (#2894).
# Last, as sway's focus back to the browser's window as a prompt answered from the keys goes can
# take it from a command bar opened at once (#2943). The dialogs page is loaded again for it.
if sway_xwayland; then
  go_from_bar "${site}dialogs.html"
  expect '^browser: title [0-9]* ready$' "the dialogs page didn't load again for the prompt's step"
  page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
  send click $page_point
  ask p Prompt prompt-back
  swaymsg 'workspace 2' > /dev/null || fail "sway didn't go to another workspace"
  rm -f "$out/other-in"
  mkfifo "$out/other-in"
  : > "$out/other-window.log" # an earlier run's window stays named in it until emptied (#4040)
  python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
  other=$!
  exec 5> "$out/other-in"
  keys_on none ||
    fail "another app's window on workspace 2 didn't take the keys from the prompt in 5 s: $(grep '^browser: onscreen ' "$log" | tail -1), $(cat "$out/other-window.log")"
  swaymsg 'workspace 1' > /dev/null || fail "sway didn't go back to the browser's workspace"
  keys_on overlay ||
    fail "back on the browser's workspace from another app's window, the prompt didn't take the keys back in 5 s, so typing would go to the page: $(grep '^browser: onscreen ' "$log" | tail -3 | tr '\n' ' ')"
  swaymsg '[title="^Other app$"] move container to workspace 1, focus' > /dev/null ||
    fail "sway didn't bring the other app's window to the browser's workspace"
  keys_on none ||
    fail "the other app's window on the browser's workspace didn't take the keys in 5 s: $(grep '^browser: onscreen ' "$log" | tail -1)"
  # It quits, and sway gives its focus back to the browser's window.
  exec 5>&-
  wait "$other" || fail "another app's window didn't go once its stdin closed: $(cat "$out/other-window.log")"
  keys_on overlay ||
    fail "once another app's window beside the browser's went, the prompt didn't take the keys back in 5 s: $(grep '^browser: onscreen ' "$log" | tail -3 | tr '\n' ' ')"
  send type ada
  answer return accept "prompt ada"
fi
# Closing a pinned tab or a Favorite from its row while its page's alert is up unloads it, its alert
# with it, and ilo goes on (#3549): a pinned tab shown, by its −; a pinned tab whose alert another
# tab took down, by a middle-click on its row; and a Favorite shown, by Close Tab in its tile's menu.
printf '%s' '<!doctype html><title>Alerts</title><script>
addEventListener("keydown", event => { if (event.key == "a") alert("closing") })</script>' \
  > "$out/site/alerts.html"
# The key a in the page of tab $1, shown, asks with its alert. $2 names the tab. The click is on
# the page at 400 across, as the two-window step can leave this window 610 wide, where 600 is the
# card's edge.
alert_up() {
  send click 400 300
  send key a
  expect "^browser: page $1 dialog Alert .* says\$" "the key a in $2's page didn't ask with its alert"
  expect '^browser: dialog settled$' "$2's alert didn't settle"
}
open_from_bar "${site}alerts.html"
held=$tab
send key d cmd
expect '^browser: command toggle-pin via page$' "⌘D with the page focused didn't reach the UI"
expect "^browser: pinned $held\$" "⌘D didn't pin the alerts page"
alert_up $held "the pinned tab"
send move $(pinned_close 0)
expect "^browser: hover tab $held close − " "the pinned tab's row didn't draw its − while its alert was up"
send click $(pinned_close 0)
expect '^browser: command close-tab via sidebar$' \
  "a click on the − of the pinned tab whose alert was up didn't run Close Tab"
# ilo quit here before #3541, with nothing saved.
expect "^browser: page $held unloaded 1\$" "the − didn't unload the pinned tab whose alert was up"
send click $(pinned_row 0)
expect "^browser: selected $held\( in .*\)\{0,1\}\$" "a click on the unloaded pinned tab didn't show it"
expect "^browser: page $held unloaded 0\$" "the unloaded pinned tab didn't load once shown"
expect "^browser: loaded $held ${site}alerts.html\$" "the pinned tab's page didn't load again once shown"
alert_up $held "the pinned tab loaded again"
# The Today tab at the top, under the pinned row, takes the alert down.
send click 100 $(($(row 0 | cut -d' ' -f2) + 34))
expect '^browser: dialog withdrawn$' "the pinned tab's alert stayed up over another tab"
send middleclick $(pinned_row 0)
expect '^browser: command close-tab via sidebar$' \
  "a middle-click on the pinned tab whose alert another tab took down didn't run Close Tab"
expect "^browser: page $held unloaded 1\$" \
  "a middle-click didn't unload the pinned tab whose alert another tab took down"
# The ⌘T is typed at once, while the withdrawn alert leaves: on Linux it gives the keys back as it
# starts to, and a key typed then opens the bar as it does after the hide (#3617).
open_from_bar "${site}alerts.html"
favorite=$tab
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type favorite tab
expect '^browser: command bar commands "favorite tab": Favorite Tab' \
  "the command bar didn't find Favorite Tab"
send key return
expect '^browser: command add-to-favorites via command bar$' "Favorite Tab didn't run from the bar"
expect "^browser: favorites $favorite\$" "Favorite Tab didn't make the alerts page a favorite"
alert_up $favorite "the Favorite"
# One favorite is one tile across the sidebar, 90 below the window's top (favorites.sh). Its menu
# stays open over the alert, and the alert has the keys again once Escape closes it: on X11 the
# right-press moves the X focus off the alert, which took the keys back from the menu (#3616).
post rightclick 114 114
expect "^browser: menu shown .*$(cased 'Close Tab')\$" "a right-click on the tile didn't offer Close Tab"
# AppKit puts the menu's window on screen some frames after it's shown, taking the keyboard then: an
# Escape before went to the alert under it and cancelled that instead (restore.sh's choose_drawn).
expect '^browser: menu drawn ' "the tile's menu over the Favorite's alert didn't come on screen"
send key escape
expect '^browser: menu window hidden$' "Escape didn't close the tile's menu over the Favorite's alert"
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  keys_on overlay ||
    fail "the Favorite's alert didn't take the keys back from its tile's menu, closed by Escape, in 5 s: $(grep '^browser: onscreen ' "$log" | tail -1)"
fi
post rightclick 114 114
expect "^browser: menu shown .*$(cased 'Close Tab')\$" "a right-click on the tile didn't offer Close Tab again"
expect '^browser: menu drawn ' "the tile's menu over the Favorite's alert didn't come on screen again"
send choose "$(cased 'Close Tab')"
expect '^browser: command close-tab via tab menu$' "Close Tab in the tile's menu didn't run"
expect "^browser: page $favorite unloaded 1\$" "Close Tab didn't unload the Favorite whose alert was up"
send move $page_point
# A page out of sight that asks with an alert, which Chromium holds until its tab shows, is let go by
# the idle check once it looks 2 hours unseen, its alert with it, and ilo goes on (#3549). It
# changes nothing else as it asks: a title changed out of sight is Chrome's sign of a page at work
# in the background, which keeps it. The core says when Chromium holds the alert for its tab.
printf '%s' '<!doctype html><title>Asks out of sight</title><script>
document.addEventListener("visibilitychange", () => {
  if (document.hidden) setTimeout(() => alert("out of sight"), 100)
})</script>' > "$out/site/asks-hidden.html"
open_from_bar "${site}asks-hidden.html"
asking=$tab
hidden=$((step + 1))
open_from_bar "${site}"
# After Chromium's prefix.
expect "browser: tab $asking dialog held\$" \
  "the page out of sight didn't ask with its alert, held for its tab" "$hidden"
send age 7200
aged=$step
open_from_bar "${site}"
expect 'browser: idle tabs checked$' "showing a tab didn't check for tabs unseen 2 hours"
# The core logs it, after Chromium's prefix.
since $aged | grep -q "browser: tab $asking without its page\$" ||
  fail "the idle check kept the page out of sight that asked with its alert"
expect "^browser: page $asking unloaded 1\$" "the row of the tab let go didn't show it without its page"
# Still running, which a page deleted with its alert held ended (#3549).
send move $page_point
exec 3>&-
quit
echo "ok: a page's alert, confirm and prompt ask over its card, answer with ↩ and Esc, and an alert"
echo "  comes back with its tab. In a split view it covers its own pane and rests there as the page"
echo "  beside takes a click or ⌃⇧2, the keys, Japanese input and a resize; a click on the resting"
echo "  card brings it live and one on its OK answers, the page beside on another site and the same:"
echo "  $out/dialog-alert-split-{two,same}-site-{rest,live}-{light,dark}.png. The confirm's Cancel and OK show the"
echo "  pointer on them. Leave site?"
echo "  stays with Esc and leaves with ↩. A site's sign-in cancels with Esc and signs in with ↩, and"
echo "  the password reaches no log. A link to Maps asks, and ↩ opens nothing. The link target shows"
echo "  and hides 20 times, between the command bar and a tab's menu, with no panic. A pinned tab"
echo "  closed from its row by its − or a middle-click, and a Favorite by its tile's Close Tab, while"
echo "  its page's alert is up, unloads, and ilo goes on. A page out of sight whose alert waits for its"
echo "  tab is let go by the idle check after 2 hours unseen, and ilo goes on (#3549)."
[ -z "$covered" ] || echo "  Under another app's window in front of ilo's ($covered), the" \
  "link target and the hover card show under it, and the link target over ilo's window once it's" \
  "gone. Windows: $out/link-target-covered-ilo.png, $out/link-target-covered-away.png," \
  "$out/link-target-uncovered.png"
[ -z "$raised_over" ] || echo "  On X11 an alert stays $raised_over ilo's, in the window" \
  "manager's stacking, and has the keys again back in ilo's window."
[ -z "$background_dialog" ] || echo "  An alert that came with another app in front of ilo's" \
  "window ($background_dialog) went under that app's window and over ilo's, and took the keyboard" \
  "as ilo's window came to the front, with no caption of Windows' behind the window buttons there" \
  "or in the launch picture: $out/dialog-background.png $out/dialog-launch-picture.png"
[ -z "$came_back" ] || echo "  With the Library's search beside the page that had the keyboard," \
  "an alert that came once ilo was back from another app in front showed at once."
[ -z "$closed_dialog" ] || echo "  Alt+F4 in a page's dialog cancels it as Esc does, and the" \
  "dialog asks again, with no caption of Windows' behind the window buttons:" \
  "$out/dialog-after-close.png"
[ -z "$two_windows" ] || echo "  With two windows side by side, a row of the one behind shows its" \
  "card over that window, and a row of the one in front over the one in front:" \
  "$out/hover-card-window-behind.png"
