# check.sh runs this at 204.
# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. A pop-up filter closes the tab a page's link opens, before its page
# reaches the server, as uBlock Origin does, and the app never lists it: a page's pop-up waits for
# the filters' answer about its address, not for its page (#2806), and the core logs how long after
# the click it was listed and how long it waited. window.open() with no address and a middle click
# are listed at once, and the tab window.open() opens with no address is blocked once it's sent on
# to such a page (#2839). A private window blocks a script a filter names, as
# its profile's requests go through the same engine (#2111's private windows gate). A filter on a
# response's header (#2758) fails a script whose response has it before the page runs any of it,
# and a redirect's response too, whose target is never asked for. My filters uBlock Origin kept, as
# typed in its own dashboard before Settings had them, carry over once: a profile uBlock Origin kept
# a filter in, launched again with the browser's own and no filters given, blocks what it names, and
# keeps it as its My filters, while uBlock Origin's storage stays for the way back.

# An engine from before the page warning lets the page through, and one from before the carry-over
# leaves My filters behind.
if ! engine_has ../core/native_ad_block.cc ../core/ad_block.cc; then
  echo "skip: adblock-page. This engine is from before the native ad blocker's page warning or" \
    "its carry-over of My filters."
  exit 0
fi
serve
# The site's log from here on: an earlier run in the same call, as history's held link, logs to it too.
site_from=$(($(wc -l < "$out/site.log") + 1))
site_since() {
  tail -n +"$site_from" "$out/site.log"
}
printf '%s' '<title>the blocked page</title>' > "$out/site/blocked.html"
# A page that's all a link to a pop-up but for a strip down its left edge, so a click anywhere but
# there opens it in a new tab. Its title says when it has drawn, as Blink drops a press before a
# page's first frame, when it has the press, when it loses the keyboard and each key it gets.
printf '%s' '<title>the opener</title>
<a id=link href="popup.html" target="_blank" style="position:fixed;inset:0 0 0 100px">a pop-up</a>
<script>
link.onmousedown = () => document.title = "the opener pressed";
onblur = () => document.title = "the opener blurred";
onkeydown = event => document.title = "the opener has " + event.key;
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "the opener ready"));
</script>' > "$out/site/opener.html"
printf '%s' '<title>the pop-up</title>' > "$out/site/popup.html"
# Pages whose click opens a tab the filters let through: a link's, to /held, whose page comes only
# once the run lets it, and window.open()'s. And window.open() with no address, sent on to the
# pop-up after. Each says in its title when it has drawn and when it has the press, as the opener.
said() {
  printf '<script>
onmousedown = () => document.title = "%s pressed";
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "%s ready"));
</script>' "$1" "$1"
}
printf '%s' '<title>the held opener</title>
<a href="held" target="_blank" style="position:fixed;inset:0">a pop-up let through</a>' \
  > "$out/site/opener-held.html"
said "the held opener" >> "$out/site/opener-held.html"
printf '%s' '<title>the script opener</title>
<script>onclick = () => open("held")</script>' > "$out/site/opener-open.html"
said "the script opener" >> "$out/site/opener-open.html"
printf '%s' '<title>the blank opener</title>
<script>onclick = () => { open("").location = "popup.html" }</script>' > "$out/site/opener-blank.html"
said "the blank opener" >> "$out/site/opener-blank.html"
# Three scripts: a's response has the filter's header, b's another value, and c is where a
# redirect with the header goes. Its title names those that ran.
printf '%s' '<title>headers</title>
<script>var ran = [];</script>
<script src="headed?x-ad=1&n=a"></script>
<script src="headed?x-ad=0&n=b"></script>
<script src="headed?x-ad=1&to=headed%3Fn%3Dc"></script>
<script>document.title = "ran " + ran.join(" ");</script>' > "$out/site/headers.html"
# A page whose script a filter names, which says in its title whether the script ran.
printf '%s' 'self.checkAd = true;' > "$out/site/check-ad.js"
printf '%s' '<title>loading</title><script src="check-ad.js"></script>
<script>document.title = self.checkAd ? "ad script ran" : "ad script blocked";</script>' \
  > "$out/site/ad.html"
