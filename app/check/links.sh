# check.sh runs this at 280.
# Links from other apps, as `open` sends them on the Mac, the way a link clicked in Mail does, and
# off it as a second launch with the link does, which `xdg-open` makes on Linux and the command the
# installer registers for links on Windows. Each opens once: Chrome opens no tab of its own for it
# beside the one the app places (#1413). A launch with no link, from the Start menu, the taskbar,
# a shortcut or the Dock, brings the minimized window back and to the front, as the system says:
# Windows' IsIconic and GetForegroundWindow, the X server's mapped and active windows under Xvfb,
# and AppKit's ordered and key windows on the Mac. With the app
# running, one opens in a Little ilo window of its own for the Space the window in front shows, and
# loads there, as Settings › Links sends everything else there until the user picks another, and
# the same link again brings that window back to the front, and a sign-in's page there closes it
# once done (#2327), leaving nothing to reopen (#2527). On Windows one sent to the Store's MSIX,
# which Windows activates for it, opens the same way, once (#1427). Sent
# instead to the Space in front, one opens as a new tab in the Space the window in front shows. With
# a private window in front, it goes in the regular window, which comes to the front. Settings › Links
# makes rules: a link whose URL holds a rule's text, or is its page, opens in the rule's Space, or
# Little ilo, the first rule that matches winning, a drag reorders them, everything else goes to
# the Space picked, and the trash takes a rule away, which ⌘Z brings back until Settings closes;
# the rules are kept for the launches after. Settings › General says the browser isn't the
# default, as a dev build never is: on the Mac it declares no links
# (build.sh), and making it the default asks in a system dialog the driver can't answer; on Linux
# the desktop's defaults name no dev build. A link that launches the browser opens over the Spaces
# it brings back, before the UI comes out from under the launch picture, or with no rules, in Little
# ilo, over them; on the Mac the notes of an update that launch is the first of open beside it as a
# tab in the window, never in Little ilo. On Windows a click on a site's notification, which comes
# as a launch too, reaches the page (#1426). On the Mac ⌘Q keeps the
# sidebar's picture, which both the launch without a link and the one with it show; Linux has no
# launch picture (host_show_launch_window). Reports the launch times from the process's start,
# without a link and with one.

serve
log="$out/links.log"
step=0
# On Windows another app's link, Teams' or `start https://…`'s, runs the command ilo.iss registers
# for http and https: chrome.exe with these switches, then the link. Chromium reads none as a
# switch, so chrome.exe hands the link straight to the browser running on the engine's own profile
# folder, %LOCALAPPDATA%\ilo\User Data, the prod build's, before the core has added the
# --no-startup-window that keeps Chrome from opening it too (#1413). Where that folder isn't the
# user's ilo's, as on CI, the run takes it, so its links go that way, and gives it back after.
# Elsewhere the run keeps its own and names it in each link's command, which only Chrome's slower
# way takes, as an ilo of the user's could be the one running on the engine's.
fast=
via_msix=
if windows; then
  switches=$(sed -n 's/^Root: HKCU; Subkey: "Software\\Classes\\IloHTML\\shell\\open\\command";.* ValueData: """{app}\\chrome\.exe"" \(.*\) %1"$/\1/p' \
    platform/windows/package/ilo.iss)
  [ -n "$switches" ] || fail "ilo.iss registers no command for links"
  # A launch with no link runs chrome.exe alone, as the Start menu's shortcut does and the command
  # ilo.iss registers to start the browser itself.
  grep -q '^Root: HKCU; Subkey: "Software\\Clients\\StartMenuInternet\\{#AppName}\\shell\\open\\command"; ValueType: string; ValueData: """{app}\\chrome\.exe"""$' \
    platform/windows/package/ilo.iss || fail "ilo.iss no longer starts the browser by chrome.exe alone"
  engines="$(cygpath -u "$LOCALAPPDATA")/ilo"
  if [ -e "$engines/User Data" ]; then
    echo "skip: links by chrome.exe's fast way, as Windows hands them over: the run keeps its" \
      "profile, as $engines/User Data is an ilo's of the user's here."
  else
    fast=1
    made="$engines/User Data"
    [ -e "$engines" ] || made=$engines
    trap 'cleanup; rm -rf "$made" 2> /dev/null || true' EXIT
    check_saved=$saved
    check_launch_frame=$launch_frame
    profile_dir="$engines/User Data"
    # The app keeps its own files, the link rules and the launch frame among them, where this says.
    BROWSER_PROFILE_DIR="$(cygpath -w "$profile_dir")"
    export BROWSER_PROFILE_DIR
    saved="$profile_dir/Spaces.json"
    launch_frame="$profile_dir/launch-frame"
    mkdir -p "$profile_dir"
  fi
