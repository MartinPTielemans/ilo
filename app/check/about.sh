# check.sh runs this at 420.
# About ilo shows Settings on its About tab, and the Open of each of its Licence, Notices and
# Libraries rows opens the file the build carries for it (tools/licences.sh) in a tab, which loads.
# Off the Mac, where no menu bar holds ilo's menu, the footer's menu control before the + opens it
# (#3148): escape closes it, and down and return in it run Settings…, which shows Settings.
# Sync & Import, which fits its card, doesn't scroll under a scroll.
# It names the engine's Chromium version wherever the engine carries one: the Mac's framework
# always, and beside chrome Linux's VERSION and Windows's manifest, in engines packaged since. On
# Windows, first, a click in the page once its window is back from minimized gives it the keyboard.

engine_version= engine_version_said= minimized= app_menu_said= build_said=
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
  settings_on_general "⌘, from the page didn't show Settings"
  settings_drew General
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  minimized=" A click on the page once its window was back from minimized gave it the keyboard, and ⌘, from it showed Settings."
fi
if [ "$(uname)" != Darwin ]; then
  # The control's middle: 8 and a pitch of 34 in from the sidebar's 228, 25 above the window's foot.
  check_layout
  control="170 $((${content#*x} - 25))"
  app_menu=$(cased "Settings…, Import from Other Browsers…, Privacy Policy, About $product, Quit $product")
  post click $control
  expect "^browser: menu shown $app_menu\$" "a click on the footer's menu control didn't show ilo's menu: \
$(grep '^browser: menu shown' "$log" | tail -1)"
  expect '^browser: menu drawn ' "the menu control's menu didn't come on screen"
  ../tools/window-screenshot.sh "$pid" "$out/app-menu.png" || fail "no window to capture"
  expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
  send key escape
  expect '^browser: menu closed' "escape didn't close the menu control's menu"
  post click $control
  expect "^browser: menu shown $app_menu\$" "a second click on the menu control didn't show its menu"
  expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
  send key down
  send key return
  expect '^browser: command settings via footer$' "down and return in the menu control's menu didn't run Settings…"
  expect '^browser: settings shown General$' "Settings… from the menu control didn't show Settings on its General tab"
  settings_drew General
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings shown from the menu control"
  app_menu_said=" The footer's menu control showed ilo's menu, which escape closed, and down and return in it showed Settings: $out/app-menu.png."
fi
loaded=
for row in licence:LICENSE notices:NOTICE libraries:THIRD-PARTY.txt; do
  id=${row%%:*} file=${row#*:}
  send menu "About $product"
  # Settings says which tab it shows as it opens, changes tab or draws, which differ by OS.
  expect '^browser: settings \(shown\|tab\|drew\) About$' "About $product didn't show Settings' About tab"
  if [ -z "$loaded" ] && [ -n "$engine_version" ]; then
    await '^browser: settings about Version .*, on Chromium [0-9]*\.[0-9.]*$' ||
      fail "About didn't name the engine's Chromium version"
    said=$(sed -n 's/^browser: settings about .*on Chromium //p' "$log" | tail -1)
    engine_version_said=" on Chromium $said"
  fi
  # The build number About shows beside the version, on the Mac: a prod build's or a release's is
  # its plist's CFBundleVersion, the one Sparkle orders updates by, and a dev build's is none, as
  # build.sh gives only a prod build one, so a new commit doesn't recompile the browser (#3917).
  if [ -z "$loaded" ] && [ "$(uname)" = Darwin ]; then
    shown=$(sed -n 's/^browser: settings about [A-Za-z]* [^ ,]* (\([0-9]*\)),.*/\1/p' "$log" | tail -1)
    numbered=$(plutil -extract CFBundleVersion raw -o - "$app/Contents/Info.plist")
    if [ "$checked_app_id" = dev.browser.app ]; then
      [ -z "$shown" ] || fail "a dev build's About showed build $shown, which only a prod build has"
      build_said=" A dev build's About showed no build number."
    else
      [ "$shown" = "$numbered" ] ||
        fail "About showed build ${shown:-none}, not the $numbered its CFBundleVersion has"
      build_said=" About showed build $shown, its CFBundleVersion."
    fi
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
# A section that fits its card doesn't scroll: a scroll over Sync & Import, signed out, holds
# nothing of its name at the card's top, over its title (#2102).
send menu "About $product"
expect '^browser: settings \(shown\|tab\|drew\) About$' "About $product didn't show Settings again"
# shellcheck disable=SC2046 # x and y
send clickkey $(drawn "toolbar Sync & Import")
expect '^browser: settings tab Sync & Import$' "a click on Sync & Import didn't show its tab"
settings_drew 'Sync & Import'
sight=$(grep '^browser: settings card shows ' "$log" | tail -1 | cut -d' ' -f5-)
at=$(drawn "toolbar Sync & Import")
send scrollkey $((${at% *} + 300)) $(((${sight%% to *} + ${sight##* to }) / 2)) 120 4
# The scroll's frames, drawn by the time the next command's done.
send state
held=$(sed -n "/^browser: drive $((step - 1)): /,\$p" "$log" | grep '^browser: settings held ' | tail -1)
[ -z "$held" ] || [ "$held" = 'browser: settings held none' ] ||
  fail "a scroll over Sync & Import, which fits its card, scrolled it: $held"
../tools/window-screenshot.sh "$(app_pid)" "$out/settings-sync-scrolled.png" ||
  fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
exec 3>&-
quit
[ -n "$engine_version" ] || echo "skip: about's engine version. This engine was packaged before" \
  "it carried one (engine/linux/package.sh)."
echo "ok: About $product showed About$engine_version_said, and its rows' Open loaded$loaded in tabs.$build_said$minimized$app_menu_said Windows: $out/about.png, $out/about-opened.png"