# localhost, where the check's server is too, as a list writes a host: a line of its own.
blocked="http://localhost:$port/blocked.html"
filters="$(printf '%s\n' localhost '/popup.html$popup' '/headed?$script,header=x-ad:1' \
  '/check-ad.js')"
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
site_since | grep -q '/blocked\.html' && fail "the listed page reached the server"
# A page gets no press or key before its first frame: Blink drops them, as it shows nothing to
# press yet, and the title comes before that frame. So the run captures the window until the
# warning shows, as dark marks on its card's middle that the blank page before it hasn't (#2840).
# warning_shown <capture>: whether it does.
warning_shown() {
  if windows; then
    to_bmp "$1" "$out/warning.bmp" || fail "PowerShell couldn't read $1"
  elif [ "$(uname)" = Linux ]; then
    magick "$1" BMP3:"$out/warning.bmp" || fail "ImageMagick couldn't read $1"
  else
    sips -s format bmp "$1" --out "$out/warning.bmp" > /dev/null || fail "sips couldn't read $1"
  fi
  python3 -I - "$out/warning.bmp" << 'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
offset, = struct.unpack_from("<I", data, 10)
width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
stride = (bits * width + 31) // 32 * 4
def at(x, y):
    row = y if height < 0 else abs(height) - 1 - y
    i = offset + row * stride + x * bits // 8
    return data[i:i + 3]
# The card's own colour, high in its right half, against its middle, where the heading, the
# address and Open anyway are.
card = at(int(width * 0.9), int(abs(height) * 0.2))
marks = (sum(abs(a - b) for a, b in zip(at(x, y), card)) > 150
         for y in range(int(abs(height) * 0.4), int(abs(height) * 0.6), 2)
         for x in range(int(width * 0.45), int(width * 0.75), 2))
sys.exit(0 if any(marks) else 1)
PY
}
shown=
for _ in $(seq 100); do
  ../tools/window-screenshot.sh "$pid" "$out/adblock-page.png" || fail "no window to capture"
  warning_shown "$out/adblock-page.png" && shown=1 && break
  sleep 0.1
done
[ -n "$shown" ] || fail "the warning's page never drew its warning: $out/adblock-page.png"

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
pressed=$step
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
site_since | grep -q '/popup\.html' && fail "the pop-up's page reached the server"
# The page that opened it keeps the keyboard throughout, through the hold too, which at some 5 ms
# is too short to type into: no other view took it, or the page would have blurred, and a key
# after the tab closed reaches it. On Windows, the pop-up's tab once showed the browser window
# again, which moved the keyboard to the window itself (#2806).
since "$pressed" | grep -q "^browser: title $tab the opener blurred\$" &&
  fail "the page that opened the pop-up the filter closed lost the keyboard: $(since "$pressed" | grep -e '^browser: drive' -e "^browser: title $tab " | tr '\n' ' ')"
send key x
expect "^browser: title $tab the opener has x\$" \
  "a key after the pop-up filter closed the link's tab didn't reach the page that opened it: the keyboard is in $key, on $focus"
