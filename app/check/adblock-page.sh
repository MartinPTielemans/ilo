# check.sh runs this at 204.
# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. A pop-up filter closes the tab a page's link opens, before its page
# reaches the server, as uBlock Origin does. A private window blocks a script a filter names, as
# its profile's requests go through the same engine (#2111's private windows gate).

# An engine from before the page warning lets the page through.
if ! engine_has ../core/native_ad_block.cc ../core/ad_block.cc; then
  echo "skip: adblock-page. This engine is from before the native ad blocker's page warning."
  exit 0
fi
serve
printf '%s' '<title>the blocked page</title>' > "$out/site/blocked.html"
# A page that's all a link to a pop-up, which a click anywhere on it opens in a new tab. Its title
# says when it has drawn, as Blink drops a press before a page's first frame, and when it has the
# press.
printf '%s' '<title>the opener</title>
<a id=link href="popup.html" target="_blank" style="position:fixed;inset:0">a pop-up</a>
<script>
link.onmousedown = () => document.title = "the opener pressed";
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "the opener ready"));
</script>' > "$out/site/opener.html"
printf '%s' '<title>the pop-up</title>' > "$out/site/popup.html"
# A page whose script a filter names, which says in its title whether the script ran.
printf '%s' 'self.checkAd = true;' > "$out/site/check-ad.js"
printf '%s' '<title>loading</title><script src="check-ad.js"></script>
<script>document.title = self.checkAd ? "ad script ran" : "ad script blocked";</script>' \
  > "$out/site/ad.html"
# localhost, where the check's server is too, as a list writes a host: a line of its own.
blocked="http://localhost:$port/blocked.html"
filters="$(printf '%s\n' localhost '/popup.html$popup' '/check-ad.js')"
profile_dir="$PWD/$out/adblock-page/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/adblock-page.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$blocked" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" --ad-block-engine=native --ad-block-filters="$filters" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
# No command is sent yet, so the wait reads the whole log.
await "^browser: title $tab Page blocked\$" ||
  fail "the listed page didn't show the warning: $(grep "^browser: title $tab " "$log" | tail -1)"
grep -q '/blocked\.html' "$out/site.log" && fail "the listed page reached the server"
../tools/window-screenshot.sh "$pid" "$out/adblock-page.png" || fail "no window to capture"

# Open anyway, the only button with no page to go back to: Tab from the page reaches it.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: adblock-page, after the warning. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
send key tab
send key space
expect "^browser: title $tab the blocked page\$" \
  "Open anyway didn't open the page: $(grep "^browser: title $tab " "$log" | tail -1)"
served '/blocked\.html' "Open anyway didn't ask the server for the page"
../tools/window-screenshot.sh "$pid" "$out/adblock-page-opened.png" || fail "no window to capture"

# A pop-up a filter names: its tab closes before the page reaches the server.
go_from_bar "${site#http://}opener.html"
expect "^browser: title $tab the opener ready\$" "the opener page didn't draw"
# The link opens its tab on the release, which the browser allows as a pop-up only with the
# activation the page sends it on the press, on another pipe (#2510). So the release waits for the
# page to have the press, as a hand's comes some 100 ms after it.
send press $page_point
expect "^browser: title $tab the opener pressed\$" "the opener page didn't take the press on its link"
send release $page_point
expect "^browser: ad block closed a pop-up at ${site}popup.html\$" \
  "the pop-up filter didn't close the tab the link opened"
# The filter decides before the app lists the pop-up's tab, so reads of Today before that tab
# closes can show it as before the click, then with the tab (#2801). Once the tab has closed,
# Today only loses it, if the app listed it at all.
expect '^browser: tab [0-9]* closed$' "the pop-up's tab didn't close"
for _ in $(seq 30); do
  [ "$(today)" = "$tab" ] && break
  sleep 0.1
done
[ "$(today)" = "$tab" ] || fail "the pop-up's tab stayed: Today is \"$(today)\""
grep -q '/popup\.html' "$out/site.log" && fail "the pop-up's page reached the server"

# A private window's profile blocks too: its requests go through the same engine, with the same
# filters, though it has a profile of its own.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
new_window_bar
send window 2
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
send insert "${site}ad.html"
send key return
expect "^browser: loaded [0-9]* ${site}ad.html\$" "the private window didn't load the ad page"
private_tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}ad.html\$|\1|p" "$log" | tail -1)
expect "^browser: title $private_tab ad script " "the private window's ad page didn't say"
grep -q "^browser: title $private_tab ad script blocked\$" "$log" ||
  fail "the private window didn't block the page's script: $(grep "^browser: title $private_tab " "$log" | tail -1)"
grep -q '/check-ad\.js' "$out/site.log" && fail "the private window's script reached the server"
../tools/window-screenshot.sh "$pid" "$out/adblock-page-private.png" all ||
  fail "no window to capture"

quit
echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
  "warning, whose Open anyway, from the keyboard, opened the page. A pop-up filter closed the tab" \
  "a link opened before its page reached the server. In a private window, a script a filter names" \
  "never reached the server. Windows: $out/adblock-page.png, $out/adblock-page-opened.png," \
  "$out/adblock-page-private.png"
