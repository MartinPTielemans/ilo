# Clicks move keyboard focus between the page and the UI, and typing goes where the focus is. With
# the page focused, the browser's reserved shortcuts reach the UI and never the page, and the others
# reach the page first, which can keep them. ⌘N, from the page and the UI, opens a window whose UI
# has the keyboard, and ⇧⌘W, from either, closes it, giving back the page it took, live, as ⌘W does
# in a window with no page. ⌘Q asks
# first: Cancel keeps the browser, and Quit, and don't ask again quits and turns the warning off.
# The command bar opens over the page with the keyboard, never showing the last query, gives the
# keyboard back when it closes, a held escape that closes it doesn't repeat into the page, and it
# loads what's typed in it, over https for a host typed without one, or searches DuckDuckGo for
# words. A .new address typed, docs.new, gets its row, which opens it in a new tab. A page it opens
# in a new tab has the keyboard: its autofocused field gets what's typed, and ⌘T opens the bar
# again. While ⌘T's is open, "New Tab" has the selected plate. Keys typed right after ⌘T, ⌘L or ⌘F,
# from the page or the UI, wait for the bar it opens, however late the page answers ⌘L, and land in
# its field, a composition too, and a page that keeps ⌘F gets them after it. Japanese input composes
# and commits in the page and in the command bar, where return and escape go to the input method
# while it composes. The Edit menu's Undo, Redo and Select All reach the focused page. ⌘Y opens the
# history page from the page and the UI. A page whose renderer crashes leaves the browser running,
# and ⌘R loads it again. Settings › Shortcuts lists every command the user can give keys, and
# gives New Tab ⌃⌘T, which stays reserved and reaches
# the UI from both sides, while ⌘T no longer runs it, and its ↺ gives ⌘T back. About ilo shows
# Settings' About tab, with Settings key and with About already showing. The Shortcuts tab lists
# the sidebar's commands, and Focus Sidebar given ⌃⌘K there takes the keyboard from the page, as
# its own F6 does once Restore Defaults gives it back. Go to Space 1 given ⌃⌘1 there shows the
# first Space, and ⌃1 no longer does. Include Favorites in ⌘1–⌘8 turned off there has ⌘1 show the
# first tab that isn't a favorite, and Last Tab Goes to the Ninth Tab has ⌘9 count as ⌘1 does,
# until Restore Defaults, after which ⌘9 shows the last tab. A shortcut the page keeps, ⌘F here,
# runs ilo's on a second press at once, and Prefer ilo on it, from a right-click on its keys
# there, runs ilo's first, until Prefer Websites.
# ⌘T as the window comes on screen, before the overlays' windows are made, makes them and opens
# the bar.

# The test page puts what reached it in its title, which the app logs. Chromium drops input
# to a page that hasn't drawn yet, so the page says when it has.
page='data:text/html,<title>loading</title>
<textarea id=text autofocus style="font:24px system-ui;width:95%25;height:90vh"></textarea>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
text.oninput = () => document.title = "value " + text.value;
// Like Google Docs, the page keeps ⌘F. It answers ⌘L late, as a busy page can, past the 0.4 s
// the keys typed after it were once held for. No number sign: it would end this data URL.
addEventListener("keydown", e => {
  if (!e.'"$cmd_key"') return;
  document.title = "page saw cmd-" + e.key;
  if (e.key == "f") e.preventDefault();
  for (const until = performance.now() + (e.key == "l" ? 700 : 0); performance.now() < until;);
}, true);
addEventListener("keydown", e => {
  if (e.key == "Escape") document.title = "page saw escape " + (e.repeat ? "repeat" : "press");
});
</script>'
log="$out/keys.log"
step=0
# The user's keys, which a failed run may have left changed.
rm -f "$saved" "$launch_frame" "$(dirname "$saved")/Shortcuts"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
# At once, as the overlays' windows are made a frame or two after the window comes on screen.
for _ in $(seq 3000); do
  grep -q '^browser: first frame at ' "$log" && break
  kill -0 "$pid" 2>/dev/null || fail "the app exited"
  sleep 0.005
done
send key t cmd
if [ "$active" = 1 ]; then
  expect '^browser: command bar shown$' "⌘T as the window came on screen didn't open the command bar"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the bar ⌘T opened at launch"
  launch_bar="ok: ⌘T as the window comes on screen opens the bar, its window made then."
  grep -q '^browser: overlays opened for a shortcut$' "$log" ||
    launch_bar="skip: ⌘T at launch came after the overlays' windows were made."
else
  launch_bar="skip: ⌘T at launch. Browser wasn't the active app."
fi
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"

send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: keyboard, focus, window states and scrolling. Browser wasn't the active app, so the"
  echo "  page ignored the click. macOS decides which app is active; a background process can't."
  exit 0
fi
[ "$focus" = RenderWidgetHostViewCocoa ] || fail "a click on the page left focus on $focus"
send type abc
expect '^browser: title [0-9]* value abc$' "typing didn't reach the focused page"
# In a text field, ⌘← and ⌘→ move the caret, so they don't come back to go back and forward.
send key left cmd
send key right cmd

