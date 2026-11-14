# check.sh runs this at 270.
# A page's submitted login brings our offer to save its password, a card 18 in from the page
# card's top right corner, which keeps its place through a resize. Showing another tab closes it,
# and showing the tab again doesn't bring it back, but the key after the host in the URL field
# does. Japanese input composes in its username field, where return commits it without saving;
# return saves the username as left in the field, and the login page fills the saved password in
# after, with a key that opens the Site Controls. Chrome's dropdown under the username field fills
# the login in again once the password is cleared. Blink's eye in the password field shows once a
# password is typed into it, never for the one filled in, and goes with the field's focus. A
# changed password brings the offer to update it, and escape there declines. On another site,
# Never answers, and that site's Site Controls list it in a Passwords row whose menu takes it off
# Never. On the first site, the row names the login, All Passwords… lists it in Settings, whose
# Remove asks first and whose Manage Passwords… opens Chrome's page listing it, and the row's
# Remove asks in the browser window's dialog and removes it, which Settings hears. Then a sign-up
# form's new-password field offers Chrome's strong password, where Choose Your Own leaves the field
# to the user and Use Strong Password fills it and saves it, with no offer. Last, Settings' Offer
# to save passwords, switched off, keeps the next login from bringing the offer, and Save and fill
# addresses switches off and on. A form's address brings Chrome's offer to save it, which return
# saves, and a card's, which escape declines. Settings' Passwords tab lists the address, then a card
# return saves, and each Remove asks first and removes it. Save and fill addresses and payment
# methods, switched off, keep a new address and the card from bringing an offer. On the Mac, Edit ›
# AutoFill, in place of the system's, opens Chrome's dropdown at the field with the focus, and with
# none, Settings › Passwords. Each run has a profile of its own, which no other run or the owner's
# copy reaches.

# The login page, in a new profile, whose passwords start empty, with a DevTools client that sees
# the eye in its password field (check/cdp.py).
serve
log="$out/passwords.log"
cdp_log="$out/passwords-cdp.log"
step=0
passwords_profile="$PWD/$out/passwords-profile"
scratch=$(mktemp -d)
start_with_cdp "${site}login.html" "$passwords_profile" /login.html
await_load
await '^browser: title [0-9]* login$' || fail "the login page didn't draw"
tab=$(today)
# Asks cdp.py whether the eye in the password field shows until it says $1, for up to 10 s, as the
# page takes keys and focus a moment after they're sent, or fails with $2.
eye() {
  for _ in $(seq 100); do
    asked=$(grep -c '^cdp: reveal ' "$cdp_log" || true)
    echo "reveal pass" >&5
    for _ in $(seq 20); do
      [ "$(grep -c '^cdp: reveal ' "$cdp_log" || true)" -gt "$asked" ] && break
      sleep 0.05
    done
    [ "$(grep '^cdp: reveal ' "$cdp_log" | tail -1)" = "cdp: reveal $1" ] && return 0
  done
  fail "$2: cdp.py said $(grep '^cdp: ' "$cdp_log" | tail -1)"
}
# The username field, across the top of the page, and the password field under it.
send click 600 60
if [ "$active" != 1 ]; then
  exec 3>&- 5>&-
  quit
  echo "skip: passwords. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# Logs in on the page shown with password $1: the username field has the keyboard.
log_in() {
  send key a cmd
  send type check@example.test
  send key tab
  send key a cmd
  send type "$1"
  send key return
}
# Waits for the page's login to bring an offer of kind $1, and for its card to take input. Sets $ox
# and $oy to where the card is in the browser window.
offered() {
  expect "^browser: offer $tab $1 check@example.test " "the login brought no offer to $1"
  expect '^browser: offer shown$' "the offer to $1 wasn't shown"
  expect "^browser: offer $tab takes input\$" "the offer to $1 never took input"
  offer_place
}
# Sets $ox and $oy to where the offer's card is in the browser window.
offer_place() {
  send layout
  child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: layout child' |
    grep ' 320x180$' | head -1 | sed 's/^browser: layout child //')
  [ -n "$child" ] || fail "the offer's card isn't over the browser window"
  ox=${child%%,*}
  oy=${child#*,}
  oy=${oy%% *}
}
# The offer's window has the keyboard, in its username field for a login.
in_offer() {
  [ "$key" = GPUIPanel ] && [ "$focus" = GPUIView ] ||
    fail "$1: the keyboard is in $key, on $focus"
}
# Where to click in the offer's username field, and on its Never, at the left of its buttons.
offer_field() {
  echo "$((ox + 56)) $((oy + 70))"
}
offer_never() {
  echo "$((ox + 24)) $((oy + 150))"
}
# The card is 18 from the top right corner of the window's content, 8 in from the page card's.
check_offer() {
  content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
  [ "$child" = "$((content - 338)),18 320x180" ] ||
    fail "$1: the offer is at $child in a window $content wide"
}

