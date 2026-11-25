# check.sh runs this at 60.
# Back and forward go through the tab's history from keys, with the page or the UI focused, from
# the header's buttons, which dim with nowhere to go, and from a swipe over the page. Reload loads
# the page again, and while a page loads, reload is stop, which ⌘., escape and the button do. A
# tab's row shows its page's favicon, and keeps it until the next page has loaded without one. A
# host typed in the command bar goes over https first, then http, or from Chromium 157 Chrome's
# warning before http, whose dialog has the keyboard and still lets ⌘L, ⌘T and ⌘W reach the app,
# also one shown again as the tab in front closes, and leaves the command bar every key as it shows,
# and leaves a page split beside it the click into it and the typing after, and shows with the
# keyboard on a click on its own pane's page, which never hears it, and a local one over http. One
# typed with http:// loads over http as written, with no https try and no warning, as in Chrome's
# address bar. Only what the user typed counts: an agent's open_tab
# and navigate, and a link, to an http:// address, are tried over https first. From 157 the agent's
# page then never opens and has no warning: its tab is put back, the log has the site, and the
# user's window keeps the keyboard. A right-click on reload lists Reload, Hard
# Reload and Clear Site Data and Reload, the last deleting the site's cookies, storage and cache,
# not another site's, before the page loads again.

# Back, forward, reload and stop. stuck.test leads to a server that takes connections and never
# answers, so its page keeps loading until it's stopped, and $plain to the check's own, which has no
# https. The profile remembers a host that fell back to http, so each run names a new one, and
# $written is another such host, typed with http://, and $agent_opened, $agent_moved and $linked
# three more, which an agent and a link go to, and $barred one a link goes to under the command bar.
# Each leads to the check's server, so none waits on the machine's DNS, which has no address for
# them: https fails at the handshake, and Chrome's warning before http comes as it would.
# $nearby is a local name for the check's server that no run typed before: Chrome's bar fills in a
# page typed once under the host and port typed, such as tabs' back.html under 127.0.0.1:$port, as
# one check.sh's runs share its server (#1148).
serve
plain="plain-$(date +%s).example.com"
nearby="history-$(date +%s).localhost"
written="written-$(date +%s).example.com"
agent_opened="agent-opened-$(date +%s).example.com"
agent_moved="agent-moved-$(date +%s).example.com"
linked="linked-$(date +%s).example.com"
barred="barred-$(date +%s).example.com"
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
# The example.com and example.org pages the steps go back and forward between come from a server of
# the run's own, which a runner always reaches (#3144). Neither has a favicon, as example.com has none.
serve_tls example '<meta charset=utf-8><link rel=icon href="data:,"><title>Example Domain</title>
<h1>Example Domain</h1>' example.com example.org
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" \
  --host-resolver-rules="MAP stuck.test 127.0.0.1:$(cat "$out/port"), \
MAP $plain 127.0.0.1:$port, MAP $nearby 127.0.0.1:$port, \
MAP $written 127.0.0.1:$port, MAP $agent_opened 127.0.0.1:$port, \
MAP $agent_moved 127.0.0.1:$port, MAP $linked 127.0.0.1:$port, \
MAP $barred 127.0.0.1:$port, $tls_rules" --ignore-certificate-errors-spki-list="$tls_key" \
  < "$out/drive" > "$log" 2>&1 &
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
# The header's buttons, centred 24 below the window's top.
back_button="158 24"
forward_button="182 24"
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
  swipe=$((step + 1))
  send pageswipe $sidebar_point back
fi
history_key r reload ui
[ -z "${swipe:-}" ] || none_since '^browser: swiped' "$swipe" || fail "a swipe over the sidebar went back"
expect "^browser: loaded $tab https://example.org/\$" "⌘R didn't load the page again"
send key r cmd shift
expect '^browser: command force-reload via ui$' "⇧⌘R didn't run force-reload"
expect "^browser: loaded $tab https://example.org/\$" "⇧⌘R didn't load the page again"
# The button is stop until the page says it has stopped loading, a little after its load (#1114).
# A press hits the button drawn last, and a reload from the run's own server can start and end
# between two frames, drawing no stop at all (#3362): so once the page has stopped loading, the
# click waits until the last button drawn is reload, whether or not a stop came before it.
expect_page example.org 1 0 "after ⇧⌘R"
for _ in $(seq 100); do
  [ "$(last 'nav shows')" = reload ] && break
  sleep 0.1
done
[ "$(last 'nav shows')" = reload ] || fail "the stop button didn't turn back into reload"
send click $reload_button
expect '^browser: command reload via sidebar$' "a click on reload didn't reload"
expect "^browser: loaded $tab https://example.org/\$" "a click on reload didn't load the page again"

# A mouse's side buttons over the page go back and forward, as Chrome's do (#688). Over the sidebar
# they show another Space, which the spaces run checks.
# A side button's click gives the page the keyboard, so this comes after the keys with the UI focused.
send sideclick $page_point back
expect_page example.com 1 1 "after the back side button over the page"
send sideclick $page_point forward
expect_page example.org 1 0 "after the forward side button over the page"

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

# A host typed without a scheme goes over https first; a local one goes over http at once. Once
# https fails, Chromium 156 falls back to http, and from 157, whose HTTPS-First balanced mode is on
# by default, Chrome's "Connection is not secure" warning comes instead, at the http address, with
# ERR_BLOCKED_BY_CLIENT, until the user continues to the site. The check's server answers a TLS
# handshake as a bad request, which it names by where spaces fall in the handshake's random bytes:
# a bad request version, a bad HTTP/0.9 request type or bad request syntax.
tls() {
  grep -a -c 'code 400, message Bad ' "$out/site.log" || true
}
await '^browser: settings about Version .*, on Chromium [0-9]' ||
  fail "the engine's version wasn't said"
major=$(sed -n 's/^browser: settings about .*on Chromium \([0-9]*\)\..*/\1/p' "$log" | head -1)
before=$(tls)
go_from_bar "$plain"
expect "^browser: page $tab back 1 forward 0 loading 0 http://$plain/\$" \
  "$plain didn't come to its http address: $(grep "^browser: page $tab" "$log" | tail -1)"
