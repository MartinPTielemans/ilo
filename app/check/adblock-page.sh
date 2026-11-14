# check.sh runs this at 204.
# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. A pop-up filter closes the tab a page's link opens, before its page
# reaches the server, as uBlock Origin does, and the app never lists it: a page's pop-up waits for
# the filters' answer about its address, not for its page (#2806), and the core logs how long after
# the click it was listed and how long it waited. window.open() with no address and a middle click
# are listed at once.

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
# Pages whose click opens a tab the filters let through: a link's, to /held, whose page comes
# only once the run lets it, and window.open()'s. And window.open() with no address, sent on to
# the pop-up after.
printf '%s' '<title>the held opener</title>
<a href="held" target="_blank" style="position:fixed;inset:0">a pop-up let through</a>' \
  > "$out/site/opener-held.html"
printf '%s' '<title>the script opener</title>
<script>onclick = () => open("held")</script>' > "$out/site/opener-open.html"
printf '%s' '<title>the blank opener</title>
<script>onclick = () => { open("").location = "popup.html" }</script>' > "$out/site/opener-blank.html"
# localhost, where the check's server is too, as a list writes a host: a line of its own.
blocked="http://localhost:$port/blocked.html"
filters="$(printf '%s\n' localhost '/popup.html$popup')"
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
# The pop-up hold, on an engine with it (#2806).
if ! engine_has ../core/popup_hold.cc ../core/native_ad_block.cc; then
  quit
  echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
    "warning, whose Open anyway, from the keyboard, opened the page. A pop-up filter closed the tab" \
    "a link opened before its page reached the server. This engine is from before the pop-up hold." \
    "Windows: $out/adblock-page.png, $out/adblock-page-opened.png"
  exit 0
fi
# The filters answer before the app hears of the tab, which it never lists or shows.
held=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: \(today\|selected\) ' |
  grep -v "^browser: \(today\|selected\) $tab\$" || true)
[ -z "$held" ] || fail "the app listed or showed the pop-up's tab before it closed: $held"

# A pop-up they let through is listed at their answer, before its page comes: /held answers only
# once held.go is there.
rm -f "$out/site/held.go"
go_from_bar "${site#http://}opener-held.html"
expect "^browser: title $tab the held opener\$" "the held opener page didn't load"
send click $page_point
expect "^browser: tab [0-9]* opened by $tab in the foreground\$" \
  "the tab the link opened wasn't listed while its page hadn't come: the hold waits for more than the filters' answer"
link_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $tab in the foreground\$/\1/p" "$log" | head -1)
# The site logs a request as it comes, and its answer's line only once it's sent.
grep -q '"GET /held HTTP' "$out/site.log" && fail "/held answered before the run let it"
link_cost=$(sed -n "s/^browser: tab $link_tab listed +\([0-9.]*\) ms after the press in tab $tab\$/\1/p" "$log")
link_held=$(sed -n "s/^browser: pop-up $link_tab held \([0-9.]*\) ms\$/\1/p" "$log")
[ -n "$link_cost" ] && [ -n "$link_held" ] ||
  fail "the core didn't say how long the link's tab took and how long it was held: $(grep "^browser: \(tab\|pop-up\) $link_tab " "$log" | head -4 | tr '\n' ' ')"
touch "$out/site/held.go"
expect "^browser: title $link_tab held\$" "the link's pop-up the filters let through didn't load"

# window.open() to an address they let through, timed the same way. Its page comes at once now.
go_from_bar "${site#http://}opener-open.html"
expect "^browser: title $link_tab the script opener\$" "the script opener page didn't load"
send click $page_point
expect "^browser: tab [0-9]* opened by $link_tab in the foreground\$" "window.open()'s tab wasn't listed"
open_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $link_tab in the foreground\$/\1/p" "$log" | head -1)
open_cost=$(sed -n "s/^browser: tab $open_tab listed +\([0-9.]*\) ms after the press in tab $link_tab\$/\1/p" "$log")
open_held=$(sed -n "s/^browser: pop-up $open_tab held \([0-9.]*\) ms\$/\1/p" "$log")
[ -n "$open_cost" ] && [ -n "$open_held" ] ||
  fail "the core didn't say how long window.open()'s tab took and how long it was held: $(grep "^browser: \(tab\|pop-up\) $open_tab " "$log" | head -4 | tr '\n' ' ')"
expect "^browser: title $open_tab held\$" "window.open()'s pop-up the filters let through didn't load"

# window.open() with no address is listed at once, not held, and closes once sent to a page a
# pop-up filter names. Until #2839 the filters judge only a pop-up's first navigation, which its
# about:blank ends, as on main, so its page loads: that's skipped by name and ⌘W closes it. #2839's
# fix takes the skip out.
go_from_bar "${site#http://}opener-blank.html"
expect "^browser: title $open_tab the blank opener\$" "the blank opener page didn't load"
send click $page_point
expect "^browser: tab [0-9]* opened by $open_tab in the foreground\$" \
  "window.open() with no address wasn't listed at once"
blank_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $open_tab in the foreground\$/\1/p" "$log" | head -1)
sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "^browser: pop-up $blank_tab held " &&
  fail "window.open() with no address was held"
expect "^browser: \(ad block closed a pop-up at ${site}popup.html\|loaded $blank_tab ${site}popup.html\)\$" \
  "window.open()'s tab, sent on to the page a pop-up filter names, neither closed nor loaded it"
if sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "^browser: loaded $blank_tab ${site}popup.html\$"; then
  echo "skip: window.open() with no address closing once sent to a page a pop-up filter names: the filters judge only a pop-up's first navigation, which its about:blank ends (#2839)."
  send key w cmd
  expect "^browser: tab $blank_tab closed\$" "⌘W didn't close window.open()'s tab"
else
  expect "^browser: tab $blank_tab closed\$" \
    "window.open()'s tab didn't close once sent to the page a pop-up filter names"
fi

# A middle click on the link the filter names opens the user's tab, behind and listed at once.
# The tab in front once window.open()'s closed is the app's to choose, so the log says which.
go_from_bar "${site#http://}opener.html"
expect "^browser: title [0-9]* the opener\$" "the opener page didn't load again"
front=$(sed -n "/^browser: drive $step: /,\$s/^browser: title \([0-9]*\) the opener\$/\1/p" "$log" | head -1)
send middleclick $page_point
expect "^browser: tab [0-9]* opened by $front in the background\$" \
  "the tab a middle click opened wasn't listed at once"

quit
echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
  "warning, whose Open anyway, from the keyboard, opened the page. A pop-up filter closed the tab" \
  "a link opened before its page reached the server, and the app never listed it. A link's pop-up" \
  "the filters let through was listed $link_cost ms after the click, held $link_held ms, before its" \
  "page came; window.open()'s $open_cost ms, held $open_held ms. window.open() with no address" \
  "and a middle click were listed at once. Windows: $out/adblock-page.png," \
  "$out/adblock-page-opened.png"