log_in hunter2.check
offered save
check_offer "after the login"
../tools/window-screenshot.sh "$pid" "$out/password-offer.png" || fail "no window to capture"
send resize 1000 700
send layout
child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: layout child' |
  grep ' 320x180$' | head -1 | sed 's/^browser: layout child //')
check_offer "after a resize"

# Another tab closes the offer, and coming back doesn't bring it back.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site#http://}welcome.html"
send key return
expect '^browser: offer hidden$' "showing another tab didn't hide the offer"
expect "^browser: offer $tab gone\$" "showing another tab didn't close Chrome's offer"
switch "$(($(pinned | wc -w) + 2)) cmd" "$tab"
back=$step
# The command bar opening and closing takes longer than the core reporting an offer.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
sed -n "/^browser: drive $back: /,\$p" "$log" | grep -q '^browser: offer shown$' &&
  fail "showing the tab again brought its closed offer back"
# The key after the host stays for the offer, and a click on it brings the offer back, which Not
# Now answers.
grep -q "^browser: key $tab offer\$" "$log" || fail "the offer gave its tab no key"
# Where the key was drawn last, its centre in the window.
key_at() { grep '^browser: key drawn at ' "$log" | tail -1 | cut -d' ' -f5-6; }
# The pointer on the key hovers the sidebar, whose tools come out in the URL field and move the
# key: it's clicked where it is with them out, or the press can land on what took its place.
click_key() {
  if [ "$(grep '^browser: sidebar tools ' "$log" | tail -1)" != "browser: sidebar tools shown" ]; then
    send move $(key_at)
    expect '^browser: sidebar tools shown$' "the pointer on the key didn't bring the sidebar's tools out"
  fi
  send click $(key_at)
}
[ -n "$(key_at)" ] || fail "the key wasn't drawn"
../tools/window-screenshot.sh "$pid" "$out/password-key.png" || fail "no window to capture"
click_key
expect "^browser: key $tab clicked\$" "a click on the key didn't reach it"
offered save
skip_on_sway_xwayland "the offer's keys and the steps after them"
send click $(offer_field)
send key escape
expect "^browser: offer $tab answered NotNow check@example.test\$" \
  "escape didn't decline the offer the key brought back"
expect '^browser: offer hidden$' "declining didn't hide the offer the key brought back"
go_from_bar "${site#http://}login.html"
expect "^browser: title $tab login\$" "the login page didn't load again"

# Japanese input composes in the offer's username field, and return commits it without saving.
send click 600 60
log_in hunter2.check
offered save
send click $(offer_field)
in_offer "after a click in the offer"
send key a cmd
send ime on
send type nihongo
[ "$marked" = 0,7 ] || fail "the offer's field didn't take the composition: marked $marked"
send key space
[ "$marked" = 0,3 ] || fail "the offer's field didn't take the conversion: marked $marked"
committed=$((step + 1))
send key return
[ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
send ime off
../tools/window-screenshot.sh "$pid" "$out/password-offer-ime.png" || fail "no window to capture"
# Return saves the username as the field has it.
send key a cmd
send type check@example.test
send key return
expect "^browser: offer $tab answered Accept check@example.test\$" "return didn't save"
[ "$(since "$committed" | grep -c "^browser: offer $tab answered")" = 1 ] ||
  fail "return committing the composition answered"
expect '^browser: offer hidden$' "saving didn't hide the offer"
expect "^browser: offer $tab gone\$" "saving didn't close Chrome's offer"

# The login page fills the saved password in. A changed one brings the offer to update it, and
# escape declines.
go_from_bar "${site#http://}login.html"
expect "^browser: title $tab filled\$" "the login page didn't fill the saved password in"
# The saved login gives the page a key, and a click on it opens the Site Controls, which list it.
await "^browser: key $tab saved\$" || fail "the saved login gave the page no key"
click_key
expect "^browser: key $tab clicked\$" "a click on the saved login's key didn't reach it"
expect '^browser: command site-controls via key$' \
  "the saved login's key didn't open the Site Controls"
expect '^browser: site controls settled$' "the Site Controls the key opened didn't settle"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls the key opened"
# Sets children to how many child windows the browser window has, Chrome's dropdown among them.
# In this shell, never in $(…), where send's count of the driver's steps would be lost.
count_children() {
  send layout
  children=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
    grep -c '^browser: layout child' || true)
}
# Waits up to 10 s, as expect does, by the clock wait_begins reads, for the browser window to have
# more child windows than $shown, or fails with $1. Chrome shows its dropdown after reading the
# saved logins, so a count of layouts would be over sooner on a quick driver.
dropdown_shown() {
  began=${SECONDS:-$(date +%s)}
  until count_children; [ "$children" -gt "$shown" ]; do
    [ "$((${SECONDS:-$(date +%s)} - began))" -lt 10 ] ||
      fail "$1: $children child windows, as before it"
    sleep 0.1
  done
}
# Waits up to 10 s for Chrome's dropdown to close, after escape, or fails with $1.
dropdown_gone() {
  send key escape
  began=${SECONDS:-$(date +%s)}
  until count_children; [ "$children" = "$shown" ]; do
    [ "$((${SECONDS:-$(date +%s)} - began))" -lt 10 ] ||
      fail "$1: $children child windows, where there were $shown"
    sleep 0.1
  done
}
# With the password cleared, a click in the username field shows Chrome's dropdown under it, where
# down previews the saved login and return fills it in again. Chrome shows it a moment after the
# click, and a down before then only opens it (a Wayland run, 43492843229), so down waits for it where
# the driver lists Chromium's popups as child windows: the Mac's and both of Linux's.
count_children
shown=$children
send click 600 60
send key tab
send key a cmd
send key backspace
expect "^browser: title $tab cleared\$" "clearing the password didn't undo the fill"
if ! windows; then
  # Emptying the password can bring a dropdown up at its field, which the click would replace.
  dropdown_gone "escape didn't close the dropdown at the emptied password"
  send click 600 60
  dropdown_shown "a click in the username field showed no dropdown of the saved login"