if [ "$major" -ge 157 ]; then
  expect "^browser: page $tab trouble network -20\$" \
    "$plain had no warning before http: $(grep "^browser: loaded $tab" "$log" | tail -1)"
  expect "^browser: loaded $tab chrome-error://chromewebdata/\$" \
    "the warning before http didn't load"
else
  expect "^browser: loaded $tab http://$plain/\$" \
    "$plain's own page didn't load over http: $(grep "^browser: loaded $tab" "$log" | tail -1)"
fi
[ "$(tls)" -gt "$before" ] || fail "$plain came to http without trying https first"
before=$(tls)
# go_from_bar's steps, with a ⌘L of its own to name: Chrome's warning is a tab dialog of Chromium's
# that holds the keyboard where Wayland gives it one, and its keys are asked about as a page's are
# (#2416).
page="$plain's page"
[ "$major" -lt 157 ] || page="Chrome's warning before http"
send key l cmd
expect '^browser: command bar shown$' "⌘L on $page didn't open the command bar"
send type "$nearby:$port"
send key return
expect '^browser: command bar hidden$' "going to $nearby:$port didn't close the command bar"
expect "^browser: page $tab back 1 forward 0 loading 0 http://$nearby:$port/\$" \
  "$nearby:$port didn't come over http: $(grep "^browser: page $tab" "$log" | tail -1)"
[ "$(tls)" = "$before" ] || fail "$nearby:$port was tried over https"
# The user wrote http://, so the address is their choice, as Chrome's bar counts it: neither 156's
# HTTPS-Upgrades nor 157's HTTPS-First tries https, and 157 shows no warning (#2417).
go_from_bar "http://$written"
expect "^browser: page $tab back 1 forward 0 loading 0 http://$written/\$" \
  "http://$written didn't come to its address: $(grep "^browser: page $tab" "$log" | tail -1)"
expect "^browser: loaded $tab http://$written/\$" \
  "http://$written didn't load as written$([ "$major" -lt 157 ] || echo ", without Chrome's warning"): \
$(grep "^browser: loaded $tab" "$log" | tail -1)"
[ "$(tls)" = "$before" ] || fail "http://$written was tried over https"

# Only an address the user typed counts (#2417): a link, and later an agent's open_tab and
# navigate, to an http:// address go as a bare host does, over https first, then to 156's http or
# 157's warning.
# Each goes to a host of its own, never $written: Chromium lets a host's later loads through once
# the user typed it with http://, or once 156 fell back to http for it.
# Waits for the tab $1 to come to http://$2/ after an https try, as $plain did, for $3.
upgraded() {
  expect "^browser: page $1 back [0-9]* forward 0 loading 0 http://$2/\$" \
    "$3 didn't come to http://$2/: $(grep "^browser: page $1 " "$log" | tail -1)"
  if [ "$major" -ge 157 ]; then
    # The core's line, which comes at each warning: the app's trouble line comes only when the
    # tab's trouble changes.
    expect "browser: tab $1 error -20\$" \
      "$3 to http://$2/ had no warning before http: $(grep "^browser: loaded $1 " "$log" | tail -1)"
  else
    expect "^browser: loaded $1 http://$2/\$" \
      "$3 to http://$2/ didn't load over http: $(grep "^browser: loaded $1 " "$log" | tail -1)"
  fi
  [ "$(tls)" -gt "$before" ] || fail "$3 came to http://$2/ without trying https first"
}
# The link clicks itself once its page has loaded: a page's own click on a link loads as a link.
printf '%s' "<title>link</title><a id=go href=\"http://$linked/\">$linked</a>
<script>onload = () => setTimeout(() => go.click());</script>" > "$out/site/link.html"
before=$(tls)
go_from_bar "$nearby:$port/link.html"
expect "^browser: loaded $tab http://$nearby:$port/link.html\$" \
  "the page with the link didn't load: $(grep "^browser: loaded $tab " "$log" | tail -1)"
upgraded "$tab" "$linked" "a link"

# A right-click on reload lists Reload, Hard Reload and Clear Site Data and Reload (#1959), shown
# light and dark. stored.html says whether its site has the cookie, the localStorage value and the
# IndexedDB entry named by its query, then stores them: $nearby's first, another site that keeps
# them, then 127.0.0.1's, which the third item deletes before the page loads again.
cat > "$out/site/stored.html" << 'PAGE'
<title>loading</title><script>
const key = location.search.slice(1);
const db = () => new Promise((ok, no) => {
  const open = indexedDB.open(key, 1);
  open.onupgradeneeded = () => open.result.createObjectStore("items");
  open.onsuccess = () => ok(open.result);
  open.onerror = () => no(open.error);
});
const kept = async () => {
  const base = await db();
  const had = await new Promise(ok => {
    const get = base.transaction("items").objectStore("items").get(key);
    get.onsuccess = () => ok(get.result === 1);
    get.onerror = () => ok(false);
  });
  await new Promise(ok => {
    const put = base.transaction("items", "readwrite");
    put.objectStore("items").put(1, key);
    put.oncomplete = ok;
  });
  base.close();
  return had;
};
(async () => {
  const cookie = document.cookie.split("; ").includes(key + "=1") ? 1 : 0;
  const local = localStorage.getItem(key) === "1" ? 1 : 0;
  const entry = await kept() ? 1 : 0;
  document.cookie = key + "=1; max-age=3600; path=/";
  localStorage.setItem(key, "1");
  document.title = `stored cookie ${cookie} local ${local} idb ${entry}`;
})();
</script>
PAGE
# Not $key, which send sets to the key window.
stored_key="kept$(date +%s)"
stored() { # <url> <cookie local idb, as 0 or 1> <what failed>
  expect "^browser: title $tab stored cookie ${2% * *} local $(echo "$2" | cut -d' ' -f2) idb ${2##* }\$" \
    "$3: $(grep "^browser: title $tab " "$log" | tail -1)"
}
reload_menu="$(cased 'Reload, Hard Reload, Clear Site Data and Reload')"
# Picks the item $1 of reload's menu, which it opens with a right-click.
reload_item() {
  post rightclick $reload_button
  expect "^browser: menu shown $reload_menu\$" \
    "a right-click on reload didn't show its menu: $(grep '^browser: menu shown' "$log" | tail -1)"
  send choose "$(cased "$1")"
}
go_from_bar "http://$nearby:$port/stored.html?$stored_key"
stored "$nearby" "0 0 0" "$nearby's page found its data before it stored it"
go_from_bar "http://127.0.0.1:$port/stored.html?$stored_key"
stored 127.0.0.1 "0 0 0" "127.0.0.1's page found its data before it stored it"
send key r cmd
stored 127.0.0.1 "1 1 1" "⌘R lost the page's cookie, localStorage or IndexedDB"
for look in dark light; do
  item=Dark
  [ $look = dark ] || item=Light
  send menu "Appearance › $item"
  expect "^browser: appearance $look\$" "View › Appearance › $item didn't change the appearance"
  post rightclick $reload_button
  expect "^browser: menu shown $reload_menu\$" \
    "a right-click on reload didn't show its menu: $(grep '^browser: menu shown' "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/reload-menu-$look.png" || fail "no window to capture"
  expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
  send key escape
  expect '^browser: menu closed' "escape didn't close reload's menu"
