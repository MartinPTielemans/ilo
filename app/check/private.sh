# check.sh runs this at 220.
# After ⌘N opens a window and ⇧⌘W closes it, ⇧⌘N opens a private window, and ⇧⌘W closes it before
# it has a page, ending its session while the browser keeps running. ⇧⌘N opens another, titled as
# one in the Window menu, whose pages load in the profile's off-the-record session: a cookie set
# there isn't seen in the regular window, and the profile's second private window sees it, as they
# share the session. Closing the last private window ends the session, and the next ⇧⌘N starts a
# new one without its cookie. Nothing private is written: after the quit the Spaces file and the
# profile's history have none of its pages, the profile's cookies don't have its cookie, and a
# relaunch brings back no private window. A private window's Library has its session's downloads
# (#669): a file it goes to downloads, which its Library's Downloads lists, and the regular
# window's doesn't, nor does the profile's history after the quit. Customize Chrome's page, typed in a
# private window, is refused there, where it crashed the browser (#2070).

# The served cookie page: its title says whether the site has the private cookie, and "p" in it
# sets it. The private windows load it at a URL of their own, which nothing may write.
serve
printf '%s' '<title>loading</title>
<script>
document.title = "cookie " + (document.cookie.includes("private=1") ? "private" : "none");
onkeydown = e => {
  if (e.key == "p") {
    document.cookie = "private=1";
    document.title = "cookie set";
  }
};
</script>' > "$out/site/private.html"
# What a private window downloads: the server sends a .bin as a file to save, not a page to show.
printf '%s' 'a private download' > "$out/site/private-notes.bin"
regular_url="${site}private.html"
private_url="${site}private.html?private-only"
log="$out/private.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$regular_url" "$browser" < "$out/drive" \
  > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* cookie none$' || fail "the cookie page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: private. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# Opens the cookie page at the private URL from the command bar of the key window, a private one,
# and sets $tab to its tab.
open_private_page() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
  send insert "$private_url"
  send key return
  expect "^browser: loaded [0-9]* $private_url\$" "the private window didn't load the cookie page"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) $private_url\$|\1|p" "$log" | tail -1)
  expect "^browser: window $1 shows $tab\$" "the private window didn't show its page"
}

# Waits up to 30 s for the app to have heard $1 private sessions end, saying so if it has exited.
ended() {
  for _ in $(seq 300); do
    [ "$(grep -c '^browser: private session ended ' "$log")" -ge "$1" ] && return 0
    kill -0 "$pid" 2>/dev/null || gone
    sleep 0.1
  done
  return 1
}

# ⌘N opens a window and ⇧⌘W closes it. Then ⇧⌘N opens a private window, which ⇧⌘W closes before
# it has had a page: Chrome ends its session as the window's Browser goes, and the browser keeps
# running. On Windows this second close crashed in what the first had left behind (#1474).
send key n cmd
expect '^browser: window opened [0-9]*$' "⌘N didn't open a window"
regular=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
new_window_bar
send key w cmd shift
expect "^browser: window closed $regular\$" "⇧⌘W didn't close the window ⌘N opened"
send window 1
send click $page_point
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
empty=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
new_window_bar
send key w cmd shift
expect "^browser: window closed $empty\$" "⇧⌘W didn't close the private window with no page"
ended 1 || fail "closing the private window with no page didn't end its session"
# send fails if the app has exited, or no longer takes the driver's commands. The drivers number
# windows in the order they opened, closed ones too, so the next private window is window 4.
send window 1

# ⇧⌘N from the page opens a private window, titled as one, which opens with no page.
send key n cmd shift
expect '^browser: command new-private-window via ' "⇧⌘N didn't run New Incognito Window"
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
first=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: window opened $first\$" "the private window didn't open"
new_window_bar
send window 4
send title
expect '^browser: key window titled Incognito$' "the private window isn't titled as one"

# Its page sets the cookie, which its session keeps.
open_private_page "$first"
p=$tab
[ "$p" != "$a" ] || fail "the private window showed the regular window's tab"
expect "^browser: title $p cookie none\$" "the private session started with the private cookie"
send window 4
send click $page_point
send key p
expect "^browser: title $p cookie set\$" "p in the private page didn't set the cookie"
send key r cmd
expect "^browser: title $p cookie private\$" "the private session lost its cookie on a reload"
../tools/window-screenshot.sh "$pid" "$out/private.png" all || fail "no window to capture"

# Its download is its Library's: going to a file from its command bar downloads it, and ⇧⌘J opens
# the Library on Downloads, which lists it alone.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
send insert "${site}private-notes.bin"
send key return
for _ in $(seq 100); do
  grep -q '^browser: download [0-9]* done private-notes.bin$' "$log" && break
  sleep 0.1
done
grep -q '^browser: download [0-9]* done private-notes.bin$' "$log" ||
  fail "the private window's download didn't finish: $(grep -c 'private-notes' "$out/site.log") requests"