for shortcut in "t new-tab" "l edit-url"; do
  page_shortcut $shortcut
  expect '^browser: command bar shown$' "$shortcut didn't open the command bar"
  # ⌘T's bar gives "New Tab" the selected plate, and the tab gets it back when the bar closes.
  if [ "$shortcut" = "t new-tab" ]; then
    expect '^browser: new tab plate 1$' "⌘T didn't give New Tab the selected plate"
    ../tools/window-screenshot.sh "$pid" "$out/new-tab-plate.png" || fail "no window to capture"
  fi
  # A shortcut the page doesn't keep comes back after the page has had it, so after the
  # driver's done.
  send state
  [ "$key" = GPUIPanel ] && [ "$focus" = GPUIView ] ||
    fail "the command bar opened without the keyboard, which is in $key, on $focus"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  if [ "$shortcut" = "t new-tab" ]; then
    expect '^browser: new tab plate 0$' "escape didn't give the tab back its plate"
  fi
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
done
# A held escape that closes the command bar ends there: its repeats don't go on to the page, which
# could take them to leave its full screen, and the next press reaches it.
page_shortcut l edit-url
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send keydown escape
expect '^browser: command bar hidden$' "escape going down didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after a held escape closed the command bar"
held=$step
send keydown escape repeat
send keydown escape repeat
send keyup escape
send key escape
expect '^browser: title [0-9]* page saw escape press$' "escape pressed again didn't reach the page"
sed -n "/^browser: drive $held: /,\$p" "$log" | grep -q '^browser: title [0-9]* page saw escape repeat$' &&
  fail "a held escape's repeats went on to the page after it closed the command bar"
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "⌘F with the page focused didn't reach the page"
# ⌘N from the page opens a window with no page, so its UI has the keyboard, and ⇧⌘W there closes
# it.
page_shortcut n new-window
expect '^browser: window opened [0-9]' "⌘N with the page focused didn't open a window"
send state
in_browser AccessKitSubclassOfGPUIView "in the window ⌘N opened"
send key w cmd shift
expect '^browser: command close-window via ui$' "⇧⌘W with the UI focused didn't reach the UI"
expect '^browser: window closed [0-9]' "⇧⌘W with the UI focused didn't close the window"
send window 1
# ⌘W in a window with no page closes it too, as ⇧⌘W does.
send click $page_point
in_browser RenderWidgetHostViewCocoa "before ⌘N opened a window for ⌘W"
page_shortcut n new-window
expect '^browser: window opened [0-9]' "⌘N with the page focused didn't open a window"
send key w cmd
expect '^browser: command close-tab via ui$' "⌘W with the UI focused didn't reach the UI"
expect '^browser: window closed [0-9]' "⌘W in a window with no page didn't close it"
send window 1

send click $sidebar_point
# GPUI's view in a window it doesn't own, which AccessKit subclasses.
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
send type x
send key f cmd
expect '^browser: command find via ui$' "⌘F with the UI focused didn't reach the UI"
send key t cmd
expect '^browser: command new-tab via ui$' "⌘T with the UI focused didn't reach the UI"
expect '^browser: command bar shown$' "⌘T with the UI focused didn't open the command bar"
send click $page_point
expect '^browser: command bar hidden$' "a click on the page didn't close the command bar"
expect '^browser: new tab plate 0$' "a click on the page didn't give the tab back its plate"
in_browser RenderWidgetHostViewCocoa "after a click on the page closed the command bar"
send type d
# Input reaches the page in order, so by now it has seen the x and the shortcuts, if they got there.
expect '^browser: title [0-9]* value abcd$' "the page got keys meant for the UI: $(grep '^browser: title' "$log" | tail -1)"
# ⌘N from the UI, and ⇧⌘W from the page in the window it opened: the page moves there on a click
# on its row, and back, live, as the window closes.
tab=$(today)
send click $sidebar_point
send key n cmd
expect '^browser: command new-window via ui$' "⌘N with the UI focused didn't reach the UI"
expect '^browser: window opened [0-9]' "⌘N with the UI focused didn't open a window"
first=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
second=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
# The driver counts windows in the order they opened, closed ones too.
send window "$(grep -c '^browser: window opened ' "$log")"
send click $(row 0)
expect "^browser: window $second shows $tab\$" \
  "a click on the page's row didn't move it to the new window"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page in the new window"
send key w cmd shift
expect '^browser: command close-window via page$' "⇧⌘W with the page focused didn't reach the UI"
expect "^browser: window closed $second\$" \
  "⇧⌘W with the page focused didn't close the window"
expect "^browser: window $first shows $tab\$" "the first window didn't get its page back"
[ "$(grep -c "^browser: loaded $tab " "$log")" = 1 ] ||
  fail "the page loaded again as it moved"
send window 1
send click $page_point
in_browser RenderWidgetHostViewCocoa "after the window ⌘N opened closed"
grep -q '^browser: title [0-9]* page saw cmd-[twnW]$' "$log" && fail "the page saw a reserved shortcut"
grep -q '^browser: title [0-9]* page saw cmd-l$' "$log" || fail "⌘L with the page focused didn't reach the page first"
grep -q '^browser: command find via page$' "$log" && fail "the page kept ⌘F, but the UI ran it too"
grep -q '^browser: command \(back\|forward\) via page$' "$log" && fail "⌘← or ⌘→ in the page's text field went back or forward"

# The driver's scripted input method stands in for Japanese input, which is system-wide.
composed=日本語
if [ "$(uname)" = Linux ]; then
  echo "skip: composing in a page: the Linux driver types plain keys there; check/ime.sh covers it with a real input method."
  composed=
