# A page's popup opens as a tab in the window, under the page. The page's context menu is Chrome's
# without the items the window can't do, shown in ilo's menu at the click: a dismiss leaves the page
# free to show another, and a text field's has its spelling and writing direction items and Select
# All, which selects the field's text for typing to replace. Its Open Link in New Tab opens a tab
# under the page without showing it, after the tabs the page opened before and above the others, and
# closing one shows the nearest other tab the page opened. A link's tab opened behind shows the
# link's address and loads only once it's first shown, and one still waiting at a quit comes back
# after the relaunch, while a window the page makes behind loads at once. Its Print and ⌘P from the
# page show print preview centred over the page, at its top, which takes the keys typed and escape
# closes. Switching or closing a full-screen page's tab ends its full screen and the window's, and
# closing it shows the tab it opened first. A sign-in popup, as Google's, keeps its opener through
# the identity site and back, sends the page what it got and closes itself, which the page sees,
# and the page shows again.

# A page's popup opens as a tab in the window, as links to new tabs do, instead of a window of
# Chrome's own.
page='data:text/html,<title>loading</title>
<a href="https://example.org/" style="font:24px system-ui">a link</a>
<div id="opens" style="position:absolute;left:0;top:100px;width:200px;height:60px">a window</div>
<textarea id="field" style="position:absolute;left:0;top:200px;width:300px;height:60px">teh note</textarea>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
// A window the page can reach, behind with ⌘ held.
document.getElementById("opens").onclick = () => open("https://example.com/");
// The text of the field as the title, for the check to read what typing did to it.
document.getElementById("field").oninput = e => document.title = e.target.value;
// A popup needs a user gesture, which a key press is.
addEventListener("keydown", e =>
  e.key == "p" && !e.'"$cmd_key"' && open("https://example.com/", "", "popup,width=400,height=300"));
addEventListener("keydown", e =>
  e.key == "f" && !e.'"$cmd_key"' && document.documentElement.requestFullscreen());
</script>'
log="$out/popup.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the popup test page didn't draw"
opener=$(grep -m1 '^browser: today [0-9]' "$log" | cut -d' ' -f3)
# A tab the page didn't open, dragged under the page, for the links it opens behind to go above.
# Opened first: a tab opened from the bar in front has the Space forget which tabs opened which, as
# Chrome's tab strip does, so after the popup it would cut the popup off from its page.
open_from_bar example.net
other=$tab
send drag $(row 0) 100 330
expect "^browser: dragged $other to today 1\$" "a drag of the top row to the bottom didn't move it"
expect "^browser: today $opener $other\$" \
  "the dragged tab didn't go under the page: $(grep '^browser: today' "$log" | tail -1)"
switch "1 cmd" "$opener"
send click $page_point
send key p
expect '^browser: tab [0-9]* opened by [0-9]* in the foreground$' "the page's popup didn't open as a tab"
popup=$(grep '^browser: tab [0-9]* opened by' "$log" | cut -d' ' -f3)
expect "^browser: today $opener $popup $other\$" "the popup's tab didn't go under the page: $(grep '^browser: today' "$log" | tail -1)"
expect '^browser: loaded [0-9]* https://example.com/$' "the popup's page didn't load in its tab"
check_layout "with the popup's tab"
../tools/window-screenshot.sh "$pid" "$out/popup.png" || fail "no window to capture"
# The page's context menu is Chrome's, with Arc's link items and ilo's Open Link in Peek (#1370),
# without those Arc's leaves out or whose UI is in Chrome's own window (Copy Link Text, Reading
# Mode, Lens, QR Code, Cast). It shows in ilo's menu, with its top left at the click. Off the Mac
# its items are in sentence case, as Chrome's are there.
if windows || [ "$(uname)" = Linux ]; then
  in_tab='Open link in new tab'
  link_items="$in_tab, Open link in Peek, Open link in new window, Open link in split view, "
  link_items="${link_items}Open link in incognito window, Save link as…"
