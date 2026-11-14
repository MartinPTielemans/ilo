# check.sh runs this at 204.
# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. A pop-up filter closes the tab a page's link opens, before its page
# reaches the server, as uBlock Origin does. A filter on a response's header (#2758) fails a script
# whose response has it before the page runs any of it, and a redirect's response too, whose target
# is never asked for.

# An engine from before the page warning lets the page through.
if ! engine_has ../core/native_ad_block.cc ../core/ad_block.cc; then
  echo "skip: adblock-page. This engine is from before the native ad blocker's page warning."
  exit 0
fi
serve
printf '%s' '<title>the blocked page</title>' > "$out/site/blocked.html"
# A page that's all a link to a pop-up, which a click anywhere on it opens in a new tab.
printf '%s' '<title>the opener</title>
<a href="popup.html" target="_blank" style="position:fixed;inset:0">a pop-up</a>' \
  > "$out/site/opener.html"
printf '%s' '<title>the pop-up</title>' > "$out/site/popup.html"
# Three scripts: a's response has the filter's header, b's another value, and c is where a
# redirect with the header goes. Its title names those that ran.
printf '%s' '<title>headers</title>
<script>var ran = [];</script>
<script src="headed?x-ad=1&n=a"></script>
<script src="headed?x-ad=0&n=b"></script>
<script src="headed?x-ad=1&to=headed%3Fn%3Dc"></script>
<script>document.title = "ran " + ran.join(" ");</script>' > "$out/site/headers.html"
# localhost, where the check's server is too, as a list writes a host: a line of its own.
blocked="http://localhost:$port/blocked.html"
filters="$(printf '%s\n' localhost '/popup.html$popup' '/headed?$script,header=x-ad:1')"
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
expect "^browser: title $tab the opener\$" "the opener page didn't load"
send click $page_point
expect "^browser: ad block closed a pop-up at ${site}popup.html\$" \
  "the pop-up filter didn't close the tab the link opened"
# Its tab may close before the app lists it, so this reads the Today the app shows last.
for _ in $(seq 30); do
  [ "$(today)" = "$tab" ] && break
  sleep 0.1
done
[ "$(today)" = "$tab" ] || fail "the pop-up's tab stayed: Today is \"$(today)\""
grep -q '/popup\.html' "$out/site.log" && fail "the pop-up's page reached the server"

# The header filter: only b runs. a reached the server, as its header comes only with its answer.
go_from_bar "${site#http://}headers.html"
expect "^browser: title $tab ran" "the headers page didn't run its scripts"
ran=$(grep "^browser: title $tab ran" "$log" | tail -1 | sed "s/^browser: title $tab //")
[ "$ran" = "ran b" ] || fail "the header filter let through what it names: the page says \
\"$ran\", where only b, whose x-ad header is 0, should run; a's is 1, and c is where a redirect \
whose x-ad is 1 goes"
served '/headed?x-ad=1&n=a' "a's request never reached the server"
grep -q '/headed?n=c' "$out/site.log" &&
  fail "the redirect whose response has the header went on to c"

quit
echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
  "warning, whose Open anyway, from the keyboard, opened the page. A pop-up filter closed the tab" \
  "a link opened before its page reached the server. A header filter failed a script and a" \
  "redirect by their responses' header. Windows: $out/adblock-page.png," \
  "$out/adblock-page-opened.png"
