# Proof for #1455 only, never merged: a window's frames once the display's scale differs from the
# one the app started at, as on a second display at another scale. The app starts at 100%, its
# window is placed, the display goes to 150%, ⌘N opens a window cascaded from it, and the app quits
# and comes back. Logs the frames the app has and Windows' own placement at each step, in
# $out/scale.txt, and fails only if a step can't run.

page='data:text/html,<title>scale</title>'
frame_of() {
  grep "^browser: window $1 frame " "$log" | tail -1 | cut -d' ' -f5-
}
windows_opened() {
  grep '^browser: window opened ' "$log" | cut -d' ' -f4
}
placement() {
  grep '^browser: placement ' "$log" | tail -1 | cut -d' ' -f3-
}
# Windows' placement of the driver's window $1, which counts windows in the order they opened.
placement_of() {
  send window "$1"
  send placement
  placed=$(placement)
}
note() {
  echo "$*" | tee -a "$out/scale.txt"
}
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
scale() {
  python3 -I ../.github/scripts/display-scale.py "$1" | tee -a "$out/scale.txt"
}

: > "$out/scale.txt"
scale 100
rm -f "$saved" "$launch_frame"
launch_logged "$out/scale-a.log" "$page"
await_load
await '^browser: window opened [0-9]' || fail "the first window didn't open"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: scale. Browser wasn't the active app."
  exit 0
fi
first=$(windows_opened | head -1)
send place 60 80 600 400
placement_of 1
note "at 100%: the app's frame $(frame_of "$first"), Windows' placement $placed"
scale 150
# Windows rescales the window over the next messages; no line says when it's done.
sleep 3
send place 60 80 600 400
placement_of 1
note "at 150%, the app started at 100%: placed at 60 80 600 400, the app's frame $(frame_of "$first"), Windows' placement $placed"
send window 1
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a window"
cascaded=$(windows_opened | tail -1)
[ "$cascaded" != "$first" ] || fail "⌘N didn't open a window"
sleep 1
placement_of 2
note "⌘N's window, cascaded from it: the app's frame $(frame_of "$cascaded"), Windows' placement $placed"
quit_by_key
exited

launch_logged "$out/scale-b.log"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: first frame at ' || fail "the restored window didn't draw"
grep '^browser: launch window frame ' "$log" | head -1 | tee -a "$out/scale.txt"
for each in $(windows_opened); do
  note "relaunched at 150%: window $each's frame $(frame_of "$each")"
done
placement_of 1
one=$placed
placement_of 2
note "Windows' placements: $one; $placed"
exec 3>&-
quit
scale 100
echo "ok: proof for #1455: $(tr '\n' '|' < "$out/scale.txt")"
