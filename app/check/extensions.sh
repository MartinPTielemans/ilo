# The menu bar's Extensions menu lists the turned-on extensions of the shown Space's profile, by
# name, over Pin to URL Field and Manage Extensions…. Choosing one runs its action on the tab shown,
# as a click on its toolbar button does in Chrome: one without a popup hears the click, and a
# pop-up window it opens is a window of its own, not a tab, and one with a popup opens it with the
# keyboard, which escape closes, giving the browser window the keyboard back. Pin to URL Field's
# items pin an extension's action at the URL field's end, in the order pinned, showing its badge
# for the tab shown as it changes. A click on a pinned action runs
# it, its popup hanging from it, and its right-click menu unpins it, opens its options or its
# details on Chrome's extensions page, or removes it once the user confirms. The actions that don't
# fit are behind an overflow button in the last place, whose menu runs them. Manage Extensions…
# opens Chrome's extensions page. The Site Controls list every turned-on extension, unpinned ones
# too, and a click on one's tile runs it, its popup hanging from the tile. A right-click on a tile
# gives its menu: Move to Top pins it first, Unpin puts it back by name, and Remove asks first. An
# extension's keyboard commands run with the page focused and with the UI, but a key it suggests
# doesn't take one of the browser's. An extension's side panel opens from its action as a card right of the page, drags
# from its left edge to resize, closes from its header's ✕ and ⋯ menu, and goes as its tab does. An
# extension reaches a desktop app's native messaging host that only another Chromium browser's
# folder has. Chrome asks whether to add an extension with the browser's dialog, which lists what
# Chrome warns it can do: escape cancels, and return adds it. Add Extension… opens the Web Store in
# a new tab.

# Four unpacked extensions of the check's own, loaded as a developer loads one. A click on the
# first gives the tab it's on a badge, then opens its page, which opens a pop-up window of its own
# as MetaMask's confirmations do and closes it once it has seen it's a window with one tab, saying
# so in its title, and its command ⇧⌘Y opens another. The
# second's popup gives it a badge on every tab, and ⌥⇧P opens it. The third has a popup and
# options, and suggests ⌘J, Jump to Selection's key, which Chrome gives away, for a command. The
# check removes it, and the next loads it again, as Chrome leaves an unpacked extension's folder.
# A click on the fourth's action opens its side panel, and its command ⌥⇧K turns that off for
# every tab but the one it's on, with a panel of its own, and opens it.
extensions="$PWD/$out/extensions"
script='chrome.action.onClicked.addListener(async tab => {
  await chrome.action.setBadgeText({tabId: tab.id, text: "7"});
  chrome.tabs.create({url: "clicked.html"});
});
chrome.commands.onCommand.addListener(() => chrome.tabs.create({url: "command.html"}));'
# Chrome keeps running the script it first registered for an extension, so a changed script is a
# new extension, in a folder of its own. In a profile the check ran in before, the old one still
# holds ⇧⌘Y and the new one gets no key, so what changes goes in its page's script instead.
clicked_dir="$extensions/clicked-$(printf %s "$script" | cksum | cut -d' ' -f1)"
mkdir -p "$clicked_dir" "$extensions/popup" "$extensions/third" "$extensions/panel"
printf '%s\n' "$script" > "$clicked_dir/background.js"
cat > "$clicked_dir/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Clicked", "version": "1", "action": {},
 "background": {"service_worker": "background.js"},
 "commands": {"hello": {"suggested_key": {"default": "Ctrl+Shift+Y"}, "description": "Hello"}}}
EOF
echo '<title>clicked</title><script src="clicked.js"></script>' > "$clicked_dir/clicked.html"
cat > "$clicked_dir/clicked.js" << 'EOF'
(async () => {
  const made = await chrome.windows.create(
    {type: "popup", url: "made.html", width: 360, height: 240, focused: false});
  const tabs = await chrome.tabs.query({windowId: made.id});
  await chrome.windows.remove(made.id);
  document.title = `clicked ${made.type}-${tabs.length}`;
})();
EOF
echo '<title>made</title>' > "$clicked_dir/made.html"
echo '<title>command</title>' > "$clicked_dir/command.html"
# Off the Mac Chrome keeps Alt+Shift+P (New Tab Group) and Ctrl+J (Downloads) for itself and gives
# no extension a key of its own, so there they suggest others.
cat > "$extensions/popup/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Popup", "version": "1",
 "action": {"default_popup": "popup.html"},
 "commands": {"_execute_action": {"suggested_key": {"default": "Alt+Shift+O", "mac": "Alt+Shift+P"}}}}
EOF
echo '<body style="width:240px;height:120px;display:grid;place-items:center;font:16px system-ui">
the popup<script src="popup.js"></script></body>' > "$extensions/popup/popup.html"
echo 'chrome.action.setBadgeText({text: "1"});' > "$extensions/popup/popup.js"
cat > "$extensions/third/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Third", "version": "1",
 "action": {"default_popup": "popup.html"}, "options_ui": {"page": "options.html"},
 "commands": {"jump": {"suggested_key": {"default": "Alt+Shift+J", "mac": "Ctrl+J"}, "description": "Jump"}}}
