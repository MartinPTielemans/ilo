# After ⌘N opens a window and ⇧⌘W closes it, ⇧⌘N opens a private window, and ⇧⌘W closes it before
# it has a page, ending its session while the browser keeps running. ⇧⌘N opens another, titled as
# one in the Window menu, whose pages load in the profile's off-the-record session: a cookie set
# there isn't seen in the regular window, and the profile's second private window sees it, as they
# share the session. Closing the last private window ends the session, and the next ⇧⌘N starts a
# new one without its cookie. Nothing private is written: after the quit the Spaces file and the
# profile's history have none of its pages, the profile's cookies don't have its cookie, and a
# relaunch brings back no private window. A private window's Library has its session's downloads
# (#669): "d" in its page downloads a file, which its Library's Downloads lists, and the regular
# window's doesn't, nor does the profile's history after the quit.

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
  if (e.key == "d") {
    document.getElementById("d").click();
  }
};
</script><a id=d download href="private-notes.txt"></a>' > "$out/site/private.html"
printf '%s' 'a private download' > "$out/site/private-notes.txt"
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
send key w cmd shift
expect "^browser: window closed $regular\$" "⇧⌘W didn't close the window ⌘N opened"
send window 1
send click $page_point
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
empty=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
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

# Its download is its Library's: ⇧⌘J opens the Library on Downloads, which lists it alone.
send click $page_point
send key d
for _ in $(seq 100); do
  grep -q '^browser: download [0-9]* done private-notes.txt$' "$log" && break
  sleep 0.1
done
grep -q '^browser: download [0-9]* done private-notes.txt$' "$log" ||
  fail "the private page's download didn't finish"
send key j cmd shift
expect '^browser: library section downloads$' "⇧⌘J didn't open the private window's Library"
expect '^browser: library lists downloads: private-notes.txt$' \
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
*private-notes.txt*) fail "the regular window's Library listed the private window's download" ;;
esac
send key j cmd shift
expect '^browser: library hidden$' "⇧⌘J didn't close the regular window's Library"

# The profile's second private window shares the session.
send window 4
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' \
  "⇧⌘N in the private window didn't open another"
second=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
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
open_private_page "$third"
expect "^browser: title $tab cookie none\$" "the new private session had the ended one's cookie"
# Chrome destroying the profile and the core finding it gone both end a session, and the app hears
# it once: the two sessions so far, the empty window's and the cookie's.
[ "$(grep -c '^browser: private session ended ' "$log")" = 2 ] ||
  fail "the app heard a session end more than once"

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
echo "ok: ⇧⌘N opens a private window titled Incognito, whose cookie the regular window doesn't see and the profile's other private window does, whose download its Library lists and the regular window's doesn't, closing the last one ends the session, the next starts without its cookie, and neither the Spaces file, the history, the cookies nor a relaunch has any of it. Windows: $out/private.png, $out/private-library.png"
