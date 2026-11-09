# An account's devices, each a copy of the app with a profile of its own, on an ilo-server of the
# run's own (ilo-server.sh). A signs in first and copies its recovery words, which the clipboard
# holds marked for clipboard managers to skip (#641). B signs in and asks the account's devices
# to add it, and A, once its window is active, asks the user, who adds it with a click: B syncs.
# A Space A made reaches B, in place of B's own untouched first one, and one B makes reaches A. C
# signs in and recovers with the words instead, with the account's Spaces. A's open tab reaches
# B's sync store, and a tab of A's agent's Space never does (#1796); B's command bar lists A's
# tab, naming A, and opens it (#1234). A saves a login, which B lists, then removes, and A hears
# it go. 90 s after A's copy the clipboard holds the words no longer. B and C sign out without
# asking, as the account has other devices, and the server forgets them. On Linux under Xvfb and on
# Windows, where each device's window takes the foreground from the others' as the run drives it
# (#2114). The Mac's clipboard and two copies of the app at once there are unrun.

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-devices. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  return 0
fi
if ! windows && { [ "$(uname)" != Linux ] || [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ]; }; then
  echo "skip: sync-devices. It runs under Xvfb on Linux and on Windows so far, where the run reaches the clipboard and each copy's windows."
  return 0
fi
rm -f "$saved" "$launch_frame" "$profile_dir/Account"
. check/ilo-server.sh

# The Spaces the device's window has, by the last list it logged.
spaces() {
  grep '^browser: spaces ' "$log" | tail -1 | awk '{ print NF - 2 }'
}
# Waits up to 30 s for the device to have $1 Spaces, and fails with $2.
await_spaces() {
  for _ in $(seq 300); do
    [ "$(spaces)" = "$1" ] && return 0
    kill -0 "$pid" 2>/dev/null || gone
    sleep 0.1
  done
  fail "$2: $(spaces) Spaces"
}
# Makes a Space named $1 in the device's window, from the Spaces menu, as spaces.sh does.
new_space() {
  had=$(spaces)
  forward
  send menu New Space…
  expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space on $on"
  await_spaces $((had + 1)) "New Space didn't make a Space on $on"
  send type "$1"
  send key return
}
# The format that marks what clipboard managers are to skip (#641).
hint=x-kde-passwordManagerHint
! windows || hint=ExcludeClipboardContentFromMonitorProcessing
others=

# A's list ends with none once B removes the login A saves, so A starts with none: the check's
# profile keeps the logins an earlier run saved, as import's. The app isn't running, so the files
# are free.
for logins in "$profile_dir/Default/Login Data" "$profile_dir/Default/Login Data For Account"; do
  [ ! -e "$logins" ] || python3 -I -c '
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute("delete from logins")
db.commit()
' "$logins" || fail "the check's profile kept its logins"
done
start a "$profile_dir" 5
open_sign_in
sign_in first
await '^browser: account FirstDevice None$' ||
  fail "signing in didn't make A the account's first device"
settings_keyboard
at=$(drawn "sync copy") || fail "Settings › Sync didn't draw Copy with the words"
send clickkey $at
copied_at=$(date +%s)
words=
for _ in $(seq 30); do
  words=$(clipboard)
  [ "$(echo "$words" | wc -w)" = 24 ] && break
  sleep 0.1
done
[ "$(echo "$words" | wc -w)" = 24 ] || fail "Copy didn't put 24 words on the clipboard"
clipboard formats | grep -qx "$hint" ||
  fail "the copied words aren't marked for clipboard managers to skip: $(clipboard formats | tr '\n' ' ')"
at=$(drawn "sync done") || fail "Settings › Sync didn't draw Done with the words"
send clickkey $at
await 'browser: sync service active$' || fail "A's sync didn't become active"
# A Space A makes reaches the devices it adds, and theirs reach it.
new_space "from A"