EOF
echo '<body style="width:240px;height:120px;display:grid;place-items:center;font:16px system-ui">
the third popup</body>' > "$extensions/third/popup.html"
echo '<body style="font:16px system-ui">the third options</body>' > "$extensions/third/options.html"
cat > "$extensions/panel/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Panel", "version": "1", "action": {},
 "permissions": ["sidePanel"], "side_panel": {"default_path": "panel.html"},
 "background": {"service_worker": "background.js"},
 "commands": {"tab": {"suggested_key": {"default": "Alt+Shift+K"}, "description": "Tab panel"}}}
EOF
# Not awaited, so open() is called while the command's gesture lasts, after the options it needs.
cat > "$extensions/panel/background.js" << 'EOF'
chrome.sidePanel.setPanelBehavior({openPanelOnActionClick: true});
chrome.commands.onCommand.addListener((command, tab) => {
  chrome.sidePanel.setOptions({enabled: false});
  chrome.sidePanel.setOptions({tabId: tab.id, path: "tab.html", enabled: true});
  chrome.sidePanel.open({tabId: tab.id});
});
EOF
echo '<title>the panel</title><body style="font:16px system-ui">the panel</body>' > \
  "$extensions/panel/panel.html"
echo '<title>the tab panel</title><body style="font:16px system-ui">the tab panel</body>' > \
  "$extensions/panel/tab.html"
page='data:text/html,<title>loading</title>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));</script>'
log="$out/extensions.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" \
  --load-extension="$(app_path "$clicked_dir"),$(app_path "$extensions/popup"),$(app_path "$extensions/third"),$(app_path "$extensions/panel")" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# The browser's own ad blocker, uBlock Origin, is in every profile, last by name.
ublock=oghkpboejbohceocapipmnjifoklonij
listing="^browser: extensions menu [a-p]* [a-p]* [a-p]* [a-p]* $ublock\$"
await "$listing" ||
  fail "the Extensions menu didn't list the four extensions and the ad blocker: $(grep '^browser: extensions' "$log")"
# By name.
ids=$(grep -m1 "$listing" "$log" | cut -d' ' -f4-)
clicked=$(echo "$ids" | cut -d' ' -f1)
panel=$(echo "$ids" | cut -d' ' -f2)
popup=$(echo "$ids" | cut -d' ' -f3)
third=$(echo "$ids" | cut -d' ' -f4)
pinned=$(grep '^browser: pinned extensions' "$log" | tail -1)
echo "$pinned" | grep -q '^browser: pinned extensions [0-9]* $' ||
  fail "an extension started pinned: $pinned"
first=$(today | cut -d' ' -f1)
# A just-installed extension's script adds its listener once it has run, and a click before then
# goes unheard, so on a fresh profile it's chosen until it's heard.
heard="^browser: loaded [0-9]* chrome-extension://$clicked/clicked.html\$"
for _ in $(seq 10); do
  send menu "Check Clicked"
  expect "^browser: run extension $clicked ran\$" \
    "Check Clicked didn't run from the Extensions menu"
  for _ in $(seq 20); do
    none_since "$heard" || break 2
    sleep 0.1
  done
done
none_since "$heard" && fail "Check Clicked didn't hear its click: it didn't open its page"
# Its pop-up window is a window of its own, in its profile, as Chrome makes one, and not a tab.
await '^browser: title [0-9]* clicked [a-z]*-[0-9]*$' ||
  fail "Check Clicked's page didn't open its pop-up window"
grep -q '^browser: title [0-9]* clicked popup-1$' "$log" ||
  fail "Check Clicked's pop-up window wasn't a window with its one tab: $(grep 'title [0-9]* clicked ' "$log")"
grep -q "chrome-extension://$clicked/made.html" "$log" &&
  fail "Check Clicked's pop-up window opened as a tab"
# Pinned in the order pinned, on the extension's page's tab, where Check Clicked has no badge.
send menu "Pin to URL Field › Check Popup"
expect "^browser: pin extension $popup\$" "Pin to URL Field › Check Popup didn't reach the UI"
expect "^browser: extension actions $popup\$" "the pinned Check Popup didn't show in the URL field"
send menu "Pin to URL Field › Check Clicked"
expect "^browser: extension actions $popup $clicked\$" \
  "the pinned Check Clicked didn't show after Check Popup"
# Its badge is the first tab's, which shows it once that tab is shown.
index=$(today | tr ' ' '\n' | grep -n "^$first\$" | cut -d: -f1)
send click $(row $((index - 1)))
expect "^browser: selected $first\( in .*\)\{0,1\}\$" \
  "a click on the first tab's row didn't show it"
expect "^browser: extension actions $popup $clicked:7\$" \
  "Check Clicked's badge for the first tab didn't show with it"
