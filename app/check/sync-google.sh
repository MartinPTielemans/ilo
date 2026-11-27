# check.sh runs this at 445, only when named.
# Continue with Google, on an ilo-server of the run's own (ilo-server.sh) that a stand-in Google on
# loopback (check/google-standin.py) serves as Google, through GOOGLE_TEST_URL, which only DEV_MODE
# honours: never the real Google or a real account. Settings › Sync & Import draws no Google
# button until the server's discovery has said it offers one, which the core reads as Sign In
# starts for the build's own server; then Continue with Google shows beside Sign In. Its window
# goes straight on to the stand-in's page, and closed there with Cancel, the sign-in didn't finish.
# Again, the stand-in's person chooses their address, a Workspace domain's that Google holds the
# mailbox of, the server links the account by Google's sub alone, the passkey's offer is declined,
# and the account's first device turns sync on. Settings lists the Google sign-in, and on Linux
# Remove…, asked first, unlinks it on the server. No log of the browser's or the server's holds
# the stand-in's code, its access token or its ID token. It needs ILO_SERVER built from
# feat/google-road or later and an engine with core/sync_sign_in.cc, so it runs only when named:
# ./check.sh sync-google.

if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: sync-google. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  return 0
fi
if ! engine_has ../core/sync_sign_in.cc ../core/sync_sign_in.h ../core/sync.cc ../core/api.cc; then
  echo "skip: sync-google. The engine predates the core's Google sign-in (core/sync_sign_in.cc)," \
    "so the browser has no way to it yet."
  return 0
fi
rm -f "$saved" "$launch_frame"
others=
scratch=$(mktemp -d)
standin_log="$out/$run-standin.log"
standin_mode="$scratch/mode"
standin_id=ilo-check.apps.googleusercontent.com
standin_secret="standin-secret-$(date +%s)"
standin_sub="1$(date +%s)0042"
standin_email="check-$(date +%s)@example.org"
echo hold > "$standin_mode"
# As count's endpoint, cleanup stops it after a failure; the run sets no sync_faults.
sync_faults=
python3 -I check/google-standin.py "$standin_id" "$standin_secret" "$standin_mode" \
  "$standin_email" "$standin_sub" > "$standin_log" 2>&1 3>&- &
collector=$!
for _ in $(seq 50); do
  grep -q '^standin: is up on ' "$standin_log" && break
  kill -0 "$collector" 2>/dev/null || fail "google-standin.py exited: $(tail -1 "$standin_log")"
  sleep 0.1
done
standin_port=$(sed -n 's/^standin: is up on //p' "$standin_log")
[ -n "$standin_port" ] || fail "google-standin.py didn't start"
# The server's settings for Google, for its start alone: a run after this one starts a server with
# none, which offers no Google.
export GOOGLE_CLIENT_ID=$standin_id GOOGLE_CLIENT_SECRET=$standin_secret GOOGLE_OFFERED=1 \
  GOOGLE_TEST_URL="http://127.0.0.1:$standin_port"