done
send menu "Appearance › Automatic"
reload_item Reload
expect '^browser: command reload via sidebar$' "the menu's Reload didn't reload"
stored 127.0.0.1 "1 1 1" "the menu's Reload lost the page's data"
reload_item "Hard Reload"
expect '^browser: command force-reload via sidebar$' "the menu's Hard Reload didn't force a reload"
stored 127.0.0.1 "1 1 1" "the menu's Hard Reload lost the page's data"
reload_item "Clear Site Data and Reload"
expect '^browser: command clear-site-data via sidebar$' "Clear Site Data and Reload didn't run"
stored 127.0.0.1 "0 0 0" "Clear Site Data and Reload kept 127.0.0.1's cookie, localStorage or IndexedDB"
go_from_bar "http://$nearby:$port/stored.html?$stored_key"
stored "$nearby" "1 1 1" "Clear Site Data and Reload on 127.0.0.1 lost $nearby's data"
# The agent's open_tab and navigate, in a Space of its own that isn't shown, with the user's tab on
# a page of the check's own. From Chromium 157 a page that answers only over http never opens in
# an agent's tab: where Chrome's warning before http would come, whose Continue to site is the
# user's to press, the core shows none, puts the tab back, closed if open_tab just made it and else
# back on the page it had, and makes one activity entry, which names the site by its host alone.
# Then ⌘L still opens the command bar in the user's window: an agent's tab never takes the user's
# keyboard, as its warning took it on Windows (#3164).
agent_steps() {
  # The agent's tab has no warning, and no window shows the tab: the user's window, which shows
  # the user's tab, has none either (#3164).
  no_agent_dialog() {
    [ "$major" -ge 157 ] || return 0
    send layout
    shown=$(since | sed -n 's/^browser: layout child //p' | head -1)
    [ -z "$shown" ] ||
      fail "a warning before http, after $1 in the agent's own Space, showed at $shown in the user's window, which shows the user's tab: $(since | grep '^browser: layout' | tr '\n' ' ')"
  }
  # The agent's tab $1 was stopped on its way to http://$2/, after an https try, by $3, the
  # driver's command $4: no warning, and one entry in the log, by the host alone.
  not_opened() {
    expect "browser: tab $1 error -20\$" \
      "$3 to http://$2/ wasn't stopped before http: $(grep "^browser: loaded $1 " "$log" | tail -1)" "$4"
    [ "$(tls)" -gt "$before" ] || fail "$3 came to http://$2/ without trying https first"
    expect "browser: tab $1 not secure, not opened: an agent's tab\$" \
      "the core didn't take the page $3 was stopped at, http://$2/, from Chrome's warning" "$4"
    entry="^browser: agent activity claude-code $1 open_page site=$2 refused: not_secure\$"
    expect "$entry" \
      "the page $3 never opened, http://$2/, had no activity entry: $(grep '^browser: agent activity claude-code ' "$log" | tail -1)" "$4"
    expect "^browser: agent logged claude-code open_page $2\$" \
      "the log didn't take the entry of http://$2/, which $3 never opened: $(grep '^browser: agent logged ' "$log" | tail -1)" "$4"
  }
  # One entry for each page that never opened, once the steps after it are through: $1 is the tab,
  # $2 the host and $3 what went there.
  one_entry() {
    entries=$(grep -c "^browser: agent activity claude-code $1 open_page site=$2 refused: not_secure\$" "$log" || true)
    [ "$entries" = 1 ] || fail "the page $3 never opened, http://$2/, has $entries activity entries, not one"
  }
  send agentspace claude-code Claude Code
  expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
  before=$(tls)
  send agentopen claude-code "http://$agent_opened/"
  opened=$step
  expect '^browser: agent claude-code opened [1-9][0-9]*$' "the agent didn't open http://$agent_opened/"
  agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
  if [ "$major" -ge 157 ]; then
    not_opened "$agent_tab" "$agent_opened" "the agent's open_tab" "$opened"
    # The tab open_tab made had no page before this one, so it closes.
    expect "^browser: tab $agent_tab closed\$" \
      "the tab the agent's open_tab made for http://$agent_opened/, which never opened, stayed open" "$opened"
    no_agent_dialog "its open_tab"
    one_entry "$agent_tab" "$agent_opened" "the agent's open_tab"
    # A tab with a page of the check's own, for the navigate to leave and be put back on.
    printf '%s' '<title>agent</title>' > "$out/site/agent.html"
    closed_tab=$agent_tab
    send agentopen claude-code "http://$nearby:$port/agent.html"
    expect "^browser: agent claude-code opened [1-9][0-9]*\$" "the agent didn't open http://$nearby:$port/agent.html"
    agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
    [ "$agent_tab" != "$closed_tab" ] || fail "the agent's second open_tab opened no tab of its own"
    expect "^browser: loaded $agent_tab http://$nearby:$port/agent\\.html\$" \
      "the agent's tab didn't load http://$nearby:$port/agent.html: $(grep "^browser: loaded $agent_tab " "$log" | tail -1)"
  else
    upgraded "$agent_tab" "$agent_opened" "the agent's open_tab"
  fi
  before=$(tls)
  send agentnavigate claude-code "$agent_tab" "http://$agent_moved/"
  moved=$step
  expect "^browser: agent claude-code navigated $agent_tab: 1\$" \
    "the core didn't take the agent's navigate to http://$agent_moved/"
  if [ "$major" -ge 157 ]; then
    not_opened "$agent_tab" "$agent_moved" "the agent's navigate" "$moved"
    # The tab had a page, so it goes back to it, with the page that never opened ahead of it.
    expect "^browser: page $agent_tab back 0 forward 1 loading 0 http://$nearby:$port/agent\\.html\$" \
      "the agent's tab wasn't put back on http://$nearby:$port/agent.html after its navigate to http://$agent_moved/ never opened: $(grep "^browser: page $agent_tab " "$log" | tail -1)" "$moved"
    no_agent_dialog "its navigate"
    one_entry "$agent_tab" "$agent_moved" "the agent's navigate"
    agent_at="put back from a page that never opened"
  else
    upgraded "$agent_tab" "$agent_moved" "the agent's navigate"
    agent_at="loaded over http"
  fi
  send key l cmd
  expect '^browser: command bar shown$' "⌘L in the user's window, with the agent's tab $agent_at, didn't open the command bar"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘L opened in the user's window"
}
# Where Chrome's warning before http is, from the window's top-left corner, and the window's height.
warning_box() {
  send layout
  warning=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n 's/^browser: layout child //p' |
    head -1)
  [ -n "$warning" ] || fail "Chrome's warning before http wasn't a dialog over the page"
  warning_x=${warning%%,*} warning_y=${warning#*,}
  warning_y=${warning_y%% *} warning_size=${warning#* }
  warning_width=${warning_size%%x*} warning_height=${warning_size#*x}
  window_height=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
    sed -n 's/^browser: layout content [0-9]*x\([0-9]*\).*/\1/p' | head -1)
}
# With Chrome's warning up, ⌘T opens the command bar for a new tab and escape closes it, and ⌘W
# closes the tab, as from a page (#2416). Where the warning has the keyboard is said, since only
# Wayland gives it one without a click: elsewhere the UI keeps the keyboard it had since the click
# on stop, and GPUI's own bindings take the keys. A click on the warning's top edge gives it the
# keyboard there, as the user's would, so its keys are the dialog's on every session.
warned() {
  go_from_bar "$plain"
  # The core says a tab's trouble as it changes, and a tab still on the warning keeps it.
  expect "^browser: loaded $tab chrome-error://chromewebdata/\$" "$plain's warning before http didn't load again"
  [ "$(grep "^browser: page $tab trouble " "$log" | tail -1)" = "browser: page $tab trouble network -20" ] ||
    expect "^browser: page $tab trouble network -20\$" "$plain had no warning before http again"
  send state
  echo "note: with Chrome's warning before http up, the keyboard is in $key, on $focus"
  [ "$key" != NativeWidgetMacNSWindow ] || return 0
  warning_box
  send click $((warning_x + warning_width / 2)) $((warning_y + 8))
  [ "$key" = NativeWidgetMacNSWindow ] ||
    fail "a click on Chrome's warning before http at $warning didn't give it the keyboard: $key, $focus"
}
# The way a shortcut on the warning reaches the UI, one way an OS: on the Mac AppKit offers a ⌘
# shortcut to the browser window's views too, so GPUI runs it from the UI; on Linux and Windows the
# key goes to the warning alone, whose keys the core asks about as a page's (dialog_keys.cc).
route=page
[ "$(uname)" != Darwin ] || route=ui
# Command $1 ran once since the last key, $2, and by this OS's way. Read once its effect is logged.
ran_on_warning() {
  ran_once "$1" "$2 on Chrome's warning before http"
  since | grep -q "^browser: command $1 via $route\$" ||
    fail "$2 on Chrome's warning before http ran $1 $(since | sed -n "s/^browser: command $1 //p"), not via $route"
}
# A page beside the warning in a split keeps a click into it and the typing after. On X11 a tab's
# dialog takes the keyboard back after a focus change that leaves no view focused (#2926), and a
# page is no view: the click into the page beside it was at risk of sending its keys to the warning
# in the other pane. The warning is clicked first, so it has had the keyboard. Then a click on the
# warned pane's page focuses that pane and shows its warning with the keyboard.
if [ "$major" -ge 157 ]; then
  printf '%s' '<title>beside</title>
<input id=field style="position:fixed;inset:0;font-size:40px" oninput="document.title = `beside has ${field.value}`">
<script>
onblur = () => document.title = "beside blurred";
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "beside ready"));
</script>' > "$out/site/beside.html"
  go_from_bar "$nearby:$port/beside.html"
  expect "^browser: title $tab beside ready\$" "$nearby:$port/beside.html didn't draw"
  beside=$tab
  # The warning goes in the new pane, focused as it opens, since a tab's warning shows only while
  # its pane is the focused one.
  send menu Add Split View
  expect '^browser: command add-split via ' "Add Split View didn't run"
  expect '^browser: command bar shown$' "Add Split View didn't open the command bar"
  send type "$plain"
  send key return
  opened=$step
  expect "^browser: splits .*$beside+[0-9][0-9]*" \
    "$plain didn't open beside $nearby:$port/beside.html: $(grep '^browser: splits' "$log" | tail -1)"
  warned_tab=$(grep '^browser: splits' "$log" | tail -1 | grep -o "$beside+[0-9]*" | cut -d+ -f2)
  expect "^browser: loaded $warned_tab chrome-error://chromewebdata/\$" "$plain's warning before http didn't load in the new pane"
  for _ in $(seq 50); do
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child ' && break
    sleep 0.1
  done
  warning_box
  panes=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
    grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' | sed 's/^WebContentsViewCocoa //')
  [ "$(echo "$panes" | wc -l)" -eq 2 ] ||
    fail "the layout didn't show two pages beside each other: $(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout')"
  send click $((warning_x + warning_width / 2)) $((warning_y + 8))
  send state
  [ "$key" = NativeWidgetMacNSWindow ] ||
    fail "a click on Chrome's warning before http at $warning, beside a page in a split, didn't give it the keyboard: $key, $focus"
  # Go to Pane 1 and 2, with the warning's keyboard, reach the UI as its other shortcuts do, and
  # move the focus between the panes, the warning hiding and showing again with it (#3214). No
  # menu item has them, so on the Mac too the core asks the app about them from the warning, even
  # when AppKit hands the key to the warning as the text it types (#3230).
  # The core asks about the warning's keys once it watches it, which on the Mac is once the warning
  # is on screen, as it may be only as its pane takes the focus (#3230).
  expect "browser: tab dialog's keys heard\$" \
    "the core never heard the keys of $warned_tab's warning before http, so it can't ask the app about its shortcuts: $(since "$opened" | grep -e '^browser: drive' -e '^browser: selected' -e 'browser: tab dialog' | tail -4 | tr '\n' ' ')" \
    "$opened"
  send key 1 ctrl shift
  shortcut=$step
  expect "^browser: selected $beside in [0-9.]* ms\$" \
    "Go to Pane 1 (⌃⇧1) on $warned_tab's warning before http, in a split, didn't focus the pane beside it: $(since "$shortcut" | grep -e '^browser: drive' -e '^browser: command ' -e '^browser: selected' -e 'browser: tab dialog let' | tail -4 | tr '\n' ' ')"
  since "$shortcut" | grep -q '^browser: command focus-pane via ' ||
    fail "⌃⇧1 on $warned_tab's warning before http focused the pane beside it, but not as Go to Pane 1: $(since "$shortcut" | grep '^browser: command ' | tr '\n' ' ')"
  # The warning hides as its pane loses the focus. On Windows it had the keyboard, which the host
  # gives the browser window, as Chromium hides a window without activating another (#3330).
  ! windows || expect "^browser: a tab's dialog hid with the keyboard, which goes to its window\$" \
    "the warning before http hid after Go to Pane 1 with the keyboard, which its window didn't get"
  send key 2 ctrl shift
  expect "^browser: selected $warned_tab in [0-9.]* ms\$" \
    "Go to Pane 2 (⌃⇧2), after ⌃⇧1 on its warning before http, didn't focus $warned_tab's pane again: $(since "$shortcut" | grep -e '^browser: drive' -e '^browser: command ' -e '^browser: selected' | tail -4 | tr '\n' ' ')"
  for _ in $(seq 50); do
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child ' && break
    sleep 0.1
  done
  warning_box
  send click $((warning_x + warning_width / 2)) $((warning_y + 8))
  send state
  [ "$key" = NativeWidgetMacNSWindow ] ||
    fail "a click on Chrome's warning before http at $warning, shown again by Go to Pane 2, didn't give it the keyboard: $key, $focus"
  # The page beside it, on the left. On the Mac the press is the window's first click while the
  # warning's window has the keyboard, which reaches the page as elsewhere (#3210).
  pane=$(echo "$panes" | sed -n 1p)
  pane_x=${pane%%,*} pane_y=${pane#*,}
  pane_y=${pane_y%% *}
  send click $((pane_x + 100)) $((pane_y + 100))
  clicked=$step
  expect "^browser: selected $beside\$" "a click on the page beside $warned_tab's warning before http didn't focus its pane"
  # A task the focus change posted has run once a step after it has.
  send state
  send key x
  send key y
  expect "^browser: title $beside beside has xy\$" \
    "typing after a click on the page beside $warned_tab's warning before http didn't reach that page: the keyboard is in $key, on $focus: $(since "$clicked" | grep -e '^browser: drive' -e "^browser: title $beside " | tail -6 | tr '\n' ' ')"
  since "$clicked" | grep -q "^browser: title $beside beside blurred\$" &&
    fail "the page beside $warned_tab's warning before http lost the keyboard after the click into it: $(since "$clicked" | grep -e '^browser: drive' -e "^browser: title $beside " | tail -6 | tr '\n' ' ')"
  # A click on the warned pane's page, below its hidden warning, focuses that pane, and the warning
  # shows with the keyboard. The page under it ignores input while the warning's up, so the press
  # never reached it, and nothing focused the pane (#3196); the click still mustn't reach that page.
  pane=$(echo "$panes" | sed -n 2p)
  pane_x=${pane%%,*} pane_y=${pane#*,}
  pane_y=${pane_y%% *} pane_h=${pane#*x}
  send click $((pane_x + 100)) $((pane_y + pane_h - 60))
  pressed=$step
  expect "^browser: selected $warned_tab\$" \
    "a click on $warned_tab's page below its warning before http, beside a page in a split, didn't focus its pane: $(since "$pressed" | grep -e '^browser: drive' -e '^browser: selected' -e "^browser: title $beside " | tail -6 | tr '\n' ' ')"
  expect "^browser: title $beside beside blurred\$" "the page beside kept the keyboard after a click on $warned_tab's page below its warning before http"
  for _ in $(seq 50); do
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child ' && break
    sleep 0.1
  done
  warning_box
  send state
  [ "$key" = NativeWidgetMacNSWindow ] ||
    fail "a click on $warned_tab's page below its warning before http focused its pane, but the warning at $warning didn't take the keyboard: it's in $key, on $focus"
  send key z
  send state
  since "$pressed" | grep -q "^browser: title $beside beside has xyz\$" &&
    fail "a key after a click on $warned_tab's page below its warning before http reached the page beside it"
  since "$pressed" | grep -q "^browser: page $warned_tab back [01] forward [01] loading 1 " &&
    fail "a click on $warned_tab's page below its warning before http reached the page under the warning: $(since "$pressed" | grep "^browser: page $warned_tab " | tail -3 | tr '\n' ' ')"
  # The page beside it again, for the ⌘W that closes it, by Go to Pane 1 from the warning: shown
  # again with its pane, it still has its shortcuts asked about (#3230).
  send key 1 ctrl shift
  again=$step
  expect "^browser: selected $beside in [0-9.]* ms\$" \
    "Go to Pane 1 (⌃⇧1) on $warned_tab's warning before http, shown again as its pane took the focus, didn't focus the pane beside it: $(since "$again" | grep -e '^browser: drive' -e '^browser: command ' -e '^browser: selected' -e 'browser: tab dialog let' | tail -4 | tr '\n' ' ')"
  send key w cmd
  expect "^browser: tab $beside closed\$" "⌘W didn't close the pane beside $warned_tab's warning before http"
  expect "^browser: panes $warned_tab\$" "⌘W on the pane beside the warning didn't leave $warned_tab alone"
  tab=$warned_tab
  # On the Mac the warning, a window of its own, shows again with the keyboard once the pane beside
  # it closes. Without it the page under it is bare and has the keyboard, and drops every key, ilo's
  # shortcuts with them (#3233). The steps after press ⌘L from here.
  if [ "$(uname)" = Darwin ]; then
    send state
    if [ "$key" != NativeWidgetMacNSWindow ]; then
      send layout
      fail "Chrome's warning before http wasn't shown again with the keyboard once ⌘W closed the pane beside it: the keyboard is in $key, on $focus, and the window's children are$(since | sed -n 's/^browser: layout child / /p' | tr '\n' ',')"
    fi
  fi
fi
if [ "$major" -ge 157 ]; then
  warned
  send key t cmd
  expect '^browser: command new-tab via ' "⌘T on Chrome's warning before http didn't reach the UI"
  expect '^browser: command bar shown$' "⌘T on Chrome's warning before http didn't open the command bar"
  ran_on_warning new-tab "one ⌘T"
  # The Mac's warning is a window of its own, which had the keyboard: the bar takes it from there.
  # Elsewhere the second tab's address, typed into the bar below, shows the bar has it.
  if [ "$(uname)" = Darwin ]; then
    send state
    [ "$key" = GPUIPanel ] && [ "$focus" = GPUIView ] ||
      fail "⌘T on Chrome's warning before http opened the command bar without the keyboard, which is in $key, on $focus"
  else
    echo "skip: which window has the keyboard as the command bar opens and closes over Chrome's warning before http: the warning is a window of its own on the Mac alone, and here the second tab's address typed into the bar shows the bar has it."
  fi
  send key escape
  escaped=$step
  expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘T opened"
  # The escape is the bar's alone. Under Wayland the window hears it too, where views' focus manager
  # gave it to the warning as its Go back (#2443). The state's round trip comes after a back that
  # escape started.
  send state
  sed -n "/^browser: drive $escaped: /,\$p" "$log" | grep -q "^browser: page $tab back [01] forward [01] loading 1 " &&
    fail "escape closing the command bar over Chrome's warning before http also left the warning"
  [ "$(grep "^browser: page $tab trouble " "$log" | tail -1)" = "browser: page $tab trouble network -20" ] ||
    fail "Chrome's warning before http went as escape closed the command bar over it: $(grep "^browser: page $tab trouble " "$log" | tail -1)"
  # And gives it back to the warning, as the page under it drops every key, ilo's shortcuts with
  # them, while the warning's up (#2625, #2855). The second ⌘T, next, comes from there. Linux's
  # warning is in the browser window, which keeps the keyboard.
  if [ "$(uname)" != Linux ]; then
    [ "$key" = NativeWidgetMacNSWindow ] ||
      fail "the command bar closing over Chrome's warning before http gave the keyboard to $key, on $focus, not back to the warning"
  fi
  # F6 over Chrome's warning before http rings the tab's row, and Tab from there goes into the page
  # under the warning, whose taking the keyboard hands it to the warning. Before #3443 the warning
  # took it back from the ring, as F6 activating the window focused the page first. Windows alone,
  # whose pane the warning took it back from.
  if windows; then
    send key f6
    expect '^browser: sidebar keyed ' "F6 from Chrome's warning before http didn't ring the tab's row"
    send state
    in_browser AccessKitSubclassOfGPUIView "after F6 over Chrome's warning before http"
    # The ring hearing Tab says the sidebar kept the keyboard past the tasks F6 left, which come
    # before Tab's.
    send key tab
    expect '^browser: sidebar keys left by tab$' \
      "Tab after F6 over Chrome's warning before http didn't reach the ring: the warning took the keyboard back from it"
    expect '^browser: ring gave the page its first control$' \
      "Tab from the ring didn't go into the page under Chrome's warning before http"
    # The warning then has it, which the state can't tell from the page under it: Return on its
    # focused Go back leaves the warning, where the page would drop the key (#2625). Windows hands
    # the warning's window the keyboard before the core's take-back asks, which then logs nothing
    # (44451704979). The warning comes again for the steps after.
    send key return
    expect "^browser: page $tab trouble fine\$" \
      "Return after Tab from the ring didn't leave Chrome's warning before http: the page under it had the keyboard, not the warning"
    warned
  fi
  # A click on the page beside the warning: the page drops it, but on the Mac AppKit gives the
  # page's view the keyboard all the same, where every key is dropped. The core gives it back to
  # the warning (dialog_keys.cc), so the second ⌘T, next, still opens the bar (#2625).
  warning_box
  below=$((warning_y + warning_height + 40))
  [ "$below" -lt $((window_height - 30)) ] ||
    fail "no page showed below Chrome's warning before http, at $warning in a window $window_height high, to click"
  send click $((warning_x + warning_width / 2)) "$below"
  # The warning's in a second tab, so one ⌘W on it is seen to close that tab alone: under Wayland
  # the UI, which the closed tab gives the keyboard, read the press again and ran it twice (#2442).
  kept=$tab
  send key t cmd
  expect '^browser: command bar shown$' "⌘T after a click on the page beside Chrome's warning before http didn't open the command bar for a second tab"
  send type "$nearby:$port"
  send key return
  expect "^browser: today [0-9][0-9]* $kept\$" "the second tab didn't open over $kept: Today is \"$(today)\""
  tab=$(today | cut -d' ' -f1)
  expect "^browser: loaded $tab http://$nearby:$port/\$" "$nearby:$port didn't load in the second tab"
  warned
  send key w cmd
  expect '^browser: command close-tab via ' "⌘W on Chrome's warning before http didn't reach the UI"
  expect_tabs "$kept" "$kept" "after one ⌘W on Chrome's warning before http"
  ran_on_warning close-tab "one ⌘W"
  # The tab behind shows its own warning again, which has the keyboard with none of its views
  # focused under Wayland unless the core gives it one: every key was lost there (#3167).
  send key l cmd
  expect '^browser: command bar shown$' \
    "⌘L after ⌘W closed a tab on Chrome's warning before http didn't open the command bar over tab $kept, on its own warning (#3167)"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘L opened after the ⌘W"
  # Another app's small window beside ilo's takes the foreground, and gives it back to ilo's
  # window in front with the pointer at $2, by $1: "taskbar", as its taskbar button or Alt+Tab do,
  # where no press activates it, or "press", a real press there. Sets $how, and $back to the step
  # after.
  away_and_back() {
    # The log goes first: Git Bash's fork can empty it after the wait below has read the last
    # call's line, which ended the wait at once on an empty log.
    rm -f "$out/cover-front" "$out/other-window.log"
    python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" aside \
      "$(cygpath -w "$out/cover-front")" > "$out/other-window.log" 2>&1 &
    other=$!
    # Until it says so, for 30 s by the clock: Git Bash's kill -0 can find a native process just
    # started gone, and a hundred 0.1 s sleeps there ended before its first line came.
    started=$SECONDS
    while [ $((SECONDS - started)) -lt 30 ]; do
      grep -q '^cover ' "$out/other-window.log" 2> /dev/null && break
      sleep 0.1
    done
    grep -q '^cover .*, foreground away$' "$out/other-window.log" 2> /dev/null ||
      fail "another app's window didn't open and take the foreground from ilo's, in $((SECONDS - started)) s: $(cat "$out/other-window.log" 2>&1)"
    send move $2
    case $1 in
      taskbar) how="coming back from another app with the pointer at $2" && touch "$out/cover-front" ;;
      press) how="a press at $2 from another app" && echo press > "$out/cover-front" ;;
    esac
    wait "$other" ||
      fail "another app's window didn't give ilo's back the foreground: $(cat "$out/other-window.log")"
    back=$step
  }
  # Coming back from another app: the warning, which had the keyboard, has it again unless a press
  # on the UI brought the window forward. On Windows the window gives the page's pane the keyboard
  # back first, which the warning takes from it only then (#2910). Each starts with the warning's
  # keys, then goes away and back by $1 with the pointer at $2, as away_and_back's. $3 says where
  # the keyboard goes: "warning" or "ui".
  back_from_another_app() {
    send click $((warning_x + warning_width / 2)) $((warning_y + warning_height + 40))
    clicked=$step
    for _ in $(seq 50); do
      [ "$key" = NativeWidgetMacNSWindow ] && break
      sleep 0.1
      send state
    done
    [ "$key" = NativeWidgetMacNSWindow ] ||
      fail "a click on the page beside Chrome's warning before http left the keyboard in $key, on $focus, not in the warning: $(since "$clicked" | grep '^browser: drive' | tail -3 | tr '\n' ' ')"
    away_and_back "$1" "$2"
    case $3 in
      warning) wanted=NativeWidgetMacNSWindow ;;
      ui) wanted=BrowserCoreNSWindow ;;
    esac
    for _ in $(seq 50); do
      send state
      [ "$active" = 1 ] && [ "$key" = "$wanted" ] && break
      sleep 0.1
    done
    # The warning takes the keyboard in a task posted as the pane gets it, which has run once a
    # step after it has.
    send state
    [ "$active" = 1 ] && [ "$key" = "$wanted" ] ||
      fail "$how left the keyboard in $key, on $focus (active $active), where $kept's warning before http had it, not in the $3: $(since "$back" | grep '^browser: drive' | tail -3 | tr '\n' ' ')"
    [ "$3" = warning ] ||
      in_browser AccessKitSubclassOfGPUIView "$how, beside $kept's warning before http"
    send key t cmd
    expect '^browser: command bar shown$' \
      "⌘T after $how, beside Chrome's warning before http, didn't open the command bar"
    send key escape
    expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘T opened after $how"
  }
  if windows; then
    back_from_another_app taskbar "$sidebar_point" warning
    back_from_another_app press "$((warning_x + warning_width / 2)) $((warning_y + warning_height + 40))" warning
    back_from_another_app press "$sidebar_point" ui
    # With a second window in front and the first in the back, the core hears the second's presses
    # too, though they bring nothing forward. The second coming back by the taskbar has the
    # keyboard where a key last moved it, not where the press before the key was (#2910): a press
    # on its page and F6 to the sidebar, then a press on its sidebar, F6 and escape to the page.
    send key n cmd
    expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
    new_window_bar
    send window "$(grep -c '^browser: window opened ' "$log")"
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar in the second window"
    send insert "${site}notes.txt"
    send key return
    expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the second window didn't load ${site}notes.txt"
    # back_in_second <view> <where>: away and back by the taskbar, where the keyboard was in <view>,
    # which <where> names.
    back_in_second() {
      away_and_back taskbar "$sidebar_point"
      for _ in $(seq 50); do
        send state
        [ "$active" = 1 ] && [ "$key" = BrowserCoreNSWindow ] && [ "$focus" = "$1" ] && break
        sleep 0.1
      done
      [ "$active" = 1 ] && [ "$key" = BrowserCoreNSWindow ] && [ "$focus" = "$1" ] ||
        fail "the second window coming back from another app by the taskbar left the keyboard in $key, on $focus (active $active), where the $2 had it: $(since "$back" | grep '^browser: drive' | tail -3 | tr '\n' ' ')"
    }
    send click $page_point
    in_browser RenderWidgetHostViewCocoa "after a press on the second window's page"
    send key f6
    expect '^browser: sidebar keyed ' "F6 with the second window's page focused didn't give the sidebar the keyboard"
    in_browser AccessKitSubclassOfGPUIView "after F6 from the second window's page"
    back_in_second AccessKitSubclassOfGPUIView "sidebar, after a press on the page and F6,"
    send click $sidebar_point
    in_browser AccessKitSubclassOfGPUIView "after a press on the second window's sidebar"
    send key f6
    expect '^browser: sidebar keyed ' "F6 with the second window's sidebar focused didn't ring a row"
    send key escape
    expect '^browser: sidebar keys left by escape$' "escape didn't take F6's sidebar keyboard away in the second window"
    in_browser RenderWidgetHostViewCocoa "after escape left the second window's sidebar"
    back_in_second RenderWidgetHostViewCocoa "page, after a press on the sidebar, F6 and escape,"
    send key w cmd shift
    expect '^browser: window closed [0-9]' "⇧⌘W didn't close the second window"
    send window 1
  fi
  # A click on the page beside the warning while the UI has the keyboard, the browser window
  # active: on Windows the page's pane takes the keyboard, with no window activated for aura to
  # give the warning, and drops every key while the warning's up (#2910). The UI's sidebar, clicked
  # first, has the keyboard there. On Linux the warning takes the keyboard, but ⌘T after it reaches
  # nothing (#2926).
  send click $sidebar_point
  in_browser AccessKitSubclassOfGPUIView "a click on the sidebar beside $kept's warning before http"
  warning_box
  send click $((warning_x + warning_width / 2)) $((warning_y + warning_height + 40))
  clicked=$step
  send key t cmd
  expect '^browser: command bar shown$' \
    "⌘T after a click on the sidebar, then the page beside Chrome's warning before http, didn't open the command bar: $(since "$clicked" | grep -e '^browser: drive' -e '^browser: command ' | tr '\n' ' ')"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘T opened beside the warning"
  # The command bar keeps every key while a warning first shows under it, as a page's link goes to
  # http:// with the bar open: Return goes to the bar, never to a button of the warning, which takes
  # aura's focus as it shows (#3167). The page's link to $barred waits for /held, which the check
  # lets go once the bar is open.
  tab=$kept
  rm -f "$out/site/held.go"
  printf '%s' "<title>held link</title><a id=go href=\"http://$barred/\">$barred</a>
