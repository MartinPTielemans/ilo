# check.sh runs this at 110.
# Launched again after ⌘Q, the app brings back every regular window that was open, as Arc and
# Chrome do: each at the frame it had, in the Space it showed, stacked as they were, with the one
# in front key. A window closed before the quit, and a private window, don't come back. With
# windows open, ⌘N's window is as big as the one in front, cascaded from it. Closing the last
# regular window quits, and the next launch opens at its frame, not a private window's. On Windows,
# the window in front maximized as the app quits, or as the last window closes, comes back
# maximized, its launch window too, and un-maximizes to the frame it had, as Windows' own
# placement says (#1422). On Windows and X11, a window maximized behind the one in front comes back
# maximized behind it, and un-maximizes to its frame (#1454). Reports the launch's times with one
# window brought back and with two.

page='data:text/html,<title>relaunch</title>'
# Milliseconds from the process's start to the line "browser: $1 at <ms> ms".
launch_at() {
  grep -m1 "^browser: $1 at " "$log" | sed 's/.* at \([0-9.]*\) ms$/\1/'
}
# The frame logged last for the window with the id $1, as "<x> <y> <width> <height>".
frame_of() {
  grep "^browser: window $1 frame " "$log" | tail -1 | cut -d' ' -f5-
}
# Wayland shows no client its window's place, so there the app keeps each window's size alone, at
# 0, 0, and the compositor places it (frames.rs).
wayland=
[ "$(uname)" != Linux ] || [ -z "${WAYLAND_DISPLAY:-}" ] || wayland=1
# Whether the frames $1 and $2, as "<x> <y> <width> <height>", are the same: on Wayland, their
# sizes.
same_frame() {
  if [ -n "$wayland" ]; then
    [ "$(echo "$1" | cut -d' ' -f3-)" = "$(echo "$2" | cut -d' ' -f3-)" ]
  else
    [ "$1" = "$2" ]
  fi
}
# The name of the Space the window with the id $1 shows.
space_of() {
  grep "^browser: window $1 in space " "$log" | tail -1 | cut -d' ' -f6-
}
# The frame $1 $2 $3 $4 on the screen the app opens windows on, in place's terms: on the Mac with
# BROWSER_DISPLAY=builtin that's the Mac's own, not the main display place counts from (#904).
at() {
  echo "$(($1 + screen_x)) $(($2 + screen_y)) $3 $4"
}
# Windows' own state of the window driven and the frame it un-maximizes to (GetWindowPlacement), as
# "<maximized|minimized|normal> <x> <y> <width> <height>", which the driver's placement logged last.
placement() {
  grep '^browser: placement ' "$log" | tail -1 | cut -d' ' -f3-
}
# Expects the placement $1 of window $2 (1) now, and fails with $3.
expect_placement() {
  send window "${2:-1}"
  send placement
  [ "$(placement)" = "$1" ] || fail "$3: Windows has the window $(placement), not $1"
}
# Waits for window $2's placement to be $1, "maximized" or "normal", and with $4 to be $4 whole on
# Windows, where it's the frame the window un-maximizes to, and on Linux once it's normal, where
# it's the frame it has; fails with $3. The window manager, or Windows, changes it a moment after
# it's asked.
await_placement() {
  wanted=${4:-$1}
  [ "$(uname)" = Linux ] && [ "$1" = maximized ] && wanted=maximized
  for _ in $(seq 50); do
    send window "$2" behind
    send placement
    case "$(placement)" in "$wanted" | "$wanted "*) return 0 ;; esac
    sleep 0.1
  done
  fail "$3: the window's placement is $(placement), not $wanted"
}
# The ids of the windows opened, in the order they opened.
windows_opened() {
  grep '^browser: window opened ' "$log" | cut -d' ' -f4
}
# Starts the app with the log $1, and the page $2 if it's given.
launch_logged() {
  log="$1"
  step=0
  if [ -n "${2:-}" ]; then
    BROWSER_DRIVE=1 BROWSER_URL="$2" "$browser" < "$out/drive" > "$log" 2>&1 &
  else
    BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
}

