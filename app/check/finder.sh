# check.sh runs this at 261, only when named.
# The Mini Player over Finder's window in full screen: media.sh's over-fullscreen step with the
# other app being Finder itself, the one a user has open. Finder is driven only through its own
# AppleScript dictionary (make window, activate, close) and, for the full-screen step itself,
# the Accessibility AXFullScreen attribute on its standard window, set pid-exact through System
# Events' process lookup by unix id: keys posted to a pid land as zoom on this macOS, and the
# menu's Enter Full Screen item tiles instead, while the attribute is the one the window obeys.
# The gate is the check's own (the driver finds the Mini Player over the Space Finder made full
# screen, then the SC floating capture at that frame reads video-bright), with the raw display as
# the tie-breaker when the capture is refused. The scene needs its engine: the core tells the
# Mini Player's window it may join the Space (mini_player.cc), which an older engine skips.
# Whichever gate passes, a Space round trip follows: away to the browser's own window and back
# to Finder's, the player over Finder's window still, which reads the core's Space-change
# recovery (mini_player.cc) holding where Ardia only notes the loss.

if [ "$(uname)" != Darwin ]; then
  echo "skip: finder. Only the Mac drives Finder full screen."
  return 0
fi

serve

log="$out/finder.log"
step=0
rm -f "$saved" "$launch_frame"

# The run's way back, as policies' run_undo is: a function's name, so cleanup finds it whether
# the check passed or failed. Finder leaves full screen and its window closes, as the run found
# it; a Finder without a full-screen window, as a failed run may leave it, is a Finder the run
# would have to make full screen itself, so the empty case is the clean one.
finder_back() {
  local fpid
  fpid=$(pgrep -x Finder | head -1)
  [ -n "$fpid" ] || return 0
  # A window left in the Dock by the re-made-Space round comes back first, so the full-screen
  # back-out finds the window it expects.
  osascript -e 'tell application "Finder" to set collapsed of front window to false' > /dev/null 2>&1 || true
  osascript -e 'tell application "System Events"' \
    -e 'tell (first process whose unix id is '"$fpid"')' \
    -e 'set value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow") to false' \
    -e 'end tell' -e 'end tell' > /dev/null 2>&1 || true
  osascript -e 'tell application "Finder" to close front window' > /dev/null 2>&1 || true
}
run_undo=finder_back

# A Space round trip, run once a gate below has passed: the user leaves Finder's full-screen Space
# for the browser's own and comes back, and the player has to be over Finder's window still. The
# leaving is what strands a shown player (the window server can keep it on the Space the user
# left, the race the core's recovery answers, mini_player.cc, where Ardia only notes it), so each
# leg dwells past the recovery's 0.6 s before the step that follows, and the last gate reads the
# raw display, which over a foreign full-screen Space no window capture is honest about.
space_round_trip() {
  # Where the player was, before the leaving leg: its PiP video is the only moving thing the
  # display can read at that point once the page stopped drawing the stream, as it does once
  # the video floats, and the window server doesn't move a carried window, so the same point
  # reads the player wherever the round trip leaves it.
  set -- ${at2:-$at}
  mx=$(awk -v w="$3" -v x="$1" 'BEGIN { printf "%.0f", x + w / 2 }')
  my=$(awk -v h="$4" -v y="$2" 'BEGIN { printf "%.0f", y + h * 0.75 }')
  rect="$((mx - 6)),$((my - 6)),12,12"
  # Away to the browser's own window, pid-exact: open -a by path would name whichever copy of the
  # app runs at that path, and the owner may have another.
  said=$(osascript -e 'tell application "System Events" to set frontmost of (first process whose unix id is '"$pid"') to true' 2>&1) ||
    fail "the browser wouldn't take the keyboard to leave Finder's Space: $said"
  front=0
  for _ in $(seq 100); do
    now=$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true' 2>/dev/null) || now=
    [ "$now" = "$pid" ] && { front=1; break; }
    sleep 0.1
  done
  [ "$front" = 1 ] || fail "the Space never turned to the browser's own: frontmost is $now"
  # Past the recovery's 0.6 s, so a player the window server left on Finder's Space has been
  # brought to the one the user is on before the leg back, and the point above reads where.
  sleep 1.5
  raw=$(screencapture -x -R"$rect" "$out/finder-away-point.png" 2>/dev/null &&
    pixel "$out/finder-away-point.png" 0.5 0.5) || raw=none
  echo "finder: away on the browser's own Space, the display at the Mini Player's last place reads $raw"
  said=$(osascript -e 'tell application id "com.apple.finder" to activate' 2>&1) ||
    fail "Finder wouldn't take the keyboard back: $said"
  front=0
  for _ in $(seq 100); do
    now=$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true' 2>/dev/null) || now=
    [ "$now" = "$fpid" ] && { front=1; break; }
    sleep 0.1
  done
  [ "$front" = 1 ] || fail "the Space never came back to Finder's: frontmost is $now"
  # Past the recovery's 0.6 s again, so a player brought away has been brought back before the gate.
  sleep 1
  send pip
  back_at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: pip at ' | cut -d' ' -f4-)
  [ -n "$back_at" ] ||
    fail "the driver found no Mini Player after the Space round trip: $(grep '^browser: pip ' "$log" | tail -1)"
  set -- $back_at
  mx=$(awk -v w="$3" -v x="$1" 'BEGIN { printf "%.0f", x + w / 2 }')
  my=$(awk -v h="$4" -v y="$2" 'BEGIN { printf "%.0f", y + h * 0.75 }')
  rect="$((mx - 6)),$((my - 6)),12,12"
  raw=$(screencapture -x -R"$rect" "$out/finder-round-trip-point.png" 2>/dev/null &&
    pixel "$out/finder-round-trip-point.png" 0.5 0.5) ||
    fail "no raw scene after the Space round trip"
  sleep 0.5
  moved=$(screencapture -x -R"$rect" "$out/finder-round-trip-point-2.png" 2>/dev/null &&
    pixel "$out/finder-round-trip-point-2.png" 0.5 0.5) ||
    fail "no second raw scene after the Space round trip"
  echo "finder: after the Space round trip the raw display at the Mini Player's middle reads $raw then $moved"
  moved_enough=$(echo "$raw $moved" | awk '{ m = 0
    for (i = 1; i <= 3; i++) { d = $(i + 3) - $i; if (d < 0) d = -d; if (d > m) m = d }
    print m }')
  [ "${moved_enough:-0}" -ge 12 ] ||
    fail "the Mini Player wasn't over Finder's full-screen window after the Space round trip: the display held still at $raw"
  brought=$(grep -c '^browser: mini player brought back to the active Space$' "$log" || true)
  note=
  [ "$brought" -ge 1 ] && note=", the core bringing it back after the window server left it behind"
  echo "finder: the Mini Player kept its place over Finder's full-screen window across a Space round trip$note"
}

