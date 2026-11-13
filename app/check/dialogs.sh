# check.sh runs this at 320.
# A page's own dialogs ask with the browser's dialog over the tab's card, in Chrome's words, each
# captured while it's up: alert(), confirm() and prompt() from the served page's keys. ↩ answers
# OK, with what was typed in a prompt, and Esc Cancel, and the page puts the answer in its title.
# The confirm's Cancel and OK are captured with the pointer on each. An alert goes with its tab
# and comes back with it. Leaving a page whose beforeunload handler asks says "Leave site?": Esc
# stays, and on X11 ⌘L the moment it has closed opens the command bar (#1711), and ↩ leaves. A site's HTTP sign-in asks for a username and password: Esc shows the page
# the site sends without them, and ↩ signs in, with the password in no log. A link to another app
# asks first, and ↩ there opens nothing. The pointer on and off a link 20 times shows and hides the
# link target each time, with the command bar and a tab's menu opened and closed between, and the
# app lives through it with no panic (#1351). On Windows, with another app's window in front of
# ilo's, the link target and the hover card show under it, and the link target over ilo's window
# again once it's gone (#1419). An alert that comes while another app has the foreground goes over
# ilo's window but under the other app's, its taskbar button asking for attention, and has the
# keyboard once ilo's window comes to the front (#1365). Alt+F4 in a page's dialog cancels it as Esc
# does, and the next question asks again (#1362). On Windows and the Mac, with a second
# window beside the first and the first in front, the pointer resting on a row of the second
# shows its card over the second, and on a row of the first over the first (#1432).

serve
log="$out/dialogs.log"
step=0
rm -f "$saved"
BROWSER_DRIVE=1 BROWSER_URL="${site}dialogs.html" "$browser" \
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

# Another tab takes the alert down, and its own brings it back.
open_from_bar example.com
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
  go_from_bar example.org
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
expect "^browser: loaded $page https://example.org/\$" "the tab didn't go to example.org once left"

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
send type lovelace
send key return
expect "^browser: page $page dialog answered accept\$" "↩ didn't sign in"
expect "^browser: title $page signed in ada\$" "the site didn't take the username and password"
# The driver's own lines say what it typed.
grep -v '^browser: drive ' "$log" | grep -q lovelace && fail "the password reached the log"

# The pointer on and off the served page's link, over and over, shows and hides the link target's
# window, with the command bar and a tab's menu opened and closed between. Each is a window the UI
# moves and shows from inside its own update, which on Windows aborted the app when the window
# landed on another monitor (#1351). One screen has no other monitor to land on, so this checks the
# app outlives them all with no panic.
go_from_bar "$site"
expect "^browser: title $page ready\$" "the served page didn't load for its link"
# 200 by 40 points, 100 in from the card's top left, which is 228 and 10 in from the window's.
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
# the OS's windows: on Windows the card is on the stack over the second, and on the Mac a child
# window of it. A row of the first shows its card over the first again.
two_windows=
if windows || [ "$(uname)" = Darwin ]; then
  first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
  send key n cmd
  expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
  second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  new_window_bar
  # The driver numbers windows in the order they opened.
  driven_second=$(grep -c '^browser: window opened ' "$log")
  # The Windows runner's screen is 1920 by 1080 (check-windows.yml).
  screen="0 0 1920 1080"
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
  children() {
    send layout
    children=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child' ||
      true)
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
  echo "skip: the hover card over the window behind another: Linux's driver can't drive a window behind another without bringing it forward."
fi
exec 3>&-
quit
echo "ok: a page's alert, confirm and prompt ask over its card, answer with ↩ and Esc, and an alert"
echo "  comes back with its tab. The confirm's Cancel and OK show the pointer on them. Leave site?"
echo "  stays with Esc and leaves with ↩. A site's sign-in cancels with Esc and signs in with ↩, and"
echo "  the password reaches no log. A link to Maps asks, and ↩ opens nothing. The link target shows"
echo "  and hides 20 times, between the command bar and a tab's menu, with no panic."
[ -z "$covered" ] || echo "  Under another app's window in front of ilo's ($covered), the" \
  "link target and the hover card show under it, and the link target over ilo's window once it's" \
  "gone. Windows: $out/link-target-covered-ilo.png, $out/link-target-covered-away.png," \
  "$out/link-target-uncovered.png"
[ -z "$background_dialog" ] || echo "  An alert that came with another app in front of ilo's" \
  "window ($background_dialog) went under that app's window and over ilo's, and took the keyboard" \
  "as ilo's window came to the front, with no caption of Windows' behind the window buttons there" \
  "or in the launch picture: $out/dialog-background.png $out/dialog-launch-picture.png"
[ -z "$closed_dialog" ] || echo "  Alt+F4 in a page's dialog cancels it as Esc does, and the" \
  "dialog asks again, with no caption of Windows' behind the window buttons:" \
  "$out/dialog-after-close.png"
[ -z "$two_windows" ] || echo "  With two windows side by side, a row of the one behind shows its" \
  "card over that window, and a row of the one in front over the one in front:" \
  "$out/hover-card-window-behind.png"