elif windows; then
  echo "skip: composing in a page: the Windows driver's input method composes in the UI's fields alone, and nothing covers a page's yet."
  composed=
else
  send ime on
  send type nihongo
  expect '^browser: title [0-9]* value abcdnihongo$' "the page didn't show the composition"
  send key space
  expect '^browser: title [0-9]* value abcd日本語$' "the page didn't show the conversion"
  send key return
  send ime off
fi
send type x
expect "^browser: title [0-9]* value abcd${composed}x\$" "return in the page did more than commit: $(grep '^browser: title' "$log" | tail -1)"
# The Edit menu's Undo, Redo and Select All reach the focused page, which does them itself.
send key z cmd
expect "^browser: title [0-9]* value abcd$composed\$" "⌘Z in the page's text field didn't undo"
send key z cmd shift
expect "^browser: title [0-9]* value abcd${composed}x\$" "⇧⌘Z in the page's text field didn't redo"
send key a cmd
send type y
expect '^browser: title [0-9]* value y$' "⌘A in the page didn't select all"

# Keys typed right after ⌘T, ⌘L or ⌘F land in the field it opens, in order and a composition
# whole, never in the page (Ardia P§7). They go with no wait after the shortcut, and where they
# land, which every caller checks, is the proof. The app holds those that come before the bar has
# the keyboard and says so, but the driver sends them a command after the shortcut, and the bar
# can have the keyboard by then, so there may be none to hold (#905).
typed_after() {
  post key "$1" cmd
  send type "$2"
}
drew() {
  grep "^browser: $1 drew" "$log" | tail -1
}
# The bar asks the core for nothing typed as it opens, and logs the answer's time from then, not
# from an ask before, which here is seconds earlier, as the bar was made at launch (#1072).
send key t cmd
expect '^browser: command bar suggestions "" in ' "the command bar didn't ask for nothing typed"
ms=$(grep '^browser: command bar suggestions "" in ' "$log" | tail -1 |
  sed 's/.* in \([0-9]*\).*/\1/')
[ "$ms" -lt 1000 ] || fail "the command bar's suggestions for nothing typed logged $ms ms"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"
for shortcut in t l; do
  typed_after $shortcut abc
  expect '^browser: command bar drew "abc"$' \
    "keys typed right after ⌘$shortcut with the page focused went elsewhere: $(drew 'command bar')"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
done
if [ "$(uname)" = Linux ]; then
  echo "skip: composing right after ⌘L: the Linux driver types plain keys in a page."
elif windows; then
  echo "skip: composing right after ⌘L: the Windows driver composes in the UI's fields alone, not in a page."
else
  send ime on
  typed_after l nihongo
  expect '^browser: command bar drew "nihongo"$' \
    "a composition typed right after ⌘L didn't go to the command bar: $(drew 'command bar')"
  send state
  [ "$marked" = 0,7 ] || fail "the command bar didn't take all of the composition: marked $marked"
  send key escape
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
  send ime off
  in_browser RenderWidgetHostViewCocoa "after the command bar closed"
fi
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
for shortcut in t l; do
  typed_after $shortcut abc
  expect '^browser: command bar drew "abc"$' \
    "keys typed right after ⌘$shortcut with the UI focused went elsewhere: $(drew 'command bar')"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar"
done
# ⌥⌫ deletes the word before the caret in the bar's field, and ⌘⌫ all before it, as in any Mac
# field. Off the Mac the word's is Ctrl+Backspace, and on Linux all's is Ctrl+Shift+Backspace, as
# in Chrome's fields there (#1076).
word=alt start=cmd
[ "$(uname)" = Darwin ] || word=cmd start="cmd shift"
typed_after t "abc def"
expect '^browser: command bar drew "abc def"$' "the command bar didn't take \"abc def\""
send key backspace $word
expect '^browser: command bar drew "abc "$' \
  "⌥⌫ in the command bar didn't delete the word before the caret: $(drew 'command bar')"
if windows; then
  echo "skip: ⌘⌫ in the command bar: Windows' fields have no key for it."
else
  send key backspace $start
  expect '^browser: command bar drew ""$' \
    "⌘⌫ in the command bar didn't delete what was before the caret: $(drew 'command bar')"
fi
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# Closed first: the find bar opened from the UI before stays open, and ⌘F would make it key at once.
send key f cmd
send key escape
expect '^browser: find bar hidden$' "escape didn't close the find bar"
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
typed_after f abc
expect '^browser: find bar drew "abc" ' \
  "keys typed right after ⌘F with the UI focused went elsewhere: $(drew 'find bar')"
send key escape
expect '^browser: find bar hidden$' "escape didn't close the find bar"
# A page that keeps the shortcut, as this one keeps ⌘F, gets the keys typed after it once it has,
# in order. Input reaches the page in order, so it has seen none of the keys the bars got.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page"
post key f cmd
send type abc
title=$(grep '^browser: title' "$log" | tail -1)
expect '^browser: title [0-9]* value yabc$' \
  "the page that keeps ⌘F didn't get the keys after it, or got a bar's: $title"