# The video's page, media.sh's own: a canvas's colours, no sound, floating on Picture in Picture.
# The gates that read movement run on Finder's Space, where the browser's window can't appear,
# so the only moving thing the raw display can show at the player's place is the player itself.
printf '%s' '<title>loading</title><body style="margin:0;background:#333">
<video id=v muted autoplay style="width:100vw;height:100vh"></video>
<script>
const canvas = Object.assign(document.createElement("canvas"), {width: 320, height: 180});
const paint = canvas.getContext("2d");
const frame = time => {
  paint.fillStyle = `hsl(${time / 20 % 360} 50% 40%)`;
  paint.fillRect(0, 0, 320, 180);
  requestAnimationFrame(frame);
};
requestAnimationFrame(frame);
v.srcObject = canvas.captureStream();
v.onplaying = () => document.title = "playing";
v.onenterpictureinpicture = () => document.title = "floating";
v.onleavepictureinpicture = () => document.title = "landed";
onkeydown = e => e.key == "x" && document.exitPictureInPicture();
</script>' > "$out/site/video.html"

BROWSER_DRIVE=1 BROWSER_URL="${site}video.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: loaded [0-9]* ${site}video.html\$" ||
  fail "the video's page didn't load"
v=$(grep "^browser: loaded [0-9]* ${site}video.html\$" "$log" | tail -1 | cut -d' ' -f3)
await "^browser: title $v playing\$" || fail "the video didn't play"
post rightclick $page_point
expect "^browser: menu shown .*$(cased 'Picture in Picture')" \
  "the video's menu had no Picture in Picture"
send choose "$(cased 'Picture in Picture')"
expect "^browser: title $v floating\$" "the video didn't float"
expect 'browser: mini player placed at ' "the core didn't place the Mini Player"

# Finder in full screen, driven only through its own dictionary and the AX attribute.
said=$(osascript -e 'tell application "Finder" to make new Finder window' 2>&1) ||
  fail "Finder didn't open its window: $said"
opened=0
for _ in $(seq 50); do
  [ "$(osascript -e 'tell application "Finder" to count windows' 2>/dev/null)" -ge 1 ] && { opened=1; break; }
  sleep 0.1
done
[ "$opened" = 1 ] || fail "Finder never opened its window"
fpid=$(pgrep -x Finder | head -1)
[ -n "$fpid" ] || fail "no Finder pid to set full screen on"