../tools/window-screenshot.sh "$pid" "$out/extension-actions.png" || fail "no window to capture"
# The actions sit at the URL field's end, 30 apart: their centres, the 228-wide sidebar's field
# ending at 220, 3 in from its end.
first_place="172 64"
second_place="202 64"
send click $first_place
expect "^browser: run extension $popup ran\$" "a click on Check Popup's action didn't run it"
expect "^browser: extension popup $popup\$" "Check Popup's popup didn't show"
expect "^browser: extension actions $popup:1 $clicked:7\$" \
  "the badge Check Popup's popup gave it didn't show at once"
../tools/window-screenshot.sh "$pid" "$out/extension-popup.png" || fail "no window to capture"
# Closes the popup: escape, where it has the keyboard. On Linux the core gives the keyboard to a
# page alone, not to a popup (#708), so a click on its action again closes it, as in Chrome, or
# on the page, $1, where it has none.
close_popup() {
  if [ "$(uname)" = Linux ] && [ -n "${1:-}" ]; then
    send click $1
    how="a click on the page"
  elif [ "$(uname)" = Linux ]; then
    send click $first_place
    how="a click on its action"
  else
    send key escape
    how=escape
  fi
  expect '^browser: extension popup none$' "$how didn't close the extension's popup"
}
if [ "$(uname)" = Linux ]; then
  echo "skip: the extension's popup taking the keyboard and escape closing it: on Linux the core gives the keyboard to a page alone (#708)."
  close_popup
else
  send state
  [ "$key" != BrowserCoreNSWindow ] || fail "the extension's popup didn't take the keyboard"
  close_popup
  send state
  [ "$key" = BrowserCoreNSWindow ] ||
    fail "the browser window didn't get the keyboard back from the popup: it's in $key"
fi
# The Extensions menu runs a pinned one too, its popup hanging from its action.
send menu "Check Popup"
expect "^browser: extension popup $popup\$" "Check Popup's popup didn't show from the menu"
close_popup
# A right-click on an action shows its menu: Manage Extension opens its details, and Unpin
# unpins it. Check Clicked has no options.
post rightclick $second_place
expect "^browser: menu shown $(cased 'Unpin, Remove, Manage Extension')\$" \
  "a right-click on Check Clicked's action didn't show its menu"
send choose "$(cased 'Manage Extension')"
expect "^browser: loaded [0-9]* chrome://extensions/?id=$clicked\$" \
  "Manage Extension didn't open Check Clicked's details"
# A third doesn't fit the field, so the second place is the overflow button's, whose menu runs the
# actions behind it, their popups hanging from it.
send menu "Pin to URL Field › Check Third"
expect "^browser: extension actions $popup:1 $clicked $third\$" \
  "the pinned Check Third didn't show after Check Clicked"
../tools/window-screenshot.sh "$pid" "$out/extension-overflow.png" || fail "no window to capture"
post click $second_place
expect '^browser: menu shown Check Clicked, Check Third$' \
  "a click on the overflow button didn't show the actions behind it"
send choose Check Third
expect "^browser: run extension $third ran\$" "Check Third didn't run from the overflow button"
expect "^browser: extension popup $third\$" "Check Third's popup didn't show"
../tools/window-screenshot.sh "$pid" "$out/extension-overflow-popup.png" ||
  fail "no window to capture"
# Its action is behind the overflow button, so on Linux a click on the page closes it.
close_popup "$page_point"
# Check Clicked unpinned, Check Third has a place of its own, where a right-click unpins it.
send menu "Pin to URL Field › Check Clicked"
expect "^browser: unpin extension $clicked\$" "Pin to URL Field › Check Clicked didn't unpin it"
expect "^browser: extension actions $popup:1 $third\$" \
  "unpinning Check Clicked didn't give Check Third its place"
post rightclick $second_place
expect "^browser: menu shown $(cased 'Unpin, Options, Remove, Manage Extension')\$" \
  "a right-click on Check Third's action didn't show its menu"
# Chrome shows options that don't ask for a tab in a dialog on its extensions page.
send choose Options
expect "^browser: loaded [0-9]* chrome://extensions/?options=$third\$" \
  "Options didn't open Check Third's options"
post rightclick $second_place
expect "^browser: menu shown $(cased 'Unpin, Options, Remove, Manage Extension')\$" \
  "a right-click on Check Third's action didn't show its menu again"
send choose Unpin
expect "^browser: unpin extension $third\$" "Unpin didn't reach the UI"
expect "^browser: extension actions $popup:1\$" "Unpin didn't take Check Third's action away"
send menu "Pin to URL Field › Check Popup"
expect "^browser: unpin extension $popup\$" "Pin to URL Field › Check Popup didn't unpin it"
expect '^browser: extension actions $' "unpinning the last action left it in the URL field"
# Their keys, as Chrome gave them: those the extensions suggested, as no one set any. Off the Mac
# Alt is ⌃ in the Mac's names (docs/ux-spec.md › Keyboard), so Alt+Shift+O is pressed as ⌃⇧O:
# Windows' driver presses ⌥ as Ctrl+Alt, which no extension's Alt key is.
if [ "$(uname)" = Darwin ]; then
  popup_key="p alt shift" popup_named=alt-shift-p popup_said=⌥⇧P third_named=cmd-j
  panel_key="k alt shift"