send window 4
send key j cmd shift
expect '^browser: library section downloads$' "⇧⌘J didn't open the private window's Library"
expect '^browser: library lists downloads: private-notes.bin$' \
  "the private window's Library didn't list its session's download alone"
../tools/window-screenshot.sh "$pid" "$out/private-library.png" || fail "no window to capture"
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J didn't close the private window's Library"

# The regular window's page, from the same site, doesn't see it, and its Library lists none of the
# private window's downloads.
send window 1
send key r cmd
expect "^browser: loaded $a $regular_url\$" "⌘R didn't reload the regular window's page"
none_since "^browser: title $a cookie private\$" ||
  fail "the regular window saw the private window's cookie"
send key j cmd shift
expect '^browser: library lists downloads: ' "⇧⌘J didn't open the regular window's Library"
listed=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: library lists downloads: ' |
  tail -1)
case $listed in
*private-notes.bin*) fail "the regular window's Library listed the private window's download" ;;
esac
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J didn't close the regular window's Library"

# The profile's second private window shares the session.
send window 4
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' \
  "⇧⌘N in the private window didn't open another"
second=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
new_window_bar
[ "$second" != "$first" ] || fail "the second private window has the first one's id"
open_private_page "$second"
expect "^browser: title $tab cookie private\$" "the second private window didn't share the session"

# Closing one keeps the session, closing the last ends it, and the next ⇧⌘N starts without the
# cookie.
send window 5
send key w cmd shift
expect "^browser: window closed $second\$" "⇧⌘W didn't close the second private window"
none_since '^browser: private session ended' || fail "the session ended with a private window open"
send window 4
send key w cmd shift
expect "^browser: window closed $first\$" "⇧⌘W didn't close the first private window"
# The second session's end: only the first's came before.
ended 2 || fail "the last private window's closing didn't end the session"
# Chrome ends it as the session's last Browser goes. When something else kept one, the core ended
# it, and its line, an error, names what.
outlived=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
  grep -m1 'browser: private session outlived its windows' || true)
[ -z "$outlived" ] || fail "the core had to end the private session: ${outlived#*] }"
send window 1
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' \
  "⇧⌘N didn't open a private window again"
third=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
new_window_bar
open_private_page "$third"
expect "^browser: title $tab cookie none\$" "the new private session had the ended one's cookie"
# Chrome destroying the profile and the core finding it gone both end a session, and the app hears
# it once: the two sessions so far, the empty window's and the cookie's.
[ "$(grep -c '^browser: private session ended ' "$log")" = 2 ] ||
  fail "the app heard a session end more than once"
# Customize Chrome's page, which ilo has no UI for, typed in a private window: its handler needs the
# New Tab page's services, which a private profile hasn't, and crashed the browser (#2070). The
# page is refused there, as a URL Chrome has no page for.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the private window"
send insert "chrome://customize-chrome-side-panel.top-chrome"
send key return
expect "^browser: page $tab trouble network -[0-9]*\$" \
  "Customize Chrome's page typed in a private window wasn't refused"

# The quit, with the private window open, writes nothing of it.
exec 3>&-
quit
[ -s "$saved" ] || fail "the quit saved no Spaces"
# A page's history is kept in Chrome's pickles, base64 in the file, so its URLs are read decoded.
pages=$(grep -oE '"[A-Za-z0-9+/]{16,}={0,2}"' "$saved" | tr -d '"' |
  while read -r each; do echo "$each" | base64 -d; done | LC_ALL=C tr -c '[:print:]' '\n')
echo "$pages" | grep -q 'private\.html' || fail "the quit didn't save the regular window's page"
echo "$pages" | grep -q 'private-only' && fail "the Spaces file has a private window's page"
history="$(dirname "$saved")/Default/History"
[ -s "$history" ] || fail "the profile has no history"
grep -q 'private-only' "$history" && fail "the profile's history has a private window's page"
grep -q 'private-notes' "$history" && fail "the profile's history has a private window's download"
cookies="$(dirname "$saved")/Default/Network/Cookies"
if command -v sqlite3 > /dev/null && [ -f "$cookies" ]; then
  [ "$(sqlite3 "$cookies" "select count(*) from cookies where name = 'private'")" = 0 ] ||
    fail "the profile's cookies have the private window's cookie"
fi

# The relaunch brings back the regular window alone.
log="$out/private-relaunch.log"
"$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await '^browser: restored in ' || fail "the relaunch didn't bring back the Spaces"
await "^browser: loaded [0-9]* $regular_url\$" || fail "the relaunch didn't load the regular page"
[ "$(grep -c '^browser: window opened ' "$log")" = 1 ] ||
  fail "the relaunch opened more than one window"
grep -q '^browser: private window opened' "$log" &&
  fail "the relaunch brought back a private window"
