# check.sh runs this at 607.
# Shared links (#680), the sending half, on an ilo-server of the run's own (ilo-server.sh), whose
# share routes check/sync-proxy.py stands in for until the server has them. Signed out, Share Space
# says to sign in, and its Open Sync Settings shows Settings › Sync, where the run signs in. The
# first share then says who can see what, and Copy Link sends the Space's pinned tab, sealed, and
# puts its link on the clipboard; the next share goes at once and replaces it at the same address.
# The link, opened in a new tab, shows the share and offers to add it, and the offer is answered no
# once the window shows another tab. Settings › Sync lists the link, whose Copy Link copies it
# again. On Linux, Unshare asks first, in Settings' own dialog, which says where its button is: a
# server that refuses it leaves the row saying so, captured as share-send-not-unshared.png, and
# the next one goes, and the list is empty. The Mac's and Windows' alerts choose Cancel on return,
# so there it's unrun. No line holds the link's id or key.
# Needs the core's shares (#680) in an engine built with them, and ILO_SERVER, an ilo-server built
# from its repo, to sign in to.

if [ ! -f ../core/share.cc ]; then
  echo "skip: share-send. This checkout has no share core (#680) yet."
  exit 0
fi
if ! engine_has ../core/share.cc ../core/share_snapshot.cc ../core/sync.cc ../core/api.cc \
  ../core/main_delegate.cc; then
  echo "skip: share-send. The engine was built before this checkout's share core (#680)."
  exit 0
fi
if [ -z "${ILO_SERVER:-}" ]; then
  echo "skip: share-send. It needs ILO_SERVER, an ilo-server built from its repo, to sign in to."
  exit 0
fi
if sway_xwayland; then
  echo "skip: share-send. It runs on Linux under Xvfb and Wayland, on Windows and on the Mac so far, where the run reaches the clipboard."
  exit 0
fi
log="$out/share-send.log"
step=0
rm -f "$saved" "$launch_frame" "$profile_dir/Account"
sync_faults="$out/share-send-faults"
. check/ilo-server.sh

# shares <method> <route> <code>: how many such answers the stand-in routes gave.
shares() { grep -c "^shares $1 $2 $3\$" "$proxy_log" || true; }
# Waits up to 3 s for the clipboard to hold a line matching $1, failing with $2.
await_clipboard() {
  for _ in $(seq 30); do
    clipboard
    printf '%s\n' "$clipped" | grep -qx -- "$1" && return 0
    sleep 0.1
  done
  fail "$2: the clipboard holds \"$clipped\""
}

BROWSER_DRIVE=1 BROWSER_URL="$server_url/page/trip" "$browser" --sync-url="$server_url" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$ilo_server" 2>/dev/null || true
  ilo_server=
  echo "skip: share-send. Browser wasn't the active app, so its keys went elsewhere."
  exit 0
fi
# A Space shares its pinned tabs.
trip=$(today)
send key d cmd
expect '^browser: command toggle-pin via page$' "⌘D with the page focused didn't reach the UI"
expect "^browser: pinned $trip\$" "⌘D didn't pin the page: the pinned tabs are \"$(pinned)\""

# Signed out: Share Space says to sign in, and its button shows Settings › Sync.
bar_command share-space "Share Space" "Share Space"
expect '^browser: share SignIn$' "Share Space, signed out, didn't say to sign in"
expect '^browser: dialog shown Sign in to share links$' "Share Space, signed out, asked nothing"
expect '^browser: dialog settled$' "the dialog saying to sign in didn't settle"
send key return
expect '^browser: dialog confirmed$' "return didn't choose Open Sync Settings"
expect '^browser: settings shown ' "Open Sync Settings didn't show Settings"
at=$(drawn "sync sign-in") || fail "Open Sync Settings didn't show Settings › Sync's Sign In"
send clickkey $at
expect '^browser: account SigningIn None$' "Sign In didn't start signing in"
await 'browser: sync sign-in loaded /authorize$' || fail "the sign-in window didn't load the server's page"
sign_in first
await '^browser: account FirstDevice None$' || fail "signing in didn't make this the account's first device"
settings_keyboard
at=$(drawn "sync done") || fail "Settings › Sync didn't draw Done with the words"
send clickkey $at
await '^browser: shares listed 0$' || fail "the account, on, didn't list its links"

# The first share says who can see what; Copy Link sends it, a new link, on the clipboard.
forward
bar_command share-space "Share Space" "Share Space"
expect '^browser: share Explain$' "the account's first share didn't say who can see what"
expect '^browser: dialog shown Share “.*” as a link?$' "the first share asked nothing"
expect '^browser: dialog settled$' "the first share's dialog didn't settle"
send key return
expect '^browser: dialog confirmed$' "return didn't choose Copy Link"
expect '^browser: share asked [0-9][0-9]*$' "Copy Link didn't ask the core for a link"
made=$(sed -n 's/^browser: share asked //p' "$log" | tail -1)
expect "^browser: share made $made: share copied replaced false left out 0\$" \
  "the core didn't make the link: $(grep "^browser: share made $made: " "$log" || echo "nothing said")"
link_line="$server_url/s/[A-Za-z0-9_-]\{22\}#[A-Za-z0-9_-]\{43\}"
await_clipboard "$link_line" "the link made isn't on the clipboard"
link=$clipped
[ "$(shares POST list 201)" = 1 ] || fail "the server wasn't sent one new link: $(tr '\n' ' ' < "$proxy_log")"
expect '^browser: toast shown share copied replaced false left out 0' "the link made said nothing"