else
  popup_key="o ctrl shift" popup_named=ctrl-shift-o popup_said=Alt+Shift+O third_named=ctrl-shift-j
  panel_key="k ctrl shift"
fi
grep -q "^browser: extension keys $clicked hello=shift-cmd-y suggested\$" "$log" &&
  grep -q "^browser: extension keys $popup _execute_action=$popup_named suggested\$" "$log" &&
  grep -q "^browser: extension keys $third jump=$third_named suggested\$" "$log" ||
  fail "the extensions' keys weren't the ones they suggested: $(grep 'extension keys' "$log")"
# Keyboard commands, on the first tab's page.
index=$(today | tr ' ' '\n' | grep -n "^$first\$" | cut -d: -f1)
send click $(row $((index - 1)))
expect "^browser: selected $first\( in .*\)\{0,1\}\$" \
  "a click on the first tab's row didn't show it"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the first tab's page"
send key y cmd shift
expect "^browser: extension command $clicked hello ran via page\$" \
  "⇧⌘Y with the page focused didn't run Check Clicked's command"
expect "^browser: loaded [0-9]* chrome-extension://$clicked/command.html\$" \
  "Check Clicked didn't hear its command from the page"
send click $sidebar_point
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
send key y cmd shift
expect "^browser: extension command $clicked hello ran via ui\$" \
  "⇧⌘Y with the UI focused didn't run Check Clicked's command"
expect "^browser: loaded [0-9]* chrome-extension://$clicked/command.html\$" \
  "Check Clicked didn't hear its command from the UI"
# An action's key runs it as a click on its button does, its popup with the keyboard. The command
# opened its page in a new tab, which can take the keyboard as it commits, so the UI takes it back,
# from the sidebar under its last row: that tab's row moved the others down onto $sidebar_point.
send click $(row "$(today | wc -w)")
in_browser AccessKitSubclassOfGPUIView "after a click on the sidebar"
send key $popup_key
expect "^browser: extension command $popup _execute_action ran via ui\$" \
  "$popup_said with the UI focused didn't run Check Popup's action"
expect "^browser: extension popup $popup\$" "$popup_said with the UI focused didn't show the popup"
close_popup "$page_point"
send click $page_point
in_browser RenderWidgetHostViewCocoa "after a click on the command page"
send key $popup_key
expect "^browser: extension command $popup _execute_action ran via page\$" \
  "$popup_said with the page focused didn't run Check Popup's action"
expect "^browser: extension popup $popup\$" "$popup_said with the page focused didn't show the popup"
../tools/window-screenshot.sh "$pid" "$out/extension-key-popup.png" || fail "no window to capture"
close_popup "$page_point"
send menu "Manage Extensions…"
expect '^browser: command manage-extensions via ui$' "Manage Extensions… didn't reach the UI"
expect '^browser: loaded [0-9]* chrome://extensions/$' \
  "Manage Extensions… didn't open Chrome's extensions page"
../tools/window-screenshot.sh "$pid" "$out/extensions.png" || fail "no window to capture"
# A suggested key that's one of the browser's stays the browser's, with the UI focused and the
# page. The sidebar below the rows, as a click on a row gives the page the keyboard.
send click 100 700
send key j cmd
expect '^browser: command jump-to-selection via ui$' "⌘J with the UI focused didn't reach the UI"
none_since "^browser: extension command $third " ||
  fail "⌘J with the UI focused ran Check Third's command, which suggested it"
send click $page_point
send key j cmd
expect '^browser: command jump-to-selection via page$' \
  "⌘J with the page focused didn't reach the UI"
none_since "^browser: extension command $third " ||
  fail "⌘J with the page focused ran Check Third's command, which suggested it"
# Remove asks first, then takes the extension from the menus and the URL field. Alone, its action
# is at the field's end.
send menu "Pin to URL Field › Check Third"
expect "^browser: extension actions $third\$" "the pinned Check Third didn't show again"
post rightclick $second_place
expect "^browser: menu shown $(cased 'Unpin, Options, Remove, Manage Extension')\$" \
  "a right-click on Check Third's action didn't show its menu to remove it"
send choose Remove
expect '^browser: dialog shown Remove “Check Third”?$' "Remove didn't ask first"
../tools/window-screenshot.sh "$pid" "$out/extension-remove.png" || fail "no window to capture"
send key return
expect "^browser: remove extension $third removed\$" "Remove didn't remove Check Third"
expect "^browser: extensions menu $clicked $panel $popup $ublock\$" \
  "the removed Check Third stayed in the Extensions menu"
