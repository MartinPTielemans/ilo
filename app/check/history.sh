# Back and forward go through the tab's history from keys, with the page or the UI focused, from
# the header's buttons, which dim with nowhere to go, and from a swipe over the page. Reload loads
# the page again, and while a page loads, reload is stop, which ⌘., escape and the button do. A
# tab's row shows its page's favicon, and keeps it until the next page has loaded without one. A
# host typed in the command bar goes over https first, then http, and a local one over http.

# Back, forward, reload and stop. stuck.test leads to a server that takes connections and never
# answers, so its page keeps loading until it's stopped, and $plain to the check's own, which has no
# https. The profile remembers a host that fell back to http, so each run names a new one.
# $nearby is a local name for the check's server that no run typed before: Chrome's bar fills in a
# page typed once under the host and port typed, such as tabs' back.html under 127.0.0.1:$port, as
# one check.sh's runs share its server (#1148).
serve
plain="plain-$(date +%s).example.com"
nearby="history-$(date +%s).localhost"
python3 -I -c '
import socket, time
server = socket.socket()
server.bind(("127.0.0.1", 0))
server.listen()
print(server.getsockname()[1], flush=True)
time.sleep(600)' > "$out/port" &
stuck=$!
for _ in $(seq 50); do
  [ -s "$out/port" ] && break
  sleep 0.1
done
[ -s "$out/port" ] || fail "the server that never answers didn't start"
# The start page's favicon is a 16 × 16 orange square.
page='data:text/html,<title>loading</title>
<link rel="icon" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGUlEQVR42mN44GDwnxLMMGrAqAGjBgwXAwCb4E8fp7RFqwAAAABJRU5ErkJggg==">
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));</script>'
log="$out/history.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" \
  --host-resolver-rules="MAP stuck.test 127.0.0.1:$(cat "$out/port"), \
MAP $plain 127.0.0.1:$port, MAP $nearby 127.0.0.1:$port" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the history test page didn't draw"
tab=$(grep -m1 '^browser: today [0-9]' "$log" | cut -d' ' -f3)
await "^browser: page $tab back 0 forward 0 loading 0 data:" || fail "a fresh tab can go back or forward"
await "^browser: favicon $tab [1-9][0-9]* bytes\$" || fail "the start page's favicon didn't reach the UI"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: history. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The header's buttons, centred 23.5 below the window's top.
back_button="138 24"
forward_button="172 24"
reload_button="206 24"

go_from_bar example.com
expect_page example.com 1 0 "after loading a page"
# example.com has no favicon, so the start page's goes, but only once example.com has loaded.
expect "^browser: favicon $tab 0 bytes\$" "the start page's favicon stayed on example.com"
dropped=$(grep -n "^browser: favicon $tab 0 bytes\$" "$log" | head -1 | cut -d: -f1)
loaded=$(grep -n "^browser: page $tab back 1 forward 0 loading 0 https://example.com/\$" "$log" | head -1 | cut -d: -f1)
[ "$dropped" -gt "$loaded" ] || fail "the start page's favicon went before example.com had loaded"
go_from_bar example.org
expect_page example.org 1 0 "after loading another"
../tools/window-screenshot.sh "$pid" "$out/history.png" || fail "no window to capture"
history_key left back page
expect_page example.com 1 1 "after ⌘←"
history_key right forward page
expect_page example.org 1 0 "after ⌘→"
send click $back_button
expect '^browser: command back via sidebar$' "a click on back didn't go back"
expect_page example.com 1 1 "after a click on back"
send click $forward_button
expect '^browser: command forward via sidebar$' "a click on forward didn't go forward"
expect_page example.org 1 0 "after a click on forward"
# The click left the keyboard with the UI.
history_key left back ui
expect_page example.com 1 1 "after ⌘← with the UI focused"
history_key right forward ui
expect_page example.org 1 0 "after ⌘→ with the UI focused"
# A swipe between pages, as three fingers send one, goes back and forward over the page, and does
# nothing over the sidebar. Windows' driver has nothing to stand in for one yet (drive.cc).
if windows; then
  echo "skip: swipe between pages: the Windows driver can't send one yet."
else
  send pageswipe $page_point back
  expect "^browser: swiped back $tab\$" "a swipe over the page didn't reach the UI"
  expect_page example.com 1 1 "after a swipe back"
  send pageswipe $page_point forward
  expect "^browser: swiped forward $tab\$" "a swipe forward over the page didn't reach the UI"
  expect_page example.org 1 0 "after a swipe forward"
  send pageswipe $sidebar_point back
  none_since '^browser: swiped' || fail "a swipe over the sidebar went back"
fi

history_key r reload ui
expect "^browser: loaded $tab https://example.org/\$" "⌘R didn't load the page again"
send key r cmd shift
expect '^browser: command force-reload via ui$' "⇧⌘R didn't run force-reload"
expect "^browser: loaded $tab https://example.org/\$" "⇧⌘R didn't load the page again"
# The button is stop until the page says it has stopped loading, a little after its load (#1114).
expect '^browser: nav shows reload$' "the stop button didn't turn back into reload"
send click $reload_button
expect '^browser: command reload via sidebar$' "a click on reload didn't reload"
expect "^browser: loaded $tab https://example.org/\$" "a click on reload didn't load the page again"

# While a page loads, the URL field shows where it's going and reload is stop.
send click $page_point
for stop in "key . cmd|page|⌘." "key escape|page|escape" "click $reload_button|sidebar|a click on stop"; do
  IFS='|' read -r action from what << EOF
$stop
EOF
  go_from_bar stuck.test
  # Chrome tries https first only for public hosts.
  expect "^browser: page $tab back 1 forward 0 loading 1 http://stuck.test/\$" "stuck.test didn't show as loading"
  if [ "$from" = sidebar ]; then
    ../tools/window-screenshot.sh "$pid" "$out/loading.png" || fail "no window to capture"
  fi
  send $action
  expect "^browser: command stop via $from\$" "$what while loading didn't run stop"
  expect_page example.org 1 0 "after $what stopped the load"
done

# A host typed without a scheme goes over https first, and over http once https fails; a local one
# goes over http at once. The check's server answers a TLS handshake as a bad request.
tls() {
  grep -a -c 'code 400, message Bad request' "$out/site.log" || true
}
before=$(tls)
go_from_bar "$plain"
expect "^browser: page $tab back 1 forward 0 loading 0 http://$plain/\$" \
  "$plain didn't come over http: $(grep "^browser: page $tab" "$log" | tail -1)"
[ "$(tls)" -gt "$before" ] || fail "$plain came over http without trying https first"
before=$(tls)
go_from_bar "$nearby:$port"
expect "^browser: page $tab back 1 forward 0 loading 0 http://$nearby:$port/\$" \
  "$nearby:$port didn't come over http: $(grep "^browser: page $tab" "$log" | tail -1)"
[ "$(tls)" = "$before" ] || fail "$nearby:$port was tried over https"
exec 3>&-
quit
kill "$stuck"
echo "ok: ⌘← ⌘→ and the header's buttons go back and forward, with the page or the UI focused, and so does a swipe over the page but not over the sidebar; reload and force reload load the page again; ⌘., escape and the stop button stop a load; the row keeps a page's favicon until the next page has loaded without one; a host typed in the command bar goes over https first and falls back to http, and a local one goes over http at once. Windows: $out/history.png, $out/loading.png"