else
  in_tab='Open Link in New Tab'
  link_items="$in_tab, Open Link in Peek, Open Link in New Window, Open Link in Split View, "
  link_items="${link_items}Open Link in Incognito Window, Save Link As…"
fi
send key 1 cmd
expect "^browser: selected $opener in " "⌘1 didn't show the page again"
post rightclick 250 30
expect "^browser: page menu $opener at 250,30\$" \
  "the page's menu isn't at the click: $(grep '^browser: page menu' "$log" | tail -1)"
expect "^browser: menu shown $link_items, " \
  "a right-click on the page's link didn't show the link's menu"
menu=$(grep '^browser: menu shown' "$log" | tail -1 | cut -d' ' -f4-)
case "$menu" in
*"Link Text"* | *"link text"* | *"Reading Mode"* | *"reading mode"* | *Lens* | *"QR Code"* | \
  *"QR code"* | *Cast*)
  fail "the link's menu offers what Arc's leaves out: $menu" ;;
esac
../tools/window-screenshot.sh "$pid" "$out/link-menu.png" || fail "no window to capture"
send choose $in_tab
expect "^browser: tab [0-9]* opened by $opener in the background\$" \
  "Open Link in New Tab didn't open a tab under the page"
none_since '^browser: selected' || fail "Open Link in New Tab showed its tab"
# Links opened behind line up after the tabs their page opened, in the order opened, and above the
# tabs it didn't open (Chrome).
behind() {
  grep "^browser: tab [0-9]* opened by $opener in the background\$" "$log" | tail -1 | cut -d' ' -f3
}
first=$(behind)
expect "^browser: today $opener $popup $first $other\$" \
  "the link's tab didn't go after the popup's: $(grep '^browser: today' "$log" | tail -1)"
# It waits to load until it's first shown (Dia), with the link's address.
expect "^browser: page $first unloaded 1\$" "the link's tab behind didn't wait to load"
expect "^browser: page $first back 0 forward 0 loading 0 https://example.org/\$" \
  "the link's tab behind doesn't have the link's address"
post rightclick 250 30
expect "^browser: menu shown $in_tab, " \
  "a right-click on the link didn't show its menu"
send choose $in_tab
expect "^browser: tab [0-9]* opened by $opener in the background\$" \
  "Open Link in New Tab didn't open a second tab"
second=$(behind)
expect "^browser: today $opener $popup $first $second $other\$" \
  "the second link's tab didn't go after the first's: $(grep '^browser: today' "$log" | tail -1)"
expect "^browser: page $second unloaded 1\$" "the second link's tab didn't wait to load"
../tools/window-screenshot.sh "$pid" "$out/links-behind.png" || fail "no window to capture"
grep -q "^browser: loaded \($first\|$second\) " "$log" && fail "a link's tab loaded behind"
# Each loads once it's shown. Closing one of them goes to the nearest other tab the page opened,
# though a tab it didn't open is below it (Chrome).
send key 4 cmd
expect "^browser: selected $second\( in .*\)\{0,1\}\$" "⌘4 didn't show the second link's tab"
expect "^browser: loaded $second https://example.org/\$" \
  "the second link's tab didn't load once shown"
expect "^browser: page $second unloaded 0\$" "the second link's tab still says it's waiting"
grep -q "^browser: loaded $first " "$log" && fail "the first link's tab loaded with the second"
send key w cmd
expect "^browser: selected $first\( in .*\)\{0,1\}\$" \
  "closing a link's tab didn't show the page's other link's tab"
expect "^browser: loaded $first https://example.org/\$" \
  "the first link's tab didn't load once shown"
switch "1 cmd" "$opener"
# A third, still waiting at the quit, for the relaunch at the end.
post rightclick 250 30
expect "^browser: menu shown $in_tab, " \
  "a right-click on the link didn't show its menu"
send choose $in_tab
expect "^browser: tab [0-9]* opened by $opener in the background\$" \
  "Open Link in New Tab didn't open a third tab"
third=$(behind)
expect "^browser: page $third unloaded 1\$" "the third link's tab didn't wait to load"
# A window the page makes itself behind, which it can reach, loads at once (Chrome).
send click 300 140 cmd
expect "^browser: tab [0-9]* opened by $opener in the background\$" \
  "⌘-click on what opens a window didn't open a tab behind"