start b "$PWD/$out/sync-devices-b" 6
! windows || forward
open_sign_in
sign_in
await '^browser: account NeedsSecret None$' || fail "B, signed in, didn't need the account's secret"
await '^browser: account Pairing None$' || fail "B didn't ask the account's devices to add it"
../tools/window-screenshot.sh "$pid" "$out/sync-devices-b-pairing.png" || fail "no window to capture"
# A takes the request once one of its windows is active, as it would be in front of the user.
# A may have been the active app already, so these are read from all A has logged.
device a
forward
await '^browser: pairing asked$' || fail "A didn't take B's request with its window active"
await '^browser: dialog shown Add the device that asked at ' || fail "A didn't ask whether to add B"
await '^browser: dialog confirm at ' || fail "A's question didn't say where Add is"
../tools/window-screenshot.sh "$pid" "$out/sync-devices-a-asked.png" || fail "no window to capture"
add=$(sed -n 's/^browser: dialog confirm at //p' "$log" | tail -1)
# Adding a device grants it the account, so Add ignores clicks for its first moment on screen.
await '^browser: dialog takes input$' || fail "A's question never took input"
send click $add
expect '^browser: dialog confirmed$' "a click on Add didn't add B"
device b
await '^browser: account On None$' || fail "B, added by A, didn't turn sync on"
await 'browser: sync service active$' || fail "B's sync didn't become active"
# B's own first Space, untouched, gives way to the account's, as Arc's does on a new device.
await_spaces 2 "B, synced, doesn't have A's two Spaces"
new_space "from B"
device a
await_spaces 3 "A doesn't have the Space B made"

start c "$PWD/$out/sync-devices-c" 7
! windows || forward
open_sign_in
sign_in
await '^browser: account NeedsSecret None$' || fail "C, signed in, didn't need the account's secret"
settings_keyboard
at=$(drawn "sync use-words") || fail "C's Settings › Sync didn't offer the words"
send clickkey $at
send type $words
send key return
await '^browser: account On None$' || fail "C didn't turn sync on with the recovery words"
await 'browser: sync service active$' || fail "C's sync didn't become active"
await_spaces 3 "C, recovered, doesn't have the account's three Spaces"
[ "$(stored "SELECT count(*) FROM devices")" = 3 ] ||
  fail "the server doesn't know the account's three devices: $(stored "SELECT group_concat(name, '|') FROM devices")"

# A's open tabs reach B, as the account's other computer's, but a tab of an agent's Space never
# does, as an agent's Space stays on its device (#1796): B's sync store gets the user's tab A
# opened, and never the agent's, though A's agent opened it first.
serve
device a
send agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "A's driver didn't make the agent's Space"
send agentopen claude-code "${site}login.html?agents-own"
expect '^browser: agent claude-code opened [1-9][0-9]*$' "A's agent didn't open a tab in its Space"
agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
await "^browser: loaded $agent_tab ${site}login.html?agents-own\$" || fail "A's agent's tab didn't load"
forward
go_from_bar "${site#http://}login.html?users-own"
expect '^browser: title [0-9]* login$' "A's own tab didn't load"
store_b="$out/sync-devices-b/Default/Sync Data/LevelDB"
synced=
for _ in $(seq 900); do
  grep -rqa 'login.html?users-own' "$store_b" 2>/dev/null && synced=1 && break
  sleep 0.1
done
[ -n "$synced" ] || fail "A's open tab never reached B's sync store, so an agent's couldn't be told apart"
grep -rqa 'login.html?agents-own' "$store_b" &&
  fail "a tab of A's agent's Space reached B's sync store, as another computer's open tab"
# B's command bar finds A's tab by its title and names A as Settings does, and never the agent's
# (#1234). ↩ opens it in B, a tab of B's own.
device b
await "^browser: other computers' tabs [1-9]" ||
  fail "B's core never told the app of A's open tab, which B's sync store has"
forward
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open B's command bar"
send type login
expect "^browser: command bar lists other computers' tabs: ${site}login.html?users-own on $label" \
  "B's command bar didn't list A's tab for \"login\""
grep "^browser: command bar lists other computers' tabs: " "$log" | grep -q 'agents-own' &&
  fail "B's command bar listed a tab of A's agent's Space"
# Its row follows the core's first, after Passwords, which "login" finds above the search row.
selected() { grep '^browser: command bar selected ' "$log" | tail -1; }
for _ in 1 2 3; do
  send key down
  selected | grep -q "login.html?users-own\$" && break
done
selected | grep -q "login.html?users-own\$" ||
  fail "↓ never selected A's tab in B's command bar: $(selected)"
send key return
expect '^browser: command bar hidden$' "↩ on A's tab didn't close B's command bar"
expect "^browser: loaded [0-9]* ${site}login.html?users-own\$" "↩ on A's tab didn't open its page in B"