# View History (⌘Y) opens Chrome's history page in a tab, from the page, which doesn't keep the
# key, and from the UI. ⌘W closes the tab, which shows the test page again.
before=$(today)
page_shortcut y view-history
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" \
  "⌘Y with the page focused didn't open a tab"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab chrome://history/\$" \
  "⌘Y with the page focused didn't open the history page"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the history tab"
send click $sidebar_point
send key y cmd
expect '^browser: command view-history via ui$' "⌘Y with the UI focused didn't reach the UI"
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" \
  "⌘Y with the UI focused didn't open a tab"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab chrome://history/\$" \
  "⌘Y with the UI focused didn't open the history page"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the history tab"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after the history tab closed"

# Settings › Shortcuts' search, over its boxes, finds a command by the words of its name or by
# its keys (#682). Under it, New Tab comes first, its keys in a chip at the row's right, 56 wide
# and 10 in from the box, and ↺, once it has other keys, 6 before the chip. A click on the chip
# records the next keys pressed.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Shortcuts")
expect '^browser: settings tab Shortcuts$' "a click on Shortcuts didn't show its tab"
settings_drew Shortcuts
send clickkey 300 120
send type new tab
# Paste URL as New Tab has the words too, under it.
expect '^browser: shortcuts search "new tab" finds New Tab, Paste URL as New Tab$' \
  "the search didn't find New Tab by name"
send key a cmd
send type cmd t
expect '^browser: shortcuts search "cmd t" finds New Tab, Restore Last Closed Tab$' \
  "the search didn't find ⌘T and ⇧⌘T by their keys"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-search.png" || fail "no window to capture"
send key a cmd
send type copy
expect '^browser: shortcuts search "copy" finds ' "the search for copy didn't end"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-rows.png" || fail "no window to capture"
# Every command the user can give keys has a row (#1062), these among them.
for name in "Duplicate Current Tab" "Favorite Tab" "Rename Current Tab" Share "Capture Full Page" \
  "Site Controls" "Export Space as Markdown" "View Media" "About $app_name" "Check for Updates" \
  "Import from Other Browsers" "Privacy Policy" Passwords "Site Search Settings" \
  "Welcome to $app_name" "Help Center" "What's New" "Report a Problem" "Copy Build Info"; do
  # The Mac's driver types only lowercase letters, dots and spaces, and the search ignores case.
  query=$(printf '%s' "$name" | tr 'A-Z' 'a-z' | sed "s/[^a-z. ].*//; s/ *\$//")
  send key a cmd
  send type $query
  expect "^browser: shortcuts search \"$query\" finds " "the search for $name didn't end"
  grep "^browser: shortcuts search \"$query\" finds " "$log" | tail -1 |
    grep -q -e "finds $name\$" -e "finds $name, " -e ", $name\$" -e ", $name, " ||
    fail "Settings › Shortcuts has no row $name"
done
send key a cmd
send key backspace
expect '^browser: shortcuts search cleared$' "⌫ didn't clear the search"
send clickkey 581 203
expect '^browser: shortcut recording new-tab$' "a click on New Tab's keys didn't record them"
send key t cmd ctrl
# Off the Mac ⌃ presses Alt, as ⌥ does (docs/ux-spec.md › Keyboard).
new_tab_key=ctrl-cmd-t
[ "$(uname)" = Darwin ] || new_tab_key=alt-cmd-t
expect "^browser: shortcut new-tab $new_tab_key\$" "⌃⌘T didn't become New Tab's key"
../tools/window-screenshot.sh "$pid" "$out/shortcuts.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# New Tab's new key is reserved, as ⌘T was: the page never sees it.
send click $page_point
send key t cmd ctrl
expect '^browser: command new-tab via page$' "⌃⌘T with the page focused didn't reach the UI"
expect '^browser: command bar shown$' "⌃⌘T with the page focused didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"
send type z
expect '^browser: title [0-9]* value yabcz$' "typing didn't reach the page after ⌃⌘T"
grep -q '^browser: title [0-9]* page saw cmd-t$' "$log" &&
  fail "the page saw ⌃⌘T, New Tab's reserved key"
# ⌘T is the page's now, and runs nothing from either side.
send key t cmd
expect '^browser: title [0-9]* page saw cmd-t$' "⌘T with the page focused didn't reach the page"
none_since '^browser: command new-tab' || fail "⌘T with the page focused still ran New Tab"
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
send key t cmd
none_since '^browser: command new-tab' || fail "⌘T with the UI focused still ran New Tab"
send key t cmd ctrl
expect '^browser: command new-tab via ui$' "⌃⌘T with the UI focused didn't reach the UI"
expect '^browser: command bar shown$' "⌃⌘T with the UI focused didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# ↺ gives New Tab ⌘T back, so later runs have the defaults.
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
# Where the app drew it: its place depends on how wide the keys' chip beside it is, ⌃⌘T on the Mac
# and Ctrl+Alt+T elsewhere.
restore=$(drawn "restore new-tab") || fail "Settings didn't draw New Tab's ↺"
send clickkey $restore
expect '^browser: shortcut new-tab cmd-t$' "↺ didn't give New Tab ⌘T back"
# The app menu's About opens Settings on its About tab: the version, the engine's and the credits.
# Settings has the keyboard from the click on ↺, so the menu's command runs inside its window's
# own update, which crashed the app when About read that window there.
[ "$key" = GPUIWindow ] || fail "Settings didn't have the keyboard after a click on ↺: $key does"
send menu About $app_name
expect '^browser: settings tab About$' "About $app_name didn't show Settings' About tab"
expect '^browser: settings drew About$' \
  "Settings didn't draw About after About $app_name with Settings key"