# A first launch, with nothing saved, opens one window as it always has. Moved and resized, it
# comes back there.
rm -f "$saved" "$launch_frame"
launch_logged "$out/relaunch-a.log" "$page"
await_load
await '^browser: window opened [0-9]' || fail "the first window didn't open"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: relaunch. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# Linux's session has one screen.
screen_x=0
screen_y=0
if [ "$(uname)" = Darwin ]; then
  send screen
  screen=$(grep -m1 '^browser: screen at ' "$log" | cut -d' ' -f4-)
  screen_x=${screen%% *}
  screen=${screen#* }
  screen_y=${screen%% *}
fi
send place "$(at 60 80 1000 700)"
quit_by_key
exited

launch_logged "$out/relaunch-b.log"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: first frame at ' || fail "the restored window didn't draw"
await '^browser: window opened [0-9]' || fail "the restored window didn't open"
first=$(windows_opened | head -1)
same_frame "$(frame_of "$first")" "$(at 60 80 1000 700)" ||
  fail "the window came back at $(frame_of "$first"), not where it was at $(at 60 80 1000 700)"
one="window on screen $(launch_at 'window on screen') ms, first frame $(launch_at 'first frame') ms"

# On Windows, maximized as the app quits, it comes back maximized, with its launch window, and
# un-maximizes to where it was.
if windows; then
  expect_placement "normal $(at 60 80 1000 700)" 1 "brought back at its frame"
  send maximize
  expect_placement "maximized $(at 60 80 1000 700)" 1 "maximized"
  quit_by_key
  exited
  launch_logged "$out/relaunch-maximized.log"
  await '^browser: restored in ' || fail "the relaunch after a maximized quit didn't restore the Spaces"
  await '^browser: first frame at ' || fail "the window quit maximized didn't draw"
  expect_placement "maximized $(at 60 80 1000 700)" 1 "quit maximized, it came back otherwise (#1422)"
  grep -q "^browser: launch window frame $(at 60 80 1000 700) maximized\$" "$log" ||
    fail "the launch window opened at $(grep '^browser: launch window frame ' "$log" | cut -d' ' -f5-), not maximized from $(at 60 80 1000 700)"
  send maximize
  expect_placement "normal $(at 60 80 1000 700)" 1 "un-maximized after the relaunch"
fi

# Two windows in two Spaces, of different sizes and places, with the second key. A third closes
# before the quit, and a private window stays open over the first.
send key n cmd
expect '^browser: command new-window via ' "⌘N didn't run New Window"
expect '^browser: window opened [0-9]' "⌘N didn't open a window"
new_window_bar
second=$(windows_opened | tail -1)
send window 2
send place "$(at 320 140 900 640)"
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run in the second window"
send type reading
send key return
expect '^browser: create space [0-9]* reading$' "return didn't name the second window's Space"
# New Space shows the Space as it makes it, under a first name that return then renames.
reading=$(grep '^browser: create space [0-9]* reading$' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: renamed space $reading reading\$" "the core didn't rename the second window's Space"
[ "$(space_of "$second")" = "$(grep "^browser: added space $reading " "$log" | tail -1 | cut -d' ' -f5-)" ] ||
  fail "the second window didn't show its Space"
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a third window"
new_window_bar
third=$(windows_opened | tail -1)
send window 3
send key w cmd shift
expect "^browser: window closed $third\$" "⇧⌘W didn't close the third window"
send window 1
send key n cmd shift
expect '^browser: private window opened [0-9]' "⇧⌘N didn't open a private window"
new_window_bar
send window 2
send order
expect '^browser: order window 2 window 4 window 1, key window 2$' \
  "before the quit, the windows weren't the second, the private one and the first, front to back"
quit_by_key
exited

# Back, each at its frame, in its Space, the second key and in front of the first, and nothing
# else.
launch_logged "$out/relaunch-c.log"
await '^browser: restored in ' || fail "the relaunch with two windows didn't restore the Spaces"
await '^browser: first frame at ' || fail "the first restored window didn't draw"
# The windows after the first open once it has drawn; the one behind shows Space 1.
await '^browser: window [0-9]* in space Space 1$' || fail "the window behind didn't come back"
[ "$(windows_opened | wc -l)" -eq 2 ] || fail "$(windows_opened | wc -l | tr -d ' ') windows came back, not 2"
grep -q '^browser: private window opened ' "$log" && fail "the private window came back"
front=$(windows_opened | head -1)
back=$(windows_opened | tail -1)
same_frame "$(frame_of "$front")" "$(at 320 140 900 640)" ||
  fail "the window in front came back at $(frame_of "$front"), not at $(at 320 140 900 640)"
same_frame "$(frame_of "$back")" "$(at 60 80 1000 700)" ||
  fail "the window behind came back at $(frame_of "$back"), not at $(at 60 80 1000 700)"
await "^browser: window $front in space reading\$" ||
  fail "the window in front came back in $(space_of "$front"), not reading"
await "^browser: window $back in space Space 1\$" ||
  fail "the window behind came back in $(space_of "$back"), not Space 1"
await "^browser: window $back shows [1-9][0-9]*\$" || fail "the window behind came back without its page"
if [ -n "$wayland" ]; then
  echo "skip: the windows' order after the relaunch: on Wayland the window behind comes back in front (#1550)."
else
  send order
  expect '^browser: order window 1 window 2, key window 1$' \
    "the windows came back out of order, or the one in front wasn't key"
fi
if windows; then
  expect_placement "normal $(at 320 140 900 640)" 1 "the window in front brought back"
  expect_placement "normal $(at 60 80 1000 700)" 2 "the window behind brought back"
fi
../tools/window-screenshot.sh "$pid" "$out/relaunch-two.png" all || fail "no window to capture"
two="window on screen $(launch_at 'window on screen') ms, first frame $(launch_at 'first frame') ms"

# Maximized behind the window in front as the app quits, by the window manager's maximize on X11,
# the window comes back maximized behind it, and un-maximizes to the frame it had; the one in front
# comes back at its own (#1454). Wayland shows no client the state, and the Mac's zoom is a frame,
# which it keeps.
behind_maximized=
if [ -z "$wayland" ] && [ "$(uname)" != Darwin ]; then
  send window 2
  send maximize
  await_placement maximized 2 "the window behind didn't maximize"
  send window 1
  quit_by_key
  exited
  launch_logged "$out/relaunch-maximized-behind.log"
  await '^browser: restored in ' || fail "the relaunch with a window maximized behind didn't restore the Spaces"
  await '^browser: first frame at ' || fail "the window in front didn't draw after the relaunch"
  await '^browser: window [0-9]* in space Space 1$' || fail "the window maximized behind didn't come back"
  [ "$(windows_opened | wc -l)" -eq 2 ] || fail "$(windows_opened | wc -l | tr -d ' ') windows came back, not 2"
  front=$(windows_opened | head -1)
  back=$(windows_opened | tail -1)
  send order
  expect '^browser: order window 1 window 2, key window 1$' \
    "the window maximized behind came back out of order, or the one in front wasn't key"
  same_frame "$(frame_of "$front")" "$(at 320 140 900 640)" ||
    fail "the window in front came back at $(frame_of "$front"), not at $(at 320 140 900 640)"
  await_placement maximized 2 "the window maximized behind came back windowed (#1454)" \
    "maximized $(at 60 80 1000 700)"
  await_placement normal 1 "the window in front came back maximized" "normal $(at 320 140 900 640)"
  send window 2
  send maximize
  await_placement normal 2 "the window behind didn't un-maximize" "normal $(at 60 80 1000 700)"
  send window 1
  behind_maximized=" A window maximized behind the one in front comes back maximized behind it, and un-maximizes to the frame it had."
fi

# With windows open, ⌘N's window is as big as the one in front, down and to the right of it as
# AppKit cascades windows, except on Wayland, where the compositor places it.
send window 1
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a window over the two brought back"
new_window_bar
cascaded=$(windows_opened | tail -1)
read -r x y width height <<EOF
$(frame_of "$cascaded")
EOF
[ "$width $height" = "900 640" ] && { [ -n "$wayland" ] ||
  { [ "${x:-0}" -gt $((screen_x + 320)) ] && [ "${y:-0}" -gt $((screen_y + 140)) ]; }; } ||
  fail "⌘N's window opened at $(frame_of "$cascaded"), not as big as the one in front at $(at 320 140 900 640) and down and to its right"

# Closed last, a regular window sets where the next launch's window opens, and a private window
# open at another frame doesn't.
send key n cmd shift
expect '^browser: private window opened [0-9]' "⇧⌘N didn't open a private window"
new_window_bar
send window 4
send place "$(at 500 300 700 500)"
send window 2
send key w cmd shift
expect "^browser: window closed $back\$" "⇧⌘W didn't close the window behind"
send window 3
send key w cmd shift
expect "^browser: window closed $cascaded\$" "⇧⌘W didn't close ⌘N's window"
send window 1
send place "$(at 140 100 1040 680)"
# The last regular window's close quits, so the driver never reports it done.
post key w cmd shift
exec 3>&-
exited

launch_logged "$out/relaunch-d.log"
await '^browser: window opened [0-9]' || fail "no window opened after the last one closed"
same_frame "$(grep -m1 '^browser: launch window frame ' "$log" | cut -d' ' -f5-)" \
  "$(at 140 100 1040 680)" ||
  fail "the launch window opened at $(grep '^browser: launch window frame ' "$log" | cut -d' ' -f5-), not where the last window closed at $(at 140 100 1040 680)"
first=$(windows_opened | head -1)
same_frame "$(frame_of "$first")" "$(at 140 100 1040 680)" ||
  fail "the window opened at $(frame_of "$first"), not where the last window closed at $(at 140 100 1040 680)"

# On Windows, maximized as it closes, the last window comes back maximized, with its launch window.
maximized=
if windows; then
  send maximize
  expect_placement "maximized $(at 140 100 1040 680)" 1 "maximized before it closes"
  post key w cmd shift
  exited
  launch_logged "$out/relaunch-e.log"
  await '^browser: first frame at ' || fail "no window drew after the last one closed maximized"
  grep -q "^browser: launch window frame $(at 140 100 1040 680) maximized\$" "$log" ||
    fail "the launch window opened at $(grep '^browser: launch window frame ' "$log" | cut -d' ' -f5-), not maximized from $(at 140 100 1040 680)"
  expect_placement "maximized $(at 140 100 1040 680)" 1 "closed maximized, the last window came back otherwise (#1422)"
  send maximize
  expect_placement "normal $(at 140 100 1040 680)" 1 "un-maximized after it came back"
  maximized=" On Windows, the window in front maximized as the app quits or as the last window closes comes back maximized, its launch window too, and un-maximizes to the frame it had."
fi
exec 3>&-
quit

echo "ok: after ⌘Q, every regular window comes back at its frame, in its Space, stacked as it was with the one in front key; a window closed before the quit and a private window don't.$behind_maximized ⌘N with windows open cascades from the one in front at its size, and after the last regular window closes the next launch opens at its frame, not a private window's.$maximized Window: $out/relaunch-two.png"
echo "  Launch with one window: $one; with two: $two"