. check/ilo-server.sh
unset GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET GOOGLE_OFFERED GOOGLE_TEST_URL
offered=$(python3 -I -c '
import json, sys, urllib.request
print(json.load(urllib.request.urlopen(sys.argv[1] + "/.well-known/openid-configuration")).get("ilo_google") is True)' \
  "$server_url") || fail "ilo-server's discovery didn't answer"
if [ "$offered" != True ]; then
  kill "$collector" "$ilo_server" 2>/dev/null || true
  collector= ilo_server=
  rm -rf "$scratch"
  scratch=
  echo "skip: sync-google. ILO_SERVER offers no Google with GOOGLE_OFFERED on: it's built from" \
    "an ilo-server before feat/google-road."
  return 0
fi

# Settings has the keyboard while the sign-in window is open, for a click on its Cancel: clickkey
# goes to the key window. On the Mac, Settings… brings Settings in front.
settings_front() {
  if [ "$(uname)" = Darwin ]; then
    send menu Settings…
  else
    settings_keyboard
  fi
}
# Waits up to 10 s for the stand-in to log a line matching $1.
standin_said() {
  for _ in $(seq 100); do
    grep -q "$1" "$standin_log" && return 0
    sleep 0.1
  done
  return 1
}

start a "$PWD/$out/sync-google-a" 5
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
send clickkey $(drawn "toolbar Sync & Import")
expect '^browser: settings tab Sync & Import$' "a click on Sync & Import didn't show its tab"
signed_out='^browser: account SignedOut None$'
at=$(drawn "sync sign-in") || fail "Settings › Sync & Import didn't draw Sign In"
! grep -q '^browser: settings sync sign-in-google at ' "$log" ||
  fail "Settings drew Continue with Google before the server said it offers it"
# Sign In reads the build's own server's discovery as it starts, and Cancel brings the form back,
# which now offers Google.
send clickkey $at
expect '^browser: account SigningIn None$' "Sign In didn't start signing in"
# The core's sync lines come through LOG(ERROR), after Chromium's [pid:tid:…] prefix.
await "^\(\[[^]]*\] \)*browser: sync request GET $server_url/.well-known/openid-configuration\$" ||
  fail "Sign In didn't read the server's discovery"
await 'browser: sync sign-in loaded /authorize$' ||
  fail "the sign-in window didn't load the server's page"
settings_front
send clickkey $(drawn_after "sync cancel" '^browser: account SigningIn None$')
expect "$signed_out" "Cancel didn't stop signing in"
settings_keyboard
at=$(drawn_after "sync sign-in-google" "$signed_out") ||
  fail "with the server offering Google, Settings drew no Continue with Google"
../tools/window-screenshot.sh "$(app_pid)" "$out/sync-google-offered.png" || fail "no window to capture"

# The stand-in's person hasn't chosen: Cancel, and the form says the sign-in didn't finish.
send clickkey $at
expect '^browser: account SigningIn None$' "Continue with Google didn't start signing in"
standin_said '^standin: auth hold$' ||
  fail "the sign-in window didn't go on to Google's page: $(tail -1 "$standin_log")"
await 'browser: sync sign-in loaded /auth$' || fail "the sign-in window didn't load Google's page"
# The window names the host of the page it shows, Google's now, as a tab's address does.
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  installed xdotool "finding the sign-in window by its title"
  timeout 10 xdotool search --sync --all --onlyvisible --pid "$(window_pid)" --name '^127\.0\.0\.1$' \
    > /dev/null || fail "the sign-in window isn't titled by Google's page's host"
else
  echo "skip: the sign-in window's title on Google's page: it's read from the X server, so only under Xvfb."
fi
settings_front
send clickkey $(drawn_after "sync cancel" '^browser: account SigningIn None$')
expect "$signed_out" "Cancel didn't stop the Google sign-in"
settings_keyboard
drawn_after "sync sign-in-google" "$signed_out" > /dev/null ||
  fail "after a Google sign-in that didn't finish, Continue with Google wasn't offered again"
../tools/window-screenshot.sh "$(app_pid)" "$out/sync-google-unfinished.png" ||
  fail "no window to capture"

# The person chooses their address: the server links it by its sub and signs the account in.
echo pass > "$standin_mode"
sign_in_window=
send clickkey $(drawn_after "sync sign-in-google" "$signed_out")
expect '^browser: account SigningIn None$' "Continue with Google didn't start signing in again"
standin_said '^standin: token ' && grep -q '^standin: token given$' "$standin_log" ||
  fail "the server didn't trade Google's code for its token: $(tail -1 "$standin_log")"
# The account's first sign-in offers a passkey, with Add a Passkey focused, which Not Now declines.
await 'browser: sync sign-in loaded /authorize/google$' ||
  fail "the server didn't take Google's answer"
sign_in_keys key tab
sign_in_keys key return
await '^browser: account FirstDevice None$' ||
  fail "signing in with Google didn't make this the account's first device"
await '^browser: account On None$' || fail "the first device's recovery words didn't turn sync on"
settings_keyboard
on='^browser: account On None$'
send clickkey $(drawn_after "sync done" "$on")
[ "$(stored "SELECT count(*) FROM identities WHERE provider = 'google' AND subject = '$standin_sub'")" = 1 ] ||
  fail "the server didn't link the account to the Google account's sub"
[ "$(stored "SELECT count(*) FROM identities")" = 1 ] ||
  fail "the server linked more than the one Google account"

# Settings lists the Google sign-in once, and Remove…, asked first, unlinks it.
await "^\(\[[^]]*\] \)*browser: sync request GET $server_url/api/identities\$" ||
  fail "signed in, Settings didn't ask which sign-ins the account has"
at=$(drawn_after "sync remove-google" "$on") ||
  fail "Settings didn't show the account's Google sign-in"
../tools/window-screenshot.sh "$(app_pid)" "$out/sync-google-row.png" || fail "no window to capture"
removed=
if [ "$(uname)" = Linux ]; then
  post clickkey $at
  expect '^browser: dialog shown Remove Google sign-in?$' "Remove… didn't ask first"
  expect '^browser: dialog settled$' "Settings' dialog didn't settle"
  expect '^browser: dialog confirm at ' "Remove's question didn't say where its button is"
  send clickkey $(sed -n 's/^browser: dialog confirm at //p' "$log" | tail -1)
  expect '^browser: dialog confirmed$' "a click on Remove didn't remove the Google sign-in"
  await "^\(\[[^]]*\] \)*browser: sync request DELETE $server_url/api/identities/google\$" ||
    fail "Remove didn't ask the server to unlink Google"
  for _ in $(seq 50); do
    [ "$(stored "SELECT count(*) FROM identities")" = 0 ] && break
    sleep 0.1
  done
  [ "$(stored "SELECT count(*) FROM identities")" = 0 ] ||
    fail "the server still links the account to Google after Remove"
  removed=", removed it, after which the server linked nothing,"
else
  echo "skip: Remove… for the Google sign-in: Settings' own dialog says where its button is only on Linux."
fi
exec 5>&- 3>&-
quit
others=
# Nothing of Google's in any log, the stand-in's own included: its codes, its access tokens, its ID
# token, by the signature part it ends in ("standin-signature", base64url), or the client's secret.
for leaked in '4/standin-' 'ya29\.' 'c3RhbmRpbi1zaWduYXR1cmU' "$standin_secret"; do
  ! grep -l -- "$leaked" "$log" "$server_log" "$standin_log" 2> /dev/null | grep . ||
    fail "a log holds Google's code, token or the client's secret ($leaked)"
done
kill "$collector" "$ilo_server" 2>/dev/null || true
collector= ilo_server=
rm -rf "$scratch"
scratch=
echo "ok: Settings offered Continue with Google once the local server's discovery said so, a Google" \
  "sign-in left at the stand-in's page didn't finish, and one chosen there signed in as the" \
  "account's first device, linked by Google's sub alone; Settings listed it$removed and no log" \
  "held Google's code or tokens. Log: $log, the server's: $server_log, the stand-in's: $standin_log"