# Again, at once, over the same link.
bar_command share-space "Share Space" "Share Space"
expect '^browser: share Share$' "the account's second share asked again"
expect '^browser: share asked [0-9][0-9]*$' "the second share didn't ask the core for a link"
made=$(sed -n 's/^browser: share asked //p' "$log" | tail -1)
expect "^browser: share made $made: share copied replaced true left out 0\$" \
  "the second share didn't replace the first: $(grep "^browser: share made $made: " "$log" || echo "nothing said")"
[ "$(shares PUT one 200)" = 1 ] || fail "the server wasn't sent the link again: $(tr '\n' ' ' < "$proxy_log")"
await_clipboard "$link" "the replaced link isn't at the same address"

# The link opens as a share, offered; showing another tab answers no.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "$link"
send key return
expect '^browser: share page shown$' "the link didn't show the core's page for its share"
expect '^browser: share found$' "the core's page for the share committed, but no share was found"
opened=$(today | cut -d' ' -f1)
expect "^browser: offer $opened share\$" "the share opened in tab $opened wasn't offered"
../tools/window-screenshot.sh "$pid" "$out/share-send-offer.png" > /dev/null 2>&1 || true
switch "1 cmd" "$trip"
expect "^browser: offer $opened answered NotNow as another tab shows\$" \
  "showing another tab didn't answer the share's offer"
expect "^browser: share offer $opened add false added false\$" "the offer's no didn't reach the core"
expect "^browser: offer $opened gone\$" "the core didn't take the share's offer back"

# Settings › Sync lists the link, and its Copy Link copies it.
# Settings is open still, behind the browser window.
send menu Settings…
settings_keyboard
at=$(drawn_after "sync share 0 copy" '^browser: shares listed 1$') ||
  fail "Settings › Sync didn't draw the link's Copy Link: $(grep '^browser: shares listed ' "$log" | tail -1)"
send clickkey $at
expect '^browser: share link copied$' "Copy Link didn't copy the link"
await_clipboard "$link" "Copy Link didn't put the link on the clipboard"

unshared=", and Unshare is unrun, as the system's alert takes return for Cancel"
if [ "$(uname)" = Linux ]; then
  # Refused, the row says so, and the link still opens.
  echo unshare > "$sync_faults"
  at=$(drawn_after "sync share 0 unshare" '^browser: shares listed 1$') ||
    fail "Settings › Sync didn't draw the link's Unshare"
  send clickkey $at
  expect '^browser: dialog shown Unshare “.*”?$' "Unshare didn't ask first"
  expect '^browser: dialog settled$' "Settings' dialog didn't settle"
  expect '^browser: dialog confirm at ' "Unshare's question didn't say where its button is"
  send clickkey $(sed -n 's/^browser: dialog confirm at //p' "$log" | tail -1)
  expect '^browser: dialog confirmed$' "a click on Unshare didn't unshare"
  expect '^browser: unshare asked$' "Unshare didn't ask the core"
  expect '^browser: unshared Some(Server)$' "the server's refusal didn't reach the app"
  expect '^browser: share Some(0) not unshared Server$' "the link's row didn't take the refusal"
  ../tools/window-screenshot.sh "$pid" "$out/share-send-not-unshared.png" > /dev/null 2>&1 || true
  [ "$(shares DELETE one 503)" = 1 ] || fail "the server wasn't asked to unshare: $(tr '\n' ' ' < "$proxy_log")"
  # Then it goes, and the list is empty.
  : > "$sync_faults"
  at=$(drawn_after "sync share 0 unshare" '^browser: share Some(0) not unshared Server$') ||
    fail "Settings › Sync didn't draw Unshare again"
  send clickkey $at
  expect '^browser: dialog settled$' "Settings' second dialog didn't settle"
  expect '^browser: dialog confirm at ' "the second Unshare's question didn't say where its button is"
  send clickkey $(sed -n 's/^browser: dialog confirm at //p' "$log" | tail -1)
  expect '^browser: dialog confirmed$' "a second click on Unshare didn't unshare"
  expect '^browser: unshared None$' "the unshare didn't go"
  expect '^browser: shares listed 0$' "the link, unshared, stayed on the account's list"
  [ "$(shares DELETE one 204)" = 1 ] || fail "the server didn't delete the link: $(tr '\n' ' ' < "$proxy_log")"
  unshared=", a refused Unshare left the row saying so, and the next one went"
fi

# Nothing of the link in any line: not its id, nor its key.
id=$(printf '%s\n' "$link" | sed 's|.*/s/\([^#]*\)#.*|\1|')
key=${link#*#}
for secret in "$id" "$key"; do
  for file in "$log" "$proxy_log"; do
    ! grep -qF -- "$secret" "$file" ||
      fail "a line of $file holds the link's id or key: $(grep -nF -- "$secret" "$file" | head -3 | sed "s|$secret|<that>|g")"
  done
done

exec 3>&-
quit
rm -f "$profile_dir/Account"
kill "$ilo_server" 2>/dev/null || true
ilo_server=
kill "$collector" 2>/dev/null || true
collector=
echo "ok: signed out, Share Space said to sign in; signed in, it explained, sent a link, copied it," \
  "and replaced it at the same address; the link offered its share, answered no as another tab showed;" \
  "Settings › Sync listed and copied it$unshared. Logs: $log, $proxy_log"
