# check.sh runs this at 420.
# About ilo shows Settings on its About tab, and the Open of each of its Licence, Notices and
# Libraries rows opens the file the build carries for it (tools/licences.sh) in a tab, which loads.
# Sync, which fits its card, doesn't scroll under a scroll.
# It names the engine's Chromium version wherever the engine carries one: the Mac's framework
# always, and beside chrome Linux's VERSION and Windows's manifest, in engines packaged since. On
# Windows, first, a click in the page once its window is back from minimized gives it the keyboard.

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
# A section that fits its card doesn't scroll: a scroll over Sync, signed out, holds nothing of its
# name at the card's top, over its title (#2102).
send menu "About ilo"
expect '^browser: settings \(shown\|tab\|drew\) About$' "About ilo didn't show Settings again"
# shellcheck disable=SC2046 # x and y
send clickkey $(drawn "toolbar Sync")
expect '^browser: settings tab Sync$' "a click on Sync didn't show its tab"
settings_drew Sync
sight=$(grep '^browser: settings card shows ' "$log" | tail -1 | cut -d' ' -f5-)
at=$(drawn "toolbar Sync")
send scrollkey $((${at% *} + 300)) $(((${sight%% to *} + ${sight##* to }) / 2)) 120 4
# The scroll's frames, drawn by the time the next command's done.
send state
held=$(sed -n "/^browser: drive $((step - 1)): /,\$p" "$log" | grep '^browser: settings held ' | tail -1)
[ -z "$held" ] || [ "$held" = 'browser: settings held none' ] ||
  fail "a scroll over Sync, which fits its card, scrolled it: $held"
../tools/window-screenshot.sh "$(app_pid)" "$out/settings-sync-scrolled.png" ||
  fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
exec 3>&-
quit
[ -n "$engine_version" ] || echo "skip: about's engine version. This engine was packaged before" \
  "it carried one (engine/linux/package.sh)."
echo "ok: About ilo showed About$engine_version_said, and its rows' Open loaded$loaded in tabs.$minimized Windows: $out/about.png, $out/about-opened.png"
