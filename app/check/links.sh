# Links from other apps, as `open` sends them on the Mac, the way a link clicked in Mail does, and
# off it as a second launch with the link does, which `xdg-open` makes on Linux. With the app
# running, one opens in a Little ilo window of its own for the Space the window in front shows, and
# loads there, as Settings › Links sends everything else there until the user picks another, and
# the same link again brings that window back to the front. Sent
# instead to the Space in front, one opens as a new tab in the Space the window in front shows. With
# an agent's Space shown, it goes in the user's Space the window showed before, which comes back;
# with a private window in front, in the regular window, which comes to the front. Settings › Links
# makes rules: a link whose URL holds a rule's text, or is it exactly, opens in the rule's Space, or
# Little ilo, the first rule that matches winning, a drag reorders them, everything else goes to
# the Space picked, and − takes a rule away; the rules are kept for the launches after. Settings › General
# says the browser isn't the default, as a dev build never is: on the Mac it declares no links
# (build.sh), and making it the default asks in a system dialog the driver can't answer; on Linux
# the desktop's defaults name no dev build. A link that launches the browser opens over the Spaces
# it brings back, before the UI comes out from under the launch picture, or with no rules, in Little
# ilo, over them. On the Mac ⌘Q keeps the
# sidebar's picture, which both the launch without a link and the one with it show; Linux has no
# launch picture (host_show_launch_window). Reports the launch times from the process's start,
# without a link and with one.