else
  send click 600 60
fi
send key down
expect "^browser: title $tab filled\$" "down didn't preview the saved login"
../tools/window-screenshot.sh "$pid" "$out/password-fill.png" || fail "no window to capture"
# Chrome ignores a suggestion accepted in the first 500 ms its dropdown is drawn
# (kIgnoreEarlyClicksOnSuggestionsDuration), and nothing marks when that ends.
sleep 0.6
send key return
expect "^browser: title $tab chosen\$" "Chrome's dropdown didn't fill the saved login in"
# The password filled in shows no eye in its field, and one typed into the emptied field does,
# until the field loses focus.
send click 600 60
send key tab
eye hidden "the eye showed for the password filled in"
send key a cmd
send key backspace
send type hunter3.check
eye shown "the eye didn't show once a password was typed"
../tools/window-screenshot.sh "$pid" "$out/password-reveal.png" || fail "no window to capture"
send click 600 60
eye hidden "the eye stayed once the password field lost focus"
send key return
offered update
../tools/window-screenshot.sh "$pid" "$out/password-update.png" || fail "no window to capture"
send click $(offer_field)
in_offer "after a click in the update offer"
send key escape
expect "^browser: offer $tab answered NotNow check@example.test\$" "escape didn't decline"
expect '^browser: offer hidden$' "declining didn't hide the offer"

# On another site, Never answers.
go_from_bar "localhost:$port/login.html"
expect "^browser: title $tab login\$" "the login page didn't load on localhost"
send click 600 60
log_in hunter2.check
offered save
send click $(offer_never)
expect "^browser: offer $tab answered Never check@example.test\$" "Never didn't answer"
expect '^browser: offer hidden$' "Never didn't hide the offer"

# The Site Controls list the site set to Never in a Passwords line, whose menu takes it off Never at
# once. The panel logs where it drew the line.
last_controls() { grep '^browser: site controls http' "$log" | tail -1; }
controls="^browser: site controls http://localhost:$port: "
bar_command site-controls "Site Controls" "site controls"
expect "${controls}.*; passwords never localhost:$port\$" \
  "the Site Controls didn't list localhost as set to Never: $(last_controls)"
expect '^browser: site controls settled$' "the Site Controls didn't open"
../tools/window-screenshot.sh "$pid" "$out/site-controls-never.png" || fail "no window to capture"
click_placed post "line Passwords"
expect "^browser: menu shown $(cased 'Offer to Save Passwords, All Passwords…')\$" \
  "the Passwords line didn't offer to take localhost off Never"
send choose "$(cased 'Offer to Save Passwords')"
expect "^browser: site controls allow saving localhost:$port\$" "Offer to Save Passwords didn't run"
expect "^browser: remove login [0-9]* never localhost:$port\$" \
  "Offer to Save Passwords didn't take localhost off Never"
