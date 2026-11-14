# check.sh runs this at 60.
# Back and forward go through the tab's history from keys, with the page or the UI focused, from
# the header's buttons, which dim with nowhere to go, and from a swipe over the page. Reload loads
# the page again, and while a page loads, reload is stop, which ⌘., escape and the button do. A
# tab's row shows its page's favicon, and keeps it until the next page has loaded without one. A
# host typed in the command bar goes over https first, then http, or from Chromium 157 Chrome's
# warning before http, whose dialog has the keyboard and still lets ⌘L, ⌘T and ⌘W reach the app,
# and a local one over http. One typed with http:// loads over http as written, with no https try
# and no warning, as in Chrome's address bar. Only what the user typed counts: an agent's open_tab
# and navigate, and a link, to an http:// address, are tried over https first. A right-click on reload lists Reload, Hard Reload
# and Clear Site Data and Reload, the last deleting the site's cookies, storage and cache, not
# another site's, before the page loads again.

# Back, forward, reload and stop. stuck.test leads to a server that takes connections and never
# answers, so its page keeps loading until it's stopped, and $plain to the check's own, which has no
# https. The profile remembers a host that fell back to http, so each run names a new one, and
# $written is another such host, typed with http://, and $agent_opened, $agent_moved and $linked
# three more, which an agent and a link go to.
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
MAP $plain 127.0.0.1:$port, MAP $nearby 127.0.0.1:$port, \
MAP $written 127.0.0.1:$port, MAP $agent_opened 127.0.0.1:$port, \
MAP $agent_moved 127.0.0.1:$port, MAP $linked 127.0.0.1:$port" < "$out/drive" > "$log" 2>&1 &
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
expect '^browser: nav shows reload$' "the stop button didn't turn back into reload"
send click $reload_button
expect '^browser: command reload via sidebar$' "a click on reload didn't reload"
expect "^browser: loaded $tab https://example.org/\$" "a click on reload didn't load the page again"

# A mouse's side buttons over the page go back and forward, as Chrome's do (#688). Over the sidebar
# they show another Space, which the spaces run checks.
# A side button's click gives the page the keyboard, so this comes after the keys with the UI focused.
if windows; then
  echo "skip: side buttons over the page: the Windows driver can't press them yet."
else
  send sideclick $page_point back
  expect_page example.com 1 1 "after the back side button over the page"
  send sideclick $page_point forward
  expect_page example.org 1 0 "after the forward side button over the page"
fi

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
# handshake as a bad request.
tls() {
  grep -a -c 'code 400, message Bad request' "$out/site.log" || true
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

# Only an address the user typed counts (#2417): an agent's open_tab and navigate, and a link, to
# an http:// address go as a bare host does, over https first, then to 156's http or 157's warning.
# Each goes to a host of its own, never $written: Chromium lets a host's later loads through once
# the user typed it with http://, or once 156 fell back to http for it.
# Waits for the tab $1 to come to http://$2/ after an https try, as $plain did, for $3.
upgraded() {
  expect "^browser: page $1 back [0-9]* forward 0 loading 0 http://$2/\$" \
    "$3 didn't come to http://$2/: $(grep "^browser: page $1 " "$log" | tail -1)"
  if [ "$major" -ge 157 ]; then
    expect "^browser: page $1 trouble network -20\$" \
      "$3 to http://$2/ had no warning before http: $(grep "^browser: loaded $1 " "$log" | tail -1)"
  else
    expect "^browser: loaded $1 http://$2/\$" \
      "$3 to http://$2/ didn't load over http: $(grep "^browser: loaded $1 " "$log" | tail -1)"
  fi
  [ "$(tls)" -gt "$before" ] || fail "$3 came to http://$2/ without trying https first"
}
send agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
before=$(tls)
send agentopen claude-code "http://$agent_opened/"
expect '^browser: agent claude-code opened [1-9][0-9]*$' "the agent didn't open http://$agent_opened/"
agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
upgraded "$agent_tab" "$agent_opened" "the agent's open_tab"
before=$(tls)
send agentnavigate claude-code "$agent_tab" "http://$agent_moved/"
expect "^browser: agent claude-code navigated $agent_tab: 1\$" \
  "the core didn't take the agent's navigate to http://$agent_moved/"
upgraded "$agent_tab" "$agent_moved" "the agent's navigate"
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
  fi
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar ⌘T opened"
  # And gives it back to the warning, as the page under it drops every key, ilo's shortcuts with
  # them, while the warning's up (#2625). The second ⌘T, next, comes from there.
  if [ "$(uname)" = Darwin ]; then
    send state
    [ "$key" = NativeWidgetMacNSWindow ] ||
      fail "the command bar closing over Chrome's warning before http gave the keyboard to $key, on $focus, not back to the warning"
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
fi
exec 3>&-
quit
kill "$stuck"
echo "ok: ⌘← ⌘→ and the header's buttons go back and forward, with the page or the UI focused, and so does a swipe over the page but not over the sidebar; reload and force reload load the page again; ⌘., escape and the stop button stop a load; the row keeps a page's favicon until the next page has loaded without one; a host typed in the command bar goes over https first and then http, or from Chromium 157 Chrome's warning before http, where ⌘L, ⌘T and ⌘W reach the UI, and a local one goes over http at once; a right-click on reload lists Reload, Hard Reload and Clear Site Data and Reload, which each reload, the last with the site's cookie, localStorage and IndexedDB gone and another site's kept. Windows: $out/history.png, $out/loading.png, $out/reload-menu-dark.png, $out/reload-menu-light.png"