serve
log="$out/links.log"
step=0
rules_file="$(dirname "$saved")/Link rules"
rm -f "$saved" "$launch_frame" "$rules_file"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
first=$(space)
window=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
# The line the app logs for a link from another app, $1 being what follows the site's address.
link_line() {
  echo "^browser: link from another app ${site}$1: tab [1-9][0-9]* in space $2 of window $3\$"
}
# Sends $1 to the browser as another app does: off the Mac a second launch on the run's profile,
# which hands it to the running browser and exits.
send_link() {
  if [ "$(uname)" = Darwin ]; then
    open -a "$app" "$1"
  else
    "$browser" "$1" >> "$out/links-sent.log" 2>&1 || fail "the second launch with $1 failed"
  fi
}
# The line the app logs for a link from another app in a Little ilo window for Space $2.
little_line() {
  echo "^browser: link from another app ${site}$1: tab [1-9][0-9]* in little window [0-9]* of space $2\$"
}
# Sends https://…/$1 as another app does, and waits for its tab in a Little ilo window for Space $2
# to load it, that window being $little.
open_little_link() {
  send_link "${site}$1"
  await "$(little_line "$1" "$2")" ||
    fail "the link ${site}$1 didn't open in Little ilo for Space $2: $(grep '^browser: link' "$log" | tail -1)"
  tab=$(grep "$(little_line "$1" "$2")" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
  little=$(grep "$(little_line "$1" "$2")" "$log" | tail -1 | sed 's/.* little window \([0-9]*\) .*/\1/')
  await "^browser: loaded $tab ${site}$1\$" || fail "the link ${site}$1 didn't load in tab $tab"
}
# Sends https://…/$1 as another app does, and waits for its tab in Space $2 of window $3 to load it.
open_link() {
  send_link "${site}$1"
  await "$(link_line "$1" "$2" "$3")" ||
    fail "the link ${site}$1 didn't open in Space $2 of window $3: $(grep '^browser: link' "$log" | tail -1)"
  tab=$(grep "$(link_line "$1" "$2" "$3")" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
  await "^browser: loaded $tab ${site}$1\$" || fail "the link ${site}$1 didn't load in tab $tab"
}
# With no rules, everything else goes to Little ilo. The driver numbers windows in the order they
# were made, the closed ones too.
open_little_link "?ext" "$first"
await "^browser: window $little shows $tab\$" || fail "the Little ilo window doesn't show the link"
grep -q "^browser: link from another app .* of window " "$log" &&
  fail "the link from another app opened as a tab too"
send window 2
../tools/window-screenshot.sh "$pid" "$out/links-little.png" all || fail "no window to capture"
# The same link again, with the regular window in front, brings that Little ilo window back.
send window 1
expect "^browser: key window $window\$" "the regular window didn't come to the front"
send_link "${site}?ext"
expect_late "^browser: link from another app ${site}?ext: little window $little again\$" \
  "the same link again didn't bring its Little ilo window back: $(grep '^browser: link' "$log" | tail -1)"
expect "^browser: key window $little\$" "the link's Little ilo window didn't come to the front"
[ "$(grep -c "$(little_line "?ext" "[0-9]*")" "$log")" = 1 ] ||
  fail "the same link again opened a second Little ilo window"
send window 2
send key w cmd
await "^browser: window closed $little\$" || fail "⌘W didn't close the link's Little ilo window"
send window 1
send click $page_point
driven=$active
if [ "$driven" = 1 ]; then
  # Settings says the browser isn't the default and offers to make it so.
  send key , cmd
  expect '^browser: settings shown General$' "⌘, didn't show Settings on its General tab"
  settings_drew General
  expect '^browser: default browser no$' "Settings didn't read whether the browser is the default"
  ../tools/window-screenshot.sh "$pid" "$out/links-settings.png" || fail "no window to capture"
  # Settings › Links, its second tab, says what rules do while it has none, and that everything
  # else goes to Little ilo, the default, which its pop-up, 20 under +, sets to the Space in front.
  # Settings' box of rules starts 108 down, each rule's row 41 below the one before, 129 the
  # first's middle: the pop-up of its condition there at 142, its field at 305, its arrow at 407,
  # its Space's pop-up at 500 and − at 597. + sits 11 under the box, and the pop-up of where
  # everything else goes 20 under +, at 510.
  rules_drawn() {
    echo "^browser: link rules drawn $1; everything else $2\$"
  }
  front=$(cased "The Space in Front")
  send clickkey $(drawn "toolbar Links")
  expect '^browser: settings tab Links$' "a click on Links didn't show its tab"
  settings_drew Links
  expect "$(rules_drawn none "Little ilo")" "the Links tab didn't draw its empty box, everything else to Little ilo"
  ../tools/window-screenshot.sh "$pid" "$out/links-rules-empty.png" || fail "no window to capture"
  post clickkey 510 228
  expect "^browser: menu shown $front, [^,]*, Little ilo (checked)\$" \
    "everything else's pop-up didn't offer the Space in front, the user's Space and Little ilo"
  send choose "$front"
  expect "$(rules_drawn none "$front")" "everything else's pop-up didn't set the Space in front"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  open_link "?ext" "$first" "$window"
  await "^browser: selected $tab\( in .*\)\{0,1\}\$" || fail "the link's tab isn't shown"
  # A second Space of the user's, for the link rules to send links to.
  send menu New Space…
  expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
  second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
  send type work
  send key return
  expect "^browser: create space $second work\$" "return didn't name the second Space"
  send key 1 ctrl
  expect "^browser: space $first\( in .*\)\{0,1\}\$" "⌃1 didn't show the first Space again"
  # With an agent's Space shown, the link goes in the user's, which the window shows again.
  send agentspace claude-code Claude Code
  expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
  agent=$(grep '^browser: made space' "$log" | tail -1 | cut -d' ' -f4)
  send key 3 ctrl
  expect "^browser: space $agent\( in .*\)\{0,1\}\$" "⌃2 didn't show the agent's Space"
  open_link "?agent" "$first" "$window"
  [ "$(space)" = "$first" ] || fail "the window shows Space $(space), not the user's $first"
  # With a private window in front, the link goes to the regular window, which comes to the front.
  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
  private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
  open_link "?private" "$first" "$window"
  await "^browser: key window $window\$" || fail "the regular window didn't come to the front"
  grep -q "^browser: link from another app .* of window $private\$" "$log" &&
    fail "a link from another app opened in the private window"
  # + adds a rule to the first Space, whose field takes what's typed, and its Space's pop-up picks
  # the second.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send clickkey 76 173
  expect '^browser: link rule 0 field has the keyboard$' "+ didn't give the new rule's field the keyboard"
  send type work
  expect "$(rules_drawn 'URL contains "work" → [^,;]*' "$front")" "the rule didn't take the text typed"
  post clickkey 500 129
  expect '^browser: menu shown [^,]* (checked), work, Little ilo$' \
    "the rule's Space pop-up didn't list the user's Spaces and Little ilo"
  send choose work
  expect "$(rules_drawn 'URL contains "work" → work' "$front")" "the rule's pop-up didn't set its Space"
  # A link whose URL holds the text goes to the rule's Space, which the regular window switches to.
  open_link "?work" "$second" "$window"
  expect "^browser: link rule 0 sends ${site}?work to space $second\$" "the rule didn't send the link"
  # A second rule, for a URL exactly, to the first Space, which a URL longer than it doesn't meet.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send clickkey 76 173
  expect '^browser: link rule 1 field has the keyboard$' "+ didn't give the second rule's field the keyboard"
  send insert "${site}?exact"
  post clickkey 142 170
  expect '^browser: menu shown URL contains (checked), URL is equal to$' \
    "the rule's condition pop-up didn't offer contains and is equal to"
  send choose "URL is equal to"
  exact="URL is equal to \"${site}?exact\" → [^,;]*"
  expect "$(rules_drawn "URL contains \"work\" → work, $exact" "$front")" "the second rule didn't draw"
  open_link "?exactly" "$second" "$window"
  none_since "^browser: link rule [0-9]" || fail "a URL longer than the rule's went by the rule"
  open_link "?exact" "$first" "$window"
  expect "^browser: link rule 1 sends ${site}?exact to space $first\$" "the exact rule didn't send its link"
  # Dragged by its arrow onto the first row, the second rule goes first.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send dragkey 407 170 407 129
  expect "$(rules_drawn "$exact, URL contains \"work\" → work" "$front")" "the drag didn't put the second rule first"
  ../tools/window-screenshot.sh "$pid" "$out/links-rules.png" || fail "no window to capture"
  # Everything else, now to the second Space.
  post clickkey 510 269
  expect "^browser: menu shown $front (checked), [^,]*, work, Little ilo\$" \
    "everything else's pop-up didn't offer the Space in front, the user's Spaces and Little ilo"
  send choose work
  expect "$(rules_drawn "$exact, URL contains \"work\" → work" work)" "everything else's pop-up didn't set its Space"
  open_link "?else" "$second" "$window"
  # − takes the first rule away.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send clickkey 597 129
  expect "$(rules_drawn 'URL contains "work" → work' work)" "− didn't take the rule away"
  # A rule to Little ilo opens its links in one, for the Space in front: the second, the last link
  # to the window having shown it.
  post clickkey 500 129
  expect '^browser: menu shown [^,]*, work (checked), Little ilo$' \
    "the rule's Space pop-up didn't offer Little ilo"
  send choose "Little ilo"
  expect "$(rules_drawn 'URL contains "work" → Little ilo' work)" "the rule's pop-up didn't pick Little ilo"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  open_little_link "?work-little" "$second"
  expect "^browser: link rule 0 sends ${site}?work-little to little ilo\$" \
    "the rule didn't send its link to Little ilo"
  # Back to the second Space, for the launch after.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  post clickkey 500 129
  expect '^browser: menu shown [^,]*, work, Little ilo (checked)$' \
    "the rule's Space pop-up didn't check Little ilo"
  send choose work
  expect "$(rules_drawn 'URL contains "work" → work' work)" "the rule's pop-up didn't set the second Space"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