<script>fetch('/held').then(() => go.click());</script>" > "$out/site/held-link.html"
  go_from_bar "$nearby:$port/held-link.html"
  expect "^browser: loaded $tab http://$nearby:$port/held-link.html\$" \
    "the page whose link waits for /held didn't load: $(grep "^browser: loaded $tab " "$log" | tail -1)"
  served "^GET chromium [^ ]*/held " "the page's wait for /held didn't reach the check's server"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar over the page whose link waits"
  opened=$step
  before=$(tls)
  touch "$out/site/held.go"
  upgraded "$tab" "$barred" "the link followed with the command bar open"
  expect "^browser: loaded $tab chrome-error://chromewebdata/\$" \
    "Chrome's warning before http for $barred didn't load under the command bar"
  send state
  none_since '^browser: command bar hidden$' "$opened" ||
    fail "the command bar closed as Chrome's warning before http first showed under it: $(since "$opened" | grep -e '^browser: drive' -e '^browser: command bar' | tr '\n' ' ')"
  send type "$nearby:$port"
  send key return
  returned=$step
  expect "^browser: command bar go http://$nearby:$port/\$" \
    "Return in the command bar, over the warning that first showed under it, didn't go from the bar: $(since "$returned" | grep -e '^browser: drive' -e '^browser: command ' -e "^browser: page $tab " | tr '\n' ' ')"
  expect '^browser: command bar hidden$' "going to $nearby:$port from the bar over the warning didn't close it"
  expect "^browser: loaded $tab http://$nearby:$port/\$" \
    "$nearby:$port, typed in the bar over the warning, didn't load: $(grep "^browser: loaded $tab " "$log" | tail -1)"
  # The warning's buttons: Go back loads the page with the link again, Continue $barred over http.
  if since "$returned" | grep -q -e "^browser: loaded $tab http://$barred/\$" \
    -e "^browser: page $tab back [0-9]* forward [0-9]* loading [01] http://$nearby:$port/held-link.html\$"; then
    fail "Return in the command bar also pressed a button of the warning under it: $(since "$returned" | grep "^browser: \(page\|loaded\) $tab " | tr '\n' ' ')"
  fi