expect "${controls}[^;]*; [^;]*\$" \
  "the Site Controls kept the Passwords row once localhost was off Never: $(last_controls)"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"

# On the first site the row names the saved login, and All Passwords… lists it in Settings, whose
# Remove asks in an alert, where Cancel keeps it. With one profile there's no pop-up, so the box at
# the top holds the profile's offers to save passwords, addresses and cards, where Save and fill
# addresses switches off and on again. Settings logs where it draws each.
go_from_bar "${site#http://}login.html"
expect "^browser: title $tab filled\$" "the login page didn't fill the saved password in again"
controls="^browser: site controls http://127.0.0.1:$port: "
bar_command site-controls "Site Controls" "site controls"
expect "${controls}.*; passwords 127.0.0.1:$port check@example.test\$" \
  "the Site Controls didn't list the saved login: $(last_controls)"
expect '^browser: site controls settled$' "the Site Controls didn't open on the first site"
../tools/window-screenshot.sh "$pid" "$out/site-controls-passwords.png" ||
  fail "no window to capture"
click_placed post "line Passwords"
expect "^browser: menu shown $(cased 'Remove “check@example.test”…, All Passwords…')\$" \
  "the Passwords line didn't offer to remove the saved login"
send choose "$(cased 'All Passwords…')"
expect '^browser: site controls run passwords$' "All Passwords… didn't run"
expect '^browser: settings tab Passwords$' "All Passwords… didn't show Settings' Passwords tab"
settings_drew Passwords
expect "^browser: passwords [0-9]*: 127.0.0.1:$port check@example.test\$" \
  "Settings didn't list the saved login alone: $(grep '^browser: passwords ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/settings-passwords.png" || fail "no window to capture"
post clickkey $(drawn "remove login 0")
# Settings asks in an alert, the Mac's or Windows' task dialog, and on Linux, which has none, in its
# dialog, where escape cancels. The dialog covers Settings, taking its keys, until it has gone.
asks=alert
[ "$(uname)" = Linux ] && asks=dialog
expect "^browser: $asks shown Remove the password for “check@example.test” on 127.0.0.1:$port?\$" \
  "Remove in Settings didn't ask first"
if [ "$asks" = dialog ]; then
  expect '^browser: dialog settled$' "Settings' dialog didn't settle"
  send key escape
  expect '^browser: dialog hidden$' "escape didn't cancel Settings' dialog"
else
  # The alert runs a loop of its own, where the UI's tasks and timers go on: held open a while, none
  # may find the UI in an update, which GPUI would drop or panic on (#1358).
  asked=$(wc -l < "$log")
  sleep 2 # the alert held open, its loop running the UI's timers
  send choose Cancel
  ! tail -n "+$((asked + 1))" "$log" | grep -q 'gpui dropped\|panicked\|already borrowed' ||
    fail "the UI's work in the alert's loop found it in an update: $(tail -n "+$((asked + 1))" "$log" |
      grep 'gpui dropped\|panicked\|already borrowed' | head -3 | tr '\n' ';')"
fi
none_since '^browser: remove login' || fail "Cancel removed the login"
click_in_sight "switch save and fill addresses"
expect '^browser: save and fill addresses off$' "a click on Save and fill addresses didn't switch it off"
click_in_sight "switch save and fill addresses"
expect '^browser: save and fill addresses on$' "a second click didn't switch addresses back on"
# Manage Passwords…, under the box of the login's row, opens Chrome's password manager page in a
# new tab, where a search for the username finds the login. Its device check guards only Show,
# Copy, a login's details and Export, which the run never asks for, so no prompt comes. ⌘W closes
# it, and ⌘2 shows the login page's tab again, under the welcome page's.
post clickkey $(drawn "manage passwords")
expect '^browser: manage passwords [0-9]*$' "a click on Manage Passwords… didn't run it"
expect '^browser: settings hidden$' "Manage Passwords… didn't hide Settings"
await '^browser: loaded [0-9]* chrome://password-manager/passwords$' ||
  fail "Manage Passwords… didn't open Chrome's password manager page"
manager=$(grep '^browser: loaded [0-9]* chrome://password-manager/passwords$' "$log" | tail -1 |
  cut -d' ' -f3)
[ "$manager" != "$tab" ] || fail "Manage Passwords… opened Chrome's page in the login's tab"
go_from_bar 'chrome://password-manager/passwords?q=check@example.test'
await "^browser: loaded $manager chrome://password-manager/passwords?q=check\(@\|%40\)example.test\$" ||
  fail "Chrome's password manager page didn't search for the login"
