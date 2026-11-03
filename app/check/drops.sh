#!/bin/bash
# On Linux under Wayland: drags from other apps reach the page and the UI, as pinned.sh and
# split.sh check on the Mac and X11, whose drivers drag through AppKit and XDND. Not a check.sh
# run, whose driver drives X11 alone: run check/drops.sh after ./build.sh.
# It runs the app in a headless sway session (tools/headless.sh), where check/drag, another app's
# window with a virtual pointer, which a headless seat has no other of, drags into it. It checks:
# - A link dragged over the page brings the split's drop targets: Chromium takes the drags over
#   its own surface.
# - Dragged on over the sidebar's list, which GPUI's surface hands the embedder
#   (take_embedder_drops), a line marks its place and the source hears it'd be taken. Let go
#   there, it opens as a tab.
# - Text that's a link but not a web link, let go over the list, isn't taken, and nothing opens.
#   Other text is searched for there.
# Leaves a capture of the link's line and the logs in build/check/drops/.
# Needs sway, grim, ImageMagick, jq and cargo, which builds check/drag.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "${1:-}" = --steps ]; then
  # Inside the session (headless.sh --run), on its Wayland display.
  out=$2
  : > "$out/failed"
  fail() {
    echo "FAIL: $1" | tee -a "$out/failed" >&2
    exit 1
  }
  # Waits up to 10 s for a line matching $1 in the file $3, the app's log by default, after line $2.
  await() {
    for _ in $(seq 100); do
      tail -n "+$(($2 + 1))" "${3:-$LOG}" | grep -q -- "$1" && return 0
      sleep 0.1
    done
    fail "no \"$1\" in ${3:-the log}"
  }
  mark() { wc -l < "${1:-$LOG}"; }
  # The app, in a process group of its own, which its helpers are in too. It's up once it has
  # loaded its page and shown its UI, which a still picture of a software renderer may never say.
  LOG=$out/app.log
  setsid build/Browser/chrome > "$LOG" 2>&1 &
  app=$!
  trap 'kill -- "-$app" 2> /dev/null || true' EXIT
  for _ in $(seq 300); do
    grep -q '^browser: loaded ' "$LOG" && break
    sleep 0.1
  done
  await '^browser: UI shown ' 0
  step() { echo "$*" >&3; }
  # The browser window's place on the output, and its size.
  read -r wx wy ww wh < <(swaymsg -t get_tree | jq -r \
    '[.. | select(.app_id? != null and .app_id != "ilo-drag")][0].rect | "\(.x) \(.y) \(.width) \(.height)"')
  px=0 py=0
  # Moves the pointer to ($1, $2) in the browser window in steps, as a hand does.
  to() {
    local x=$((wx + $1)) y=$((wy + $2))
    for i in 1 2 3 4 5; do
      step move $((px + (x - px) * i / 5)) $((py + (y - py) * i / 5))
      sleep 0.05
    done
    px=$x py=$y
  }
  # As await, with the pointer moving a pixel back and forth, as a hand's does: Chromium updates a
  # drag over the page only as it moves, from once it has read what the drag carries.
  hover() {
    for i in $(seq 100); do
      step move $((px + i % 2)) "$py"
      tail -n "+$(($2 + 1))" "${3:-$LOG}" | grep -q -- "$1" && return 0
      sleep 0.1
    done
    fail "no \"$1\" in ${3:-the log}"
  }
  # Starts another app's drag of what it carries, types and data in turn, from its window in the
  # output's corner.
  n=0
  drag() {
    n=$((n + 1))
    dlog=$out/drag-$n.log
    exec 3> >("$DRAG" 1280x800 "$@" > "$dlog" 2>&1)
    await '^drag: ready$' 0 "$dlog"
    swaymsg '[app_id=ilo-drag] move absolute position 1220 740' > /dev/null
    px=1244 py=764
    step move $px $py
    await '^drag: entered$' 0 "$dlog"
    step press
    await '^drag: started$' 0 "$dlog"
  }

  # As a browser drags a link: Chromium reads a URI list as files alone.
  drag text/x-moz-url $'https://example.com/\r\nExample' text/uri-list $'https://example.com/\r\n'
  at=$(mark)
  to $((ww * 2 / 3)) $((wh / 2))
  hover '^browser: drop targets for ' "$at"
  at=$(mark)
  to 100 300
  hover '^browser: drop line in ' "$at"
  await '^drag: target text/uri-list$' 0 "$dlog"
  sleep 0.3
  grim "$out/line.png"
  at=$(mark)
  step release
  await '^browser: dropped 1 in ' "$at"
  await '^drag: finished$' 0 "$dlog"
  exec 3>&-

  # Text that's a link but not a web link isn't taken, and nothing opens.
  drag 'text/plain;charset=utf-8' "javascript:document.title='dropped'"
  at=$(mark)
  to 100 300
  hover '^drag: target none$' 0 "$dlog"
  step release
  await '^drag: cancelled$' 0 "$dlog"
  exec 3>&-
  ! grep -q '^drag: finished$' "$dlog" || fail "the list took a javascript: URL dragged as text"
  ! tail -n "+$((at + 1))" "$LOG" | grep -q '^browser: dropped' ||
    fail "dragged text that isn't a web link opened"

  # Other text, let go over the list, is searched for there, as pinned.sh checks on the Mac.
  drag 'text/plain;charset=utf-8' 'just some words'
  at=$(mark)
  to 100 300
  hover '^browser: drop line in ' "$at"
  step release
  await '^browser: dropped 1 in ' "$at"
  await '^drag: finished$' 0 "$dlog"
  exec 3>&-
  await '^browser: page [0-9]* .* https://duckduckgo.com/?q=just+some+words$' "$at"
  rm "$out/failed"
  exit 0
fi

for tool in sway grim magick jq cargo; do
  command -v "$tool" > /dev/null || { echo "FAIL: needs $tool" >&2; exit 1; }
done
[ -x build/Browser/chrome ] || { echo "FAIL: no build/Browser/chrome; run ./build.sh" >&2; exit 1; }
cargo build -q --release --manifest-path check/drag/Cargo.toml --target-dir target
export DRAG=$PWD/target/release/drag
out=$PWD/build/check/drops
rm -rf "$out"
mkdir -p "$out"
home=$(mktemp -d)
# Chromium's sandboxed helpers may still be writing the profile as they go.
trap 'rm -rf "$home" 2> /dev/null || true' EXIT
env BROWSER_PROFILE_DIR="$home/profile" BROWSER_URL='data:text/html,<title>drops</title>' \
  BROWSER_ONBOARDING=skip \
  ../tools/headless.sh --run check/drops.sh --steps "$out" > "$out/run.log" 2>&1 || true
if [ ! -f "$out/line.png" ] || [ -f "$out/failed" ]; then
  cat "$out/failed" 2> /dev/null || echo "FAIL: see $out/run.log" >&2
  exit 1
fi
echo ok
