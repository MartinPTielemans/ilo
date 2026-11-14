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
# - Ctrl+T's command bar opens over the composing page, which commits what it composed. Under
#   Wayland the bar then composes "xyz", which it draws; Escape cancels it and leaves the bar open,
#   taking the next key, "q"; composing again and Enter commits "qxyz" without opening anything,
#   and the bar takes the next key, "w". Under X11 those steps are skipped, as below.
# - Under Wayland, a click in the page while the bar composes leaves no composition behind: the
#   page composes "fg" on its own.
# - On X11, Settings' search field (Ctrl+, then Ctrl+F) composes "pip", which it draws and
#   searches for only once Enter commits it; Escape cancels a composition there and leaves the
#   search, and the next Escape empties it. Under Wayland Settings' window takes no input method,
#   which the run says.
# - With fcitx5-hangul, Korean through its 2-set Hangul engine (#2457), where one key can commit a
#   syllable and start the next: the page's textarea composes 한글, backspace taking a letter off
#   the syllable composing, and 안녕하세요, a digit committing each; under Wayland the bar composes
#   안녕하세요; on X11 Settings' search hears 한 committed before ㄱ composes, over XIM.
# - no-keyboard: on Wayland with no keyboard on the seat, where activating the input method makes
#   fcitx5's virtual keyboard the seat's first, with no keymap, the app lives on (#1138), and after
#   another keyboard comes and goes.
# Under Wayland the bar composes through the core's input method (bc_window_ui_text_input), so its
# steps need an engine with that core. Leaves the captures of each composition and the logs in
# build/check/ime/.
# Needs sway, grim, ImageMagick, fcitx5, dbus-run-session, xkbcli and cargo, which builds check/vkbd,
# and fcitx5-hangul for the Korean steps, which are skipped without it.
set -euo pipefail
cd "$(dirname "$0")/.."

# A process's PID and start time, which tell it from a later one given the same PID.
started() { awk -v p="$1" '{ sub(/.*\) /, ""); print p, $20 }' "/proc/$1/stat" 2>/dev/null; }