made=$(behind)
expect "^browser: loaded $made https://example.com/\$" \
  "the window the page made behind didn't load at once"
# Print preview is the tab's dialog, a child window centred over the page, at its top. Once $1
# has asked for it: waits for it, captures it in $out/$2 once it has drawn over before-print.png,
# a capture of the window before, and closes it with escape. With $3, types it and a tab first,
# which go to the preview, whose focus moves, and not to the page's field under it or the app.
print_preview() {
  child=""
  for _ in $(seq 50); do
    send layout
    child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout child' |
      cut -d' ' -f4-)
    [ -n "$child" ] && break
    sleep 0.1
  done
  [ -n "$child" ] || fail "$1 didn't show print preview"
  shown=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' | cut -d' ' -f2-)
  x=${child%%,*} y=${child#*,} y=${y%% *} width=${child#* } width=${width%x*}
  page_x=${shown%%,*} page_y=${shown#*,} page_y=${page_y%% *} page_width=${shown#* }
  page_width=${page_width%x*}
  off=$((2 * x + width - 2 * page_x - page_width))
  [ "$y" -eq "$page_y" ] && [ "$off" -ge -1 ] && [ "$off" -le 1 ] ||
    fail "$1 showed print preview at $child, over the page at $shown"
  # Its window is on screen before its page draws, which then takes escape once it has loaded.
  for _ in $(seq 50); do
    ../tools/window-screenshot.sh "$pid" "$out/$2" || fail "no window to capture"
    cmp -s "$out/before-print.png" "$out/$2" || break
    sleep 0.1
  done
  if [ -n "${3:-}" ]; then
    typed=$step
    send type "$3"
    send key tab
    ../tools/window-screenshot.sh "$pid" "$out/${2%.png}-typed.png" || fail "no window to capture"
    sed -n "/^browser: drive $typed: /,\$p" "$log" | grep -E '^browser: (title|command) ' &&
      fail "typing with $1's print preview shown reached the page or the app"
  fi
  for _ in $(seq 50); do
    send key escape
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child' || break
    sleep 0.1
  done
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child' &&
    fail "escape didn't close the print preview $1 showed"
  rm "${out:?}/before-print.png"
}
send click $page_point
../tools/window-screenshot.sh "$pid" "$out/before-print.png" || fail "no window to capture"
# A menu closed without a choice runs nothing and leaves the page free to show the next, which a
# page whose menu Chrome still thought open wouldn't.
post rightclick $page_point
expect '^browser: menu shown Back' "a right-click on the page didn't show the page's menu"
send dismiss
expect '^browser: menu closed$' "dismiss didn't close the page's menu"
post rightclick $page_point
expect '^browser: menu shown Back' "the page didn't show its menu again after a dismiss"
menu=$(grep '^browser: menu shown' "$log" | tail -1 | cut -d' ' -f4-)
# No tab came before it, and none after.
if windows || [ "$(uname)" = Linux ]; then
  page_items="Back (disabled), Forward (disabled), Reload, Save as…, Print…, Cast…, View page source, Inspect"
else
  page_items="Back (disabled), Forward (disabled), Reload, Save As…, Print…, Cast…, View Page Source, Inspect"
fi
[ "$menu" = "$page_items" ] ||
  fail "the page's menu is $menu"
../tools/window-screenshot.sh "$pid" "$out/page-menu.png" || fail "no window to capture"
if [ "$(uname)" = Linux ]; then
  # Print preview there never gets the keyboard, so escape can't close it, and the driver can't
  # see it (#708).
  echo "skip: Print and ⌘P showing print preview, which escape closes: on Linux it never gets the keyboard and the driver can't see it (#708)."
  send dismiss
  expect '^browser: menu closed$' "dismiss didn't close the page's menu"
  rm "${out:?}/before-print.png"
else
  send choose Print…
  print_preview "the page menu's Print…" print.png
  # ⌘P, as File › Print, with the page's text field focused, as what's typed there shows. Its last
  # word stays spelt right, where the field's menu below is opened: a misspelt word's has no Select
  # All, Language Settings or Writing Direction (Chrome), and Windows checks the spelling.
  send click 300 250
  send type s
  expect "^browser: title $opener .*notes\$" "typing in the page's text field didn't reach it"
  ../tools/window-screenshot.sh "$pid" "$out/before-print.png" || fail "no window to capture"
  page_shortcut p print
  print_preview ⌘P print-key.png xy
fi
# A text field's menu has what Chrome gives it, its Language Settings for spelling and its Writing
# Direction submenu, and Select All selects the field's text, which typing then replaces. The check
# chooses nothing that uses the pasteboard.
if windows || [ "$(uname)" = Linux ]; then
  select_all='Select all' languages='Language settings'
else
  select_all='Select All' languages='Language Settings'
fi
send click 300 250
post rightclick 300 250
expect "^browser: menu shown .*$select_all" \
  "a right-click in the page's text field didn't show its menu"
menu=$(grep '^browser: menu shown' "$log" | tail -1 | cut -d' ' -f4-)
case "$menu" in
*"Paste"*"$select_all"*"$languages"*"Writing Direction › Default"*"Inspect") ;;
*) fail "the text field's menu lacks Paste, $select_all, $languages, Writing Direction or Inspect, in that order: $menu" ;;
esac
../tools/window-screenshot.sh "$pid" "$out/field-page-menu.png" || fail "no window to capture"
send choose "$select_all"
send type ok
expect "^browser: title $opener ok\$" \
  "Select All in the field's menu didn't select its text for typing to replace"