../tools/window-screenshot.sh "$pid" "$out/about.png" || fail "no window to capture"
# Again with About showing: Settings comes forward and draws it again, with no tab to change.
send menu About $app_name
expect '^browser: settings drew About$' \
  "Settings didn't draw About after About $app_name with About already showing"
none_since '^browser: settings tab ' || fail "About $app_name with About showing changed the tab"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
page_shortcut t new-tab
expect '^browser: command bar shown$' "⌘T didn't open the command bar once it was New Tab's again"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
in_browser RenderWidgetHostViewCocoa "after the command bar closed"

# The sidebar's commands are in Settings › Shortcuts, where Focus Sidebar, on F6 at first, Chrome's
# key out of the page (#676), can get another. Given ⌃⌘K, it gives the sidebar the keyboard from
# the page, and Restore Defaults gives F6 back, which does the same.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Shortcuts")
expect '^browser: settings tab Shortcuts$' "a click on Shortcuts didn't show its tab"
settings_drew Shortcuts
send clickkey 300 120
send type sidebar
expect '^browser: shortcuts search "sidebar" finds ' "the search for sidebar didn't end"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-sidebar.png" || fail "no window to capture"
found='Reveal Tab in Sidebar, Focus Sidebar, Show/Hide Sidebar, Restore Earlier Sidebar'
grep -q "^browser: shortcuts search \"sidebar\" finds $found\$" "$log" ||
  fail "the search didn't find the sidebar's commands: $(grep '"sidebar" finds' "$log" | tail -1)"
send key a cmd
send type pinned tabs
expect '^browser: shortcuts search "pinned tabs" finds Collapse Pinned Tabs, Expand Pinned Tabs$' \
  "the search didn't find Collapse and Expand Pinned Tabs"
send key a cmd
send type focus sidebar
expect '^browser: shortcuts search "focus sidebar" finds Focus Sidebar$' \
  "the search didn't find Focus Sidebar"
send clickkey 581 203
expect '^browser: shortcut recording focus-sidebar$' \
  "a click on Focus Sidebar's keys didn't record them"
send key k cmd ctrl
sidebar_key=ctrl-cmd-k
[ "$(uname)" = Darwin ] || sidebar_key=alt-cmd-k
expect "^browser: shortcut focus-sidebar $sidebar_key\$" "⌃⌘K didn't become Focus Sidebar's key"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key k cmd ctrl
expect '^browser: sidebar keyed ' "⌃⌘K with the page focused didn't give the sidebar the keyboard"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't take the sidebar's keyboard away"
in_browser RenderWidgetHostViewCocoa "after escape left the sidebar"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey 300 120
send key a cmd
send type focus sidebar
expect '^browser: shortcuts search "focus sidebar" finds Focus Sidebar$' \
  "the search didn't find Focus Sidebar again"
# Restore Defaults, under the one row found, where ↺'s place follows its chip's width, which is
# the key's name's off the Mac. New Tab has its default already.
send clickkey 126 246
expect '^browser: shortcuts restored$' "a click on Restore Defaults didn't restore the keys"
expect '^browser: shortcut focus-sidebar f6$' \
  "Restore Defaults didn't give Focus Sidebar F6 back"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the page"
send key f6
expect '^browser: sidebar keyed ' "F6 with the page focused didn't give the sidebar the keyboard"
send key escape
expect '^browser: sidebar keys left by escape$' "escape didn't take F6's sidebar keyboard away"
in_browser RenderWidgetHostViewCocoa "after escape left the sidebar F6 gave the keyboard"

# A shortcut the page keeps, as this one keeps ⌘F, runs ilo's command on a second press at once,
# as Arc's does. Prefer ilo, from a right-click on Find's keys in Settings › Shortcuts, has ⌘F
# run Find before the page sees it, and Prefer Websites gives the page it first again (#682).
send click $page_point
send type x
expect '^browser: title [0-9]* value .*x$' "the page didn't get the x typed before ⌘F"
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "the page didn't get ⌘F first"
send key f cmd
expect '^browser: second press cmd-f$' "a second ⌘F at once wasn't taken as a second press"
expect '^browser: command find via page$' "a second ⌘F at once didn't run Find"
expect '^browser: find bar drew ' "a second ⌘F at once didn't open the find bar"
send key escape
expect '^browser: find bar hidden$' "escape didn't close the find bar the second ⌘F opened"
prefer() {
  send menu Settings…
  expect '^browser: settings shown Shortcuts$' \
    "Settings… didn't show the Shortcuts tab it showed last"
  send clickkey 300 120
  send key a cmd
  send type find
  expect '^browser: shortcuts search "find" finds \(.*, \)\{0,1\}Find\(, .*\)\{0,1\}$' \
    "the search didn't find Find"
  at=$(drawn "shortcut find") || fail "Find's keys weren't drawn"
  post rightclickkey $at
  expect "^browser: menu shown $(cased "$1")\$" "a right-click on Find's keys didn't offer $3: \
$(grep '^browser: menu shown' "$log" | tail -1)"
  send choose "$(cased "$3")"
  expect "^browser: shortcut find prefers $2\$" "$3 didn't take for Find"
}
prefer "Prefer Websites (checked), Prefer $app_name" ilo "Prefer $app_name"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-prefer.png" || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send type y
expect '^browser: title [0-9]* value .*y$' "the page didn't get the y typed before ⌘F"
send key f cmd
pressed=$step
expect '^browser: command find via page$' "⌘F with Prefer $app_name didn't run Find at once"
expect '^browser: find bar drew ' "⌘F with Prefer $app_name didn't open the find bar"
saw='^browser: title [0-9]* page saw cmd-f$'
sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q "$saw" &&
  fail "the page saw ⌘F with Prefer $app_name"