fi
rules_file="$(dirname "$saved")/Link rules"
rm -f "$saved" "$launch_frame" "$rules_file"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
first=$(space)
# The app can say which window opened after its page has loaded.
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
window=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
# The line the app logs for a link from another app, $1 being what follows the site's address.
link_line() {
  echo "^browser: link from another app ${site}$1: tab [1-9][0-9]* in space $2 of window $3\$"
}
# Runs the command Windows runs for the link $1 from another app, or with none, chrome.exe alone. It
# names no profile, so it's the engine's, and on any other the run names it, which sends the launch
# Chrome's slower way. `registered exec $1` runs it in place of the shell, so in the background its
# $! is the app's.
registered() {
  run=
  [ "$1" = exec ] && { run=exec; shift; }
  [ $# = 0 ] || set -- $switches "$1"
  if [ -n "$fast" ]; then
    $run "$app/chrome.exe" "$@"
  else
    $run "$app/chrome.exe" "--user-data-dir=$(cygpath -w "$profile_dir")" "$@"
  fi
}
# Sends $1 to the browser as another app does, or with none launches it again as the user does:
# off the Mac a second launch on the run's profile, which hands its command line to the running
# browser and exits, and on the Mac `open`, which with no link reopens the app. On Windows
# chrome.exe's fast way says nothing, where Chrome's slower one prints that it's opening in the
# browser already running.
send_link() {
  sent=$(wc -l < "$log")
  what="the link ${1:-}"
  [ $# = 1 ] || what="the launch with no link"
  if [ "$(uname)" = Darwin ]; then
    open -a "$app" "$@"
  elif [ -n "$via_msix" ]; then
    python3 -I check/msix.py "$CHECK_MSIX" "$1" >> "$out/links-sent.log" 2>&1 ||
      fail "Windows didn't activate the MSIX for $what: $(tail -1 "$out/links-sent.log")"
    # Its chrome.exe hands the link to the browser running on the engine's folder and ends, as the
    # installer's does, rather than starting a browser of its own.
    for _ in $(seq 100); do
      pgrep -f "/WindowsApps/$CHECK_MSIX/" > /dev/null || break
      sleep 0.1
    done
    if pgrep -f "/WindowsApps/$CHECK_MSIX/" > /dev/null; then
      pkill -9 -f "/WindowsApps/$CHECK_MSIX/" || true
      fail "the MSIX's chrome.exe didn't hand $what over and end"
    fi
  elif windows; then
    status=0
    registered "$@" < /dev/null > "$out/links-sent-now.log" 2>&1 || status=$?
    cat "$out/links-sent-now.log" >> "$out/links-sent.log"
    if [ -n "$fast" ]; then
      [ "$status" = 0 ] && [ ! -s "$out/links-sent-now.log" ] ||
        fail "$what didn't go chrome.exe's fast way to the running browser ($status):" \
          "$(tr '\n' ' ' < "$out/links-sent-now.log")"
    fi
  elif [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ] && [ -n "${link_token:-}" ]; then
    XDG_ACTIVATION_TOKEN=$link_token "$browser" "$@" >> "$out/links-sent.log" 2>&1 ||
      fail "the second launch with $what failed"
  elif [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    # Wayland brings a window forward only with an activation token, which a launcher gives the
    # app it starts, and the second launch hands it the running browser. sway's exec gives its
    # command one, which this launch takes. A script, as sway reads a newline or ; in the command
    # it's sent as the start of another.
    activation_token="$PWD/$out/activation-token"
    rm -f "$activation_token"
    printf '%s\n' 'printf %s "$XDG_ACTIVATION_TOKEN" > "$1.part" && mv "$1.part" "$1"' \
      > "$out/activation-token.sh"
    swaymsg exec "sh '$PWD/$out/activation-token.sh' '$activation_token'" > "$out/swaymsg.log" 2>&1 ||
      fail "sway didn't run the command that gets $what an activation token: $(cat "$out/swaymsg.log")"
    for _ in $(seq 100); do
      [ ! -e "$activation_token" ] || break
      sleep 0.05
    done
    [ -s "$activation_token" ] || fail "sway gave $what no activation token"
    XDG_ACTIVATION_TOKEN=$(cat "$activation_token") "$browser" "$@" >> "$out/links-sent.log" 2>&1 ||
      fail "the second launch with $what failed"
  else
    "$browser" "$@" >> "$out/links-sent.log" 2>&1 || fail "the second launch with $what failed"
  fi
}
# How many tabs loaded the link $1 after line $2 of the log. Chrome opened a tab of its own for a
# link from another app on Windows, beside the one the app placed (#1413), in the task that hands
# the app the link, so once the app's tab has loaded, or the driver has done a command since, its
# own would show.
tabs_loading() {
  tail -n "+$(($2 + 1))" "$log" | awk -v url="$1" '
    $1 == "browser:" && $2 == "page" && $(NF - 2) == "loading" && $NF == url && !seen[$3]++ { n++ }
    END { print n + 0 }'
}
# Fails unless the link $1 send_link sent loaded in one tab.
opened_once() {
  tabs=$(tabs_loading "$1" "$sent")
  [ "$tabs" = 1 ] || fail "the link $1 loaded in $tabs tabs, not one: Chrome opened one of its own too"
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
  opened_once "${site}$1"
}
# Sends https://…/$1 as another app does, and waits for its tab in Space $2 of window $3 to load it.
open_link() {
  send_link "${site}$1"
  await "$(link_line "$1" "$2" "$3")" ||
    fail "the link ${site}$1 didn't open in Space $2 of window $3: $(grep '^browser: link' "$log" | tail -1)"
  tab=$(grep "$(link_line "$1" "$2" "$3")" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
  await "^browser: loaded $tab ${site}$1\$" || fail "the link ${site}$1 didn't load in tab $tab"
  opened_once "${site}$1"
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
# Under Wayland the link comes as from another app the user clicked it in, with the token for that
# click (other_app_click), which sway brings an open window forward for.
link_token=
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  other_app_click
  link_token=$other_app_token
fi
send_link "${site}?ext"
expect_late "^browser: link from another app ${site}?ext: little window $little again\$" \
  "the same link again didn't bring its Little ilo window back: $(grep '^browser: link' "$log" | tail -1)"
expect "^browser: key window $little\$" "the link's Little ilo window didn't come to the front"
other_app_closes
link_token=
[ "$(grep -c "$(little_line "?ext" "[0-9]*")" "$log")" = 1 ] ||
  fail "the same link again opened a second Little ilo window"
send window 2
[ "$(tabs_loading "${site}?ext" "$sent")" = 0 ] ||
  fail "the same link again loaded in a tab of Chrome's own beside its Little ilo window"
send key w cmd
await "^browser: window closed $little\$" || fail "⌘W didn't close the link's Little ilo window"
# A sign-in another app's link opened in Little ilo closes its window by itself once it's done
# (#2327), on a signal that isn't the page's words: a load onto a loopback address carrying a code,
# from another site. The provider here is localhost, another site than the callback's 127.0.0.1, as
# a CLI's or an MCP client's login ends at its own server. The bar says it's closing and the window
# closes 2 s on. Its address carries a single-use code, so the browser keeps nothing of it (#2527):
# ⇧⌘T brings back the Little ilo window closed before it instead. One carrying an error stays, and
# the same page in a tab the user opened stays.
printf '%s' '<title>sign-in</title><script>
addEventListener("load", () => setTimeout(() => location = "/bounce?to=" +
  encodeURIComponent("http://127.0.0.1:" + location.port + "/signed-in.html" + location.search), 100));
</script>' > "$out/site/signin.html"
printf '%s' '<title>signed in</title>' > "$out/site/signed-in.html"
callback="http://127.0.0.1:$port/signed-in.html"
provider="http://localhost:$port"
# Sends the link $1 as another app does, and waits for its tab, $tab, in Little ilo window $little.
sign_in_link() {
  sign_in_line="^browser: link from another app $1: tab [1-9][0-9]* in little window [0-9]* of space $first\$"
  send_link "$1"
  await "$sign_in_line" ||
    fail "the link $1 didn't open in Little ilo: $(grep '^browser: link' "$log" | tail -1)"
  tab=$(grep "$sign_in_line" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
  little=$(grep "$sign_in_line" "$log" | tail -1 | sed 's/.* little window \([0-9]*\) .*/\1/')
}
sign_in_link "$provider/signin.html?code=abc"
await "^browser: loaded $tab $callback?code=abc\$" || fail "the sign-in didn't come back to its callback"
await "^browser: little window $little sign-in done at $callback?code=abc\$" ||
  fail "the callback with a code didn't end the sign-in"
grep -q "^browser: tab $tab page forgotten\$" "$log" || fail "the sign-in's callback page wasn't forgotten"
await "^browser: little bar closing true\$" || fail "the sign-in's Little ilo bar didn't say it was closing"
# The bar's words, for the PR: the window has the 2 s of its pause to be caught in.
../tools/window-screenshot.sh "$pid" "$out/links-sign-in-closing.png" all ||
  echo "note: the sign-in's Little ilo window closed before its capture"
await "^browser: little window $little closed after its sign-in\$" ||
  fail "the sign-in's Little ilo window didn't close after its pause"
await "^browser: window closed $little\$" || fail "the sign-in's Little ilo window stayed open"
send window 1
send key t cmd shift
expect "^browser: little window [0-9]* reopened [0-9]* as [0-9]*\$" \
  "⇧⌘T didn't bring back the Little ilo window closed before the sign-in's: $(grep '^browser: little window' "$log" | tail -1)"
grep '^browser: little window [0-9]* reopened ' "$log" | tail -1 | grep -q " reopened $tab as " &&
  fail "⇧⌘T brought back the sign-in's callback page, whose code the browser keeps nowhere"
send window "$(grep -c '^browser: window opened ' "$log")"
send key w cmd
expect "^browser: window closed [0-9]*\$" "⌘W didn't close the reopened Little ilo window"
sign_in_link "$provider/signin.html?error=denied"
await "^browser: loaded $tab $callback?error=denied\$" || fail "the failed sign-in didn't come back to its callback"
# The user's ⌘W closes it, and only then: the load decided it, before the key was sent.
send window "$(grep -c '^browser: window opened ' "$log")"
send key w cmd
expect "^browser: window closed $little\$" "⌘W didn't close the failed sign-in's Little ilo window"
grep -q "^browser: little window $little \(sign-in done\|closed\)" "$log" &&
  fail "a failed sign-in's Little ilo window closed by itself: $(grep "^browser: little window $little " "$log" | tail -1)"
# In a tab the user opened, it doesn't count.
send window 1
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar for the user's sign-in"
send type "$provider/signin.html?code=user"
send key return
await "^browser: loaded [0-9]* $callback?code=user\$" || fail "the user's sign-in page didn't reach its callback"
user_sign_in=$(sed -n "s|^browser: loaded \([0-9]*\) $callback?code=user\$|\1|p" "$log" | tail -1)
send key w cmd
expect "^browser: tab $user_sign_in closed\$" "⌘W didn't close the user's sign-in tab"
grep -q "sign-in done at $callback?code=user\$" "$log" && fail "a sign-in in a tab the user opened counted as done"
# The Store's MSIX declares http and https in its manifest, not by ilo.iss's command, so Windows
# activates the package for a link sent to it, and the package's chrome.exe, which names no
# profile, hands the link to the browser running on the engine's folder (#1427). check-windows.yml
# installs an MSIX of this build, signed for its runner alone, and names it in CHECK_MSIX.
msix=
if windows; then
  if [ -z "${CHECK_MSIX:-}" ]; then
    echo "skip: a link sent to the Store's MSIX. None of this build is installed here; CHECK_MSIX names one."
  elif [ -z "$fast" ]; then
    echo "skip: a link sent to the Store's MSIX, whose chrome.exe hands it only to a browser on the" \
      "engine's folder, which the run keeps for an ilo of the user's."
  else
    via_msix=1
    open_little_link "?msix" "$first"
    via_msix=
    msix=1
    # The window the MSIX's link opened, the last the app made: the driver counts them in the order
    # they opened, closed ones too, and the sign-ins' Little ilo windows came before it (#2603).
    send window "$(grep -c '^browser: window opened ' "$log")"
    send key w cmd
    await "^browser: window closed $little\$" || fail "⌘W didn't close the MSIX's link's Little ilo window"
  fi
fi
send window 1
send click $page_point
driven=$active
again=
if [ "$driven" = 1 ]; then
  # A launch with no link while the window is minimized, from the Start menu, the taskbar or a
  # shortcut, brings it back and to the front, which the core asks of the app as it keeps Chrome
  # from opening a window of its own, and on the Mac a Dock click or `open`, which Chrome answers.
  if windows; then
    # The browser window's states, as Windows has them: minimized is IsIconic, and foreground is
    # GetForegroundWindow's.
    window_states() {
      echo " $(python3 -I check/app-windows.py list "$(cat "/proc/$pid/winpid")" |
        sed -n 's/.*" browser visible\(.*\)$/\1/p' | tr '\n' ' ') "
    }
    send minimize
    case $(window_states) in
    *" minimized "*) ;;
    *) fail "the driver didn't minimize the window:$(window_states)" ;;
    esac
    send_link
    for _ in $(seq 50); do
      states=$(window_states)
      case $states in *" minimized "*) ;; *" foreground "*) break ;; esac
      sleep 0.1
    done
    case $states in
    *" minimized "*) fail "the launch with no link left the window minimized:$states" ;;
    *" foreground "*) again=1 ;;
    *) fail "the launch with no link didn't bring the window to the front:$states" ;;
    esac
  elif [ "$(uname)" = Linux ] && [ "${BROWSER_HEADLESS:-xvfb}" = xvfb ]; then
    # The X server's: the window mapped, as openbox unmaps a minimized one, and the active one,
    # which openbox names.
    xid=$(xdotool getactivewindow) || fail "openbox names no active window"
    mapped() {
      xdotool search --onlyvisible --pid "$pid" 2> /dev/null | grep -qx "$xid"
    }
    timeout 5 xdotool windowminimize --sync "$xid" || fail "openbox didn't minimize the window"
    ! mapped || fail "the minimized window is still mapped"
    send_link
    for _ in $(seq 50); do
      mapped && [ "$(xdotool getactivewindow 2> /dev/null)" = "$xid" ] && break
      sleep 0.1
    done
    mapped || fail "the launch with no link left the window minimized"
    [ "$(xdotool getactivewindow 2> /dev/null)" = "$xid" ] ||
      fail "the launch with no link didn't bring the window to the front"
    again=1
  elif [ "$(uname)" = Darwin ]; then
    # AppKit's: whether the window is on screen, as the driver does nothing while it's out of sight
    # but bring it back, saying it can't to the rest, and then the browser windows front to back
    # with the key one.
    out_of_sight() {
      post order
      await "^browser: drive $step done" || fail "the driver didn't finish \"order\""
      grep -q "^browser: drive $step: can't do that" "$log"
    }
    last_order() {
      grep '^browser: order' "$log" | tail -1
    }
    send minimize
    for _ in $(seq 50); do
      out_of_sight && break
      sleep 0.1
    done
    out_of_sight || fail "the driver didn't minimize the window: $(last_order)"
    send_link
    for _ in $(seq 50); do
      out_of_sight || break
      sleep 0.1
    done
    ! out_of_sight || fail "the launch with no link left the window minimized"
    for _ in $(seq 50); do
      send order
      case $(last_order) in "browser: order window 1"*", key window 1") break ;; esac
      sleep 0.1
    done
    case $(last_order) in
    "browser: order window 1"*", key window 1") again=1 ;;
    *) fail "the launch with no link didn't bring the window to the front: $(last_order)" ;;
    esac
  else
    echo "skip: the launch with no link while the window is minimized. It reads the window's" \
      "state from Windows, Xvfb's X server or AppKit."
  fi
  send click $page_point
  # Settings says the browser isn't the default and offers to make it so.
  send key , cmd
  expect '^browser: settings shown General$' "⌘, didn't show Settings on its General tab"
  settings_drew General
  expect '^browser: default browser no$' "Settings didn't read whether the browser is the default"
  ../tools/window-screenshot.sh "$pid" "$out/links-settings.png" || fail "no window to capture"
  # Settings › Links, its second tab, starts with the Meet rule, as Arc seeds its Google Meet route
  # (#692): a link holding meet.google.com opens as a tab in the Space in front, not in Little ilo.
  # Once the trash takes it, the box says what rules do while it has none, and that everything
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
  meet="URL contains \"meet.google.com\" → $front"
  expect "$(rules_drawn "$meet" "Little ilo")" \
    "the Links tab didn't start with the Meet rule, everything else to Little ilo"
  ../tools/window-screenshot.sh "$pid" "$out/links-rules-meet.png" || fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  open_link "?meet.google.com" "$first" "$window"
  expect "^browser: link rule 0 sends ${site}?meet.google.com to the space in front\$" \
    "the Meet rule didn't send its link to the Space in front"
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  click_in_sight "link remove 0"
  expect "$(rules_drawn none "Little ilo")" "the trash didn't take the Meet rule, leaving the empty box"
  ../tools/window-screenshot.sh "$pid" "$out/links-rules-empty.png" || fail "no window to capture"
  post clickkey $(drawn "link otherwise")
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
  # With a private window in front, the link goes to the regular window, which comes to the front.
  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
  private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
  new_window_bar
  open_link "?private" "$first" "$window"
  await "^browser: key window $window\$" || fail "the regular window didn't come to the front"
  grep -q "^browser: link from another app .* of window $private\$" "$log" &&
    fail "a link from another app opened in the private window"
  # + adds a rule to the Space in front, as Arc's New Route goes to Most Recent Space, whose field
  # takes what's typed, and its Space's pop-up picks the second.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  click_in_sight "link add"
  expect '^browser: link rule 0 field has the keyboard$' "+ didn't give the new rule's field the keyboard"
  send type work
  expect "$(rules_drawn 'URL contains "work" → [^,;]*' "$front")" "the rule didn't take the text typed"
  post clickkey $(drawn "link space 0")
  expect "^browser: menu shown $front (checked), [^,]*, work, Little ilo\$" \
    "the rule's Space pop-up didn't list the Space in front, the user's Spaces and Little ilo"
  send choose work
  expect "$(rules_drawn 'URL contains "work" → work' "$front")" "the rule's pop-up didn't set its Space"
  # A link whose URL holds the text goes to the rule's Space, which the regular window switches to.
  open_link "?work" "$second" "$window"
  expect "^browser: link rule 0 sends ${site}?work to space $second\$" "the rule didn't send the link"
  # A second rule, for a page, to the Space in front, here the second. "Is equal to" compares host
  # and path alone, as Arc's (#692): the page with a query meets it, and the site's home with the
  # page in its query doesn't.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  click_in_sight "link add"
  expect '^browser: link rule 1 field has the keyboard$' "+ didn't give the second rule's field the keyboard"
  send insert "${site}index.html"
  post clickkey $(drawn "link condition 1")
  expect '^browser: menu shown URL contains (checked), URL is equal to$' \
    "the rule's condition pop-up didn't offer contains and is equal to"
  send choose "URL is equal to"
  exact="URL is equal to \"${site}index.html\" → [^,;]*"
  expect "$(rules_drawn "URL contains \"work\" → work, $exact" "$front")" "the second rule didn't draw"
  open_link "?index.html" "$second" "$window"
  none_since "^browser: link rule [0-9]" || fail "a link to another path went by the is-equal-to rule"
  open_link "index.html?exact" "$second" "$window"
  expect "^browser: link rule 1 sends ${site}index.html?exact to the space in front\$" \
    "the is-equal-to rule didn't send its page with a query"
  # Dragged by its arrow onto the first row, the second rule goes first.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send dragkey $(drawn "link arrow 1") $(drawn "link arrow 0")
  expect "$(rules_drawn "$exact, URL contains \"work\" → work" "$front")" "the drag didn't put the second rule first"
  ../tools/window-screenshot.sh "$pid" "$out/links-rules.png" || fail "no window to capture"
  # Everything else, now to the second Space.
  post clickkey $(drawn "link otherwise")
  expect "^browser: menu shown $front (checked), [^,]*, work, Little ilo\$" \
    "everything else's pop-up didn't offer the Space in front, the user's Spaces and Little ilo"
  send choose work
  expect "$(rules_drawn "$exact, URL contains \"work\" → work" work)" "everything else's pop-up didn't set its Space"
  open_link "?else" "$second" "$window"
  # The trash takes the first rule away, ⌘Z brings it back to its place, and the trash again.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  click_in_sight "link remove 0"
  expect "$(rules_drawn 'URL contains "work" → work' work)" "the trash didn't take the rule away"
  send key z cmd
  expect "$(rules_drawn "$exact, URL contains \"work\" → work" work)" \
    "⌘Z didn't bring the rule the trash took back to its place"
  ../tools/window-screenshot.sh "$pid" "$out/links-rule-back.png" || fail "no window to capture"
  click_in_sight "link remove 0"
  expect "$(rules_drawn 'URL contains "work" → work' work)" "the trash didn't take the rule away again"
  # A rule to Little ilo opens its links in one, for the Space in front: the second, the last link
  # to the window having shown it.
  post clickkey $(drawn "link space 0")
  expect "^browser: menu shown $front, [^,]*, work (checked), Little ilo\$" \
    "the rule's Space pop-up didn't offer Little ilo"
  send choose "Little ilo"
  expect "$(rules_drawn 'URL contains "work" → Little ilo' work)" "the rule's pop-up didn't pick Little ilo"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  open_little_link "?work-little" "$second"
  expect "^browser: link rule 0 sends ${site}?work-little to little ilo\$" \
    "the rule didn't send its link to Little ilo"
  # Back to the second Space, for the launch after. Settings closed since the trash, so ⌘Z has
  # nothing of it to bring back.
  send menu Settings…
  expect '^browser: settings shown Links$' "Settings… didn't show Settings on its Links tab"
  settings_drew Links
  send key z cmd
  expect '^browser: settings undo had nothing$' "⌘Z didn't reach Settings"
  none_since '^browser: link rules drawn' || fail "⌘Z brought back a rule after Settings closed"
  post clickkey $(drawn "link space 0")
  expect "^browser: menu shown $front, [^,]*, work, Little ilo (checked)\$" \
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
# The sidebar's picture, which the launches below show until their UI has drawn, on the Mac and
# Windows.
mac=
[ "$(uname)" = Darwin ] && mac=1
pictures=$mac
! windows || pictures=1
if [ -n "$mac" ]; then
  # A window the screen has less room for at the next launch opens smaller, and past
  # launch_picture.rs's tolerance with no picture. One filling the hosted Mac's 1024 by 768 screen
  # opened 2 or 3 points shorter whenever the Dock had grown since (#1528), so the window quits
  # well inside the screen, and the launch below is made 3 points shorter on purpose (#1450).
  send screen
  corner=$(grep '^browser: screen at ' "$log" | tail -1 | cut -d' ' -f4,5)
  send place $((${corner% *} + 40)) $((${corner#* } + 60)) 900 560
  expect '^browser: drawn at 900x560$' "the window didn't draw at 900 by 560"
elif [ -z "$pictures" ]; then
  echo "skip: the sidebar's picture ⌘Q keeps, shown by the launches after: Linux has no launch picture."
fi
quit_by_key
[ -z "$pictures" ] || grep -q '^browser: launch picture taken$' "$log" ||
  fail "⌘Q didn't keep the sidebar's picture for the next launch"
exited
[ -z "$pictures" ] || ls "$(dirname "$saved")"/launch-picture-*.png > /dev/null 2>&1 ||
  fail "the sidebar's picture ⌘Q took wasn't on disk once the app quit"
# The next launch opens 3 points shorter than the picture, as when the Dock or the menu bar has
# grown since the quit (#1450), and still shows it, its foot at the window's bottom: the launch
# window's frame and the browser window's, which the screen would have clamped alike.
capture=
if [ -n "$mac" ]; then
  shorter=$(awk '{ printf "%.0f %.0f %.0f %.0f", $1, $2, $3, $4 - 3 }' "$launch_frame")
  echo "$shorter" > "$launch_frame"
  python3 -I -c '
import json, sys
with open(sys.argv[1]) as file:
    saved = json.load(file)
saved["windows"][0]["bounds"][3] -= 3
with open(sys.argv[1], "w") as file:
    json.dump(saved, file)
' "$saved" || fail "the window kept in $saved has no frame to make shorter"
  # Whole: `open` starts the app in /.
  capture="$PWD/$out/links-launch-shorter.png"
elif windows; then
  # In Windows' terms, which the app opens it by.
  capture=$(cygpath -w "$PWD/$out/links-launch.png")
fi

# Launched as the system launches the default browser, by `open` on the Mac and by the command
# registered for links on Windows, the app logs to $1 and is $pid once it runs. Then $2, if given, is
# the link that launched it.
launch() {
  log=$1
  rm -f "$log"
  if [ -n "$mac" ]; then
    # The first launch's window is captured as it was, $capture.
    open -a "$app" --env BROWSER_DISPLAY="$BROWSER_DISPLAY" \
      ${capture:+--env "BROWSER_LAUNCH_CAPTURE=$capture"} --stdout "$log" --stderr "$log" ${2:+"$2"}
    capture=
    pid=
    for _ in $(seq 100); do
      pid=$(pgrep -nf "$browser" || true)
      [ -n "$pid" ] && break
      sleep 0.1
    done
  elif windows && [ -n "${2:-}" ]; then
    registered exec "$2" < /dev/null > "$log" 2>&1 &
    pid=$!
  else
    BROWSER_LAUNCH_CAPTURE=$capture "$browser" ${2:+"$2"} > "$log" 2>&1 &
    pid=$!
    capture=
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
# On the Mac and Windows, fails with $1 unless the launch showed the sidebar's picture, which it
# leaves out when it opens at another size than the picture's, so the failure names both.
pictured() {
  [ -z "$pictures" ] || grep -q '^browser: launch picture shown$' "$log" || {
    kept=$(ls "$(dirname "$saved")" | grep '^launch-picture-' | tr '\n' ' ')
    fail "$1: it opened at $(grep -m1 '^browser: launch window frame ' "$log" | cut -d' ' -f5-), the pictures kept being ${kept:-none}"
  }
}

launch "$out/links-plain.log"
await '^browser: restored in ' || fail "the Spaces didn't come back"
if [ "$driven" = 1 ]; then
  await '^browser: link rules read 1$' || fail "the link rule didn't come back with the launch"
fi
pictured "the launch without a link didn't show the sidebar's picture"
[ -z "$mac" ] || grep -q "^browser: launch window frame $shorter\$" "$log" ||
  fail "the launch opened at $(grep -m1 '^browser: launch window frame ' "$log" | cut -d' ' -f5-), not 3 points shorter at $shorter"
plain="launch window $(at 'launch window') ms, started $(at 'started') ms, first frame $(at 'first frame') ms, UI shown $(at 'UI shown') ms"
quit_launched
! windows || [ -s "$out/links-launch.png" ] || fail "the launch window wasn't captured as it closed"

# The link that launches the browser opens over the Spaces it brings back, before the UI comes out
# from under the launch picture, so the restored tab never shows selected first. The UI's first
# frame can come before the link, under the picture.
launch "$out/links-cold.log" "${site}?cold"
await '^browser: restored in ' || fail "the Spaces didn't come back under the link"
pictured "the launch by a link didn't show the sidebar's picture"
await "$(link_line "?cold" "[0-9]*" "[0-9]*")" || fail "the link that launched the browser didn't open"
tab=$(grep "$(link_line "?cold" "[0-9]*" "[0-9]*")" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
await "^browser: loaded $tab ${site}?cold\$" || fail "the link that launched the browser didn't load"
sent=0
opened_once "${site}?cold"
opened=$(grep -n -m1 '^browser: link from another app ' "$log" | cut -d: -f1)
await '^browser: UI shown at ' || fail "the UI didn't come out from under the launch picture"
shown=$(grep -n -m1 '^browser: UI shown at ' "$log" | cut -d: -f1)
[ "$opened" -lt "$shown" ] || fail "the link opened after the UI came out from under the launch picture"
../tools/window-screenshot.sh "$pid" "$out/links-cold.png" || fail "no window to capture"
cold="launch window $(at 'launch window') ms, started $(at 'started') ms, first frame $(at 'first frame') ms, UI shown $(at 'UI shown') ms"
quit_launched
rm -f "$rules_file"

# On the Mac the first launch of an update opens the notes Sparkle kept for it, here the run's, as
# What's New, ilo's own page: a tab in the window's Space, never where links from other apps go
# (#1484). Off it no updater keeps notes.
if [ -n "$mac" ]; then
  defaults write "$app_id" UpdateNotes -dict \
    version "$(plutil -extract CFBundleVersion raw "$app/Contents/Info.plist")" url "${site}?notes"
fi
# With no rules, the link that launches the browser opens in Little ilo, for the Space the window
# brings back, over it.
launch "$out/links-little.log" "${site}?little"
if [ -n "$mac" ]; then
  notes_line="^browser: update notes ${site}?notes: tab [1-9][0-9]* in space [1-9][0-9]* of window [0-9]*\$"
  await "$notes_line" ||
    fail "the update's notes didn't open as a tab in the window: $(grep '^browser: \(update notes\|link\)' "$log" | head -2)"
  tab=$(grep "$notes_line" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
  await "^browser: loaded $tab ${site}?notes\$" || fail "the update's notes didn't load in their tab"
  ! grep -q "^browser: link from another app ${site}?notes" "$log" ||
    fail "the update's notes opened as a link from another app"
  ! defaults read "$app_id" UpdateNotes > /dev/null 2>&1 ||
    fail "the update's notes were kept for the next launch too"
else
  echo "skip: an update's notes opening as What's New: off the Mac no updater keeps notes."
fi
await '^browser: restored in ' || fail "the Spaces didn't come back under the link"
await "$(little_line "?little" "[0-9]*")" ||
  fail "the link that launched the browser didn't open in Little ilo: $(grep '^browser: link' "$log" | tail -1)"
tab=$(grep "$(little_line "?little" "[0-9]*")" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
await "^browser: loaded $tab ${site}?little\$" || fail "the link that launched the browser didn't load in Little ilo"
sent=0
opened_once "${site}?little"
grep -q '^browser: link from another app .* of window ' "$log" &&
  fail "the link that launched the browser opened as a tab too"
../tools/window-screenshot.sh "$pid" "$out/links-little-launch.png" all || fail "no window to capture"
quit_launched

# On Windows a click on a toast comes as a launch: notification_helper.exe, which Windows starts for
# it, runs chrome.exe with the toast's id, which hands it to the browser running, and Chrome gives
# the page its click, as the user allowed the site to notify (#1426). The id is the one Chrome
# gives the toast of a page's notification (NotificationLaunchId): a click, on a page's
# notification, its profile's, the app's name, not private, the site, and the notification, which
# Chrome names by the site and its tag. Chrome gives Windows a toast only for a copy whose Start
# menu shortcut the installer made, which the run's has none of, so the run sends the toast's click.
toasted=
if windows; then
  printf '%s' '<title>loading</title><script>
const toast = new Notification("ilo check", {tag: "check"});
toast.onshow = () => document.title = "toast shown";
toast.onerror = () => document.title = "toast refused " + Notification.permission;
// As a site does, it shows its tab.
toast.onclick = () => {
  focus();
  document.title = "toast clicked";
};
</script>' > "$out/site/toast.html"
  # Allows or takes back ($1) the site's notifications in the profile, while the app isn't running,
  # as it writes its preferences as it quits.
  site_notifies() {
    MSYS2_ARG_CONV_EXCL='*' python3 -I - "$(cygpath -w "$profile_dir/Default/Preferences")" \
      "${site%/},*" "$1" << 'PY' || fail "the profile's notifications for the site couldn't be set"
import json, os, sys
path, pattern, allow = sys.argv[1], sys.argv[2], sys.argv[3] == "allow"
prefs = {}
if os.path.exists(path):
    with open(path, encoding="utf-8") as f:
        prefs = json.load(f)
sites = prefs.setdefault("profile", {}).setdefault("content_settings", {})
sites = sites.setdefault("exceptions", {}).setdefault("notifications", {})
if allow:
    sites[pattern] = {"setting": 1}
else:
    sites.pop(pattern, None)
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w", encoding="utf-8") as f:
    json.dump(prefs, f)
PY
  }
  site_notifies allow
  launch "$out/links-toast.log" "${site}toast.html"
  await "$(little_line "toast.html" "[0-9]*")" ||
    fail "the notifying page didn't open in Little ilo: $(grep '^browser: link' "$log" | tail -1)"
  tab=$(grep "$(little_line "toast.html" "[0-9]*")" "$log" | sed 's/.*: tab \([0-9]*\) .*/\1/')
  await "^browser: title $tab toast shown\$" ||
    fail "the page's notification didn't show: $(grep "^browser: title $tab " "$log" | tail -1)"
  click_toast() {
    set -- "--notification-launch-id=0|1|Default|$(cat branding/name)|0|$site|n#${site%/}#check"
    [ -n "$fast" ] || set -- "--user-data-dir=$(cygpath -w "$profile_dir")" "$@"
    # Its | and # are the id's own, which Git Bash would take a path list of.
    MSYS2_ARG_CONV_EXCL='*' "$app/chrome.exe" "$@" < /dev/null >> "$out/links-sent.log" 2>&1 || true
  }
  click_toast
  await "^browser: title $tab toast clicked\$" || fail "the notification's click didn't reach the page"
  quit_launched
  site_notifies take-back
  toasted=1
fi
# The engine's profile folder goes back to how the run found it, with the check's profile.
if [ -n "$fast" ]; then
  rm -rf "$made"
  trap cleanup EXIT
  profile_dir=$check_profile_dir
  BROWSER_PROFILE_DIR=$(native_path "$profile_dir")
  saved=$check_saved
  launch_frame=$check_launch_frame
fi

if [ "$driven" = 1 ]; then
  [ -z "$again" ] ||
    echo "ok: a launch with no link while the window is minimized brings it back and to the front."
  echo "ok: links from another app open once, in Little ilo by default, the same link again bringing its window back, and sent to the Space in front as new tabs in the front window's Space and load there, and in the regular window, brought to the front, over a private one; Settings says the browser isn't the default. Settings › Links starts with the Meet rule, which sends a Meet link to the Space in front and which the trash takes; its rules send a link whose URL holds a rule's text, or is its page whatever the query, to the rule's Space or Little ilo, the first that matches winning, a drag reorders them, everything else goes to the Space picked, the trash takes a rule away and ⌘Z brings it back until Settings closes, and the rules come back with the launch. Windows: $out/links-little.png, $out/links-settings.png, $out/links-rules-meet.png, $out/links-rules-empty.png, $out/links-rules.png, $out/links-rule-back.png"
else
  echo "ok: a link from another app opens once, in Little ilo by default, and loads there."
  echo "skip: the launch with no link while the window is minimized, the private window, Settings and the link rules. Browser wasn't the active app, so the page ignored the click."
fi
[ -z "$msix" ] || echo "ok: a link sent to the Store's MSIX, which Windows activates for it, opens" \
  "once, in Little ilo, and loads there, the package's chrome.exe handing it over and ending."
echo "ok: a link that launches the browser opens once, over the Spaces it brings back, before the UI comes out from under the launch picture, and loads, and both launches show the sidebar's picture; with no rules, it opens in Little ilo. Windows: $out/links-cold.png $out/links-little-launch.png${mac:+, the launch window 3 points shorter than its picture: $out/links-launch-shorter.png}$(! windows || echo ", the launch window: $out/links-launch.png")"
[ -z "$toasted" ] || echo "ok: a click on a site's notification, which notification_helper.exe hands" \
  "chrome.exe as a launch with the toast's id, reaches the page, in the browser running."
[ -z "$mac" ] || echo "ok: an update's notes open as a tab in the window at its first launch, with links from other apps going to Little ilo, and only then."
echo "  Launch without a link: $plain; with one: $cold"
