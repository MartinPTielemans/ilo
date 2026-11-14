# check.sh runs this at 204.
# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. A pop-up filter closes the tab a page's link opens, before its page
# reaches the server, as uBlock Origin does. My filters uBlock Origin kept, as typed in its own
# dashboard before Settings had them, carry over once: a profile uBlock Origin kept a filter in,
# launched again with the browser's own and no filters given, blocks what it names, and keeps it as
# its My filters, while uBlock Origin's storage stays for the way back.

# An engine from before the page warning lets the page through, and one from before the carry-over
# leaves My filters behind.
if ! engine_has ../core/native_ad_block.cc ../core/ad_block.cc; then
  echo "skip: adblock-page. This engine is from before the native ad blocker's page warning or" \
    "its carry-over of My filters."
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
expect "^browser: title $tab the opener ready\$" "the opener page didn't draw"
# The link opens its tab on the release, which the browser allows as a pop-up only with the
# activation the page sends it on the press, on another pipe (#2510). So the release waits for the
# page to have the press, as a hand's comes some 100 ms after it.
send press $page_point
expect "^browser: title $tab the opener pressed\$" "the opener page didn't take the press on its link"
send release $page_point
expect "^browser: ad block closed a pop-up at ${site}popup.html\$" \
  "the pop-up filter didn't close the tab the link opened"
# Its tab may close before the app lists it, so this reads the Today the app shows last.
for _ in $(seq 30); do
  [ "$(today)" = "$tab" ] && break
  sleep 0.1
done
[ "$(today)" = "$tab" ] || fail "the pop-up's tab stayed: Today is \"$(today)\""
grep -q '/popup\.html' "$out/site.log" && fail "the pop-up's page reached the server"

quit

# My filters carried over from uBlock Origin. The page asks for its script every 100 ms, as the
# filter applies a moment after the launch's page starts, once the settings are read.
printf '%s' 'self.carried = true;' > "$out/site/carried-ad.js"
printf '%s' '<title>loading</title>
<script>
let n = 0;
setInterval(() => fetch("carried-ad.js?" + n++).then(
  () => document.title = "carried script loaded",
  () => document.title = "carried script blocked"), 100);
</script>' > "$out/site/carried.html"
carried="${site}carried.html"
carry_dir="$PWD/$out/adblock-page/carry"
rm -rf "$carry_dir"
mkdir -p "$carry_dir"
# uBlock Origin keeps the filter it's given in its own storage, as one typed in its dashboard.
for carry_engine in ublock native; do
  log="$out/adblock-page-carry-$carry_engine.log"
  step=0
  if [ "$carry_engine" = ublock ]; then
    carry_switch=--ad-block-filters=/carried-ad.js
  else
    carry_switch=--ad-block-engine=native
  fi
  BROWSER_DRIVE=1 BROWSER_URL="$carried" BROWSER_PROFILE_DIR="$carry_dir" \
    "$browser" "$carry_switch" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: loaded [0-9]* $carried\$" ||
    fail "$carry_engine: the app didn't load the carried page"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) $carried\$|\1|p" "$log" | head -1)
  await "^browser: title $tab carried script blocked\$" ||
    fail "$carry_engine: the carried filter didn't block the page's script, which last said:" \
      "$(grep "^browser: title $tab " "$log" | tail -1)"
  quit
done
python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    filters = json.load(f).get("settings", {}).get("ad_block_filters")
if filters != "/carried-ad.js":
    sys.exit("My filters after the carry-over: %r" % (filters,))
' "$carry_dir/Spaces.json" ||
  fail "the carried filter isn't the profile's My filters in Spaces.json"
[ -d "$carry_dir/Default/Local Extension Settings/oghkpboejbohceocapipmnjifoklonij" ] ||
  fail "uBlock Origin's storage went with the carry-over, so the way back has no filters"

echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
  "warning, whose Open anyway, from the keyboard, opened the page. A pop-up filter closed the tab" \
  "a link opened before its page reached the server. A filter uBlock Origin kept in its own" \
  "storage blocked with the browser's own engine on that profile's next launch, given no filters," \
  "and stayed as its My filters, with uBlock Origin's storage kept. Windows:" \
  "$out/adblock-page.png, $out/adblock-page-opened.png"
