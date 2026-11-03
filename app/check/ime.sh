#!/bin/bash
# On Linux: typing through an input method, on Wayland and on X11 (sway's Xwayland), in the page's
# fields and the app's own. Not a check.sh run, which drives the Mac: run check/ime.sh, or
# check/ime.sh wayland, x11 or no-keyboard for one, after ./build.sh.
# Each runs the app in a headless sway session (tools/headless.sh) with an fcitx5 of its own, its
# configuration made here, in a D-Bus session that starts nothing else, so the user's input method
# and screens are untouched. fcitx5's quick phrase (Ctrl+;) composes what's typed inline, as
# Japanese input does, and commits it on Enter; Escape cancels it. check/vkbd types and clicks
# through the session's virtual keyboard and pointer, which a headless seat has no other of.
# It checks:
# - The page's textarea composes "abc" (its title follows its composition events and value), and
#   Enter commits it.
# - Ctrl+T's command bar, with what the page composed committed in the page, composes "xyz", which
#   it draws; Escape cancels it and leaves the bar open, taking the next key, "q"; composing again
#   and Enter commits "qxyz" without opening anything, and the bar takes the next key, "w".
# - A click in the page while the bar composes leaves no composition behind: the page composes
#   "fg" on its own.
# - no-keyboard: on Wayland with no keyboard on the seat, where activating the input method makes
#   fcitx5's virtual keyboard the seat's first, with no keymap, the app lives on (#1138), and after
#   another keyboard comes and goes.
# Under Wayland the bar composes through the core's input method (bc_window_ui_text_input), so its
# steps need an engine with that core. Leaves the captures of each composition and the logs in
# build/check/ime/.
# Needs sway, grim, ImageMagick, fcitx5, dbus-run-session, xkbcli and cargo, which builds check/vkbd.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "${1:-}" = --steps ]; then
  # Inside the session (headless.sh --then), with its displays and the app's log in $LOG.
  mode=$2 out=$3
  : > "$out/$mode.failed"
  fail() {
    echo "FAIL ($mode): $1" | tee -a "$out/$mode.failed" >&2
    exit 1
  }
  # Waits up to 10 s for a line matching $1 in the log after line $2.
  await() {
    for _ in $(seq 100); do
      tail -n "+$(($2 + 1))" "$LOG" | grep -q -- "$1" && return 0
      sleep 0.1
    done
    fail "no \"$1\" in the log"
  }
  mark() { wc -l < "$LOG"; }
  # The app's own process, not its helpers, is still there and logged no crash after line $1.
  alive() {
    local p up=
    for p in $(pgrep -x chrome); do
      grep -qa -- --type= "/proc/$p/cmdline" || up=1
    done
    [ -n "$up" ] || fail "the app quit $2"
    ! tail -n "+$(($1 + 1))" "$LOG" | grep -qE 'Received signal|panicked' || fail "the app crashed $2"
  }
  click() { "$VKBD" --click "$1" "$2"; }
  key() { "$VKBD" "$1"; }
  compose() { key '^;'; key "$1"; }
  # Under X11 the seat's keyboard for the run is this one, as run.sh has no Wayland display to
  # make it on: with none left the app gets a keyboard leave, which closes an overlay.
  if [ "$mode" = x11 ]; then
    "$VKBD" --hold 120 &
    hold=$!
    trap 'kill $hold 2>/dev/null || true' EXIT
    sleep 0.5
  fi

  if [ "$mode" = no-keyboard ]; then
    at=$(mark); click 330 40
    sleep 2
    alive "$at" "when the input method's keyboard came with no keymap"
    # The keyboard that types goes after, leaving the input method's, with no keymap, the seat's
    # only one again, which crashed the app up to 10 s later. The keys may not reach the page, as
    # the input method can pass them on through its keyboard, whose keys can't be read.
    key abc
    for _ in $(seq 20); do
      sleep 0.5
      alive "$at" "once the keyboard that typed went"
    done
    grim "$out/$mode-after.png"
    mv "$out/$mode.failed" "$out/$mode.passed"
    exit 0
  fi

  # The page's textarea.
  click 330 40
  sleep 0.5
  at=$(mark); compose abc
  await "title [0-9]* composing abc$" "$at"
  sleep 0.3; grim "$out/$mode-page-preedit.png"
  at=$(mark); key $'\n'
  await "title [0-9]* value abc$" "$at"

  # The bar, with the page composing as it opens: the page keeps what it composed.
  at=$(mark); compose de
  await "title [0-9]* composing abcde$" "$at"
  at=$(mark); key '^t'
  await "title [0-9]* value abcde$" "$at"
  await "command bar shown" "$at"
  at=$(mark); compose xyz
  await 'command bar drew "xyz"' "$at"
  sleep 0.3; grim "$out/$mode-bar-preedit.png"
  at=$(mark); key $'\e'
  await 'command bar drew ""' "$at"
  key q
  await 'command bar drew "q"' "$at"
  at=$(mark); compose xyz
  await 'command bar drew "qxyz"' "$at"
  key $'\n'
  key w
  await 'command bar drew "qxyzw"' "$at"
  ! tail -n "+$((at + 1))" "$LOG" | grep -q "command bar go" || fail "Enter while composing opened the URL"

  # A click in the page while the bar composes.
  at=$(mark); compose zz
  await 'command bar drew "qxyzwzz"' "$at"
  at=$(mark); click 330 60
  sleep 1
  ! tail -n "+$((at + 1))" "$LOG" | grep -q 'command bar drew ".*zzz' || fail "the bar committed its composition twice"
  at=$(mark); compose fg
  await "title [0-9]* composing [a-z]*fg[a-z]*$" "$at"
  ! tail -n "+$((at + 1))" "$LOG" | grep -q "title [0-9]* composing .*zz" || fail "the bar's composition went to the page"
  key $'\e'
  mv "$out/$mode.failed" "$out/$mode.passed"
  exit 0