# A saves a login, which reaches B through the server, as only its ciphertext (sync-account reads
# the store): B's Settings › Passwords lists it. B removes it, and A's list hears it go.
device a
forward
go_from_bar "${site#http://}login.html"
expect '^browser: title [0-9]* login$' "A's login page didn't load"
tab=$(sed -n 's/^browser: title \([0-9]*\) login$/\1/p' "$log" | tail -1)
send click 600 60
[ "$active" = 1 ] || fail "A wasn't the active app, so its login page ignored the click"
send key a cmd
send type check@example.test
send key tab
send key a cmd
send type hunter2.sync
send key return
expect "^browser: offer $tab save check@example.test " "A's login brought no offer to save it"
expect "^browser: offer $tab takes input\$" "A's offer to save the login never took input"
# The offer takes the keyboard with a click in its field, 56 in and 70 down, as passwords' does.
# Its card is the window's 320x180 child, which the driver lists on every platform.
send layout
offer_at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: layout child' |
  grep ' 320x180$' | head -1 | sed 's/^browser: layout child \([0-9]*\),\([0-9]*\) .*/\1 \2/')
[ -n "$offer_at" ] || fail "A's offer card isn't over its window"
send click $((${offer_at% *} + 56)) $((${offer_at#* } + 70))
send key return
expect "^browser: offer $tab answered Accept check@example.test\$" "return didn't save A's login"
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show A's Settings"
settings_keyboard
send clickkey $(drawn "toolbar Passwords")
expect '^browser: settings tab Passwords$' "a click on Passwords didn't show A's tab"
login="127.0.0.1:$port check@example.test"
await "^browser: passwords [0-9]*: $login\$" || fail "A's Settings didn't list the login it saved"
device b
settings_keyboard
send clickkey $(drawn "toolbar Passwords")
expect '^browser: settings tab Passwords$' "a click on Passwords didn't show B's tab"
await "^browser: passwords [0-9]*: $login\$" ||
  fail "B didn't get the login A saved: $(grep '^browser: passwords ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/sync-devices-b-password.png" || fail "no window to capture"
post clickkey $(drawn "remove login 0")
# Settings asks in Windows' task dialog, and on Linux, which has none, in its dialog, as passwords'
# does.
asks=dialog
! windows || asks=alert
expect "^browser: $asks shown Remove the password for “check@example.test” on 127.0.0.1:$port?\$" \
  "Remove on B didn't ask first"
if [ "$asks" = dialog ]; then
  expect '^browser: dialog settled$' "B's dialog asking to remove the login didn't settle"
  send key return
else
  send choose Remove
fi
expect "^browser: remove login [0-9]* $login\$" "return didn't remove the login on B"
send clickkey $(drawn "toolbar Sync")
expect '^browser: settings tab Sync$' "a click on Sync didn't show B's tab again"
device a
for _ in $(seq 300); do
  [ "$(grep '^browser: passwords ' "$log" | tail -1 | sed 's/.*: //')" = none ] && break
  sleep 0.1
done
[ "$(grep '^browser: passwords ' "$log" | tail -1 | sed 's/.*: //')" = none ] ||
  fail "A didn't hear B remove the login: $(grep '^browser: passwords ' "$log" | tail -1)"

# A lets the words go 90 s after it copied them.
while [ $(($(date +%s) - copied_at)) -lt 92 ]; do
  sleep 1
done
[ -z "$(clipboard)" ] || fail "the clipboard still held the words 90 s after A copied them"

# B and C sign out without a question, as A keeps the account, and the server forgets them.
for d in c b; do
  device $d
  settings_keyboard
  at=$(drawn "sync sign-out") || fail "$d's Settings › Sync didn't draw Sign Out"
  send clickkey $at
  await '^browser: account SignedOut None$' || fail "Sign Out didn't sign $d out"
done
for _ in $(seq 50); do
  left=$(stored "SELECT count(*) FROM devices")
  [ "$left" = 1 ] && break
  sleep 0.2
done
[ "$left" = 1 ] || fail "B and C signed out, but the server knows $left devices"
for d in c b; do
  device $d
  exec 3>&-
  quit_app
  wait "$pid" || fail "$d exited with status $?"
done
exec 6>&- 7>&-
others=
device a
exec 5>&- 3>&-
quit
# What A's sync carries, as the core names its data types once sync is active.
types=$(grep -h 'browser: sync types ' "$a_log" | tail -1 | sed 's/.*browser: sync types //')
[ -n "$types" ] || fail "A's sync never said what it carries"
# The runs after start signed out.
rm -f "$profile_dir/Account"
kill "$ilo_server" 2>/dev/null || true
ilo_server=
echo "ok: A added B, C recovered with the copied words, which the clipboard let go of after 90 s," \
  "A's own open tab reached B, whose command bar listed and opened it, and its agent's never did," \
  "a login A saved reached B, whose removal reached A," \
  "and B and C signed out, on a local ilo-server as $email. A synced: $types." \
  "Logs: $out/sync-devices-*.log"