send key escape
expect '^browser: find bar hidden$' "escape didn't close the find bar"
prefer "Prefer Websites, Prefer $app_name (checked)" websites "Prefer Websites"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key f cmd
expect '^browser: title [0-9]* page saw cmd-f$' "the page didn't get ⌘F first with Prefer Websites"

send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send ime on
send type nihongo
[ "$marked" = 0,7 ] || fail "the command bar didn't take the composition: marked $marked"
send key escape
none_since '^browser: command bar hidden$' || fail "escape while composing closed the command bar"
[ "$marked" = none ] || fail "escape didn't cancel the composition: marked $marked"
send type nihongo
send key space
[ "$marked" = 0,3 ] || fail "the command bar didn't take the conversion: marked $marked"
../tools/window-screenshot.sh "$pid" "$out/ime.png" || fail "no window to capture"
send key return
none_since '^browser: command bar typed' || fail "return while composing went somewhere"
[ "$marked" = none ] || fail "return didn't commit the composition: marked $marked"
send ime off
send type .invalid
send key return
expect '^browser: command bar typed 日本語.invalid$' "the command bar didn't keep the committed text: $(grep '^browser: command bar typed' "$log" | tail -1)"
# ⌘L goes to the page first, and a key the page being replaced hasn't answered leaves with it.
expect '^browser: loaded [0-9]* https://duckduckgo.com/' "the search for the committed text didn't load"

send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type example.com
../tools/window-screenshot.sh "$pid" "$out/command-bar.png" || fail "no window to capture"
send key return
expect '^browser: command bar go https*://example.com/$' "return in the command bar didn't go to what was typed: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: command bar hidden$' "going somewhere didn't close the command bar"
expect '^browser: loaded [0-9]* https://example.com/$' "the page didn't load what was typed in the command bar"
send state
in_browser RenderWidgetHostViewCocoa "after the command bar loaded a page"
# Words that name no page are a search, with DuckDuckGo until the user picks an engine.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type arc browser
send key return
expect '^browser: command bar go https://duckduckgo.com/?q=arc+browser$' "words in the command bar didn't search: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: loaded [0-9]* https://duckduckgo.com/?q=arc+browser' "the search didn't load"
../tools/window-screenshot.sh "$pid" "$out/search.png" || fail "no window to capture"
# A .new address typed names what it makes, first, and ↩ opens it in a new tab though ⌘L's bar
# edits the tab shown (#698). docs.new sends a browser that isn't signed in to Google's sign-in.
before=$(today)
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type docs.new
expect '^browser: command bar new document docs.new: New Google Doc$' \
  "docs.new typed in the command bar didn't get its row"
../tools/window-screenshot.sh "$pid" "$out/new-document.png" || fail "no window to capture"
send key return
expect '^browser: command bar new tab https://docs.new/$' "the docs.new row didn't open a new tab"
expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the docs.new row's tab isn't in Today"
tab=$(today | cut -d' ' -f1)
expect "^browser: loaded $tab https://[a-z]*\.google\.com/" "docs.new didn't go to Google Docs"
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the docs.new tab"
# A page the bar opens in a new tab has the keyboard, so its field autofocused gets what's typed
# and the browser's shortcuts reach the UI, with no click in it first (#428, where X11 lost them).
serve
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site}autofocus.html"
send key return
expect "^browser: command bar go ${site}autofocus.html\$" \
  "the command bar didn't go to the page typed: $(grep '^browser: command bar go' "$log" | tail -1)"