# Opens a pop-up from tab $1's page, titled $2, once it has drawn, with a press it has had and its
# release, as the opener above.
open_from() {
  expect "^browser: title $1 $2 ready\$" "the $2 page didn't draw"
  send press $page_point
  expect "^browser: title $1 $2 pressed\$" "the $2 page didn't take the press"
  send release $page_point
}
# The pop-up hold, on an engine with it (#2806).
held_said=". This engine is from before the pop-up hold"
if engine_has ../core/popup_hold.cc ../core/native_ad_block.cc; then
  # The filters answer before the app hears of the tab, which it never lists or shows.
  held=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: \(today\|selected\) ' |
    grep -v "^browser: \(today\|selected\) $tab\$" || true)
  [ -z "$held" ] || fail "the app listed or showed the pop-up's tab before it closed: $held"

  # A pop-up they let through is listed at their answer, before its page comes: /held answers only
  # once held.go is there.
  rm -f "$out/site/held.go"
  go_from_bar "${site#http://}opener-held.html"
  open_from "$tab" "the held opener"
  expect "^browser: tab [0-9]* opened by $tab in the foreground\$" \
    "the tab the link opened wasn't listed while its page hadn't come: the hold waits for more than the filters' answer"
  link_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $tab in the foreground\$/\1/p" "$log" | head -1)
  # The site logs a request as it comes, and its answer's line only once it's sent.
  site_since | grep -q '"GET /held HTTP' && fail "/held answered before the run let it"
  link_cost=$(sed -n "s/^browser: tab $link_tab listed +\([0-9.]*\) ms after the press in tab $tab\$/\1/p" "$log")
  link_held=$(sed -n "s/^browser: pop-up $link_tab held \([0-9.]*\) ms\$/\1/p" "$log")
  [ -n "$link_cost" ] && [ -n "$link_held" ] ||
    fail "the core didn't say how long the link's tab took and how long it was held: $(grep "^browser: \(tab\|pop-up\) $link_tab " "$log" | head -4 | tr '\n' ' ')"
  touch "$out/site/held.go"
  expect "^browser: title $link_tab held\$" "the link's pop-up the filters let through didn't load"
  # Its page has the keyboard: the hold kept content from focusing it (#2806), so the app's focus,
  # as it shows the tab, is the one it takes.
  send key x
  expect "^browser: title $link_tab held has x\$" \
    "a key after the link's pop-up the filters let through loaded didn't reach its page: the keyboard is in $key, on $focus"

  # window.open() to an address they let through, timed the same way. Its page comes at once now.
  go_from_bar "${site#http://}opener-open.html"
  open_from "$link_tab" "the script opener"
  expect "^browser: tab [0-9]* opened by $link_tab in the foreground\$" "window.open()'s tab wasn't listed"
  open_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $link_tab in the foreground\$/\1/p" "$log" | head -1)
  open_cost=$(sed -n "s/^browser: tab $open_tab listed +\([0-9.]*\) ms after the press in tab $link_tab\$/\1/p" "$log")
  open_held=$(sed -n "s/^browser: pop-up $open_tab held \([0-9.]*\) ms\$/\1/p" "$log")
  [ -n "$open_cost" ] && [ -n "$open_held" ] ||
    fail "the core didn't say how long window.open()'s tab took and how long it was held: $(grep "^browser: \(tab\|pop-up\) $open_tab " "$log" | head -4 | tr '\n' ' ')"
  expect "^browser: title $open_tab held\$" "window.open()'s pop-up the filters let through didn't load"

  # window.open() with no address is listed at once, not held, and closes once sent to a page a
  # pop-up filter names, as the filters judge the first page it's sent to (#2839).
  go_from_bar "${site#http://}opener-blank.html"
  open_from "$open_tab" "the blank opener"
  expect "^browser: tab [0-9]* opened by $open_tab in the foreground\$" \
    "window.open() with no address wasn't listed at once"
  blank_tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: tab \([0-9]*\) opened by $open_tab in the foreground\$/\1/p" "$log" | head -1)
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "^browser: pop-up $blank_tab held " &&
    fail "window.open() with no address was held"
  expect "^browser: tab $blank_tab closed\$" \
    "window.open()'s tab didn't close once sent to the page a pop-up filter names"

  # A middle click on the link the filter names opens the user's tab, behind and listed at once.
  # The tab in front once window.open()'s closed is the app's to choose, so the log says which, and
  # the steps after go on in it.
  go_from_bar "${site#http://}opener.html"
  expect "^browser: title [0-9]* the opener ready\$" "the opener page didn't draw again"
  tab=$(sed -n "/^browser: drive $step: /,\$s/^browser: title \([0-9]*\) the opener ready\$/\1/p" "$log" | head -1)
  send middleclick $page_point
  expect "^browser: tab [0-9]* opened by $tab in the background\$" \
    "the tab a middle click opened wasn't listed at once"
  held_said=", and the app never listed it. A link's pop-up the filters let through was listed \
$link_cost ms after the click, held $link_held ms, before its page came; window.open()'s \
$open_cost ms, held $open_held ms. window.open() with no address and a middle click were listed at \
once"
fi

