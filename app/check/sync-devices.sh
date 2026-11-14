# check.sh runs this at 450.
# An account's devices, each a copy of the app with a profile of its own, on an ilo-server of the
# run's own (ilo-server.sh). A signs in first and copies its recovery words, which the clipboard
# holds marked for clipboard managers to skip (#641). B signs in and asks the account's devices
# to add it, and A, once its window is active, asks the user, who adds it with a click: B syncs.
# A Space A made reaches B, in place of B's own untouched first one, and one B makes reaches A. C
# signs in and recovers with the words instead, with the account's Spaces. A Today tab A opens,
# navigates, moves to another Space and archives does the same on B within 2.5 s, where it never
# loads, and a busy session's commits are counted and never throttled (#2098). A's Little ilo tab
# reaches B's sync store, and a tab of A's agent's Space never does (#1796); B's command bar lists
# A's tab, naming A, and opens it (#1234). B and C drop a plain bookmark the run's server sends. A
# saves a login, which B lists, then removes, and A hears it go. 90 s after A's copy the clipboard
# holds the words no longer. B and C sign out without asking, as the account has other devices, and
# the server forgets them. On Linux under Xvfb and Wayland and on Windows, where each device's
# window takes the foreground from the others' as the run drives it (#2114), and on the Mac, where
# the copy in front hands the next its activation (#589).

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-devices. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  return 0
fi
if sway_xwayland; then
  echo "skip: sync-devices. It runs on Linux under Xvfb and Wayland, on Windows and on the Mac so far, where the run reaches the clipboard and each copy's windows."
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
# The format that marks what clipboard managers are to skip (#641): on the Mac, nspasteboard.org's.
hint=x-kde-passwordManagerHint
! windows || hint=ExcludeClipboardContentFromMonitorProcessing
[ "$(uname)" != Darwin ] || hint=org.nspasteboard.ConcealedType
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
  clipboard
  words=$clipped
  [ "$(echo "$words" | wc -w | tr -d " ")" = 24 ] && break
  sleep 0.1
done
[ "$(echo "$words" | wc -w | tr -d " ")" = 24 ] || fail "Copy didn't put 24 words on the clipboard"
clipboard formats
echo "$clipped" | grep -qx "$hint" ||
  fail "the copied words aren't marked for clipboard managers to skip: $(echo "$clipped" | tr '\n' ' ')"
at=$(drawn "sync done") || fail "Settings › Sync didn't draw Done with the words"
send clickkey $at
await 'browser: sync service active$' || fail "A's sync didn't become active"
# A Space A makes reaches the devices it adds, and theirs reach it.
new_space "from A"
# A bookmark in plain text, which no device of an account with a passphrase writes, put in the
# account's store as a server could: B and C drop it as they set up, and it reaches no model of
# theirs (sync_guard.h, KeepSyncUpdate).
python3 -I - "$db" << 'PY' || fail "the server's store took no plain bookmark"
import sqlite3, sys, time

def varint(n):
    out = b""
    while True:
        out += bytes([(n & 0x7F) | (0x80 if n > 0x7F else 0)])
        n >>= 7
        if not n:
            return out

def field(number, value):
    if isinstance(value, int):
        return varint(number << 3) + varint(value)
    value = value.encode() if isinstance(value, str) else value
    return varint(number << 3 | 2) + varint(len(value)) + value

BOOKMARKS = 32904
db = sqlite3.connect(sys.argv[1], timeout=30)
# The server makes its folders as A first asks for bookmarks.
for _ in range(300):
    folder = db.execute(
        "SELECT account_id, id FROM sync_entities WHERE server_tag = 'bookmark_bar'").fetchone()
    if folder:
        break
    time.sleep(0.1)
account, parent = folder
db.execute("UPDATE sync_stores SET version = version + 1 WHERE account_id = ?", (account,))
version = db.execute(
    "SELECT version FROM sync_stores WHERE account_id = ?", (account,)).fetchone()[0]
guid = "6f1c2a4e-0b7d-4c55-9a3e-2d8f1e7b9c01"
bookmark = (field(1, "https://plain.example.test/") + field(3, "Plain") + field(10, guid) +
            field(11, "Plain") + field(14, "0bc5d13f-2cba-5d74-951f-3f233fe6c908") + field(15, 1))