if [ "${1:-}" = --steps ]; then
  # Inside the session (headless.sh --then), with its displays and the app's log in $LOG.
  mode=$2 out=$3
  : > "$out/$mode.failed"
  # The leg's browser's processes, for the run to name any that outlive the leg: its own session's
  # (run.sh wrote its PID, which headless.sh's setsid made the session's), and its crash handler,
  # which Crashpad starts in a session of its own, by the leg's profile it's given. By name, as the
  # sandbox makes the helpers' executables unreadable; fcitx5 and vkbd share the session.
  track() {
    local sid p
    sid=$(cat "$out/$mode.pid")
    for p in $(ps -e -o pid= -o sid= | awk -v s="$sid" '$2 == s { print $1 }') \
      $(pgrep -f -- "$BROWSER_PROFILE_DIR"); do
      case $(cat "/proc/$p/comm" 2>/dev/null) in chrome*) started "$p" || true ;; esac
    done >> "$out/$mode.pids"
  }
  hold=
  trap 'track; [ -z "$hold" ] || kill "$hold" 2>/dev/null || true' EXIT
  track
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
    kill -0 "$(cat "$out/$mode.pid")" 2>/dev/null || fail "the app quit $2"
    ! tail -n "+$(($1 + 1))" "$LOG" | grep -qE 'Received signal|panicked' || fail "the app crashed $2"
  }
  click() { "$VKBD" --click "$1" "$2"; }
  shot() {
    sleep 0.3 # for sway to show the frame the app logged drawing
    grim "$out/$mode-$1.png"
  }
  key() { "$VKBD" "$1"; }
  compose() { key '^;'; key "$1"; }
  # Under X11 the seat's keyboard for the run is this one, as run.sh has no Wayland display to
  # make it on: with none left the app gets a keyboard leave, which closes an overlay.
  if [ "$mode" = x11 ]; then
    "$VKBD" --hold 120 &
    hold=$!
    sleep 0.5 # for the held keyboard to join the seat
  fi

  if [ "$mode" = no-keyboard ]; then
    at=$(mark); click 330 40
    sleep 2 # the 2 s the app crashed in once the input method's keyboard came
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
  at=$(mark); click 330 40
  await "title [0-9]* clicked" "$at"
  at=$(mark); compose abc
  await "title [0-9]* composing abc$" "$at"
  shot page-preedit
  at=$(mark); key $'\n'
  await "title [0-9]* value abc$" "$at"

  # Korean, through fcitx5's own Hangul engine on a 2-set keyboard (#2457), Ctrl+Space switching
  # to it and back: one key commits a syllable and starts the next, backspace takes a letter off
  # the syllable composing, and a digit commits it and goes on to the field.
  if [ -n "${HANGUL:-}" ]; then
    key '^ '
    at=$(mark); key gksr
    await "title [0-9]* composing abc한ㄱ$" "$at"
    at=$(mark); key mf
    await "title [0-9]* composing abc한글$" "$at"
    at=$(mark); key $'\b'
    await "title [0-9]* composing abc한그$" "$at"
    at=$(mark); key f1
    await "title [0-9]* value abc한글1$" "$at"
    at=$(mark); key dkssudgktpdy
    await "title [0-9]* composing abc한글1안녕하세요$" "$at"
    shot page-korean
    at=$(mark); key 2
    await "title [0-9]* value abc한글1안녕하세요2$" "$at"
    key '^ '
    # The steps after take the textarea as it was.
    at=$(mark); key '^a'; key $'\b'; key abc
    await "title [0-9]* value abc$" "$at"
  fi

  # The bar, with the page composing as it opens: the page keeps what it composed.
  at=$(mark); compose de
  await "title [0-9]* composing abcde$" "$at"
  at=$(mark); key '^t'
  await "title [0-9]* value abcde$" "$at"
  await "command bar shown" "$at"
  # Under X11 this leg runs in sway's Xwayland, and Ubuntu 24.04's sway is on wlroots 0.17. Its
  # window manager lets a client move the X focus only between windows of one process, and learns
  # a window's process only once the window has drawn. The bar's window takes the focus as it's
  # first shown, before then, so the focus goes back to the browser window, which the bar takes for
  # the user leaving, and closes. That happened to 7 of 30 launches' first bars on CI. It never
  # happened once the bar had been shown, or under Xvfb (#2365). wlroots 0.18 learns the process as
  # the window is made (wlroots 7c080c3b). So under X11 the leg checks only that the page kept what
  # it composed, until the runner's sway is on wlroots 0.18. check.sh's keys run composes in the
  # bar under Xvfb's X11, through the driver's input method.
  if [ "$mode" = x11 ]; then
    echo "skip (x11): composing in the command bar: wlroots 0.17's Xwayland sends the first bar's X focus back (#2365)" |
      tee "$out/$mode.skipped" >&2
    # Escape closes the bar if it's still open, and does nothing in the page if it isn't.
    key $'\e'
    await "command bar hidden" "$at"
  else
    at=$(mark); compose xyz
    await 'command bar drew "xyz"' "$at"
    shot bar-preedit
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
    shown=qxyzw

    # Korean in the bar, through text-input-v3, whose commit and next syllable come in one done: in
    # one burst, as the page takes it, the bar keeps every key. Before its caret waited for the
    # field's layout, the bar committed a syllable early and lost the next (#2473).
    if [ -n "${HANGUL:-}" ]; then
      key '^ '
      at=$(mark); key dkssudgktpdy
      await 'command bar drew "qxyzw안녕하세요"' "$at"
      shot bar-korean
      at=$(mark); key 1
      await 'command bar drew "qxyzw안녕하세요1"' "$at"
      key '^ '
      shown=qxyzw안녕하세요1
    fi

    # A click in the page while the bar composes.
    at=$(mark); compose zz
    await "command bar drew \"${shown}zz\"" "$at"
    clicked=$(mark); click 330 60
    await "title [0-9]* clicked" "$clicked"
    at=$(mark); compose fg
    await "title [0-9]* composing [a-z]*fg[a-z]*$" "$at"
    ! tail -n "+$((clicked + 1))" "$LOG" | grep -q 'command bar drew ".*zzz' || fail "the bar committed its composition twice"
    ! tail -n "+$((at + 1))" "$LOG" | grep -q "title [0-9]* composing .*zz" || fail "the bar's composition went to the page"
    key $'\e'
  fi

  # Settings' search (#1636): Ctrl+F gives its field the keyboard, the composition shows in the
  # field and finds nothing until it commits, then the word committed finds its rows. Escape while
  # composing cancels the composition and leaves the search. Under Wayland Settings' window takes
  # no input method: only the browser windows compose, through the core's, so its fields get the
  # keys plainly there.
  if [ "$mode" = wayland ]; then
    echo "skip (wayland): Settings' window takes no input method under Wayland, only the browser windows do" |
      tee "$out/$mode.skipped" >&2
    mv "$out/$mode.failed" "$out/$mode.passed"
    exit 0
  fi
  at=$(mark); key '^,'
  await "settings drew " "$at"
  key '^f'
  at=$(mark); compose pip
  await 'settings search composing "pip"' "$at"
  shot settings-preedit
  ! tail -n "+$((at + 1))" "$LOG" | grep -q 'settings search "pip" finds' || fail "Settings searched for the composition"
  at=$(mark); key $'\n'
  await 'settings search "pip" finds Mini Player$' "$at"
  shot settings-committed
  at=$(mark); compose xyz
  await 'settings search composing "pipxyz"' "$at"
  at=$(mark); key $'\e'
  await 'settings search "pip" finds Mini Player$' "$at"
  ! tail -n "+$((at + 1))" "$LOG" | grep -q 'settings search cleared' || fail "Escape while composing ended the search"
  at=$(mark); key $'\e'
  await 'settings search cleared' "$at"

  # Korean in Settings' search, over XIM, where a syllable's commit and the next syllable come as
  # two messages: the commit must reach the field before the next composes, or the search at the
  # end isn't for "한글1". Both can come before one frame, so the search may never log 한 alone.
  # Escape left the field, so Ctrl+F gives it the keys again.
  if [ -n "${HANGUL:-}" ]; then
    key '^f'
    key '^ '
    at=$(mark); key gksr
    await 'settings search composing "한ㄱ"' "$at"
    at=$(mark); key mf
    await 'settings search composing "한글"' "$at"
    at=$(mark); key $'\b'
    await 'settings search composing "한그"' "$at"
    at=$(mark); key f1
    await 'settings search "한글1" finds' "$at"
    shot settings-korean
    key '^ '
  fi
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
# A helper Chromium leaves can still be writing the profile as this runs, and under set -e a failed
# rm would turn the runs' passes into a failure.
trap 'rm -rf "$home" || true' EXIT
# fcitx5's own configuration: the US keyboard, quick phrase on Ctrl+;, and on X11 the composition
# drawn by the app (on-the-spot XIM), not by fcitx5 over it.
mkdir -p "$home/config/fcitx5/conf"
# With fcitx5-hangul, Korean is the group's second input method, Ctrl+Space switching to it.
HANGUL=
[ -f /usr/share/fcitx5/addon/hangul.conf ] && HANGUL=1
export HANGUL
[ -n "$HANGUL" ] || echo "skip: Korean: no fcitx5-hangul" >&2
cat > "$home/config/fcitx5/profile" <<EOF
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=keyboard-us

