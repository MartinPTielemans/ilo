# A page's own dialogs ask with the browser's dialog over the tab's card, in Chrome's words, each
# captured while it's up: alert(), confirm() and prompt() from the served page's keys. ↩ answers
# OK, with what was typed in a prompt, and Esc Cancel, and the page puts the answer in its title.
# The confirm's Cancel and OK are captured with the pointer on each. An alert goes with its tab
# and comes back with it. Leaving a page whose beforeunload handler asks says "Leave site?": Esc
# stays and ↩ leaves. A site's HTTP sign-in asks for a username and password: Esc shows the page
# the site sends without them, and ↩ signs in, with the password in no log. A link to another app
# asks first, and ↩ there opens nothing. The pointer on and off a link 20 times shows and hides the
# link target each time, with the command bar and a tab's menu opened and closed between, and the
# app lives through it with no panic (#1351).

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
exec 3>&-
quit
echo "ok: a page's alert, confirm and prompt ask over its card, answer with ↩ and Esc, and an alert"
echo "  comes back with its tab. The confirm's Cancel and OK show the pointer on them. Leave site?"
echo "  stays with Esc and leaves with ↩. A site's sign-in cancels with Esc and signs in with ↩, and"
echo "  the password reaches no log. A link to Maps asks, and ↩ opens nothing. The link target shows"
echo "  and hides 20 times, between the command bar and a tab's menu, with no panic."