now = int(time.time() * 1000)
entity = (field(1, "plain-bookmark") + field(2, parent) + field(4, version) + field(5, now) +
          field(6, now) + field(7, "Plain") + field(21, field(BOOKMARKS, bookmark)))
db.execute("INSERT INTO sync_entities (account_id, id, data_type, version, parent_id, deleted, "
           "client_tag_hash, server_tag, entity) VALUES (?, ?, ?, ?, ?, 0, NULL, NULL, ?)",
           (account, "plain-bookmark", BOOKMARKS, version, parent, entity))
db.commit()
PY

# An engine older than the core's guard on what sync takes, as the Mac's can be, takes the plain
# bookmark, and another user's invitations to share a password.
guarded=
! engine_has ../core/sync_guard.cc ../engine/patches/ours/chromium-sync-hooks.patch || guarded=1
[ -n "$guarded" ] || echo "skip: sync-devices' plain bookmark. The engine was built before sync's guard."
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
[ -z "$guarded" ] || await "browser: sync dropped an update of Bookmarks that isn't ciphertext$" ||
  fail "B didn't drop the server's plain bookmark"
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
[ -z "$guarded" ] || await "browser: sync dropped an update of Bookmarks that isn't ciphertext$" ||
  fail "C didn't drop the server's plain bookmark"
await_spaces 3 "C, recovered, doesn't have the account's three Spaces"
[ "$(stored "SELECT count(*) FROM devices")" = 3 ] ||
  fail "the server doesn't know the account's three devices: $(stored "SELECT group_concat(name, '|') FROM devices")"

# A Today tab is the account's, as its Space is (#2098). Each change A makes to one reaches B within
# 2.5 s, the 2 s aimed for with room for a busy runner: its page and title, without loading it,
# which B's core says as it puts the tab in its Space.
serve
# The Mac's date has no %N, but its perl has Time::HiRes.
now_ms() {
  case $(date +%N) in
  *N) perl -MTime::HiRes=time -e 'printf "%d\n", time() * 1000' ;;
  *) echo $(($(date +%s%N) / 1000000)) ;;
  esac
}
# Waits up to 5 s for B to have more lines matching $1 than the $2 it had, then for them to have
# come within 2.5 s of $3, A's change, failing with $4 if either doesn't. took is how long it took.
b_gets() {
  for _ in $(seq 100); do
    if [ "$(grep -c "$1" "$b_log")" -gt "$2" ]; then
      took=$(($(now_ms) - $3))
      [ "$took" -le 2500 ] || fail "$4 in $took ms, past 2.5 s"
      took_all="$took_all $took"
      return 0
    fi
    sleep 0.05
  done
  fail "$4 within 5 s"
}
took_all=
synced_line="browser: synced today [0-9]* in space [0-9]* at [0-9]* unloaded ${site}login.html?today"
device a
forward
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open A's command bar"
send insert "${site}login.html?today-1"
had=$(grep -c "$synced_line-1\$" "$b_log" || true)
send key return
changed=$(now_ms)
expect "^browser: loaded [0-9]* ${site}login.html?today-1\$" "A's Today tab didn't load"
a_tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}login.html?today-1\$|\1|p" "$log" | tail -1)
b_gets "$synced_line-1\$" "$had" "$changed" "A's new Today tab didn't reach B"
b_line=$(grep -o "$synced_line-1\$" "$b_log" | tail -1)
b_tab=$(echo "$b_line" | cut -d' ' -f4)
b_space=$(echo "$b_line" | cut -d' ' -f7)

navigated="browser: synced today $b_tab in space $b_space at [0-9]* unloaded ${site}login.html?today-2\$"
had=$(grep -c "$navigated" "$b_log" || true)
go_from_bar "${site#http://}login.html?today-2"
changed=$(now_ms)
expect "^browser: loaded $a_tab ${site}login.html?today-2\$" "A's Today tab didn't go to its next page"
b_gets "$navigated" "$had" "$changed" "A's navigation didn't take B's tab, unloaded, to the same page"