# Full screen through the Accessibility attribute on Finder's standard window, pid-exact through
# System Events' process lookup by unix id: the Mac keeps its menu bar over a full-screen Space
# (the calibration against check/other-app.mm's own full-screen window shows the same y=33), so
# the gate asks the window itself, as AppKit keeps it.
said=$(osascript -e 'tell application id "com.apple.finder" to activate' 2>&1) ||
  fail "Finder didn't take the keyboard: $said"
said=$(osascript -e 'tell application "System Events"' \
  -e 'tell (first process whose unix id is '"$fpid"')' \
  -e 'set value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow") to true' \
  -e 'end tell' -e 'end tell' 2>&1) ||
  fail "Finder's window wouldn't go full screen: $said"
# The check runs on the built-in display (BROWSER_DISPLAY=builtin), the one with the menu bar.
# The window's own attribute is the gate, as the calibration showed: AppKit keeps a full-screen
# window's AppleScript bounds below the menu bar, so bounds can't tell full screen from zoomed.
full=0
for _ in $(seq 100); do
  # A query can miss while the full-screen Space is still settling; the poll just tries again.
  axfull=$(osascript -e 'tell application "System Events" to tell (first process whose unix id is '"$fpid"') to get value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow")' 2>/dev/null) || axfull=
  [ "$axfull" = "true" ] && { full=1; break; }
  sleep 0.1
done
[ "$full" = 1 ] || fail "Finder's window didn't go full screen: $axfull"

# The check's own gate, as media.sh reads it, with the raw display as the tie-breaker below.
send pip
at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: pip at ' | cut -d' ' -f4-)
[ -n "$at" ] ||
  fail "the driver found no Mini Player over Finder's full-screen window: $(grep '^browser: pip ' "$log" | tail -1)"