expect '^browser: extension actions $' "the removed Check Third's action stayed"
# The Site Controls list every turned-on extension, here the three unpinned ones and the ad
# blocker, by name, and a click on one's tile runs it, its popup hanging from the tile. On the
# first tab. The grid's third tile has its middle at 234 187: the tiles are 56 apart from 122, and
# their first row is 23 under the heading's cap, which is 46 under the action tiles' top at 102.
index=$(today | tr ' ' '\n' | grep -n "^$first\$" | cut -d: -f1)
send click $(row $((index - 1)))
expect "^browser: selected $first\( in .*\)\{0,1\}\$" \
  "a click on the first tab's row didn't show it"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type site controls
expect '^browser: command bar commands "site controls": Site Controls$' \
  "the command bar didn't find Site Controls"
send key return
expect '^browser: command site-controls via command bar$' "Site Controls didn't run from the bar"
expect "^browser: site controls extensions $clicked $panel $popup $ublock\$" \
  "the Site Controls didn't list the unpinned extensions by name: $(grep \
    '^browser: site controls extensions' "$log" | tail -1)"
expect '^browser: site controls settled$' "the Site Controls didn't finish opening"
../tools/window-screenshot.sh "$pid" "$out/site-controls-extensions.png" ||
  fail "no window to capture"
send click 234 187
expect '^browser: site controls run extension$' "a click on Check Popup's tile didn't run it"
expect "^browser: run extension $popup ran\$" "Check Popup didn't run from its tile"
# Its popup hangs from the tile it was clicked on, 48 by 32.
from=$(grep "^browser: run extension $popup from " "$log" | tail -1 | cut -d' ' -f6-)
echo "$from" | awk '{ exit !($3 == 48 && $4 == 32 && $1 <= 234 && 234 <= $1 + $3 &&
  $2 <= 187 && 187 <= $2 + $4) }' ||
  fail "Check Popup's popup didn't hang from its tile, but from $from"
expect "^browser: extension popup $popup\$" "Check Popup's popup didn't show from its tile"
../tools/window-screenshot.sh "$pid" "$out/site-controls-extension-popup.png" ||
  fail "no window to capture"
send key escape
expect '^browser: extension popup none$' "escape didn't close Check Popup's popup"
# A right-click on a tile gives its menu, the pinned action's with Move to Top, logged as the grid
# draws. Move to Top on the unpinned Check Popup pins it first, the panel staying with its tile
# first; on the first tile it's dimmed, and Unpin puts the tile back by name. Remove closes the
# panel for the browser window's dialog, which asks first, and escape keeps the extension.
site_controls() {
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type site controls
  send key return
  expect '^browser: command site-controls via command bar$' "Site Controls didn't run from the bar"
  expect '^browser: site controls settled$' "the Site Controls didn't finish opening"
}
site_controls
post rightclick 234 187
expect "^browser: menu shown $(cased 'Move to Top, Pin, Remove, Manage Extension')\$" \
  "a right-click on Check Popup's tile didn't show its menu"
send choose "$(cased 'Move to Top')"
expect "^browser: site controls extension move-to-top $popup\$" "Move to Top didn't reach the panel"
# With the badge its popup set for every tab, which the extension keeps.
expect "^browser: extension actions $popup:1\$" "Move to Top didn't pin Check Popup"
expect "^browser: site controls drew extensions $popup $clicked $panel $ublock\$" \
  "the grid didn't draw Check Popup first: $(grep '^browser: site controls drew' "$log" | tail -1)"
none_since '^browser: site controls closing' || fail "Move to Top closed the Site Controls"
../tools/window-screenshot.sh "$pid" "$out/site-controls-move-to-top.png" ||
  fail "no window to capture"
post rightclick 122 187
expect "^browser: menu shown $(cased 'Move to Top') (disabled), $(cased 'Unpin, Remove, Manage Extension')\$" \
  "a right-click on the first tile didn't show its menu with Move to Top dimmed"
send choose Unpin
expect "^browser: unpin extension $popup\$" "Unpin on Check Popup's tile didn't unpin it"
expect '^browser: extension actions $' "the unpinned Check Popup's action stayed"
expect "^browser: site controls drew extensions $clicked $panel $popup $ublock\$" \
  "the grid didn't draw Check Popup back by name"
post rightclick 234 187
expect "^browser: menu shown $(cased 'Move to Top, Pin, Remove, Manage Extension')\$" \
  "a right-click on Check Popup's tile didn't show its menu to remove it"
send choose Remove
expect '^browser: site controls run extension remove$' "Remove didn't close the Site Controls"
expect '^browser: dialog shown Remove “Check Popup”?$' "Remove on a tile didn't ask first"
expect '^browser: dialog settled$' "the dialog asking to remove Check Popup didn't settle"
../tools/window-screenshot.sh "$pid" "$out/site-controls-remove.png" || fail "no window to capture"
send key escape
expect '^browser: dialog cancelled$' "escape didn't cancel removing Check Popup"
none_since "^browser: remove extension $popup" || fail "a cancelled Remove removed Check Popup"
# Check Panel's side panel, on the first tab, still shown. Its script sets its action to open the
# panel once it has run, so on a fresh profile it's chosen until the panel opens.
for _ in $(seq 10); do
  send menu "Check Panel"
  for _ in $(seq 20); do
    none_since "^browser: side panel $panel " || break 2
    sleep 0.1
  done