# Switching or closing a full-screen page's tab ends its full screen, and the window's with it.
send click $page_point
send key f
expect '^browser: page entered full screen$' "the page didn't go full screen"
expect '^browser: window entered full screen$' "the window didn't go full screen with the page"
send key down cmd alt
expect '^browser: page left full screen$' "switching tabs didn't take the page out of full screen"
expect '^browser: window left full screen$' "the window stayed full screen as its page's tab left"
check_layout "after switching away from a full-screen page"
send key up cmd alt
expect "^browser: selected $opener in " "⌥⌘↑ didn't show the page again"
send key f
expect '^browser: window entered full screen$' "the page didn't go full screen again"
send key w cmd
expect '^browser: page left full screen$' "closing its tab didn't take the page out of full screen"
# Closing a page goes to the nearest tab it opened (Chrome).
expect "^browser: selected $popup\( in .*\)\{0,1\}\$" \
  "closing the page didn't show the tab it opened first"
expect '^browser: window left full screen$' "the window didn't leave full screen as its page closed"
check_layout "after closing a full-screen page's tab"
exec 3>&-
quit

# After the relaunch, the third link's tab is back with the link's address, and loads once shown.
log="$out/popup-relaunch.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: today [0-9]* [0-9]* [0-9]* [0-9]* [0-9]*$' ||
  fail "the relaunch didn't bring back the popup's, the links', the page's window and the other tab"
waited=$(today | cut -d' ' -f3)
await "^browser: page $waited back 0 forward 0 loading 0 https://example.org/\$" ||
  fail "the link's tab waiting at the quit didn't come back with its address"
grep -q "^browser: loaded $waited " "$log" &&
  fail "the link's tab loaded after the relaunch before it was shown"
# Chromium drops a key for the page before its renderer is up, as the shown tab's is at a launch.
await '^browser: loaded [0-9]* https://example.com/$' ||
  fail "the shown tab didn't load after the relaunch"
send key 3 cmd
expect "^browser: selected $waited\( in .*\)\{0,1\}\$" "⌘3 didn't show the link's tab"
expect "^browser: loaded $waited https://example.org/\$" \
  "the link's tab didn't load once shown after the relaunch"
exec 3>&-
quit

