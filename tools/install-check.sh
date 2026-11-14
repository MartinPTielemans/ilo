#!/bin/bash
# The native install end to end, as a user runs it (#2453), on a machine or in a container with what
# a desktop has, as an ordinary user. It runs the line against <release url>, starts ilo from its
# launcher on chrome://sandbox in Xvfb with the sandbox as it comes, and reads each renderer's
# sandbox from /proc: seccomp-bpf (Seccomp: 2) and user and PID namespaces of its own, which are
# chrome://sandbox's layers. Then the line again, which finds it up to date, and --uninstall, which
# keeps the profile. Leaves the logs and a capture in <out>.
# Usage: tools/install-check.sh <release url> <out directory>
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: install-check.sh <release url> <out directory>" >&2 && exit 2; }
export ILO_RELEASE_URL=$1
out=$2
mkdir -p "$out"
app=$HOME/.local/share/ilo
bin=$HOME/.local/bin/ilo
fail() {
  echo "install-check: $*" >&2
  exit 1
}
line() { curl -fsSL "$ILO_RELEASE_URL/install.sh" | sh -s -- "$@"; }

# shellcheck disable=SC1091 # The machine's own.
echo "$(. /etc/os-release && echo "$PRETTY_NAME"), $(getconf GNU_LIBC_VERSION)"
line | tee "$out/install.log"
grep -q 'ilo is installed' "$out/install.log" || fail "the line didn't install ilo"
for file in "$app/chrome" "$app/libBrowser.so" "$bin" "$HOME/.local/share/applications/ilo.desktop" \
  "$HOME/.local/share/icons/hicolor/scalable/apps/ilo.svg"; do
  [ -e "$file" ] || fail "no $file"
done
grep -qx "Exec=\"$bin\" %U" "$HOME/.local/share/applications/ilo.desktop" ||
  fail "the desktop entry doesn't start the launcher"

display=:$((90 + RANDOM % 9))
Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X${display#:}" ] && break; sleep 0.1; done
DISPLAY=$display dbus-run-session -- "$bin" chrome://sandbox >"$out/ilo.log" 2>&1 &
ilo=$!
for _ in $(seq 120); do
  grep -q 'browser: UI shown' "$out/ilo.log" && break
  kill -0 $ilo 2>/dev/null || fail "ilo quit as it started: $(grep -m1 FATAL "$out/ilo.log" || tail -n3 "$out/ilo.log")"
  sleep 1
done
grep -q 'browser: UI shown' "$out/ilo.log" || fail "ilo's window didn't show in 120 s"
grep -q 'browser: updated by install.sh' "$out/ilo.log" ||
  fail "ilo didn't take install.sh for what updates it (Settings › About)"
browser=$(pgrep -o -f "^$app/chrome")
renderers=
for _ in $(seq 30); do
  renderers=$(pgrep -f -- "^$app/chrome --type=renderer" || true)
  [ -n "$renderers" ] && break
  sleep 1
done
[ -n "$renderers" ] || fail "no renderer started"
for pid in $renderers; do
  seccomp=$(sed -n 's/^Seccomp:\s*//p' "/proc/$pid/status")
  [ "$seccomp" = 2 ] || fail "renderer $pid has no seccomp-bpf filter (Seccomp: $seccomp)"
  for ns in user pid; do
    [ "$(readlink "/proc/$pid/ns/$ns")" != "$(readlink "/proc/$browser/ns/$ns")" ] ||
      fail "renderer $pid shares the browser's $ns namespace"
  done
  echo "renderer $pid: seccomp-bpf, its own user and PID namespaces"
done
! command -v import >/dev/null || DISPLAY=$display import -window root "$out/started.png" || true
! line --uninstall >"$out/running.log" 2>&1 || fail "--uninstall went ahead while ilo ran"
grep -q 'Quit ilo first' "$out/running.log" || fail "--uninstall didn't say to quit ilo"
kill "$browser"
for _ in $(seq 40); do pgrep -f "^$app/chrome" >/dev/null || break; sleep 0.5; done
! pgrep -f "^$app/chrome" >/dev/null || fail "ilo didn't quit"
wait $ilo || true

line | tee "$out/again.log"
grep -q 'ilo is up to date' "$out/again.log" || fail "the line again didn't find ilo up to date"
line --uninstall | tee "$out/uninstall.log"
for file in "$app" "$bin" "$HOME/.local/share/applications/ilo.desktop"; do
  [ ! -e "$file" ] || fail "--uninstall left $file"
done
[ -d "$HOME/.config/ilo" ] || fail "--uninstall took the profile"
echo "install-check: installed, sandboxed, up to date, removed"