done
none_since "^browser: side panel $panel " && fail "Check Panel's action didn't open its side panel"
expect "^browser: side panel $panel the panel\$" "the side panel's page didn't load"
expect '^browser: side panel at ' "the side panel didn't show right of the page"
# Where the panel's card is: "left top width height".
panel_at() {
  grep '^browser: side panel at ' "$log" | tail -1 | cut -d' ' -f5-
}
read -r px py pw ph <<EOF
$(panel_at)
EOF
../tools/window-screenshot.sh "$pid" "$out/side-panel.png" || fail "no window to capture"
# Its page takes the keyboard on a click, under the 36-tall header.
send click $((px + pw / 2)) $((py + ph / 2))
in_browser RenderWidgetHostViewCocoa "after a click on the side panel's page"
# Its left edge, in the gap before it, drags it 40 narrower, or 40 wider if that's under 320.
to=$((pw - 40))
[ "$to" -ge 320 ] || to=$((pw + 40))
send press $((px - 5)) $((py + ph / 2))
send dragto $((px - 5 + (pw - to) / 2)) $((py + ph / 2))
send dragto $((px - 5 + pw - to)) $((py + ph / 2))
send release $((px - 5 + pw - to)) $((py + ph / 2))
expect "^browser: side panel width $to\$" "a drag on the side panel's edge didn't make it $to wide"
# It took that width as it was dragged, before the release.
for _ in $(seq 50); do
  [ "$(panel_at | cut -d' ' -f2-)" = "$py $to $ph" ] && break
  sleep 0.1
done
[ "$(panel_at | cut -d' ' -f2-)" = "$py $to $ph" ] ||
  fail "the side panel didn't show $to wide: it's at $(panel_at)"
read -r px py pw ph <<EOF
$(panel_at)
EOF
../tools/window-screenshot.sh "$pid" "$out/side-panel-resized.png" || fail "no window to capture"
# The header's ✕ is its last button, 30 square, 3 in from the card's corner.
send click $((px + pw - 18)) $((py + 18))
expect '^browser: side panel none$' "the header's ✕ didn't close the side panel"
# ⌥⇧K, Alt+Shift+K off the Mac, opens the panel for this tab alone, which goes on a switch to a tab
# without one and comes back with its tab.
send key $panel_key
expect "^browser: extension command $panel tab ran via " "⌥⇧K didn't run Check Panel's command"
expect "^browser: side panel $panel the tab panel\$" "⌥⇧K didn't open the tab's side panel"
other=$(today | tr ' ' '\n' | grep -v "^$first\$" | head -1)
index=$(today | tr ' ' '\n' | grep -n "^$other\$" | cut -d: -f1)
send click $(row $((index - 1)))
expect "^browser: selected $other\( in .*\)\{0,1\}\$" "a click on another tab's row didn't show it"
expect '^browser: side panel none$' "the tab's side panel stayed on a tab without one"
index=$(today | tr ' ' '\n' | grep -n "^$first\$" | cut -d: -f1)
send click $(row $((index - 1)))
expect "^browser: selected $first\( in .*\)\{0,1\}\$" \
  "a click on the first tab's row didn't show it"
expect "^browser: side panel $panel the tab panel\$" "the tab's side panel didn't come back with it"
expect "^browser: side panel at [0-9]* $py $to $ph\$" "the side panel didn't keep its width"
# The header's ⋯ is left of its ✕, and its menu closes the panel too.
post click $((px + pw - 51)) $((py + 18))
expect "^browser: menu shown $(cased 'Manage Extension, Close Side Panel')\$" \
  "a click on the side panel's ⋯ didn't show its menu"
send choose "$(cased 'Close Side Panel')"
expect '^browser: side panel none$' "Close Side Panel didn't close the side panel"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited
echo "ok: the Extensions menu lists the profile's extensions by name, and choosing one runs its action on the tab shown: one without a popup hears the click, and opens a pop-up window of its own, not a tab, and one with a popup shows it with the keyboard, which escape closes, giving the browser window the keyboard back. Pin to URL Field pins and unpins them, and the pinned actions show at the URL field's end in the order pinned, with their badges for the tab shown as they change. A click on one runs it, with its popup hanging from it, and its right-click menu unpins it, opens its options and its details, and removes it after asking. The ones that don't fit are behind the overflow button, whose menu runs them. Manage Extensions… opens Chrome's extensions page. The Site Controls list the unpinned extensions by name, and a click on one's tile runs it, its popup hanging from the tile, and its right-click menu moves it to the top, pinning it, unpins it, and asks before removing it. Their keyboard commands and their actions' keys run from the page and the UI, but one they suggest that's the browser's stays the browser's. An extension's side panel opens from its action as a card right of the page with its page loaded, its page takes the keyboard, its edge drags it narrower, its header's ✕ and ⋯ menu close it, and a tab's own panel goes on a switch to a tab without one and comes back with it at the width given. Windows: $out/extension-actions.png, $out/extension-popup.png, $out/extension-overflow.png, $out/extension-overflow-popup.png, $out/extension-key-popup.png, $out/extensions.png, $out/extension-remove.png, $out/site-controls-extensions.png, $out/site-controls-extension-popup.png, $out/site-controls-move-to-top.png, $out/site-controls-remove.png, $out/side-panel.png, $out/side-panel-resized.png"

