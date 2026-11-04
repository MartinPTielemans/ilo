# The Spaces' version guard (#589): a device whose account holds Spaces a newer ilo wrote sends
# none of its Spaces' changes, which would write over what it can't read, and Settings › Sync says
# to update it. A copy that plays a newer ilo (--sync-spaces-version=2) signs in as the account's
# first device, on an ilo-server of the run's own, and its Spaces reach the server. The same
# profile, started again as this ilo, finds them a newer one's: where it would send its Spaces it
# keeps them back, the server's stay as the newer ilo wrote them, and Sync says Spaces are paused.
# Linux's alone so far, as it signs in through the X server's keyboard (ilo-server.sh).

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-versions. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  return 0
fi
if [ "$(uname)" != Linux ] || [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ]; then
  echo "skip: sync-versions. It runs under Xvfb on Linux so far, where xdotool reaches it."
  return 0
fi
. check/ilo-server.sh
versions_profile="$PWD/$out/sync-versions-profile"
rm -rf "$versions_profile"
mkdir -p "$versions_profile"

# Starts the app on the run's profile, logging to sync-versions-$1.log, with the switches after.
begin() {
  log="$out/sync-versions-$1.log"
  step=0
  shift
  BROWSER_DRIVE=1 BROWSER_URL="data:text/html,<title>versions</title>" \
    BROWSER_PROFILE_DIR="$versions_profile" "$browser" --sync-url="$server_url" "$@" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
}
# The Spaces the server has, and the newest version of any, which a commit raises.
spaces_stored() {
  stored "SELECT count(*) || ' ' || coalesce(max(version), 0) FROM sync_entities
    WHERE data_type = 500000001 AND deleted = 0"
}
show_sync() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  send clickkey $(drawn "toolbar Sync")
  expect '^browser: settings tab Sync$' "a click on Sync didn't show its tab"
}

begin newer --sync-spaces-version=2
show_sync
send clickkey $(drawn "sync sign-in")
expect '^browser: account SigningIn None$' "Sign In didn't start signing in"
await 'browser: sync sign-in loaded /authorize$' || fail "the sign-in window didn't load the server's page"
sign_in first
await '^browser: account FirstDevice None$' || fail "signing in didn't make this the first device"
settings_keyboard
send clickkey $(drawn "sync done")
await 'browser: sync service active$' || fail "the newer ilo's sync didn't become active"
for _ in $(seq 60); do
  written=$(spaces_stored)
  case $written in "0 "*) ;; *) break ;; esac
  sleep 0.5
done
case $written in "0 "*) fail "the newer ilo's Spaces didn't reach the server" ;; esac
quit

begin older
await 'browser: sync service active$' || fail "started again, the device's sync didn't become active"
await 'browser: sync spaces held, a newer ilo wrote them$' ||
  fail "this ilo didn't find the Spaces a newer one wrote"
await 'browser: sync spaces kept back$' ||
  fail "this ilo didn't keep its Spaces back from the newer one's"
[ "$(spaces_stored)" = "$written" ] ||
  fail "this ilo wrote over the newer one's Spaces: the server had $written, now $(spaces_stored)"
show_sync
expect '^browser: sync activity [a-z]* Some(Outdated)$' "Sync didn't say Spaces are paused"
../tools/window-screenshot.sh "$pid" "$out/sync-versions-paused.png" || fail "no window to capture"
[ "$(spaces_stored)" = "$written" ] ||
  fail "this ilo wrote over the newer one's Spaces: the server had $written, now $(spaces_stored)"
exec 3>&-
quit
kill "$ilo_server" 2>/dev/null || true
ilo_server=
rm -rf "$versions_profile"
echo "ok: a device whose Spaces a newer ilo wrote ($written on the server) kept its own back," \
  "and Sync said Spaces are paused, on a local ilo-server as $email." \
  "Logs: $out/sync-versions-*.log, the window: $out/sync-versions-paused.png"