fi

modes=${1:-"wayland x11 no-keyboard"}
for tool in sway grim magick fcitx5 dbus-run-session xkbcli cargo; do
  command -v "$tool" >/dev/null || { echo "FAIL: needs $tool" >&2; exit 1; }
done
[ -x build/Browser/chrome ] || { echo "FAIL: no build/Browser/chrome; run ./build.sh" >&2; exit 1; }
cargo build -q --release --manifest-path check/vkbd/Cargo.toml --target-dir target
export VKBD=$PWD/target/release/vkbd
out=$PWD/build/check/ime
rm -rf "$out"
mkdir -p "$out"
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
# fcitx5's own configuration: the US keyboard, quick phrase on Ctrl+;, and on X11 the composition
# drawn by the app (on-the-spot XIM), not by fcitx5 over it.
mkdir -p "$home/config/fcitx5/conf"
cat > "$home/config/fcitx5/profile" <<EOF
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=keyboard-us

[Groups/0/Items/0]
Name=keyboard-us

[GroupOrder]
0=Default
EOF
printf 'Choose Modifier=None\nSpell=False\n\n[TriggerKey]\n0=Control+semicolon\n' \
  > "$home/config/fcitx5/conf/quickphrase.conf"
echo UseOnTheSpot=True > "$home/config/fcitx5/conf/xim.conf"
# A bus that starts nothing on demand: what a desktop would start would get the user's displays.
cat > "$home/bus.conf" <<EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=$home</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF
# What headless.sh runs: fcitx5 on the session's displays, then the app, in its process group.
# Under Wayland, first a keyboard on the seat for the whole run, as a desk has: fcitx5's own virtual
# keyboard takes its keymap from it, and with none the app gets a keyboard with no keymap, whose keys
# it can't read (#1138), as the no-keyboard run checks. A seat with no keyboard left would also send
# the app a keyboard leave, which closes an overlay as the user leaving it.
cat > "$home/run.sh" <<EOF
#!/bin/bash
[ -z "\${WAYLAND_DISPLAY:-}" ] || [ "\$1" = no-keyboard ] || "$VKBD" --hold 600 &
env XDG_CONFIG_HOME=$home/config XDG_DATA_HOME=$home/data XDG_CACHE_HOME=$home/cache \\
  fcitx5 --disable=notificationitem,notifications,kimpanel,clipboard,fcitx4frontend \\
  > "$out/\$1-fcitx5.log" 2>&1 &
shift
for _ in \$(seq 100); do
  dbus-send --session --print-reply --dest=org.freedesktop.DBus / \\
    org.freedesktop.DBus.NameHasOwner string:org.fcitx.Fcitx5 2>/dev/null | grep -q true && break
  sleep 0.1
done
export XMODIFIERS=@im=fcitx GTK_IM_MODULE=fcitx QT_IM_MODULE=fcitx
exec "\$@"
EOF
chmod +x "$home/run.sh"
# A textarea whose title says what it holds and whether it composes.
page='<title>start</title><textarea id=t autofocus style="font-size:24px;width:400px;height:120px"></textarea><script>
const t=document.getElementById("t");let c=0;
function u(){document.title=(c?"composing ":"value ")+t.value}
t.addEventListener("compositionstart",()=>{c=1;u()});
t.addEventListener("compositionupdate",()=>setTimeout(u));
t.addEventListener("compositionend",()=>{c=0;setTimeout(u)});
t.addEventListener("input",()=>setTimeout(u));
</script>'
url="data:text/html,$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$page")"
failed=
for mode in $modes; do
  x11=; [ "$mode" = x11 ] && x11=--x11
  rm -rf "$home/profile"
  dbus-run-session --config-file="$home/bus.conf" -- \
    env BROWSER_PROFILE_DIR="$home/profile" BROWSER_URL="$url" \
    ../tools/headless.sh $x11 --then "$PWD/check/ime.sh --steps $mode $out" \
    "$out/$mode.png" 'browser: loaded' "$home/run.sh" "$mode" build/Browser/chrome \
    > "$out/$mode-run.log" 2>&1 || true
  if [ ! -f "$out/$mode.passed" ]; then
    failed=1
    cat "$out/$mode.failed" 2>/dev/null || echo "FAIL ($mode): see $out/$mode-run.log" >&2
  else
    echo "ok: $mode"
  fi
done
[ -z "$failed" ]
