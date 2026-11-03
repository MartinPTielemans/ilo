# About ilo shows Settings on its About tab, and the Open of each of its Licence, Notices and
# Libraries rows opens the file the build carries for it (tools/licences.sh) in a tab, which loads.
# It names the engine's Chromium version wherever the engine carries one: the Mac's framework
# always, and beside chrome Linux's VERSION and Windows's manifest, in engines packaged since. On
# Windows, first, a click in the page once its window is back from minimized gives it the keyboard.
# Settings works from the keyboard, opened by ⌘, from the page and from the UI: Tab rings the tab
# shown, → and ← change tab, Tab goes on to Warn before quitting, which space turns off and on
# again, and to the archive's pop-up, which space opens on the time chosen, where ↓ goes on to the
# next and escape closes it, giving Settings back the keyboard, where Tab goes on (#1526).

engine_version= engine_version_said= minimized=
[ "$(uname)" != Darwin ] || engine_version=yes
for file in "$app"/VERSION "$app"/[0-9]*.[0-9]*.manifest; do
  [ ! -e "$file" ] || engine_version=yes
done

log="$out/about.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>about</title>' "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# On Windows a click in the page once its window is back from minimized gives the page the
# keyboard, and ⌘, reaches the UI from it. The driver minimizes the window without its letting go of
# being active, and the page focused meanwhile kept aura's focus without Windows' keyboard, which
# stayed with the window itself, where no shortcut is heard (#1464).
if windows; then
  send click $page_point
  in_browser RenderWidgetHostViewCocoa "a click on the page didn't give it the keyboard"
  send minimize
  send minimize
  send click $page_point
  in_browser RenderWidgetHostViewCocoa \
    "a click on the page once the window was back from minimized didn't give it the keyboard"
  page_shortcut , settings
  expect '^browser: settings shown General$' "⌘, from the page didn't show Settings on its General tab"
  settings_drew General
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  minimized=" A click on the page once its window was back from minimized gave it the keyboard, and ⌘, from it showed Settings."
fi
# Settings from the keyboard, opened from the page.
send click $page_point
in_browser RenderWidgetHostViewCocoa "a click on the page didn't give it the keyboard"
page_shortcut , settings
expect '^browser: settings shown General$' "⌘, from the page didn't show Settings on its General tab"
settings_drew General
send key tab
expect '^browser: settings keyed General$' "Tab in Settings didn't ring its General tab"
send key right
expect '^browser: settings tab Links$' "→ on the General tab didn't show Links"
expect '^browser: settings keyed Links$' "→ on the General tab didn't ring Links"
settings_drew Links
send key left
expect '^browser: settings tab General$' "← on the Links tab didn't show General"
settings_drew General
send key tab
expect '^browser: settings keyed warn before quitting$' \
  "Tab from the tabs didn't ring Warn before quitting"
../tools/window-screenshot.sh "$pid" "$out/settings-keyed.png" all || fail "no window to capture"
send key space
expect '^browser: warn before quitting \(on\|off\)$' "space on Warn before quitting didn't turn it"
turned=$(sed -n 's/^browser: warn before quitting //p' "$log" | tail -1)
send key return
expect '^browser: warn before quitting \(on\|off\)$' "return on Warn before quitting didn't turn it back"
[ "$(sed -n 's/^browser: warn before quitting //p' "$log" | tail -1)" != "$turned" ] ||
  fail "return on Warn before quitting turned it $turned again"
send key tab
expect '^browser: settings keyed ArchiveAfter$' "Tab didn't go on to the archive's pop-up"
send key space
expect '^browser: menu shown 12 hours' "space on the archive's pop-up didn't open its menu"
# It opens on the time chosen, as a click's does, and the keys are its: ↓ goes to the one after.
after=$(sed -n 's/^browser: menu shown //p' "$log" | tail -1 |
  awk -F', ' '{ for (i = 1; i < NF; i++) if ($i ~ / \(checked\)$/) { print $(i + 1); exit } }')
[ -n "$after" ] || fail "the archive's menu had no time checked with one after it"
send key down
expect "^browser: menu highlights $after at " \
  "↓ in the archive's menu opened from the keyboard didn't highlight $after, after the time chosen"
send key escape
expect '^browser: menu closed$' "escape didn't close the archive's menu"
send key tab
expect '^browser: settings keyed unload idle tabs$' \
  "with the menu closed, Tab didn't go on from the archive's pop-up"
send key w cmd
expect '^browser: settings hidden$' "⌘W with a control ringed didn't hide Settings"
# And from the UI, where it opens with nothing ringed.
send click $sidebar_point
send key , cmd
expect '^browser: command settings via ui$' "⌘, from the UI didn't run Settings"
expect '^browser: settings shown General$' "⌘, from the UI didn't show Settings"
settings_drew General
send key tab
expect '^browser: settings keyed General$' "Tab in Settings opened from the UI didn't ring General"
send key tab
expect '^browser: settings keyed warn before quitting$' \
  "Tab in Settings opened from the UI didn't go on to Warn before quitting"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
keyed=" Settings went by the keyboard from the page and the UI: Tab, ← and →, space and return on a switch, and space, ↓ and escape on a pop-up."
loaded=
for row in licence:LICENSE notices:NOTICE libraries:THIRD-PARTY.txt; do
  id=${row%%:*} file=${row#*:}
  send menu "About ilo"
  # Settings says which tab it shows as it opens, changes tab or draws, which differ by OS.
  expect '^browser: settings \(shown\|tab\|drew\) About$' "About ilo didn't show Settings' About tab"
  if [ -z "$loaded" ] && [ -n "$engine_version" ]; then
    await '^browser: settings about Version .*, on Chromium [0-9]*\.[0-9.]*$' ||
      fail "About didn't name the engine's Chromium version"
    said=$(sed -n 's/^browser: settings about .*on Chromium //p' "$log" | tail -1)
    engine_version_said=" on Chromium $said"
  fi
  # About logs where each Open is once, on its first draw.
  await "^browser: settings open $id at " || fail "About didn't draw the $id row's Open"
  [ -n "$loaded" ] || ../tools/window-screenshot.sh "$pid" "$out/about.png" all ||
    fail "no window to capture"
  # shellcheck disable=SC2046 # x and y
  send clickkey $(sed -n "s/^browser: settings open $id at //p" "$log" | tail -1)
  expect '^browser: settings hidden$' "a click on the $id row's Open didn't hide Settings"
  expect "^browser: loaded [0-9]* file://.*/${file%.txt}" "a click on the $id row's Open didn't load $file"
  loaded="$loaded $file"
done
../tools/window-screenshot.sh "$pid" "$out/about-opened.png" all || fail "no window to capture"
exec 3>&-
quit
[ -n "$engine_version" ] || echo "skip: about's engine version. This engine was packaged before" \
  "it carried one (engine/linux/package.sh)."
echo "ok: About ilo showed About$engine_version_said, and its rows' Open loaded$loaded in tabs.$minimized$keyed Windows: $out/settings-keyed.png, $out/about.png, $out/about-opened.png"
