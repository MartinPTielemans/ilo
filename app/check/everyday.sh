# check.sh runs this at 590.
# Settings' everyday rows that are Chromium's (#1636), browser-wide, which the core writes into
# every profile, on a page of the check's own server, a secure context as notifications need:
# - General › Page zoom at 125% zooms the page the user is on, as its devicePixelRatio says, and
#   100% gives it back.
# - Privacy › Let sites ask to send notifications off: the page's Notification.requestPermission
#   is denied with no question.
# - General › Ask where to save each file on: the page's download doesn't save to the downloads
#   folder by itself, where it does with it off.
# Needs an engine with the core's bc_page_zoom, bc_notification_prompts and bc_ask_where_to_save.

serve
mkdir -p "$out/site/everyday"
printf 'everyday\n' > "$out/site/everyday/file.txt"
# Its title says what each key did: r its zoom, n the notifications' answer, d a download.
cat > "$out/site/everyday/index.html" << 'HTML'
<title>everyday</title><a id=d href="file.txt" download="everyday.txt">d</a>
<script>
document.addEventListener('keydown', event => {
  if (event.key == 'r') document.title = 'ratio ' + window.devicePixelRatio;
  if (event.key == 'n') {
    Notification.requestPermission().then(answer => document.title = 'notifications ' + answer);
  }
  if (event.key == 'd') document.getElementById('d').click();
});
</script>
HTML

log="$out/everyday.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}everyday/index.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load

# Shows Settings' section $1, drawn: Settings opens on the one it showed last, which a click on
# it again doesn't draw anew.
everyday_section() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if [ "$(sed -n 's/^browser: settings shown //p' "$log" | tail -1)" != "$1" ]; then
    send clickkey $(drawn "toolbar $1")
    settings_drew "$1"
  fi
}
# Back to the page, which has the keyboard, and the key $1 typed in it.
everyday_key() {
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  send click $page_point
  send type "$1"
}

send click $page_point
send type r
expect '^browser: title [0-9]* ratio 1$' "the page isn't at 100% before the zoom changes"
before=$(sed -n 's/^browser: title [0-9]* ratio \(.*\)$/\1/p' "$log" | tail -1)

# Page zoom at 125%, then back to 100%.
everyday_section General
click_in_sight "page zoom"
expect '^browser: menu shown ' "a click on Page zoom didn't open its menu"
post choose 125%
expect '^browser: page zoom 125%$' "choosing 125% didn't set the page zoom"
../tools/window-screenshot.sh "$pid" "$out/everyday-general.png" || fail "no window to capture"
everyday_key r
expect '^browser: title [0-9]* ratio 1\.25$' \
  "at 125% the page's devicePixelRatio isn't 1.25: $(grep 'ratio' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/everyday-zoomed.png" || fail "no window to capture"
everyday_section General
click_in_sight "page zoom"
expect '^browser: menu shown ' "a click on Page zoom didn't open its menu again"
post choose "100% (default)"
expect '^browser: page zoom 100% (default)$' "choosing 100% didn't set the page zoom back"
everyday_key r
expect "^browser: title [0-9]* ratio $before\$" "back at 100% the page isn't at its zoom before"

# Notifications' question off: the page's request is denied without asking.
everyday_section Privacy
click_in_sight "switch notification prompts"
expect '^browser: notification prompts off$' "a click didn't turn notification prompts off"
../tools/window-screenshot.sh "$pid" "$out/everyday-privacy.png" || fail "no window to capture"
everyday_key n
expect '^browser: title [0-9]* notifications denied$' \
  "with the question off, the page's request wasn't denied: $(grep 'notifications' "$log" | tail -1)"
everyday_section Privacy
click_in_sight "switch notification prompts"
expect '^browser: notification prompts on$' "a click didn't turn notification prompts on again"

