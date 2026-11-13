# check.sh runs this at 680.
# Keys typed in a launch that follows another on the same X server reach the UI as typed (#2511).
# The first launch's last key goes with ⌘, which Linux's driver has the server's keyboard hold, as
# a lock, while it does, and the launch quits holding it. The second types a search in Settings,
# which finds what it typed. The server keeps a lock the client that made it leaves, and GPUI's
# X11 client takes the server's locked modifiers into each key, so a lock left over made each key
# a shortcut. X11's alone, Xvfb's and Xwayland's: the other drivers leave nothing past their launch.

if [ "$(uname)" != Linux ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: later-launch. A lock left on the keyboard is the X server's."
else
  rm -f "$saved"
  log="$out/later-launch-1.log"
  step=0
  BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: window opened [0-9]' || fail "the first launch opened no window"
  # The launch's last key, with ⌘ (Ctrl on Linux): the command bar, which the quit closes.
  send key l cmd
  quit

  log="$out/later-launch-2.log"
  step=0
  BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: window opened [0-9]' || fail "the second launch opened no window"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  send clickkey $(drawn "search")
  send type zoom
  expect '^browser: settings search "zoom" finds ' \
    "the search typed in the second launch didn't take its keys"
  quit
  echo "ok: later-launch: keys typed in a later launch reach the UI as typed"
fi