# Asks cdp.py, for up to 10 s, until the page has drawn its list.
for _ in $(seq 100); do
  asked=$(grep -c '^cdp: shows ' "$cdp_log" || true)
  echo "shows chrome://password-manager/ check@example.test" >&5
  for _ in $(seq 20); do
    [ "$(grep -c '^cdp: shows ' "$cdp_log" || true)" -gt "$asked" ] && break
    sleep 0.05
  done
  [ "$(grep '^cdp: shows ' "$cdp_log" | tail -1)" = "cdp: shows yes" ] && break
done
[ "$(grep '^cdp: shows ' "$cdp_log" | tail -1)" = "cdp: shows yes" ] ||
  fail "Chrome's password manager page didn't list the login: cdp.py said $(tail -1 "$cdp_log")"
../tools/window-screenshot.sh "$pid" "$out/password-manager.png" || fail "no window to capture"
send key w cmd
expect "^browser: tab $manager closed\$" "⌘W didn't close Chrome's password manager page"
send key 2 cmd
expect "^browser: selected $tab\( in .*\)\{0,1\}\$" "⌘2 didn't show the login's tab again"

# The row's Remove closes the panel and asks in the browser window's dialog, where return removes
# the login, and Settings, which asked for the logins, hears that it's gone. A click on the page
# gives the browser window the keyboard back.
send click $page_point
bar_command site-controls "Site Controls" "site controls"
expect '^browser: site controls settled$' "the Site Controls didn't open again"
click_placed post "line Passwords"
expect "^browser: menu shown $(cased 'Remove “check@example.test”…, All Passwords…')\$" \
  "the Passwords line didn't offer to remove the saved login again"
send choose 'Remove “check@example.test”…'
expect '^browser: site controls run remove-login$' "Remove didn't run"
expect "^browser: dialog shown Remove the password for “check@example.test” on 127.0.0.1:$port?\$" \
  "Remove didn't ask first"
expect '^browser: dialog settled$' "the dialog asking to remove the login didn't settle"
../tools/window-screenshot.sh "$pid" "$out/password-remove.png" || fail "no window to capture"
send key return
expect "^browser: remove login [0-9]* 127.0.0.1:$port check@example.test\$" \
  "return didn't remove the login"
expect "^browser: passwords [0-9]*: none\$" "Settings didn't hear the login was removed"

# With no login saved for the site, Chrome's popup in a sign-up form's new-password field offers
# Choose Your Own and Use Strong Password, and previews its password in the field. Up selects
# Choose Your Own, whose return leaves the field to the user. On the page again, up then down
# selects Use Strong Password, whose return fills the password in; Chrome saves it in the profile,
# which Settings hears, and the sign-up brings no offer to save it. Made-up usernames only. Up
# first, since the popup's first down selects Choose Your Own on 156 and Use Strong Password on
# 157, where up selects Choose Your Own on both, and either key then moves to the other button.
# Loads the sign-up page and types username $1, then tab takes the keyboard to the password field,
# where down brings the popup: Chrome's desktop Autofill opens on a click, typing or an arrow key in
# a field, not on focus from the keyboard.
generation_offered() {
  go_from_bar "${site#http://}signup.html"
  expect "^browser: title $tab signup\$" "the sign-up page didn't load"
  send click 600 60
  send type "$1"
  send key tab
  send key down
  expect "^browser: title $tab offered\$" "Chrome's popup didn't offer a password to the field"
}
generation_offered own@example.test
../tools/window-screenshot.sh "$pid" "$out/password-generation.png" || fail "no window to capture"
send key up
send key return
expect "^browser: title $tab own\$" "Choose Your Own didn't leave the password field to the user"
generation_offered new@example.test
send key up
send key down
send key return
expect "^browser: title $tab generated\$" "Use Strong Password didn't fill a password in"
await "^browser: passwords [0-9]*: 127.0.0.1:$port new@example.test\$" ||
  fail "Settings didn't hear the save: $(grep '^browser: passwords ' "$log" | tail -1)"
send click 600 60
send key return
expect "^browser: title $tab welcome\$" "the sign-up didn't go through"
none_since "^browser: offer $tab " || fail "the generated password brought an offer to save it"

# Chrome offers a strong password only while Offer to save passwords is on, so it switches off last,
# in Settings' Passwords tab, which the Site Controls show again from the generated login's row.
bar_command site-controls "Site Controls" "site controls"
expect "${controls}.*; passwords 127.0.0.1:$port new@example.test\$" \
  "the Site Controls didn't list the generated login: $(last_controls)"