# A desktop app's native messaging host that only another Chromium browser's folder has, as
# 1Password writes its own only for the browsers it knows. The launch has a profile without one and
# a folder of other browsers of its own, in place of Application Support, with the host in its
# Chrome Beta folder, so no folder of the owner's is read. ($HOME wouldn't do: macOS gives Chromium
# the real home.) A click on the extension's action asks the host, whose answer opens a page.
home="$PWD/$out/native-home"
rm -rf "$home"
# Chrome Beta's folder is google-chrome-beta in ~/.config off the Mac.
beta="Google/Chrome Beta"
[ "$(uname)" = Darwin ] || beta=google-chrome-beta
host_dir="$home/browsers/$beta/NativeMessagingHosts"
mkdir -p "$host_dir" "$home/extension" "$home/profile"
cat > "$home/host.sh" << 'EOF'
#!/bin/sh
# One answer in native messaging's framing: its length, 11, in four bytes, then the JSON.
printf '\013\000\000\000{"ok":true}'
cat > /dev/null
EOF
chmod +x "$home/host.sh"
cat > "$home/extension/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Native", "version": "1", "action": {},
 "permissions": ["nativeMessaging"], "background": {"service_worker": "background.js"}}
EOF
cat > "$home/extension/background.js" << 'EOF'
chrome.action.onClicked.addListener(() =>
  chrome.runtime.sendNativeMessage("dev.browser.check", {}, answer =>
    chrome.tabs.create({url: answer && answer.ok ? "answered.html" :
      "unanswered.html#" + encodeURIComponent(chrome.runtime.lastError?.message ?? "no error")})));
EOF
echo '<title>answered</title>' > "$home/extension/answered.html"
echo '<title>unanswered</title>' > "$home/extension/unanswered.html"
log="$out/native-host.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$home/profile" \
  "$browser" --load-extension="$home/extension" \
  --other-browsers-dir="$home/browsers" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# Check Native, then uBlock Origin, last by name, which no longer reloads itself away (#954).
await "^browser: extensions menu [a-p]* $ublock\$" ||
  fail "the Extensions menu didn't list Check Native"
native=$(grep -m1 "^browser: extensions menu [a-p]* $ublock\$" "$log" | cut -d' ' -f4)
# The host allows only the extension, whose id comes from its folder. Windows runs a host that isn't
# an .exe through cmd, so there it's a .bat that runs the script with Git Bash.
host="$home/host.sh"
if windows; then
  printf '@"%s" "%s"\r\n' "$(cygpath -w "$(command -v bash)")" "$(app_path "$home/host.sh")" \
    > "$home/host.bat"
  host=$(app_path "$home/host.bat")
fi
printf '{"name": "dev.browser.check", "description": "Check", "path": "%s", "type": "stdio",
 "allowed_origins": ["chrome-extension://%s/"]}\n' "$host" "$native" \
  > "$host_dir/dev.browser.check.json"
# Windows finds other browsers' hosts in the registry, not in folders (native_hosts_win.cc): the
# user's key of Edge's names the manifest, until the run ends.
if windows; then
  edge_host='HKCU\Software\Microsoft\Edge\NativeMessagingHosts\dev.browser.check'
  forget_host() { MSYS2_ARG_CONV_EXCL='*' reg delete "$edge_host" /f > /dev/null 2>&1 || true; }
  trap 'cleanup; forget_host' EXIT
  MSYS2_ARG_CONV_EXCL='*' reg add "$edge_host" /ve /t REG_SZ /f \
    /d "$(cygpath -w "$host_dir/dev.browser.check.json")" > /dev/null ||
    fail "couldn't register the host under Edge's key"
fi
# As Check Clicked's, the script's listener comes once it has run.
heard='^browser: native host dev.browser.check from '
for _ in $(seq 10); do
  send menu "Check Native"
  expect "^browser: run extension $native ran\$" "Check Native didn't run from the Extensions menu"
  for _ in $(seq 20); do
    none_since "$heard" || break 2
    sleep 0.1
  done
done
grep -q '^browser: native host lookup dev.browser.check$' "$log" ||
  fail "the browser never looked past its own folders for the host: $(grep -o \
    'chrome-extension://[^ ]*unanswered[^ ]*' "$log" | sort -u | tr '\n' ' ')"
none_since "$heard" && fail "the browser didn't find the host in Chrome Beta's folder"
found=$host_dir
! windows || found=$(cygpath -w "$host_dir")
grep -qxF "browser: native host dev.browser.check from $found" "$log" ||
  fail "the host came from another folder: $(grep "$heard" "$log")"
