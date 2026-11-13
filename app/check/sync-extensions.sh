# check.sh runs this at 700, only when named.
# What a device chooses to sync besides the Spaces (#1235), on an ilo-server of the run's own
# (ilo-server.sh), each device a copy of the app with a profile of its own. A adds uBlock Origin
# Lite from the Chrome Web Store, as the store run does, and picks View › Appearance › Dark, then
# signs in first, which starts with Also sync's Settings and Extensions on, and turns Little ilo
# off. B recovers with A's words and takes them by itself: Dark, which its windows draw in, Little
# ilo's setting, and the extension, which it installs from the store. B turns Settings off, and the
# core takes it. It needs the network and Google's store, so it runs only when named:
# ./check.sh sync-extensions. On Linux under Xvfb, on Windows, where each copy's window takes the
# foreground from the other's as the run drives it, as sync-devices' do, and on the Mac, where each
# copy activates itself as it starts.

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-extensions. It needs ILO_SERVER, an ilo-server built from its repo," \
    "to sign in to."
  return 0
fi
if [ "$(uname)" = Linux ] && [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ]; then
  echo "skip: sync-extensions. On Linux it runs under Xvfb so far," \
    "where xdotool and xclip reach it."
  return 0
fi
rm -f "$saved" "$launch_frame"
. check/ilo-server.sh
. check/webstore.sh
others=
# Chromium's log of the installs sync asks for, which an official build writes only when asked.
device_args="--enable-logging=stderr --vmodule=extension_downloader=2,extension_updater=2"
device_args="$device_args,pending_extension_manager=2,extension_sync_service=2"

device_url=$store
start a "$PWD/$out/sync-extensions-a" 5
device_url=
send state
[ "$active" = 1 ] || fail "A wasn't the active app, so the store's page wouldn't get its keys"
store_shown
focus_button "add to chrome"
send key return
expect '^browser: dialog shown Add "uBlock Origin Lite"?$' \
  "the store's Add to Chrome didn't ask with the browser's dialog on A"
await '^browser: dialog settled$' || fail "A's install dialog didn't settle"
expect '^browser: dialog takes input$' "A's install dialog never took input"
send key return
expect '^browser: dialog confirmed$' "return didn't confirm adding the extension on A"
await "^browser: extensions menu.* $store_id" || fail "A didn't add the store's extension"
# Dark goes up with A's settings as A signs in first.
send menu "Appearance › Dark"
expect '^browser: appearance dark$' "View › Appearance › Dark didn't change A's appearance"

open_sign_in
sign_in first
await '^browser: account FirstDevice None$' ||
  fail "signing in didn't make A the account's first device"
settings_keyboard
click_in_sight "sync copy"
words=
for _ in $(seq 30); do
  clipboard
  words=$clipped
  [ "$(echo "$words" | wc -w | tr -d " ")" = 24 ] && break
  sleep 0.1
done
[ "$(echo "$words" | wc -w | tr -d " ")" = 24 ] || fail "Copy didn't put A's 24 words on the clipboard"
click_in_sight "sync done"
await 'browser: sync service active$' || fail "A's sync didn't become active"
await '^browser: account syncs settings true extensions true$' ||
  fail "A, signed in, didn't start syncing its settings and extensions"
../tools/window-screenshot.sh "$pid" "$out/sync-extensions-a.png" || fail "no window to capture"
send clickkey $(drawn "toolbar Features")
expect '^browser: settings tab Features$' "a click on Features didn't show A's tab"
click_in_sight "switch little windows"
expect '^browser: little windows off$' "a click on Little ilo's switch didn't turn it off on A"

start b "$PWD/$out/sync-extensions-b" 6
! windows || forward
listed && fail "B had the store's extension before it signed in"
open_sign_in
sign_in
await '^browser: account NeedsSecret None$' ||
  fail "B, signed in, didn't need the account's secret"
settings_keyboard
click_in_sight "sync use-words"
send type $words
send key return
await '^browser: account On None$' || fail "B didn't turn sync on with A's words"
await 'browser: sync service active$' || fail "B's sync didn't become active"
await '^browser: account syncs settings true extensions true$' ||
  fail "B, signed in, didn't start syncing its settings and extensions"
await 'browser: sync settings took little_windows off$' ||
  fail "B didn't take Little ilo's setting: $(grep 'sync settings took' "$log" | tr '\n' ' ')"
await 'browser: sync settings took appearance 2$' ||
  fail "B didn't take A's appearance: $(grep 'sync settings took' "$log" | tr '\n' ' ')"
await '^browser: appearance followed dark$' || fail "B's windows didn't take A's appearance"
# Sync asks the store for the extension, as an update check would, and installs it.
installed=
for _ in $(seq 1200); do
  listed && installed=1 && break
  kill -0 "$pid" 2>/dev/null || gone
  sleep 0.1
done
[ -n "$installed" ] || fail "B didn't install the extension A added: $(grep -i \
  "$store_id\|pending\|extension_downloader" "$log" | tail -8 | tr '\n' ' ')"
../tools/window-screenshot.sh "$pid" "$out/sync-extensions-b.png" || fail "no window to capture"

# B stops syncing its settings, and the core takes the choice.
settings_keyboard
click_in_sight "sync switch settings"
expect '^browser: sync settings off$' "a click on Settings' switch didn't turn it off on B"
# Sync starts again without ilo's settings, and with the extensions.
types=
for _ in $(seq 300); do
  types=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep 'browser: sync types ' | tail -1)
  case $types in *Extensions*) case $types in *"ilo Settings"*) ;; *) break ;; esac ;; esac
  types=
  sleep 0.1
done
[ -n "$types" ] || fail "B's sync didn't start again without its settings: $(grep \
  'browser: sync types ' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/sync-extensions-b-off.png" || fail "no window to capture"

device b
exec 3>&-
# On the Mac quit_app asks by the app's bundle id, which A's copy shares, so B takes SIGINT there,
# as on Linux.
if windows; then quit_app; else kill -INT "$pid"; fi
wait "$pid" || fail "B exited with status $?"
exec 6>&-
others=
device a
exec 5>&- 3>&-
quit
kill "$ilo_server" 2>/dev/null || true
ilo_server=
echo "ok: A, signed in first, synced its settings and extensions; B, recovered with A's words," \
  "took A's Dark and Little ilo setting and installed the extension A added from the Web Store," \
  "by itself;" \
  "and B's core took turning Settings off. On a local ilo-server as $email." \
  "Windows: $out/sync-extensions-a.png, $out/sync-extensions-b.png," \
  "$out/sync-extensions-b-off.png"