# A sign-in popup (#388): the page opens the identity site, another, whose Continue redirects back to
# the page's site, which posts what it got to the opener and closes the popup itself, as Sign in
# with Google does. Its title says whether it found its opener.
serve
printf '%s' '<title>loading</title><script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let got = "", gone = false;
const say = () => document.title = (got ? "signed in " + got : "waiting") + (gone ? " closed" : "");
onmessage = e => {
  if (e.origin == location.origin) {
    got = e.data;
    say();
  }
};
// A popup needs a user gesture, which a key press is.
onkeydown = e => {
  if (e.key != "o" || e.'"$cmd_key"') {
    return;
  }
  const popup = open("http://localhost:" + location.port + "/consent.html", "",
    "popup,width=500,height=600");
  // Sign-in libraries watch for the popup to close, which ends the sign-in.
  const watch = setInterval(() => {
    if (popup.closed) {
      clearInterval(watch);
      gone = true;
      say();
    }
  }, 100);
};
</script>' > "$out/site/sign-in-page.html"
printf '%s' '<title>loading</title><a id=a style="position:fixed;inset:0;font:24px system-ui">Continue</a>
<script>
a.href = "/bounce?to=" + encodeURIComponent(
  "http://127.0.0.1:" + location.port + "/callback.html?ada");
requestAnimationFrame(() => setTimeout(() => document.title = "consent"));
</script>' > "$out/site/consent.html"
printf '%s' '<title>loading</title><script>
if (!opener) {
  document.title = "no opener";
}
opener.postMessage(location.search.slice(1), location.origin);
close();
</script>' > "$out/site/callback.html"
log="$out/popup-sign-in.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}sign-in-page.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the sign-in page didn't draw"
page=$(grep -m1 '^browser: today [0-9]' "$log" | cut -d' ' -f3)
send click $page_point
send key o
expect "^browser: tab [0-9]* opened by $page in the foreground\$" \
  "the sign-in popup didn't open as a tab"
signing=$(grep "^browser: tab [0-9]* opened by $page" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: title $signing consent\$" "the identity site didn't draw in the popup's tab"
../tools/window-screenshot.sh "$pid" "$out/sign-in.png" || fail "no window to capture"
send click $page_point
served "^GET chromium 127.0.0.1:$port/callback.html?ada " \
  "Continue didn't bring the popup back to the page's site"
expect "^browser: title \($page signed in ada\|$signing no opener\)" \
  "the page didn't get the popup's message"
grep -q "^browser: title $signing no opener\$" "$log" &&
  fail "the popup lost its opener on its way through the identity site"
expect "^browser: tab $signing closed\$" "the popup's window.close() didn't close its tab"
expect "^browser: title $page signed in ada closed\$" "the page didn't see its popup close"
expect "^browser: selected $page\( in .*\)\{0,1\}\$" \
  "closing the popup didn't show the page that opened it"
expect "^browser: today $page\$" "the popup's tab stayed in Today: $(today)"
../tools/window-screenshot.sh "$pid" "$out/signed-in.png" || fail "no window to capture"
exec 3>&-
quit
echo "ok: a page's popup opens as a tab in the window, under the page. The page's context menu leaves out what the window can't do and shows in ilo's menu at the click, a dismiss leaves the page free to show the next, and a text field's has its spelling and writing direction items and a Select All that works. Open Link in New Tab opens tabs under the page, after the ones it opened and above the others, which load only once shown and come back after a quit unloaded, while a window the page makes behind loads at once, and closing one shows the nearest other tab the page opened. Print and ⌘P show print preview centred over the page, at its top, which takes the keys typed and escape closes. Switching or closing a full-screen page's tab ends its full screen and the window's, and closing it shows the tab it opened first. A sign-in popup keeps its opener through the identity site and back, sends the page what it got and closes itself, which the page sees, and the page shows again. Windows: $out/popup.png, $out/link-menu.png, $out/links-behind.png, $out/page-menu.png, $out/print.png, $out/print-key.png, $out/print-key-typed.png, $out/field-page-menu.png, $out/sign-in.png, $out/signed-in.png. The text field's menu: $(grep '^browser: menu shown .*Select All' "$log" | tail -1 | cut -d' ' -f4-)"
