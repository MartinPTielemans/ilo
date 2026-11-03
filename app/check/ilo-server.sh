# An ilo-server of the run's own, for the runs that sign in: SQLite, DEV_MODE, which logs its
# sign-in codes, and a throwaway address, never the hosted server or a real account. ILO_SERVER
# names the server's binary, built from its repo (check-linux.yml builds it); a run checks for it
# before it sources this. The server is ilo_server, so cleanup stops it after a failure.

db="$out/$run-server.db"
server_log="$out/$run-server.log"
rm -f "$db"
# One answer from the server's database: the query $1.
stored() {
  python3 -I -c '
import sqlite3, sys
print(sqlite3.connect(sys.argv[1]).execute(sys.argv[2]).fetchone()[0])' "$db" "$1"
}
server_port=$(python3 -I -c '
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])')
# localhost, as passkeys, which the server offers, take no address for their origin.
server_url="http://localhost:$server_port"
DEV_MODE=1 HOST=127.0.0.1 PORT=$server_port PUBLIC_URL=$server_url NO_COLOR=1 \
  DATABASE_URL="sqlite:$db?mode=rwc" "$ILO_SERVER" > "$server_log" 2>&1 3>&- &
ilo_server=$!
for _ in $(seq 100); do
  grep -q 'ilo-server is up' "$server_log" && break
  kill -0 "$ilo_server" 2>/dev/null || fail "ilo-server exited: $(tail -1 "$server_log")"
  sleep 0.1
done
grep -q 'ilo-server is up' "$server_log" || fail "ilo-server didn't start"
email="check-$(date +%s)@example.org"
# What the core calls this device to the server and the account's other devices.
label="ilo on Linux"
[ "$(uname)" = Darwin ] && label="ilo on Mac"
! windows || label="ilo on Windows"

# Keys for the sign-in window, a window of Chromium's own, which the driver's keys don't reach on
# Linux, as they go to the browser windows, overlays and Settings alone: there they're the X
# server's, from xdotool, to the app's sign-in window, which it gives the keyboard first. The Mac's
# go through the driver (unrun there).
sign_in_window=
sign_in_keys() {
  if [ "$(uname)" != Linux ]; then
    send "$@"
    return
  fi
  if [ -z "$sign_in_window" ]; then
    sign_in_window=$(timeout 10 xdotool search --sync --all --onlyvisible --pid "$pid" \
      --name '^localhost$' | head -1)
    [ -n "$sign_in_window" ] || fail "the X server has no sign-in window"
  fi
  xdotool windowactivate --sync "$sign_in_window" 2>/dev/null ||
    xdotool windowfocus --sync "$sign_in_window" ||
    fail "the sign-in window couldn't have the keyboard"
  case $1 in
  type) xdotool type --delay 20 "$2" ;;
  key) xdotool key "${3:+$3+}$(echo "$2" | sed 's/return/Return/; s/tab/Tab/')" ;;
  esac
}
# The sign-in codes the server has logged, so that each sign-in takes its own.
codes=0
# Signs in as $email in the sign-in window Sign In opened, with the code the server logs. The
# account's first sign-in ($1 first or passkey) offers a passkey, with Add a Passkey focused, which
# Not Now, next, declines, or with passkey is left for the run to add. $2, if given, is where to
# capture the window with the address typed.
sign_in() {
  sign_in_window=
  # The page focuses its field once it's there.
  sign_in_keys type "$email"
  [ -z "${2:-}" ] || [ -z "$sign_in_window" ] || import -window "$sign_in_window" "$2" || true
  sign_in_keys key return
  await 'browser: sync sign-in loaded /authorize/email$' || fail "the server didn't take the address"
  sent=$codes
  for _ in $(seq 100); do
    sent=$(grep -c ' is your ilo sign-in code' "$server_log" || true)
    [ "$sent" -gt "$codes" ] && break
    sleep 0.1
  done
  [ "$sent" -gt "$codes" ] || fail "ilo-server logged no sign-in code"
  codes=$sent
  code=$(grep -o '[0-9]\{6\} is your ilo sign-in code' "$server_log" | tail -1 | cut -c1-6)
  sign_in_keys type "$code"
  sign_in_keys key return
  [ "${1:-}" = first ] || [ "${1:-}" = passkey ] || return 0
  await 'browser: sync sign-in loaded /authorize/code$' || fail "the server didn't take the code"
  [ "$1" = first ] || return 0
  sign_in_keys key tab
  sign_in_keys key return
}
# Once the sign-in window has closed, Settings has the keyboard again: on Linux as the window
# manager would give it, from xdotool, as Settings' window is titled by its tab; the Mac's AppKit
# gives it.
settings_keyboard() {
  [ "$(uname)" = Linux ] || return 0
  settings=$(timeout 10 xdotool search --all --onlyvisible --pid "$pid" --name '^Sync$' | head -1)
  [ -n "$settings" ] || fail "the X server has no Settings window"
  xdotool windowactivate --sync "$settings" 2>/dev/null || xdotool windowfocus --sync "$settings" ||
    fail "Settings couldn't have the keyboard again"
}