fi

# With the user's tab back on a page of the check's own.
[ -z "${kept:-}" ] || tab=$kept
go_from_bar "$nearby:$port"
expect "^browser: loaded $tab http://$nearby:$port/\$" \
  "$nearby:$port didn't load before the agent's steps: $(grep "^browser: loaded $tab " "$log" | tail -1)"
agent_steps
exec 3>&-
quit
kill "$stuck"
kill "$tls" 2> /dev/null || true
tls=
# Each example.com and example.org page the launch loaded came from the run's server; the hosts
# under example.com that lead to the check's own aren't these.
examples=$(sed -n 's|^browser: loaded [0-9]* \(https\{0,1\}://example\.[a-z]*/.*\)$|\1|p' "$log" | sort -u)
[ -n "$examples" ] || fail "the launch loaded no example.com or example.org page"
for url in $examples; do
  grep -qxF "example: GET $url" "$out/example-tls.log" ||
    fail "$url loaded, but not from the run's server: $(grep -v '^example: ' "$out/example-tls.log" | tail -3)"
done
echo "ok: the example.com and example.org pages the launch loaded came from the run's own server: $(echo $examples)"
echo "ok: ⌘← ⌘→ and the header's buttons go back and forward, with the page or the UI focused, and so does a swipe over the page but not over the sidebar; reload and force reload load the page again; ⌘., escape and the stop button stop a load; the row keeps a page's favicon until the next page has loaded without one; a host typed in the command bar goes over https first and then http, or from Chromium 157 Chrome's warning before http, where ⌘L, ⌘T and ⌘W reach the UI and escape closes the command bar alone, ⌘L still opens it after ⌘W closed a warned tab over another, and the bar keeps Return as a warning first shows under it, and a local one goes over http at once; a link and an agent's open_tab and navigate to http:// go over https first, and ⌘L still opens the command bar with the agent's tab at its last load; a right-click on reload lists Reload, Hard Reload and Clear Site Data and Reload, which each reload, the last with the site's cookie, localStorage and IndexedDB gone and another site's kept. Windows: $out/history.png, $out/loading.png, $out/reload-menu-dark.png, $out/reload-menu-light.png"