expect '^browser: site controls settled$' "the Site Controls didn't open after the sign-up"
click_placed post "line Passwords"
expect "^browser: menu shown $(cased 'Remove “new@example.test”…, All Passwords…')\$" \
  "the Passwords line didn't offer to remove the generated login"
send choose "$(cased 'All Passwords…')"
expect '^browser: site controls run passwords$' "All Passwords… didn't run again"
expect '^browser: settings shown Passwords$' "All Passwords… didn't show Settings' Passwords tab again"
settings_drew Passwords
click_in_sight "switch offer to save passwords"
expect '^browser: offer to save passwords off$' "a click on Offer to save passwords didn't switch it off"
../tools/window-screenshot.sh "$pid" "$out/settings-passwords-off.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"

# Switched off, the profile doesn't offer to save the next login. The page it posts to loading, and
# the command bar opening and closing, take longer than the core reporting an offer. A click on the
# page gives the browser window the keyboard back from Settings.
send click $page_point
go_from_bar "${site#http://}login.html"
expect "^browser: title $tab login\$" "the login page didn't load after the sign-up"
send click 600 60
off=$step
log_in hunter2.off
expect "^browser: title $tab welcome\$" "the login didn't post"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
sed -n "/^browser: drive $off: /,\$p" "$log" | grep -q "^browser: offer $tab " &&
  fail "the login brought an offer with Offer to save passwords off"

# A form's address brings Chrome's offer to save it, in the same card with its lines to read,
# which takes the keyboard when clicked, and return saves it; the form then offers it in Chrome's
# dropdown.
# A card's offer names it by its last four digits, and escape declines it. Made-up data only.
# Types $1 in the form's first field, then each of the rest after a tab, and submits it.
submit_form() {
  send click 600 60
  send type "$1"
  shift
  for value in "$@"; do
    send key tab
    send type "$value"
  done
  send key return
}
# Waits for the page's form to bring an offer of kind $1, and for its card to take input, then
# gives the card the keyboard with a click on its title, as the login's takes it.
offered_to_save() {
  expect "^browser: offer $tab $1 [0-9]*\$" "the $1 form brought no offer to save it"
  expect '^browser: offer shown$' "the offer to save the $1 wasn't shown"
  expect "^browser: offer $tab takes input\$" "the offer to save the $1 never took input"
  offer_place
  send click $((ox + 160)) $((oy + 24))
  in_offer "after a click on the offer to save the $1"
}
go_from_bar "${site#http://}address.html"
expect "^browser: title $tab address\$" "the address page didn't load"
submit_form 'Ada Lovelace' '1 Check Street' Springfield IL 62701 'United States' ada@example.test
offered_to_save address
../tools/window-screenshot.sh "$pid" "$out/address-offer.png" || fail "no window to capture"
send key return
expect "^browser: offer $tab answered Accept \$" "return didn't save the address"
expect "^browser: offer $tab gone\$" "saving the address left Chrome's offer open"
go_from_bar "${site#http://}address.html"
expect "^browser: title $tab address\$" "the address page didn't load again"
send click 600 60
send key down
expect "^browser: title $tab filled\$" "Chrome's dropdown didn't offer the saved address"
../tools/window-screenshot.sh "$pid" "$out/address-fill.png" || fail "no window to capture"
send key escape
go_from_bar "${site#http://}card.html"
expect "^browser: title $tab card\$" "the card page didn't load"
submit_form 'Ada Lovelace' 4111111111111111 12 2030
offered_to_save card
../tools/window-screenshot.sh "$pid" "$out/card-offer.png" || fail "no window to capture"
send key escape
expect "^browser: offer $tab answered NotNow \$" "escape didn't decline the card"
expect "^browser: offer $tab gone\$" "declining the card left Chrome's offer open"