# The header filter: only b runs. a reached the server, as its header comes only with its answer.
go_from_bar "${site#http://}headers.html"
expect "^browser: title $tab ran" "the headers page didn't run its scripts"
ran=$(grep "^browser: title $tab ran" "$log" | tail -1 | sed "s/^browser: title $tab //")
[ "$ran" = "ran b" ] || fail "the header filter let through what it names: the page says \
\"$ran\", where only b, whose x-ad header is 0, should run; a's is 1, and c is where a redirect \
whose x-ad is 1 goes"
served '/headed?x-ad=1&n=a' "a's request never reached the server"
site_since | grep -q '/headed?n=c' &&
  fail "the redirect whose response has the header went on to c"
# A pop-up window.open() opens with no address commits about:blank at once, and the filters still
# judge the first page it's sent to (#2839): sent on to the page a pop-up filter names, its tab
# closes before that page reaches the server. The press first, as the opener's above.
printf '%s' '<title>the empty opener</title>
<script>
onclick = () => { open("").location = "popup.html?empty" };
onmousedown = () => document.title = "the empty opener pressed";
requestAnimationFrame(() => requestAnimationFrame(() => document.title = "the empty opener ready"));
</script>' > "$out/site/opener-empty.html"
go_from_bar "${site#http://}opener-empty.html"
expect "^browser: title $tab the empty opener ready\$" "the empty opener page didn't draw"
send press $page_point
expect "^browser: title $tab the empty opener pressed\$" "the empty opener page didn't take the press"
send release $page_point
expect "^browser: \(ad block closed a pop-up at ${site}popup.html?empty\|loaded [0-9]* ${site}popup.html?empty\)\$" \
  "the tab window.open() opened with no address, sent on to the page a pop-up filter names, neither closed nor loaded it"
grep -q "^browser: loaded [0-9]* ${site}popup.html?empty\$" "$log" &&
  fail "the tab window.open() opened with no address loaded the page a pop-up filter names, sent on to it after its about:blank"
expect '^browser: tab [0-9]* closed$' "the empty pop-up's tab didn't close"
site_since | grep -q '/popup\.html?empty' && fail "the empty pop-up's page reached the server"
# A private window's profile blocks too: its requests go through the same engine, with the same
# filters, though it has a profile of its own.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
new_window_bar
send window 2
resting_key t "private window"
send insert "${site}ad.html"
send key return
expect "^browser: loaded [0-9]* ${site}ad.html\$" "the private window didn't load the ad page"
private_tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}ad.html\$|\1|p" "$log" | tail -1)
expect "^browser: title $private_tab ad script " "the private window's ad page didn't say"
grep -q "^browser: title $private_tab ad script blocked\$" "$log" ||
  fail "the private window didn't block the page's script: $(grep "^browser: title $private_tab " "$log" | tail -1)"
site_since | grep -q '/check-ad\.js' && fail "the private window's script reached the server"
../tools/window-screenshot.sh "$pid" "$out/adblock-page-private.png" all ||
  fail "no window to capture"

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
  "a link opened before its page reached the server$held_said. A pop-up filter also" \
  "closed the tab window.open() opened with no address, once sent on to the page it names. A" \
  "header filter failed a script and a" \
  "redirect by their responses' header. In a private window, a script a filter names never" \
  "reached the server. A filter uBlock Origin kept in its own storage blocked with the browser's" \
  "own engine on that profile's next launch, given no filters, and stayed as its My filters, with" \
  "uBlock Origin's storage kept. Windows: $out/adblock-page.png, $out/adblock-page-opened.png," \
  "$out/adblock-page-private.png"