[Groups/0/Items/0]
Name=keyboard-us

$([ -z "$HANGUL" ] || printf '[Groups/0/Items/1]\nName=hangul\n')

[GroupOrder]
0=Default
EOF
printf 'Choose Modifier=None\nSpell=False\n\n[TriggerKey]\n0=Control+semicolon\n' \
  > "$home/config/fcitx5/conf/quickphrase.conf"
printf '[Hotkey/TriggerKeys]\n0=Control+space\n' > "$home/config/fcitx5/config"
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
echo \$\$ > "$out/\$1.pid"
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
t.addEventListener("pointerdown",()=>{document.title="clicked "+t.value});
</script>'
url="data:text/html,$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$page")"
failed=
for mode in $modes; do
  x11=; [ "$mode" = x11 ] && x11=--x11
  # Each leg its own profile, as the last one's browser may not be gone yet.
  dbus-run-session --config-file="$home/bus.conf" -- \
    env BROWSER_PROFILE_DIR="$home/profile-$mode" BROWSER_URL="$url" \
    ../tools/headless.sh $x11 --then "$PWD/check/ime.sh --steps $mode $out" \
    "$out/$mode.png" 'browser: loaded' "$home/run.sh" "$mode" build/Browser/chrome \
    > "$out/$mode-run.log" 2>&1 || true
  # Any of the leg's browser's processes still there now that the leg is over, by the PIDs it
  # tracked, with how long each has run. One that has exited and waits only to be reaped (Z) can
  # write nothing, so it isn't named.
  if [ -s "$out/$mode.pids" ]; then
    sort -u "$out/$mode.pids" | while read -r p start; do
      [ "$(started "$p")" = "$p $start" ] || continue
      ps -o stat=,pid=,ppid=,sid=,etimes=,args= -p "$p" | awk -v m="$mode" '$1 !~ /^Z/ {
        printf "still running after the %s leg: pid %s, parent %s, session %s, up %s s: ", m, $2, $3, $4, $5
        $1 = $2 = $3 = $4 = $5 = ""; sub(/^ +/, ""); print substr($0, 1, 300) }' || true
    done | tee "$out/$mode.alive"
  fi
  if [ ! -f "$out/$mode.passed" ]; then
    failed=1
    cat "$out/$mode.failed" 2>/dev/null || echo "FAIL ($mode): see $out/$mode-run.log" >&2
  else
    echo "ok: $mode"
  fi
done
[ -z "$failed" ]