# Edit › AutoFill, in the Mac's menu bar, the only one: ilo's group after Find, in place of the
# system's, opens Chrome's dropdown at the field the page's focus is in, as a click in it does, and
# Passwords… one with every login. With the page's focus off its fields, Settings › Passwords.
if [ "$(uname)" = Darwin ]; then
  send menubar Edit
  edit='^browser: menu bar Edit: '
  expect "$edit.*, Find › [^,]*, AutoFill › Contact…, AutoFill › Passwords…, AutoFill › Credit Card…" \
    "Edit lacks ilo's AutoFill after Find: $(grep "$edit" "$log" | tail -1)"
  [ "$(grep "$edit" "$log" | tail -1 | grep -o 'AutoFill ›' | wc -l)" -eq 3 ] ||
    fail "Edit has the system's AutoFill too: $(grep "$edit" "$log" | tail -1)"
  go_from_bar "${site#http://}address.html"
  expect "^browser: title $tab address\$" "the address page didn't load for Edit › AutoFill"
  count_children
  shown=$children
  send click 600 60
  dropdown_shown "a click in the address field showed no dropdown"
  dropdown_gone "escape didn't close the dropdown the click showed"
  send menu "AutoFill › Contact…"
  expect "^browser: autofill $tab dropdown\$" "Edit › AutoFill › Contact… didn't find the focused field"
  dropdown_shown "Edit › AutoFill › Contact… showed no dropdown at the focused field"
  ../tools/window-screenshot.sh "$pid" "$out/autofill-menu.png" || fail "no window to capture"
  send key down
  expect "^browser: title $tab filled\$" "the dropdown Edit › AutoFill showed didn't offer the address"
  dropdown_gone "escape didn't close the dropdown Edit › AutoFill showed"
  send menu "AutoFill › Passwords…"
  expect "^browser: autofill $tab dropdown\$" "Edit › AutoFill › Passwords… didn't find the field"
  dropdown_shown "Edit › AutoFill › Passwords… showed no dropdown at the focused field"
  dropdown_gone "escape didn't close the dropdown of logins"
  # A click below the form takes the page's focus off its fields, which the core hears from the
  # page after the click's done. On the page: the window's content is 700 tall and the page ends
  # 10 above that, so a click at 700 lands on the window's edge, which leaves the field focused.
  send click 600 600
  expect "^browser: title $tab unfocused\$" "a click below the form left a field with the page's focus"
  send menu "AutoFill › Credit Card…"
  expect "^browser: autofill $tab no field\$" "Edit › AutoFill found a field after a click off them"
  expect '^browser: settings [a-z]* Passwords$' \
    "Edit › AutoFill with no field focused didn't show Settings' Passwords tab"
  settings_drew Passwords
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings after Edit › AutoFill"
else
  echo "skip: Edit › AutoFill: the menu bar is the Mac's alone."
fi
# Settings' Passwords tab, which ⌘, shows again, lists the saved address under the logins, and its
# Remove asks first, as a login's does, and removes it, which Settings hears. The card declined
# isn't listed.
# Settings says "autofill <profile>: …"; the core's "autofill <tab> no field" isn't its count.
saved_autofill() { grep '^browser: autofill [0-9]*: ' "$log" | tail -1 | sed 's/^browser: autofill [0-9]*: //'; }
send key , cmd
expect '^browser: settings shown Passwords$' "⌘, didn't show Settings' Passwords tab again"
settings_drew Passwords
for _ in $(seq 100); do
  [ "$(saved_autofill)" = "1 addresses, 0 cards" ] && break
  sleep 0.1
done
[ "$(saved_autofill)" = "1 addresses, 0 cards" ] ||
  fail "Settings didn't hear of the saved address alone: $(saved_autofill)"
remove_at=$(drawn "remove autofill 0") || fail "Settings didn't list the saved address"
../tools/window-screenshot.sh "$pid" "$out/settings-address.png" || fail "no window to capture"
post clickkey $remove_at
expect "^browser: $asks shown Remove the address for “.*”?\$" "Remove on the address didn't ask first"
if [ "$asks" = dialog ]; then
  expect '^browser: dialog settled$' "Settings' dialog didn't settle"
  send key return
else
  send choose Remove
fi
expect '^browser: remove address [0-9]*$' "Remove in the dialog didn't remove the address"
for _ in $(seq 100); do
  [ "$(saved_autofill)" = "0 addresses, 0 cards" ] && break
  sleep 0.1
done
[ "$(saved_autofill)" = "0 addresses, 0 cards" ] ||
  fail "Settings didn't hear the address was removed: $(saved_autofill)"

# A card return saves is listed too, its network and expiry over its name and never a digit of its
# number, and its Remove asks first and removes it.
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings after removing the address"
send click $page_point
go_from_bar "${site#http://}card.html"
expect "^browser: title $tab card\$" "the card page didn't load again"
submit_form 'Grace Hopper' 5555555555554444 11 2031
offered_to_save card
send key return
expect "^browser: offer $tab answered Accept \$" "return didn't save the card"
expect "^browser: offer $tab gone\$" "saving the card left Chrome's offer open"
send click $page_point
send key , cmd
expect '^browser: settings shown Passwords$' "⌘, didn't show Settings' Passwords tab with the card"
settings_drew Passwords
for _ in $(seq 100); do
  [ "$(saved_autofill)" = "0 addresses, 1 cards" ] && break
  sleep 0.1
