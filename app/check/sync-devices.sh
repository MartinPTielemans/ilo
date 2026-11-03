# An account's devices, each a copy of the app with a profile of its own, on an ilo-server of the
# run's own (ilo-server.sh). A signs in first and copies its recovery words, which the clipboard
# holds marked for clipboard managers to skip (#641). B signs in and asks the account's devices
# to add it, and A, once its window is active, asks the user, who adds it with a click: B syncs.
# C signs in and recovers with the words instead, and syncs too. 90 s after A's copy the
# clipboard holds the words no longer. B and C sign out without asking, as the account has other
# devices, and the server forgets them. Linux's alone so far: the Mac's and Windows' clipboards
# and two copies of the app at once there are unrun.

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-devices. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  return 0
fi
if [ "$(uname)" != Linux ] || [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ]; then
  echo "skip: sync-devices. It runs under Xvfb on Linux so far, where xdotool and xclip reach it."
  return 0
fi
rm -f "$saved" "$launch_frame" "$profile_dir/Account"
. check/ilo-server.sh

# The device the helpers drive from now, $1: its process, log, driver and step.
on=
device() {
  case $on in
  a) a_step=$step ;;
  b) b_step=$step ;;
  c) c_step=$step ;;
  esac
  on=$1
  eval "pid=\$${1}_pid log=\$${1}_log step=\$${1}_step"
  case $1 in
  a) exec 3>&5 ;;
  b) exec 3>&6 ;;
  c) exec 3>&7 ;;
  esac
}
# Starts device $1, in the profile directory $2, on driver fd $3, and drives it.
start() {
  eval "${1}_log=\$out/sync-devices-$1.log ${1}_step=0"
  mkdir -p "$2"
  mkfifo "$out/drive-$1"
  BROWSER_DRIVE=1 BROWSER_URL="data:text/html,<title>device $1</title>" BROWSER_PROFILE_DIR="$2" \
    "$browser" --sync-url="$server_url" < "$out/drive-$1" > "$out/sync-devices-$1.log" 2>&1 \
    3>&- 5>&- 6>&- 7>&- &
  eval "${1}_pid=\$!"
  others="$others $!"
  eval "exec $3> \"\$out/drive-$1\""
  device "$1"
  await_load
}
# Shows Settings › Sync and starts signing in.
open_sign_in() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings on $on"
  send clickkey $(drawn "toolbar Sync")
  expect '^browser: settings tab Sync$' "a click on Sync didn't show its tab on $on"
  at=$(drawn "sync sign-in") || fail "Settings › Sync didn't draw Sign In on $on"
  send clickkey $at
  expect '^browser: account SigningIn None$' "Sign In didn't start signing in on $on"
  await 'browser: sync sign-in loaded /authorize$' ||
    fail "the sign-in window didn't load the server's page on $on"
}
# What the X server's clipboard holds, as target $1 (text if none).
clipboard() {
  timeout 5 xclip -selection clipboard -o ${1:+-t "$1"} 2>/dev/null || true
}
others=

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
clipboard TARGETS | grep -qx 'x-kde-passwordManagerHint' ||
  fail "the copied words aren't marked for clipboard managers to skip: $(clipboard TARGETS | tr '\n' ' ')"
at=$(drawn "sync done") || fail "Settings › Sync didn't draw Done with the words"
send clickkey $at
await 'browser: sync service active$' || fail "A's sync didn't become active"

start b "$PWD/$out/sync-devices-b" 6
open_sign_in
sign_in
await '^browser: account NeedsSecret None$' || fail "B, signed in, didn't need the account's secret"
await '^browser: account Pairing None$' || fail "B didn't ask the account's devices to add it"
../tools/window-screenshot.sh "$pid" "$out/sync-devices-b-pairing.png" || fail "no window to capture"
# A takes the request once one of its windows is active, as it would be in front of the user.
# A may have been the active app already, so these are read from all A has logged.
device a
send window 1
await '^browser: pairing asked$' || fail "A didn't take B's request with its window active"
await '^browser: dialog shown Add the device that asked at ' || fail "A didn't ask whether to add B"
await '^browser: dialog confirm at ' || fail "A's question didn't say where Add is"
../tools/window-screenshot.sh "$pid" "$out/sync-devices-a-asked.png" || fail "no window to capture"
add=$(sed -n 's/^browser: dialog confirm at //p' "$log" | tail -1)
send click $add
expect '^browser: dialog confirmed$' "a click on Add didn't add B"
device b
await '^browser: account On None$' || fail "B, added by A, didn't turn sync on"
await 'browser: sync service active$' || fail "B's sync didn't become active"

start c "$PWD/$out/sync-devices-c" 7
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
[ "$(stored "SELECT count(*) FROM devices")" = 3 ] ||
  fail "the server doesn't know the account's three devices: $(stored "SELECT group_concat(name, '|') FROM devices")"

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
  kill -INT "$pid"
  wait "$pid" || fail "$d exited with status $?"
done
exec 6>&- 7>&-
others=
device a
exec 5>&- 3>&-
quit
# The runs after start signed out.
rm -f "$profile_dir/Account"
kill "$ilo_server" 2>/dev/null || true
ilo_server=
echo "ok: A added B, C recovered with the copied words, which the clipboard let go of after 90 s," \
  "and B and C signed out, on a local ilo-server as $email. Logs: $out/sync-devices-*.log"
