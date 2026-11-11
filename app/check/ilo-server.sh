# An ilo-server of the run's own, for the runs that sign in: SQLite, DEV_MODE, which logs its
# sign-in codes, and a throwaway address, never the hosted server or a real account. ILO_SERVER
# names the server's binary, built from its repo (check-linux.yml, check-windows.yml and
# check-mac.yml build it); a run checks for it before it sources this. The server is ilo_server, so cleanup stops it after a failure.

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
# The server is a native program on Windows, which reads the database's path as Windows names it:
# Git Bash converts a path it's given alone, never one inside a URL.
db_path=$db
! windows || db_path=$(cygpath -m "$db")
# Each query logged, so a run can count the commits it sent (sync-devices).
DEV_MODE=1 HOST=127.0.0.1 PORT=$server_port PUBLIC_URL=$server_url NO_COLOR=1 LOG=info,sqlx::query=debug \
  DATABASE_URL="sqlite:$db_path?mode=rwc" "$ILO_SERVER" > "$server_log" 2>&1 3>&- &
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
# go through the driver (unrun there), as do Wayland's, whose keys go to the window sway focuses,
# the sign-in window as it opens.
sign_in_window=
sign_in_keys() {
  if [ "$(uname)" != Linux ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
    send "$@"
    return
  fi
  if [ -z "$sign_in_window" ]; then
    sign_in_window=$(timeout 10 xdotool search --sync --all --onlyvisible --pid "$(window_pid)" \
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
# manager would give it, from xdotool or sway, as Settings' window is titled by its tab; the Mac's
# AppKit gives it. On Windows and under Wayland the device's Settings takes the keyboard from
# another copy's window, as a click on it would (#2114). On the Mac, with other copies running, the
# device's copy is made the active app, and Settings… brings its Settings, GPUI's window, in front
# of its browser window.
settings_keyboard() {
  if [ "$(uname)" = Darwin ]; then
    [ -n "${others:-}" ] || return 0
    take_activation
    case $done in *" key GPUIWindow,"*) ;; *) send menu Settings… ;; esac
    return 0
  fi
  if windows; then
    python3 -I check/app-windows.py foreground "$(cat "/proc/$pid/winpid")" Sync > /dev/null ||
      fail "Settings couldn't have the keyboard again"
    return 0
  fi
  if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    swaymsg "[pid=$pid title=\"^Sync\$\"] focus" > /dev/null ||
      fail "sway couldn't give $on's Settings the keyboard"
    return 0
  fi
  [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ] || return 0
  settings=$(timeout 10 xdotool search --all --onlyvisible --pid "$(window_pid)" --name '^Sync$' | head -1)
  [ -n "$settings" ] || fail "the X server has no Settings window"
  xdotool windowactivate --sync "$settings" 2>/dev/null || xdotool windowfocus --sync "$settings" ||
    fail "Settings couldn't have the keyboard again"
}

# Sets clipped to what the clipboard holds, the X server's, the Wayland session's or Windows': its
# text, or with formats the formats it holds. On the Mac the driver gives each copy a pasteboard of
# its own in its place, which nothing else reads, so it's the device's, as its driver logs it. A
# variable, not output, since the Mac's asks the driver, whose steps a subshell would lose count of.
clipboard() {
  if windows; then
    clipped=$(python3 -I check/clipboard-windows.py "${1:-text}" 2>/dev/null || true)
  elif [ "$(uname)" = Darwin ]; then
    send pasteboard
    held=text
    [ -z "${1:-}" ] || held=types
    clipped=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
      sed -n "s/^browser: pasteboard $held //p" | tail -1)
    [ "$held" = text ] || clipped=$(echo "$clipped" | tr ' ' '\n')
  elif [ -n "${WAYLAND_DISPLAY:-}" ]; then
    clipped=$(timeout 5 wl-paste -n ${1:+-l} 2>/dev/null || true)
  else
    clipped=$(timeout 5 xclip -selection clipboard -o ${1:+-t TARGETS} 2>/dev/null || true)
  fi
}
# The device's first browser window in front with the keyboard, as a click on it gives it. On
# Windows the driver's window activates a window within its own copy alone, so the copy's window
# takes the foreground from the others' too. Under Wayland a window comes forward only for the
# copy sway has focused, so the copy is focused first, as a click in it would. On the Mac each copy
# activates itself only as it starts, and no background process can make one active, so the
# active copy hands its activation over, as a click in the other's window would take it.
forward() {
  if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    swaymsg "[pid=$pid] focus" > /dev/null || fail "sway couldn't focus $on's window"
  fi
  [ "$(uname)" != Darwin ] || take_activation
  send window 1
  ! windows || python3 -I check/app-windows.py foreground "$(cat "/proc/$pid/winpid")" > /dev/null ||
    fail "$on's window couldn't take the foreground"
}
# On the Mac, the device's copy made the active app: each other running copy yields to it, as only
# the active one can, and the device waits up to 5 s to be active.
take_activation() {
  to=$on
  to_pid=$pid
  for d in a b c; do
    eval "d_pid=\${${d}_pid:-}"
    [ "$d" != "$to" ] && [ -n "$d_pid" ] && kill -0 "$d_pid" 2>/dev/null || continue
    device "$d"
    send yield "$to_pid"
  done
  device "$to"
  for _ in $(seq 50); do
    send state
    [ "$active" = 1 ] && return 0
    sleep 0.1
  done
  fail "$to didn't become the active app as the others yielded to it"
}

# The runs with several devices, each a copy of the app with a profile of its own: a, b and c, on
# driver fds 5, 6 and 7, each started by start. The copies are in others, which cleanup stops.
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
# Starts device $1, in the profile directory $2, on driver fd $3, and drives it, on the page
# $device_url if set, with the switches $device_args.
start() {
  eval "${1}_log=\$out/$run-$1.log ${1}_step=0"
  mkdir -p "$2"
  mkfifo "$out/drive-$1"
  # shellcheck disable=SC2086
  BROWSER_DRIVE=1 BROWSER_URL="${device_url:-data:text/html,<title>device $1</title>}" \
    BROWSER_PROFILE_DIR="$2" "$browser" --sync-url="$server_url" ${device_args:-} \
    < "$out/drive-$1" > "$out/$run-$1.log" 2>&1 3>&- 5>&- 6>&- 7>&- &
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