done
[ "$(saved_autofill)" = "0 addresses, 1 cards" ] ||
  fail "Settings didn't hear of the saved card: $(saved_autofill)"
remove_at=$(drawn "remove autofill 0") || fail "Settings didn't list the saved card"
../tools/window-screenshot.sh "$pid" "$out/settings-autofill.png" || fail "no window to capture"
post clickkey $remove_at
expect "^browser: $asks shown Remove the card “Grace Hopper”?\$" "Remove on the card didn't ask first"
if [ "$asks" = dialog ]; then
  expect '^browser: dialog settled$' "Settings' dialog didn't settle"
  send key return
else
  send choose Remove
fi
expect '^browser: remove card [0-9]*$' "Remove didn't remove the card"
for _ in $(seq 100); do
  [ "$(saved_autofill)" = "0 addresses, 0 cards" ] && break
  sleep 0.1
done
[ "$(saved_autofill)" = "0 addresses, 0 cards" ] ||
  fail "Settings didn't hear the card was removed: $(saved_autofill)"

# Save and fill addresses and Save and fill payment methods, switched off for the profile, keep a
# new address and the card just removed from bringing an offer. The pages they post to loading,
# and the command bar opening and closing, take longer than the core reporting an offer.
click_in_sight "switch save and fill addresses"
expect '^browser: save and fill addresses off$' "a click on Save and fill addresses didn't switch it off"
click_in_sight "switch save and fill payment methods"
expect '^browser: save and fill payment methods off$' \
  "a click on Save and fill payment methods didn't switch it off"
../tools/window-screenshot.sh "$pid" "$out/settings-autofill-off.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings after switching autofill off"
send click $page_point
go_from_bar "${site#http://}address.html"
expect "^browser: title $tab address\$" "the address page didn't load with addresses off"
off=$step
submit_form 'Grace Hopper' '2 Check Street' Arlington VA 22201 'United States' grace@example.test
expect "^browser: title $tab welcome\$" "the address didn't post with addresses off"
go_from_bar "${site#http://}card.html"
expect "^browser: title $tab card\$" "the card page didn't load with payment methods off"
submit_form 'Grace Hopper' 5555555555554444 11 2031
expect "^browser: title $tab welcome\$" "the card didn't post with payment methods off"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
sed -n "/^browser: drive $off: /,\$p" "$log" | grep -q "^browser: offer $tab " &&
  fail "an address or card brought an offer with Save and fill switched off"
exec 3>&- 5>&-
quit
rm -rf "$scratch"
echo "ok: a submitted login offers to save its password in a card 18 from the page's top right, through a resize; another tab closes it, and the key after the host brings it back; Japanese input composes in its username field and return commits it without saving; return saves the username as left, and the login page fills the saved password in, with a key that opens the Site Controls, and Chrome's dropdown fills it in again; the eye shows in the password field once one is typed, never for the one filled in, and goes with the field's focus; a changed password offers to update it, which escape declines; and Never answers on another site, whose Site Controls take it off Never; the first site's Site Controls name its login, All Passwords… lists it in Settings, where Remove asks and Cancel keeps it and Manage Passwords… opens Chrome's password manager page listing it, and the row's Remove asks in the dialog and removes it, which Settings hears; a sign-up form offers Chrome's strong password, which Choose Your Own leaves to the user and Use Strong Password fills in and saves, with no offer; and Settings' Offer to save passwords switched off keeps the next login from bringing an offer, and Save and fill addresses switches off and on; a form's address offers to save it in the card, return saves it and Chrome's dropdown then offers it, and a card's offer declines with escape; on the Mac, Edit › AutoFill has ilo's items alone, after Find, whose Contact… and Passwords… open Chrome's dropdown at the focused field, and Credit Card… with none focused opens Settings › Passwords; Settings lists the address, and then a card return saved, and Remove asks first and removes each; and Save and fill addresses and payment methods switched off keep a new address and the card from bringing an offer. Windows: $out/password-offer.png, $out/password-key.png, $out/password-fill.png, $out/password-reveal.png, $out/password-offer-ime.png, $out/password-update.png, $out/site-controls-never.png, $out/site-controls-passwords.png, $out/settings-passwords.png, $out/password-manager.png, $out/password-remove.png, $out/password-generation.png, $out/settings-passwords-off.png, $out/address-offer.png, $out/address-fill.png, $out/card-offer.png, $out/settings-address.png, $out/settings-autofill.png, $out/settings-autofill-off.png, $out/autofill-menu.png (Mac)"