# Moved to the Space B shows, it shows there, still without its page. Today's rows start at row 0,
# as "from A" has no pinned tabs.
post rightclick $(row 0)
expect "^browser: menu shown .*Move to › from B" "A's Today tab's menu didn't offer to move it to \"from B\""
# Any Space but the one it was in.
moved="browser: synced today $b_tab in space [0-9]* at 0 unloaded ${site}login.html?today-2\$"
had=$(grep -c "$moved" "$b_log" || true)
send choose "from B"
changed=$(now_ms)
b_gets "$moved" "$had" "$changed" "A's move to \"from B\" didn't move B's tab"
grep "browser: synced today $b_tab in space " "$b_log" | tail -1 | grep -q " in space $b_space " &&
  fail "B's tab stayed in Space $b_space when A moved it"
for _ in $(seq 50); do
  grep '^browser: today' "$b_log" | tail -1 | tr ' ' '\n' | grep -qx "$b_tab" && break
  sleep 0.1
done
grep '^browser: today' "$b_log" | tail -1 | tr ' ' '\n' | grep -qx "$b_tab" ||
  fail "B's sidebar doesn't list the tab A moved to the Space B shows: $(grep '^browser: today' "$b_log" | tail -1)"
# Xvfb's capture is the screen, so B's window goes in front of A's for it.
device b
forward
../tools/window-screenshot.sh "$pid" "$out/sync-devices-b-today.png" || fail "no window to capture"
grep -q "^browser: selected $b_tab\( in .*\)\{0,1\}\$" "$log" && fail "B showed the Today tab A moved to the Space B shows"

# Archived on A, it goes to B's archive. A's window went with the tab it moved, to "from B".
device a
forward
for _ in $(seq 50); do
  [ "$(today)" = "$a_tab" ] && break
  sleep 0.1
done
[ "$(today)" = "$a_tab" ] || fail "A's \"from B\" doesn't hold just the tab A moved there: $(today)"
post rightclick $(row 0)
expect "^browser: menu shown .*$(cased 'Archive Tab')" "A's Today tab's menu didn't offer Archive Tab"
archived=$(grep '^browser: archive [0-9]* tabs$' "$b_log" | tail -1 | cut -d' ' -f3)
send choose "$(cased 'Archive Tab')"
changed=$(now_ms)
b_gets "browser: synced close $b_tab\$" 0 "$changed" "A's archiving its tab didn't close B's"
for _ in $(seq 50); do
  [ "$(grep '^browser: archive [0-9]* tabs$' "$b_log" | tail -1 | cut -d' ' -f3)" -gt "${archived:-0}" ] && break
  sleep 0.1
done
[ "$(grep '^browser: archive [0-9]* tabs$' "$b_log" | tail -1 | cut -d' ' -f3)" -gt "${archived:-0}" ] ||
  fail "the tab A archived didn't go to B's archive"
grep -q "^browser: loaded $b_tab " "$b_log" && fail "B loaded the Today tab sync brought it"

# A busy run on A: ten pages opened at once, as links are, ten navigations and ten closes. Its
# commits, which the server counts against the account's minute, are counted, and none is
# throttled.
# Each commit locks the account's store once, and each item it keeps takes the store's next version.
commits() {
  grep -c 'UPDATE sync_stores SET version = version WHERE' "$server_log" || true
}
kept() {
  grep -c 'UPDATE sync_stores SET version = version + 1' "$server_log" || true
}
sent_lines() {
  grep -c 'browser: sync spaces sent ' "$log" || true
}
busy_commits=$(commits)
busy_kept=$(kept)
busy_sent=$(sent_lines)
busy_started=$(now_ms)
for i in 1 2 3 4 5 6 7 8 9 10; do
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open A's command bar for busy tab $i"
  send insert "${site}login.html?busy-$i"
  send key return
done
expect "^browser: loaded [0-9]* ${site}login.html?busy-10\$" "A's tenth busy tab didn't load"
for i in 1 2 3 4 5 6 7 8 9 10; do
  go_from_bar "${site#http://}login.html?busy-10-$i"
  expect "^browser: loaded [0-9]* ${site}login.html?busy-10-$i\$" "A's busy navigation $i didn't load"
done
for i in 1 2 3 4 5 6 7 8 9 10; do
  send key w cmd