await "^browser: loaded [0-9]* chrome-extension://$native/answered.html\$" ||
  fail "Check Native didn't hear the host's answer: $(grep -o 'chrome-extension://[^ ]*' "$log" |
    sort -u | tr '\n' ' ')"
../tools/window-screenshot.sh "$pid" "$out/native-host.png" || fail "no window to capture"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited
if windows; then
  forget_host
  trap cleanup EXIT
fi
echo "ok: an extension reaches a native messaging host that only Chrome Beta's folder has, or on Windows only Edge's key, and hears its answer. Window: $out/native-host.png"

# An extension's .crx from the served page, which Chrome installs from off the Web Store with
# ungoogled-chromium's flag, asking first. It's added in a profile of the launch's own, which the
# check starts without, so no other run ever has it. Chrome takes a .crx signed with the key its id
# comes from.
serve
crx="$PWD/$out/install"
mkdir -p "$crx/source" "$crx/profile/Default"
cat > "$crx/source/manifest.json" << 'EOF'
{"manifest_version": 3, "name": "Check Install", "version": "1", "permissions": ["tabs"],
 "host_permissions": ["https://example.org/*"]}
EOF
installed=$(python3 -I -c '
import hashlib, io, os, struct, subprocess, sys, zipfile

source, crx = sys.argv[1], sys.argv[2]
key = os.path.join(os.path.dirname(source), "key.pem")
subprocess.run(["openssl", "genrsa", "-out", key, "2048"], check=True, capture_output=True)
public = subprocess.run(["openssl", "rsa", "-in", key, "-pubout", "-outform", "DER"],
                        check=True, capture_output=True).stdout
archive = io.BytesIO()
with zipfile.ZipFile(archive, "w") as z:
    for name in sorted(os.listdir(source)):
        z.write(os.path.join(source, name), name)
archive = archive.getvalue()

# A length-delimited protobuf field.
def field(number, data):
    out = b""
    for value in (number << 3 | 2, len(data)):
        while value > 0x7f:
            out += bytes([value & 0x7f | 0x80])
            value >>= 7
        out += bytes([value])
    return out + data

# CRX3 (components/crx_file/crx3.proto): the key, and its signature of the id and the zip.
crx_id = hashlib.sha256(public).digest()[:16]
signed = field(1, crx_id)
signature = subprocess.run(
    ["openssl", "dgst", "-sha256", "-sign", key], check=True, capture_output=True,
    input=b"CRX3 SignedData\x00" + struct.pack("<I", len(signed)) + signed + archive).stdout
header = field(2, field(1, public) + field(2, signature)) + field(10000, signed)
with open(crx, "wb") as f:
    f.write(b"Cr24" + struct.pack("<II", 3, len(header)) + header + archive)
print("".join(chr(ord("a") + int(c, 16)) for c in crx_id.hex()))
' "$crx/source" "$out/site/check.crx") || fail "couldn't make the check's .crx"
printf '{"download": {"default_directory": "%s", "prompt_for_download": false}}' \
  "$(native_path "$PWD/$out/downloads" | sed 's/\\/\\\\/g')" > "$crx/profile/Default/Preferences"
log="$out/install.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$site" BROWSER_PROFILE_DIR="$crx/profile" \
  "$browser" --extension-mime-request-handling=always-prompt-for-install \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the install test page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: installing an extension. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# Chrome's words, with its warnings in the log.
send key x
expect '^browser: dialog shown Add "Check Install"?$' "the .crx didn't ask to add Check Install"
grep "^browser: extension question $installed asks " "$log" |
  grep 'Read your browsing history' | grep -q 'Read and change your data on example.org' ||
  fail "the question didn't list Chrome's warnings: $(grep 'extension question' "$log")"
await '^browser: dialog settled$' || fail "the install dialog didn't settle"
../tools/window-screenshot.sh "$pid" "$out/extension-install.png" || fail "no window to capture"
send key escape
expect '^browser: dialog cancelled$' "escape didn't cancel adding Check Install"
expect '^browser: extension question none$' "Chrome didn't hear the install was cancelled"
grep -q "^browser: extensions menu.* $installed" "$log" && fail "the cancelled install added it"
send click $page_point
send key z
expect '^browser: dialog shown Add "Check Install"?$' "the .crx didn't ask again"
# Return adds it once it has been on screen a moment.
expect '^browser: dialog takes input$' "the install dialog never took input"
send key return
expect '^browser: dialog confirmed$' "return didn't confirm adding Check Install"
expect "^browser: extensions menu.* $installed" "Check Install wasn't added"
expect '^browser: extension question none$' "Chrome didn't hear the install was confirmed"
before=$(today)
send menu "Add Extension…"
expect '^browser: command add-extension via ui$' "Add Extension… didn't reach the UI"
expect "^browser: today [0-9]* $before\$" "Add Extension… didn't open a tab"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't quit"
exited
echo "ok: a .crx from a page asks to be added with the browser's dialog, in Chrome's words, listing its warnings. Escape cancels it, return adds it, and Add Extension… opens a tab. Window: $out/extension-install.png"