expect '^browser: command bar hidden$' "going somewhere didn't close the command bar"
expect "^browser: loaded [0-9]* ${site}autofocus.html\$" "the page typed in the bar didn't load"
tab=$(grep "^browser: loaded [0-9]* ${site}autofocus.html\$" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: title $tab ready\$" "the page the bar opened didn't draw"
send type new
expect "^browser: title $tab value new\$" \
  "typing didn't reach the field of the page the bar opened in a new tab"
page_shortcut t new-tab
expect '^browser: command bar shown$' "⌘T in the page the bar opened didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# The bar never shows the last query: empty for a new tab, and on ⌘L with the page's URL. On the
# Mac, whose window keeps what it drew while hidden, the host draws it before it goes on screen.
# X keeps nothing of an unmapped window, which shows nothing until GPUI presents into it once it's
# mapped, so on Linux the first draw after it shows is the one to be right (#1013).
after_shown=
[ "$(uname)" != Linux ] || after_shown=1
awk -v after_shown="$after_shown" '
  function wrong() { return (command == "new-tab") != (drew == "browser: command bar drew \"\"") }
  /^browser: command (new-tab|edit-url) via/ { command = $3 }
  /^browser: command bar drew/ { drew = $0; if (shown && wrong()) exit 1; shown = 0 }
  /^browser: command bar shown$/ { if (after_shown) shown = 1; else if (wrong()) exit 1 }' "$log" ||
  fail "the command bar went on screen still showing what it drew last"
# A page whose renderer goes away leaves the browser running, and ⌘R loads it again. Killed, as
# macOS kills one under memory pressure, it takes Chrome's sad tab path a crash takes; a SIGSEGV
# sent from here only prints a stack, and the command bar completes chrome://crash to another page.
# The core's line comes through Chromium's log, after its prefix.
kill_pages || fail "no renderer to kill"
expect 'browser: tab [0-9]* page gone' "a killed renderer didn't report its page gone"
send click $sidebar_point
send key r cmd
expect '^browser: loaded [0-9]* ' "⌘R didn't load the crashed page again"
# The bundled extension's process lives on, so it stays in the Extensions menu (#1091).
none_since '^browser: extensions menu $' || fail "the pages' renderers took the extension with them"
send click $page_point

# Go to Space 1, given ⌃⌘1 in Settings › Shortcuts, shows the first Space, and ⌃1 no longer does,
# as Arc's Space shortcuts can be rebound (#682). The Mac's driver types no digits, so the search
# is for all nine, Go to Space 1 first.
first=$(space)
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
send type two
send key return
expect "^browser: create space $second two\$" "return didn't name the new Space"
expect '^browser: space form gone$' "the form didn't leave after Create"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey 300 120
send key a cmd
send type go to space
expect '^browser: shortcuts search "go to space" finds Go to Space 1, Go to Space 2, ' \
  "the search didn't find Go to Space 1 first"
send clickkey 581 203
expect '^browser: shortcut recording select-space-1$' \
  "a click on Go to Space 1's keys didn't record them"
send key 1 cmd ctrl
# Off the Mac the driver presses ⌃ as Alt, which a recorded key is named by, but the default the
# keymap gives back is named as the Mac's on each.
space_key=ctrl-cmd-1
first_key=ctrl-1
[ "$(uname)" = Darwin ] || space_key=alt-cmd-1
expect "^browser: shortcut select-space-1 $space_key\$" "⌃⌘1 didn't become Go to Space 1's key"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# Off the Mac, Settings is a window of its own, and the window manager gives the browser's the
# keyboard back a moment after it's gone.
for _ in $(seq 100); do
  [ "$key" = BrowserCoreNSWindow ] && break
  sleep 0.1
  send state
done
in_browser AccessKitSubclassOfGPUIView "in the second Space, which has no page"
send key 1 ctrl
pressed=$step
send state
sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q '^browser: command select-space' &&
  fail "⌃1 still ran Go to Space 1 once it had ⌃⌘1"
send key 1 cmd ctrl
expect '^browser: command select-space via ui$' "⌃⌘1 didn't run Go to Space 1"
expect "^browser: space $first in " "⌃⌘1 didn't show the first Space"

# With Include Favorites in ⌘1–⌘8 off, ⌘1 shows the first tab that isn't a favorite, where with it
# on, the default, it shows the favorite (#682). Favorite Tab in the command bar makes the tab
# shown one, and the Today tab left first is the one ⌘1 goes to then.
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the first Space's page"
favorite=$(grep '^browser: selected' "$log" | tail -1 | cut -d' ' -f3)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type favorite tab
expect '^browser: command bar commands "favorite tab": Favorite Tab' \
  "the command bar didn't find Favorite Tab"
send key return
expect '^browser: command add-to-favorites via command bar$' "Favorite Tab didn't run from the bar"
expect "^browser: favorites $favorite\$" "Favorite Tab didn't make the tab shown a favorite"
expect '^browser: today ' "Favorite Tab didn't take the tab out of Today"
unfavored=$(today | cut -d' ' -f1)
[ -n "$unfavored" ] || fail "no Today tab was left for ⌘1 to show"
page_shortcut 2 select-tab
expect "^browser: selected $unfavored\( in .*\)\{0,1\}\$" \
  "⌘2 didn't show the first Today tab, after the favorite"
send click $page_point
page_shortcut 1 select-tab
expect "^browser: selected $favorite\( in .*\)\{0,1\}\$" "⌘1 didn't show the favorite"
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey 300 120
send key a cmd
send type include favorites
# The row names the keys as its OS does, Ctrl+1–Ctrl+8 off the Mac.
numbered='⌘1–⌘8'
[ "$(uname)" = Darwin ] || numbered='Ctrl+1–Ctrl+8'
expect "^browser: shortcuts search \"include favorites\" finds Include Favorites in $numbered\$" \
  "the search didn't find Include Favorites in $numbered"
at=$(drawn "switch favorites numbered") || fail "Include Favorites has no switch"
send clickkey $at
expect '^browser: favorites numbered off$' "a click on Include Favorites didn't turn it off"
../tools/window-screenshot.sh "$pid" "$out/shortcuts-favorites.png" || fail "no window to capture"
# Last Tab Goes to the Ninth Tab makes ⌘9 the ninth tab, counted as ⌘1–⌘8 count (#682). The
# switch took the keyboard from the search field.
send clickkey 300 120
send key a cmd
send type ninth
expect '^browser: shortcuts search "ninth" finds Last Tab Goes to the Ninth Tab$' \
  "the search didn't find Last Tab Goes to the Ninth Tab"
at=$(drawn "switch last tab ninth") || fail "Last Tab Goes to the Ninth Tab has no switch"
send clickkey $at
expect '^browser: last tab ninth on$' "a click on Last Tab Goes to the Ninth Tab didn't turn it on"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
page_shortcut 1 select-tab
expect "^browser: selected $unfavored\( in .*\)\{0,1\}\$" \
  "⌘1 with the favorites left out didn't show the first tab that isn't one"
# Past the favorite, left out: the ninth of the pinned and Today tabs, or none with fewer.
ninth=$(echo $(pinned) $(today) | tr ' ' '\n' | sed -n 9p)
send click $page_point
page_shortcut 9 select-last-tab
pressed=$step
if [ -n "$ninth" ]; then
  expect "^browser: selected $ninth\( in .*\)\{0,1\}\$" "⌘9 didn't show the ninth tab"
else
  send state
  sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep -q '^browser: selected ' &&
    fail "⌘9 showed a tab with fewer than nine to count"
fi
# Restore Defaults, under the one row found, gives ⌃1 back and counts the favorites again.
send menu Settings…
expect '^browser: settings shown Shortcuts$' \
  "Settings… didn't show the Shortcuts tab it showed last"
settings_drew Shortcuts
send clickkey 126 246
expect '^browser: shortcuts restored$' "a click on Restore Defaults didn't restore the keys"
expect "^browser: shortcut select-space-1 $first_key\$" \
  "Restore Defaults didn't give Go to Space 1 ⌃1"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
# And ⌘1 the favorite again, then ⌘9 the last tab.
page_shortcut 1 select-tab
expect "^browser: selected $favorite\( in .*\)\{0,1\}\$" \
  "⌘1 after Restore Defaults didn't show the favorite"
send click $page_point
last=$(today | tr ' ' '\n' | tail -1)
page_shortcut 9 select-last-tab
expect "^browser: selected $last\( in .*\)\{0,1\}\$" \
  "⌘9 after Restore Defaults didn't show the last tab"
send click $page_point

# ⌘Q asks first, in the dialog over the window. Escape keeps the browser running, and Quit, and
# Don't Ask Again quits. That button is at the card's bottom left, 29 in from its left and bottom
# edges; the card is centred, 450 wide and, with no detail, 172 tall.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
width=${content%x*}
height=${content#*x}
post key q cmd
expect '^browser: command quit via page$' "⌘Q with the page focused didn't reach the UI"
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask before quitting"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle"
../tools/window-screenshot.sh "$pid" "$out/quit.png" || fail "no window to capture"
send key escape
expect '^browser: dialog cancelled$' "escape didn't cancel quitting"
expect '^browser: dialog hidden$' "the dialog asking to quit didn't go"
post key q cmd
expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask again after Cancel"
expect '^browser: dialog settled$' "the dialog asking to quit didn't settle again"
# It's the driver's last command: the app doesn't report it done.
post click $((width / 2 - 225 + 29 + 40)) $((height / 2 - 86 + 1 + 38 + 22 + 46 + 18))
exec 3>&-
await '^browser: warn before quitting off$' ||
  fail "Quit, and don't ask again didn't turn the warning off"
exited
echo "ok: focus follows clicks, typing reaches the focused side, reserved shortcuts skip the page and the others reach it first, ⌘Q asks first in the dialog, and escape keeps the browser and Quit, and don't ask again quits, the command bar takes the keyboard and gives it back, a held escape that closes it doesn't repeat into the page, keys typed right after ⌘T, ⌘L and ⌘F land in the bar they open from both sides, a composition too, and reach a page that keeps ⌘F after it, with New Tab's plate while ⌘T's is open, goes to a host over https and searches for words, names docs.new typed and opens it in a new tab, and a page it opens in a new tab has the keyboard, its autofocused field and the browser's shortcuts too, Japanese input composes in both, and the Edit menu undoes, redoes and selects in the page, ⌘Y opens the history page from both sides, Settings › Shortcuts' search finds New Tab by name and ⌘T's rows by their keys, and the rows of the commands it lacked, and New Tab rebound to ⌃⌘T there stays reserved from both sides while ⌘T runs nothing, until ↺ gives it back, About ilo shows Settings' About tab with Settings key and again with About showing, Shortcuts lists the sidebar's commands and Focus Sidebar given ⌃⌘K there gives the sidebar the keyboard from the page until escape, Go to Space 1 given ⌃⌘1 there shows the first Space while ⌃1 runs nothing, Include Favorites in ⌘1–⌘8 off has ⌘1 show the first tab that isn't a favorite, Last Tab Goes to the Ninth Tab has ⌘9 count the ninth tab and Restore Defaults has it show the last again, a second ⌘F at once runs Find after the page kept the first, Prefer ilo on Find from a right-click on its keys has ⌘F run it before the page sees it and Prefer Websites gives the page it first again, and a killed page's renderer leaves the browser running and ⌘R loads the page again. Windows: $out/quit.png, $out/command-bar.png, $out/search.png, $out/new-document.png, $out/ime.png, $out/shortcuts-search.png, $out/shortcuts-rows.png, $out/shortcuts.png, $out/about.png, $out/shortcuts-sidebar.png, $out/shortcuts-favorites.png, $out/shortcuts-prefer.png"
echo "$launch_bar"