grep -q 'private-only' "$log" && fail "the relaunch brought back a private window's page"
quit
echo "ok: ⇧⌘N opens a private window titled Incognito, whose cookie the regular window doesn't see and the profile's other private window does, whose download its Library lists and the regular window's doesn't, closing the last one ends the session, the next starts without its cookie and refuses Customize Chrome's page, and neither the Spaces file, the history, the cookies nor a relaunch has any of it. Windows: $out/private.png, $out/private-library.png"

# In a private window, the extensions allowed in Incognito run as in Chrome, and the others don't:
# the Extensions menu lists only the allowed one, never uBlock Origin, the browser's own, which
# blocks there as Settings' Ad Blocking (#2107), and the allowed one pins at the URL field and has its Site Controls tile. Two
# extensions in a profile of the block's own: a first launch has Chrome list them, and Chrome's
# preferences then allow one in Incognito and not the other, as chrome://extensions' Allow in
# Incognito sets them, before the launch the block checks.
incognito="$PWD/$out/incognito"
rm -rf "$incognito"
mkdir -p "$incognito/allowed" "$incognito/not" "$incognito/profile"
echo '{"manifest_version": 3, "name": "Check Allowed", "version": "1", "action": {}}' \
  > "$incognito/allowed/manifest.json"
echo '{"manifest_version": 3, "name": "Check Not Allowed", "version": "1", "action": {}}' \
  > "$incognito/not/manifest.json"
# Launches the app on the block's profile with both, logging to $1.
incognito_launch() {
  log="$out/$1.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$regular_url" BROWSER_PROFILE_DIR="$incognito/profile" "$browser" \
    --load-extension="$(app_path "$incognito/allowed"),$(app_path "$incognito/not")" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await "^browser: extensions menu [a-p]* [a-p]*\$" ||
    fail "the Extensions menu didn't list both extensions: $(grep '^browser: extensions menu' "$log")"
}
incognito_launch private-extensions-first
# By name: Check Allowed, then Check Not Allowed.
ids=$(grep -m1 "^browser: extensions menu [a-p]* [a-p]*\$" "$log" | cut -d' ' -f4-5)
allowed=${ids% *}
not_allowed=${ids#* }
exec 3>&-
quit
# Off Linux, Chrome keeps the extensions' settings in Secure Preferences, under a MAC a check can't
# sign, so there only chrome://extensions' switch allows one in Incognito.
status=0
python3 -I -c '
import json, sys
path = sys.argv[1] + "/Default/Preferences"
prefs = json.load(open(path))
settings = prefs.get("extensions", {}).get("settings", {})
if sys.argv[2] not in settings or sys.argv[3] not in settings:
    sys.exit(3)
settings[sys.argv[2]]["incognito"] = True
settings[sys.argv[3]]["incognito"] = False
json.dump(prefs, open(path, "w"))' "$incognito/profile" "$allowed" "$not_allowed" || status=$?
if [ "$status" = 3 ]; then
  echo "skip: private's extensions allowed in Incognito: Chrome keeps the extensions' settings in Secure Preferences, under a MAC, so only chrome://extensions' switch allows one."
  exit 0
fi
[ "$status" = 0 ] || fail "couldn't allow Check Allowed in Incognito"
incognito_launch private-extensions
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
window=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: window opened $window\$" "the private window didn't open"
new_window_bar
open_private_page "$window"
send menubar Extensions
expect '^browser: menu bar Extensions: ' "the private window's Extensions menu didn't list"
menu=$(grep '^browser: menu bar Extensions: ' "$log" | tail -1)
case "$menu" in
  *"uBlock Origin"* | *"Check Not Allowed"*)
    fail "the private window's Extensions menu didn't list Check Allowed alone: $menu" ;;
  *": Check Allowed, "*) ;;
  *) fail "the private window's Extensions menu didn't list Check Allowed alone: $menu" ;;
esac
send menu "Pin to URL Field › Check Allowed"
expect "^browser: pin extension $allowed\$" "Pin to URL Field › Check Allowed didn't reach the UI"
expect "^browser: extension actions $allowed\$" "the pinned Check Allowed didn't show in the URL field"
../tools/window-screenshot.sh "$pid" "$out/private-extensions.png" || fail "no window to capture"
bar_command site-controls "Site Controls" "site controls"
expect "^browser: site controls extensions $allowed\$" \
  "the private window's Site Controls didn't list Check Allowed alone: $(grep \
    '^browser: site controls extensions' "$log" | tail -1)"
expect '^browser: site controls settled$' "the Site Controls didn't open in the private window"
../tools/window-screenshot.sh "$pid" "$out/private-site-controls.png" || fail "no window to capture"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
exec 3>&-
quit
echo "ok: a private window runs the extension allowed in Incognito, listed alone in its Extensions menu, pinned at its URL field and in its Site Controls, and not the other. Windows: $out/private-extensions.png, $out/private-site-controls.png"