done
for _ in $(seq 50); do
  [ -z "$(today)" ] && break
  sleep 0.1
done
[ -z "$(today)" ] || fail "⌘W ten times left A's \"from B\" with $(today)"
# The last close's look and commit.
sleep 2
busy_ms=$(($(now_ms) - busy_started))
busy_commits=$(($(commits) - busy_commits))
busy_kept=$(($(kept) - busy_kept))
busy_looks=$(($(sent_lines) - busy_sent))
busy_items=$(grep 'browser: sync spaces sent ' "$log" | tail -n "$busy_looks" |
  awk '{ n += $(NF - 4) + $(NF - 2); b += $NF } END { printf "%d items, %d bytes", n, b }')
grep -q "throttled an account's commit" "$server_log" &&
  fail "the server throttled a commit of the account's: $(grep "throttled an account's commit" "$server_log" | head -1)"

# A's open tabs reach B, as the account's other computer's, but a tab of an agent's Space never
# does, as an agent's Space stays on its device (#1796): B's sync store gets the user's tab A
# opened, and never the agent's, though A's agent opened it first. The user's is a Little ilo
# window's, as a Space's tabs sync as the Space's own (#2098), which the command bar leaves out.
device a
send agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "A's driver didn't make the agent's Space"
send agentopen claude-code "${site}login.html?agents-own"
expect '^browser: agent claude-code opened [1-9][0-9]*$' "A's agent didn't open a tab in its Space"
agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
await "^browser: loaded $agent_tab ${site}login.html?agents-own\$" || fail "A's agent's tab didn't load"
forward
send key n cmd alt
expect '^browser: little window opened [0-9]* ' "⌥⌘N didn't open a Little ilo window on A"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the Little ilo window's command bar"
send insert "${site}login.html?users-own"
send key return
expect "^browser: loaded [0-9]* ${site}login.html?users-own\$" "A's Little ilo tab didn't load"
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
expect "^browser: loaded [0-9]* ${site}login.html\$" "A's login page didn't load"
# By the page it loaded: a Today tab sync brings, such as the one B opened, says its title too.
tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}login.html\$|\1|p" "$log" | tail -1)
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
# Settings asks in an alert, the Mac's or Windows' task dialog, and on Linux, which has none, in its
# dialog, as passwords' does.
asks=alert
[ "$(uname)" != Linux ] || asks=dialog
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
clipboard
# The Mac's host logs when its clear ran and what it did; none yet means it hadn't run.
cleared=$(grep '^browser: secret clear ' "$log" | tail -1)
[ -z "$clipped" ] ||
  fail "the clipboard still held the words 90 s after A copied them${cleared:+ ($cleared)}"

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
# Passwords sync, as the login above showed, but not another user's invitation to share one. No
# device asks the server for those, so a server's can't add a login (engine/patches/ours/
# chromium-sync-no-incoming-password-sharing.patch).
case ", $types," in *", Passwords,"*) ;; *) fail "A's sync carries no passwords: $types" ;; esac
if engine_has ../engine/patches/ours/chromium-sync-no-incoming-password-sharing.patch; then
  case $types in
  *"Incoming Password Sharing Invitations"*) fail "A's sync takes another user's passwords: $types" ;;
  esac
else
  echo "skip: sync-devices' shared passwords. The engine was built before sync stopped taking them."
fi
# The runs after start signed out.
rm -f "$profile_dir/Account"
kill "$ilo_server" 2>/dev/null || true
ilo_server=
echo "ok: A added B, C recovered with the copied words, which the clipboard let go of after 90 s," \
  "a Today tab A opened, navigated, moved and archived did the same on B, unloaded, in ms:$took_all;" \
  "a busy run of 10 opens, 10 navigations and 10 closes on A in $busy_ms ms looked $busy_looks times," \
  "sent $busy_items, and the account's devices made $busy_commits commits of $busy_kept items, none throttled;" \
  "A's Little ilo tab reached B, whose command bar listed and opened it, and its agent's never did," \
  "a login A saved reached B, whose removal reached A," \
  "and B and C signed out, on a local ilo-server as $email. A synced, with no other user's" \
  "invitations: $types." \
  "Logs: $out/sync-devices-*.log"