# Ask where to save off, as by default: the download saves by itself. On: it doesn't.
everyday_key d
expect '^browser: download [0-9]* done everyday.txt$' "with Ask off, the page's download didn't save"
everyday_section General
# Under the card's edge where the screen is short, as on the Mac's check VMs: in_sight scrolls it up.
in_sight "switch ask where to save" || fail "Settings didn't draw Ask where to save each file"
send clickkey $at
expect '^browser: ask where to save on$' "a click didn't turn Ask where to save each file on"
everyday_key d
from=$step
# Whether Chrome's chooser asking where to save is up. Only the Mac's driver logs it, so elsewhere
# it's found among the app's windows: Windows' dialog, which may be a utility process's, or GTK's
# Save File.
chooser_shown() {
  if [ "$(uname)" = Darwin ]; then
    grep -q '^browser: save panel offers everyday' "$log"
  elif windows; then
    python3 -I check/app-windows.py dialogs "$(cat "/proc/$pid/winpid")" > "$out/dialogs.txt" &&
      grep -q . "$out/dialogs.txt"
  elif [ -n "${WAYLAND_DISPLAY:-}" ]; then
    swaymsg -t get_tree |
      jq -e --argjson pid "$(app_pid)" 'any(.. | objects; .pid? == $pid and .name? == "Save File")' > /dev/null
  else
    # --all, or xdotool takes a window meeting any one condition: any of the app's shown.
    xdotool search --all --onlyvisible --pid "$(window_pid)" --name '^Save File$' > /dev/null 2>&1
  fi
}
# Escape before the chooser is up leaves it up, the download asking, and the app's quit waiting on
# it (#2512), so the run waits for the chooser, and once it's up the download hasn't saved.
shown=
for _ in $(seq 100); do
  chooser_shown && shown=1 && break
  kill -0 "$pid" 2>/dev/null || gone
  sleep 0.1
done
if [ -z "$shown" ]; then
  ../tools/window-screenshot.sh "$pid" "$out/everyday-ask.png" all || true
  ! windows || python3 -I check/app-windows.py list "$(cat "/proc/$pid/winpid")" > "$out/windows.txt" 2>&1
  fail "with Ask on, no chooser asked where to save the page's download (everyday-ask.png)"
fi
[ "$(sed -n "/^browser: drive $from: /,\$p" "$log" | grep -c '^browser: download [0-9]* done everyday')" = 0 ] ||
  fail "with Ask on, the page's download saved by itself"
../tools/window-screenshot.sh "$pid" "$out/everyday-ask.png" all || fail "no window to capture"
if [ "$(uname)" = Darwin ]; then
  # Chrome's panel is out of process, where a posted escape never reaches it, and while it's up
  # the app won't quit: the driver keeps it off screen and cancels it.
  send savecancel
  expect '^browser: save panel cancelled$' "the save panel wasn't cancelled"
else
  if windows; then
    # The driver's keys go to the app's own windows, and the dialog may be another process's.
    python3 -I check/app-windows.py dialogs "$(cat "/proc/$pid/winpid")" cancel > /dev/null ||
      fail "the chooser asking where to save couldn't be cancelled"
  elif [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    # The driver's keys go to the app's own windows, so the X server's go to the chooser, as
    # ilo-server.sh's do to the sign-in window. Sway gives the chooser the keyboard as it opens.
    chooser=$(xdotool search --all --onlyvisible --pid "$(window_pid)" --name '^Save File$' | head -1)
    xdotool windowactivate --sync "$chooser" 2>/dev/null || xdotool windowfocus --sync "$chooser" ||
      fail "the chooser asking where to save couldn't have the keyboard"
    xdotool key Escape
  else
    send key escape
  fi
  for _ in $(seq 50); do
    chooser_shown || break
    sleep 0.1
  done
  ! chooser_shown || fail "the chooser asking where to save stayed once answered"
fi
everyday_section General
in_sight "switch ask where to save" || fail "Settings didn't draw Ask where to save each file"
send clickkey $at
expect '^browser: ask where to save off$' "a click didn't turn Ask where to save each file off again"
exec 3>&-
quit
echo "ok: Page zoom at 125% made the page's devicePixelRatio 1.25 and 100% gave back $before;" \
  "with notification prompts off the page's request was denied with no question; and with Ask" \
  "where to save each file on, its download didn't save by itself, where with it off it did." \
  "Windows: $out/everyday-general.png, $out/everyday-zoomed.png, $out/everyday-privacy.png," \
  "$out/everyday-ask.png"