else
  # As the driven run leaves everything else to a Space, so the link that launches the browser
  # below opens as a tab.
  printf 'otherwise\t1\n' > "$rules_file"
fi
# The sidebar's picture, which the launches below show until their UI has drawn.
quit_by_key
mac=
[ "$(uname)" = Darwin ] && mac=1
[ -z "$mac" ] || grep -q '^browser: launch picture taken$' "$log" ||
  fail "⌘Q didn't keep the sidebar's picture for the next launch"
exited
[ -z "$mac" ] || ls "$(dirname "$saved")"/launch-picture-*.png > /dev/null 2>&1 ||
  fail "the sidebar's picture ⌘Q took wasn't on disk once the app quit"

# Launched as the system launches the default browser, by `open` on the Mac, the app logs to $1 and
# is $pid once it runs. Then $2, if given, is the link that launched it.
launch() {
  log=$1
  rm -f "$log"
  if [ -n "$mac" ]; then
    open -a "$app" --env BROWSER_DISPLAY="$BROWSER_DISPLAY" --stdout "$log" --stderr "$log" ${2:+"$2"}
    pid=
    for _ in $(seq 100); do
      pid=$(pgrep -nf "$browser" || true)
      [ -n "$pid" ] && break
      sleep 0.1
    done
  else
    "$browser" ${2:+"$2"} > "$log" 2>&1 &
    pid=$!
  fi
  [ -n "$pid" ] || fail "open didn't launch the app"
  await '^browser: first frame at ' || fail "the window didn't draw"
}
# Quits the app `open` launched, which isn't this shell's child to wait for.
quit_launched() {
  quit_app
  for _ in $(seq 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill -0 "$pid" 2>/dev/null && fail "the app didn't quit"
  for _ in $(seq 10); do
    pgrep -f "$procs" > /dev/null || break
    sleep 0.5
  done
  pgrep -f "$procs" > /dev/null && fail "Chromium helper processes were left running"
  return 0
}
# Milliseconds from the process's start to the line "browser: $1 at <ms> ms".
at() {
  grep -m1 "^browser: $1 at " "$log" | sed 's/.* at \([0-9.]*\) ms$/\1/'
}

launch "$out/links-plain.log"
await '^browser: restored in ' || fail "the Spaces didn't come back"
if [ "$driven" = 1 ]; then
  await '^browser: link rules read 1$' || fail "the link rule didn't come back with the launch"
fi
[ -z "$mac" ] || grep -q '^browser: launch picture shown$' "$log" ||
  fail "the launch without a link didn't show the sidebar's picture"
plain="launch window $(at 'launch window') ms, started $(at 'started') ms, first frame $(at 'first frame') ms, UI shown $(at 'UI shown') ms"
quit_launched

# The link that launches the browser opens over the Spaces it brings back, before the UI comes out
# from under the launch picture, so the restored tab never shows selected first. The UI's first
# frame can come before the link, under the picture.
launch "$out/links-cold.log" "${site}?cold"
await '^browser: restored in ' || fail "the Spaces didn't come back under the link"
[ -z "$mac" ] || grep -q '^browser: launch picture shown$' "$log" ||
  fail "the launch by a link didn't show the sidebar's picture"
await "$(link_line "?cold" "[0-9]*" "[0-9]*")" || fail "the link that launched the browser didn't open"
tab=$(grep "$(link_line "?cold" "[0-9]*" "[0-9]*")" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
await "^browser: loaded $tab ${site}?cold\$" || fail "the link that launched the browser didn't load"
opened=$(grep -n -m1 '^browser: link from another app ' "$log" | cut -d: -f1)
await '^browser: UI shown at ' || fail "the UI didn't come out from under the launch picture"
shown=$(grep -n -m1 '^browser: UI shown at ' "$log" | cut -d: -f1)
[ "$opened" -lt "$shown" ] || fail "the link opened after the UI came out from under the launch picture"
../tools/window-screenshot.sh "$pid" "$out/links-cold.png" || fail "no window to capture"
cold="launch window $(at 'launch window') ms, started $(at 'started') ms, first frame $(at 'first frame') ms, UI shown $(at 'UI shown') ms"
quit_launched
rm -f "$rules_file"

# With no rules, the link that launches the browser opens in Little ilo, for the Space the window
# brings back, over it.
launch "$out/links-little.log" "${site}?little"
await '^browser: restored in ' || fail "the Spaces didn't come back under the link"
await "$(little_line "?little" "[0-9]*")" ||
  fail "the link that launched the browser didn't open in Little ilo: $(grep '^browser: link' "$log" | tail -1)"
tab=$(grep "$(little_line "?little" "[0-9]*")" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
await "^browser: loaded $tab ${site}?little\$" || fail "the link that launched the browser didn't load in Little ilo"
grep -q '^browser: link from another app .* of window ' "$log" &&
  fail "the link that launched the browser opened as a tab too"
../tools/window-screenshot.sh "$pid" "$out/links-little-launch.png" all || fail "no window to capture"
quit_launched

if [ "$driven" = 1 ]; then
  echo "ok: links from another app open in Little ilo by default, the same link again bringing its window back, and sent to the Space in front as new tabs in the front window's Space and load there, in the user's Space when an agent's is shown, and in the regular window, brought to the front, over a private one; Settings says the browser isn't the default. Settings › Links' rules send a link whose URL holds a rule's text, or is it exactly, to the rule's Space or Little ilo, the first that matches winning, a drag reorders them, everything else goes to the Space picked, − takes a rule away, and the rules come back with the launch. Windows: $out/links-little.png, $out/links-settings.png, $out/links-rules-empty.png, $out/links-rules.png"
else
  echo "ok: a link from another app opens in Little ilo by default and loads there."
  echo "skip: the agent's Space, the private window, Settings and the link rules. Browser wasn't the active app, so the page ignored the click."
fi
echo "ok: a link that launches the browser opens over the Spaces it brings back, before the UI comes out from under the launch picture, and loads, and both launches show the sidebar's picture; with no rules, it opens in Little ilo. Windows: $out/links-cold.png $out/links-little-launch.png"
echo "  Launch without a link: $plain; with one: $cold"
