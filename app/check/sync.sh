# A profile signed out of the account starts no sync traffic. Its sync service is there, disabled
# for want of an account. Neither sync's engine nor the core asks anything of the sync server it's
# given, the check's own, or of Google's, from launch past the 10 s sync's start waits for, until
# the app has quit, as the core's log, the server's and Chromium's net log of the whole run show.

log="$out/sync.log"
netlog="$out/sync-net.json"
serve
rm -f "$saved" "$launch_frame" "$profile_dir/Account" "$netlog"
asked=$(grep -c '"[A-Z]* /' "$out/site.log" || true)
started=$(date +%s)
BROWSER_URL='data:text/html,<title>sync</title>' "$browser" \
  --sync-url="$site" --log-net-log="$netlog" < /dev/null > "$log" 2>&1 &
pid=$!
await_load
await 'browser: sync service ' ||
  fail "the profile has no sync service, or the core didn't say what it does"
grep -q 'browser: sync service disabled, not signed in$' "$log" ||
  fail "signed out, sync isn't disabled for want of an account: $(grep 'browser: sync service ' "$log" | head -1)"
# What a signed-out profile mustn't do: its sync's engine making a request, or the core one of the
# account's, or the relay's.
traffic='browser: sync \(engine\|request\|relay\) '
while [ $(($(date +%s) - started)) -lt 12 ]; do
  grep -q "$traffic" "$log" && break
  kill -0 "$pid" 2>/dev/null || gone
  sleep 0.5
done
quit
grep -q "$traffic" "$log" && fail "signed out, sync made a request: $(grep "$traffic" "$log" | head -1)"
grep 'browser: sync service ' "$log" | grep -qv 'disabled, not signed in$' &&
  fail "signed out, sync's service went on to $(grep 'browser: sync service ' "$log" | grep -v 'disabled, not signed in$' | head -1)"
[ "$(grep -c '"[A-Z]* /' "$out/site.log" || true)" = "$asked" ] ||
  fail "signed out, the browser asked the sync server: $(grep '"[A-Z]* /' "$out/site.log" | tail -1)"
[ -s "$netlog" ] || fail "Chromium wrote no net log"
# Every address the net log names, and Google's but the security components' updates.
urls=$(grep -o '"url":"[a-z]*://[^"]*"' "$netlog" | cut -d'"' -f4 | sort -u)
google=$(printf '%s\n' "$urls" | google_asked)
[ -z "$google" ] || fail "signed out, the browser asked Google's servers: $(printf "%s\n" "$google" | tr "\n" " ")"
ours=$(printf '%s\n' "$urls" | grep "^${site%/}" || true)
[ -z "$ours" ] || fail "signed out, the browser asked the sync server: $(printf "%s\n" "$ours" | tr "\n" " ")"
echo "ok: signed out, sync's service stayed disabled and asked nothing of $site or Google's servers in $(($(date +%s) - started)) s. Log: $log, net log: $netlog"