if ! FRAME="$at" ../tools/window-screenshot.sh "$pid" \
  "$out/mini-player-over-finder-fullscreen.png" floating; then
  # The raw display as the tie-breaker, the probe rounds' blind-spot-free instrument: SC's listing
  # and AppKit's occlusion can share a blind spot over a foreign full-screen Space, while a raw
  # capture cannot. Two captures of the point at the video's middle, half a second apart: the
  # page's hue turns once every 7.2 s, so a window that draws moves and a display that shows only
  # Finder's window holds still. The rect, not the whole display, keeps the point's math off the
  # display's own size, and Finder's full-screen window covers the point, so the capture holds
  # nothing of the owner's work.
  set -- $at
  # One whole-display scene for the run's record, owner-safe as the probe rounds' raw reads
  # were: Finder's full-screen window covers the display, so the capture holds nothing of the
  # owner's work. The rect captures below it are what the gate reads.
  screencapture -x "$out/finder-scene.png" > /dev/null 2>&1 || true
  mx=$(awk -v w="$3" -v x="$1" 'BEGIN { printf "%.0f", x + w / 2 }')
  my=$(awk -v h="$4" -v y="$2" 'BEGIN { printf "%.0f", y + h * 0.75 }')
  rect="$((mx - 6)),$((my - 6)),12,12"
  raw=$(screencapture -x -R"$rect" "$out/finder-scene-point.png" 2>/dev/null &&
    pixel "$out/finder-scene-point.png" 0.5 0.5) ||
    fail "no Mini Player over Finder's full-screen window, and no raw scene to read"
  sleep 0.5
  moved=$(screencapture -x -R"$rect" "$out/finder-scene-point-2.png" 2>/dev/null &&
    pixel "$out/finder-scene-point-2.png" 0.5 0.5) ||
    fail "no Mini Player over Finder's full-screen window, and no second raw scene to read"
  echo "finder: the raw display at the Mini Player's middle reads $raw then $moved" \
    "(SC refused the window's own capture)"
  # A hue turn moves one channel up as it moves another down, so the test is movement in any
  # channel, not growth in all of them: any channel moving by 12 or more says the video draws.
  moved_enough=$(echo "$raw $moved" | awk '{ m = 0
    for (i = 1; i <= 3; i++) { d = $(i + 3) - $i; if (d < 0) d = -d; if (d > m) m = d }
    print m }')
  [ "${moved_enough:-0}" -ge 12 ] && {
    echo "finder: the Mini Player was drawn over Finder's full-screen window, moving as its video does"
    space_round_trip
    cleanup
    return 0
  }
  # The Space Finder made first leaves the Mini Player out, as the owner watching Ardia found: the
  # window server includes a joining window only when the Space forms again around it. Their words:
  # "the full screen app has to be minimified then bring again to full screen in order to work", and
  # on this macOS a full-screen window won't collapse to the Dock from inside its Space (the
  # dictionary takes the order and the window stays), so their dance runs the way it must: the
  # window leaves full screen, collapses to the Dock, comes back, and the full screen that returns
  # with it is the Space that forms again around the still-floating player.
  said=$(osascript -e 'tell application "System Events"' \
    -e 'tell (first process whose unix id is '"$fpid"')' \
    -e 'set value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow") to false' \
    -e 'end tell' -e 'end tell' 2>&1) ||
    fail "Finder wouldn't leave full screen for the re-made one: $said"
  for _ in $(seq 100); do
    axfull=$(osascript -e 'tell application "System Events" to tell (first process whose unix id is '"$fpid"') to get value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow")' 2>/dev/null) || axfull=
    [ "$axfull" = "false" ] && break
    sleep 0.1
  done
  said=$(osascript -e 'tell application "Finder" to set collapsed of front window to true' 2>&1) ||
    fail "Finder's window wouldn't minimize for the re-made one: $said"
  min=0
  for _ in $(seq 100); do
    collapsed=$(osascript -e 'tell application "Finder" to get collapsed of front window' 2>/dev/null) || collapsed=
    [ "$collapsed" = "true" ] && { min=1; break; }
    sleep 0.1
  done
  [ "$min" = 1 ] || fail "Finder's window never minimized: $collapsed"
  said=$(osascript -e 'tell application "Finder" to set collapsed of front window to false' 2>&1) ||
    fail "Finder's window wouldn't come back from the Dock: $said"
  # A window doesn't return to full screen by itself as it un-collapses, so the attribute sets it,
  # as the owner's hand would: the window they bring back goes full screen again, and that Space
  # is the one that forms around the player.
  said=$(osascript -e 'tell application "System Events"' \
    -e 'tell (first process whose unix id is '"$fpid"')' \
    -e 'set value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow") to true' \
    -e 'end tell' -e 'end tell' 2>&1) ||
    fail "Finder's window wouldn't go full screen again from the Dock: $said"
  full=0
  for _ in $(seq 100); do
    axfull=$(osascript -e 'tell application "System Events" to tell (first process whose unix id is '"$fpid"') to get value of attribute "AXFullScreen" of (first window whose value of attribute "AXSubrole" is "AXStandardWindow")' 2>/dev/null) || axfull=
    [ "$axfull" = "true" ] && { full=1; break; }
    sleep 0.1
  done
  [ "$full" = 1 ] || fail "Finder's window didn't come back to full screen from the Dock: $axfull"
  send pip
  at2=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: pip at ' | cut -d' ' -f4-)
  [ -n "$at2" ] ||
    fail "the driver found no Mini Player over the re-made full-screen window: $(grep '^browser: pip ' "$log" | tail -1)"
  set -- $at2
  mx=$(awk -v w="$3" -v x="$1" 'BEGIN { printf "%.0f", x + w / 2 }')
  my=$(awk -v h="$4" -v y="$2" 'BEGIN { printf "%.0f", y + h * 0.75 }')
  rect="$((mx - 6)),$((my - 6)),12,12"
  raw=$(screencapture -x -R"$rect" "$out/finder-scene-reformed-point.png" 2>/dev/null &&
    pixel "$out/finder-scene-reformed-point.png" 0.5 0.5) ||
    fail "no raw scene over the re-made full-screen window"
  sleep 0.5
  moved=$(screencapture -x -R"$rect" "$out/finder-scene-reformed-point-2.png" 2>/dev/null &&
    pixel "$out/finder-scene-reformed-point-2.png" 0.5 0.5) ||
    fail "no second raw scene over the re-made full-screen window"
  echo "finder: the raw display over the re-made full-screen window reads $raw then $moved"
  moved_enough=$(echo "$raw $moved" | awk '{ m = 0
    for (i = 1; i <= 3; i++) { d = $(i + 3) - $i; if (d < 0) d = -d; if (d > m) m = d }
    print m }')
  [ "${moved_enough:-0}" -ge 12 ] ||
    fail "the Mini Player never drew over Finder's full-screen window, even the re-made one: the display held still at $raw"
  echo "finder: the Mini Player drew over Finder's full-screen window once its Space re-formed around it, moving as its video does"
  space_round_trip
  cleanup
  return 0
fi
# media.sh's lightness, inlined: a named run sources lib.sh and its own file alone.
lightness() {
  pixel "$1" 0.125 0.5 | awk '{ max = $1; min = $1
    for (i = 2; i <= 3; i++) { if ($i > max) max = $i; if ($i < min) min = $i }
    print max + min }'
}
over=$(lightness "$out/mini-player-over-finder-fullscreen.png")
[ "$over" -ge 170 ] ||
  fail "the Mini Player over Finder's full-screen window was dark over its video, at $over where the video is about 204"
echo "finder: the Mini Player kept its place over Finder's full-screen window, at lightness $over"
space_round_trip
cleanup
