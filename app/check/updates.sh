# check.sh runs this at 780, only when named.
# It runs only when named until it passes on the Mac.
# The Mac's updates (docs/ux-spec.md › Updates), on a check build with Sparkle (build.sh updates)
# that checks a feed the run serves, for updates signed with a key the run makes and deletes after.
# Neither is ever the product's: the run never reads branding/update-feed or the owner's key.
# The app menu has Check for Updates… under About. Sparkle checks every 8 hours, from the last check.
# Check for Updates… finds the feed's update and shows it with its Markdown notes. A scheduled
# check shows no alert of Sparkle's own, full screen or not (#678): one it won't download by itself,
# as an informational update, is the sidebar's banner, Update available, whose click opens
# Sparkle's window on it with the keyboard.
# Settings › About's row checks without a window of Sparkle's own and says where the update
# stands: with nothing newer in the feed it says ilo is up to date within 5 s, with the feed gone
# the check fails, Try Again checks again once it's back, and Sparkle
# downloads the update, here the check build one build on, in the background. Once it's ready,
# the banner at the sidebar's foot says so, collapsed, and opens under the pointer, in light and
# dark, and its click says Restarting…, installs the update and relaunches into it.
# Not here: the release notes the first launch of an update opens, which Sparkle keeps for https
# notes only, so the links run opens notes of its own.
# On Linux, ilo's Flatpak, built as a release's (tools/flatpak.sh), installed from a repository of
# the run's own, where a newer build then comes, as a release puts one in ilo's. Flatpak's own
# portal, on a session bus of the run's own and looking every 10 s rather than every half hour,
# announces it, ilo installs it beside the build running and shows the banner at the sidebar's foot,
# collapsed and open under the pointer, in light and dark, whose click starts the new build once
# this one has quit. Flatpak asks the user once
# whether an app may update itself; the run says yes for them, in a home of its own, which has the
# profile too.
# It needs flatpak, flatpak-builder and ostree, and skips where ilo's Flatpak is installed from
# another repository, which it would replace.

# The update banner at the sidebar's foot, once it shows, in each look, which the window says as it
# changes: collapsed, then open with the pointer on it, then collapsed again with the pointer back
# on the rows. Captures $out/updates-banner-<look>.png and $out/updates-banner-open-<look>.png, and
# leaves the pointer off it. Sets bx, by, bw and bh to where it's drawn collapsed.
banner_looks() {
  await '^browser: update banner 0 at ' || fail "the update banner didn't show at the sidebar's foot"
  await '^browser: update banner collapsed$' || fail "the update banner didn't show collapsed"
  read -r bx by bw bh <<EOF
$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f6-)
EOF
  local tone light opened open_height
  open_height=$(sed -n 's/^update-banner-open = \([0-9]*\).*/\1/p' design/tokens.toml)
  for tone in light dark; do
    if [ $tone = light ]; then light=true; else light=false; fi
    if ! grep '^browser: window light ' "$log" | tail -1 | grep -q " $light\$"; then
      send menu "Appearance › $([ $tone = light ] && echo Light || echo Dark)"
      expect "^browser: window light $light\$" "View › Appearance didn't make the window $tone"
    fi
    ../tools/window-screenshot.sh "$pid" "$out/updates-banner-$tone.png" ||
      fail "no window to capture"
    opened=$(grep -c '^browser: update banner open$' "$log" || true)
    send move $((bx + bw / 2)) $((by + bh / 2))
    for _ in $(seq 50); do
      [ "$(grep -c '^browser: update banner open$' "$log" || true)" -gt "$opened" ] && break
      sleep 0.1
    done
    [ "$(grep -c '^browser: update banner open$' "$log" || true)" -gt "$opened" ] ||
      fail "the update banner didn't open under the pointer: $(grep '^browser: update banner' "$log" | tail -1)"
    # Once it's drawn open all the way, at tokens.toml's height, in the dialog's 0.275 s.
    for _ in $(seq 50); do
      [ "$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f9)" = "$open_height" ] &&
        break
      sleep 0.1
    done
    [ "$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f9)" = "$open_height" ] ||
      fail "the update banner didn't open to $open_height: $(grep '^browser: update banner 0 at ' "$log" | tail -1)"
    ../tools/window-screenshot.sh "$pid" "$out/updates-banner-open-$tone.png" ||
      fail "no window to capture"
    send move $sidebar_point
    expect '^browser: update banner collapsed$' "the update banner didn't close as the pointer left"
    # It says so as it starts closing; the next look's capture waits until it's drawn closed.
    for _ in $(seq 50); do
      [ "$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f9)" = "$bh" ] && break
      sleep 0.1
    done
    [ "$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f9)" = "$bh" ] ||
      fail "the update banner didn't close to $bh: $(grep '^browser: update banner 0 at ' "$log" | tail -1)"
  done
  send menu "Appearance › Automatic"
}

updates_check() {
  if [ "$(uname)" = Linux ]; then
    flatpak_updates_check
    return 0
  fi
  if [ "$(uname)" != Darwin ]; then
    echo "skip: updates. Windows' updater is only in a release build, which check.sh doesn't make."
    return 0
  fi
  serve
  scratch=$(mktemp -d)
  mkdir "$scratch/profile" "$out/site/updates"
  # The run's own key, in its temp dir, which cleanup deletes if the run fails.
  public=$(python3 -I ../tools/appcast.py key "$scratch/key")
  printf '%s\n\n%s\n' "A check update, from the updates run's own feed." \
    "- Its notes are Markdown." > "$scratch/notes.md"
  UPDATE_FEED="${site}updates/appcast.xml" UPDATE_PUBLIC_KEY="$public" ./build.sh updates \
    > /dev/null || fail "build.sh updates didn't make the check build"
  feed() {
    ../tools/appcast.sh "$scratch/notes.md" --local build/Updates.app "$out/site/updates" \
      "${site}updates/" "$scratch/key" "$@" > /dev/null ||
      fail "appcast.sh --local didn't make a feed"
  }
  feed
  next=$(($(plutil -extract CFBundleVersion raw build/Updates.app/Contents/Info.plist) + 1))
  # Sparkle's settings are in the check build's own defaults, which start empty.
  saved_app=$app saved_browser=$browser saved_procs=$procs
  app="$PWD/build/Updates.app"
  browser="$app/Contents/MacOS/Browser"
  procs="$app/Contents"
  app_id=dev.browser.app.updates
  defaults delete "$app_id" > /dev/null 2>&1 || true
  # A page that needs no server, so the feed is all the server has to serve.
  page='data:text/html,<title>updates</title>'
  # Launches the check build, which last checked for updates at the date $1, as date -v takes it.
  launch() {
    defaults write "$app_id" SULastCheckTime -date "$(date -u -v"$1" '+%Y-%m-%d %H:%M:%S +0000')"
    step=0
    BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$scratch/profile" "$browser" \
      < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await_load
    await '^browser: updater started$' || fail "the check build didn't start Sparkle"
    await '^browser: update check in [0-9]* s$' || fail "Sparkle didn't schedule its next check"
    delay=$(sed -n 's/^browser: update check in \([0-9]*\) s$/\1/p' "$log" | head -1)
  }

  # Checked a moment ago, the next check is 8 hours from it.
  log="$out/updates.log"
  launch +0S
  [ "$delay" -gt 28700 ] && [ "$delay" -le 28800 ] ||
    fail "checked a moment ago, Sparkle's next check is in $delay s, not 8 hours"
  interval=$delay
  grep '^browser: update' "$log" > "$out/updates-interval.txt"
  send state
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: updates. Browser wasn't the active app, so Sparkle's alert couldn't get the" \
      "keyboard."
  else
    send menubar "$app_name"
    expect "^browser: menu bar $app_name: About $app_name, Check for Updates…, Settings…" \
      "the app menu doesn't have Check for Updates… under About"
    post openmenu "$app_name"
    expect "^browser: menu tracked About $app_name, Check for Updates…, Settings…" \
      "the app menu didn't open"
    ../tools/window-screenshot.sh "$pid" "$out/updates-menu.png" all || fail "no window to capture"
    send dismiss
    send menu Check for Updates…
    # Found, or Sparkle's reason it isn't: none, with the feed's newest build, or failed. A check
    # that ends in none of them says how far it got, and Sparkle's log, the app's main thread and
    # the window say where it stopped.
    outcome="^browser: update \($next found\|none:\|failed:\)"
    for _ in $(seq 300); do
      grep -q "$outcome" "$log" && break
      sleep 0.1
    done
    if ! grep -q "$outcome" "$log"; then
      /usr/bin/log show --last 5m --style compact \
        --predicate 'subsystem == "org.sparkle-project.Sparkle"' > "$out/updates-sparkle.txt" 2>&1
      sample "$pid" 3 -file "$out/updates-sample.txt" > /dev/null 2>&1
      ../tools/window-screenshot.sh "$pid" "$out/updates-stalled.png" all
      got=$(grep '^browser: update \(feed loaded\|cycle ended\|alert shows\)' "$log" | tr '\n' ';')
      fail "Check for Updates… didn't end in 30 s: no update found, none or failed. Sparkle got to: \
${got:-nothing logged}. Sparkle's log: $out/updates-sparkle.txt, the app's threads: \
$out/updates-sample.txt, the window: $out/updates-stalled.png"
    fi
    if ! grep -q "^browser: update $next found\$" "$log"; then
      # Sparkle's own log, which says more, such as an item it couldn't parse.
      /usr/bin/log show --last 5m --style compact \
        --predicate 'subsystem == "org.sparkle-project.Sparkle"' > "$out/updates-sparkle.txt" 2>&1
      fail "Check for Updates… didn't find build $next: $(grep '^browser: update \(none\|failed\):' \
        "$log"). Sparkle's log: $out/updates-sparkle.txt"
    fi
    expect "^browser: update $next shown\$" "Sparkle didn't show the update it found"
    expect "^browser: update $next notes A check update, from the updates run's own feed\.\$" \
      "the update's alert didn't show its notes"
    expect "^browser: update $next alert has the keyboard\$" \
      "the update's alert didn't get the keyboard"
    ../tools/window-screenshot.sh "$pid" "$out/updates-found.png" all || fail "no window to capture"
    exec 3>&-
    quit

    # Last checked 7 hours 59 minutes 35 seconds ago, Sparkle checks 25 s after the launch, with
    # up to 15 s more of its timer's leeway, by when the window is full screen. The update is
    # informational, which an automatic check never downloads: the banner says it's available,
    # and Sparkle shows nothing of its own, in full screen or after.
    feed --informational
    log="$out/updates-scheduled.log"
    launch -28775S
    [ "$delay" -le 25 ] || fail "checked 8 hours ago less 25 s, the next check is in $delay s"
    send fullscreen
    expect '^browser: window entered full screen$' "the window didn't go full screen"
    grep -q "^browser: update $next " "$log" &&
      fail "Sparkle checked before the window went full screen, $delay s after the launch"
    # The check comes when Sparkle's timer fires, up to 40 s after the launch.
    for _ in $(seq 600); do
      grep -q "^browser: update $next \(in the banner\|shown\)\$" "$log" && break
      kill -0 "$pid" 2>/dev/null || fail "the app exited"
      sleep 0.1
    done
    grep -q "^browser: update $next in the banner\$" "$log" ||
      fail "the scheduled check didn't leave its update to the banner: $(grep "^browser: update" "$log")"
    await '^browser: update stage available' || fail "the banner wasn't told the update is available"
    ../tools/window-screenshot.sh "$pid" "$out/updates-scheduled-full-screen.png" all ||
      fail "no window to capture"
    send fullscreen
    expect '^browser: window left full screen$' "the window didn't leave full screen"
    await '^browser: update banner collapsed$' || fail "the update banner didn't show collapsed"
    # A round trip after the window left full screen, by when Sparkle would have shown its alert.
    send state
    grep -q '^browser: update alert shows$\|^browser: update .* alert has the keyboard$' "$log" &&
      fail "the scheduled check showed Sparkle's alert: $(grep '^browser: update' "$log" | tr '\n' ';')"
    ../tools/window-screenshot.sh "$pid" "$out/updates-available.png" all || fail "no window to capture"
    read -r bx by bw bh <<EOF
$(grep '^browser: update banner 0 at ' "$log" | tail -1 | cut -d' ' -f6-)
EOF
    send move $((bx + bw / 2)) $((by + bh / 2))
    expect '^browser: update banner open$' "the update banner didn't open under the pointer"
    send click $((bx + bw / 2)) $((by + bh / 2))
    expect '^browser: command show-update via sidebar$' "Update available didn't take the click"
    expect '^browser: update show asked$' "Update available didn't ask for Sparkle's window"
    # Sparkle resumes the update it held rather than showing it anew, so its window says only that
    # it has the keyboard.
    expect "^browser: update $next alert has the keyboard\$" \
      "the banner's click didn't show Sparkle's window with the keyboard"
    ../tools/window-screenshot.sh "$pid" "$out/updates-shown-from-banner.png" all ||
      fail "no window to capture"
    exec 3>&-
    quit
    about_updates
    echo "ok: the check build's app menu has Check for Updates… under About, and Sparkle checks 8" \
      "hours after the last check, in $interval s here. Check for Updates… found the run's" \
      "signed update and showed it with its Markdown notes. A scheduled check in full screen" \
      "showed no alert, then or after, and left its update to the banner, Update available," \
      "whose click showed Sparkle's window with the keyboard. About's check said it was up to" \
      "date in $current_in s with nothing newer in the feed, failed with no feed, and" \
      "Try Again downloaded build $next, the banner showed collapsed and opened under the" \
      "pointer, and its click said Restarting… and relaunched into build $next." \
      "Interval: $out/updates-interval.txt." \
      "Windows: $out/updates-menu.png, $out/updates-found.png," \
      "$out/updates-scheduled-full-screen.png, $out/updates-available.png," \
      "$out/updates-shown-from-banner.png, $out/updates-about-*.png, $out/updates-banner-*.png," \
      "$out/updates-relaunched.png"
  fi
  defaults delete "$app_id" > /dev/null 2>&1 || true
  rm -rf "$scratch"
  scratch=
  app=$saved_app browser=$saved_browser procs=$saved_procs app_id=dev.browser.app
}
# About's row and Restart to Update, on the Mac, from launch to the relaunch into the update.
about_updates() {
  # The update: the check build one build and one version on, as a release is, sealed again after
  # the change.
  mkdir "$scratch/next"
  cp -cR "$app" "$scratch/next/"
  local copy version
  copy=$scratch/next/$(basename "$app")
  version=$(plutil -extract CFBundleShortVersionString raw "$copy/Contents/Info.plist" |
    awk -F. '{ print $1 "." $2 "." $3 + 1 }')
  plutil -replace CFBundleVersion -string "$next" "$copy/Contents/Info.plist"
  plutil -replace CFBundleShortVersionString -string "$version" "$copy/Contents/Info.plist"
  codesign --force --deep --sign - "$copy" 2> "$out/updates-codesign.txt" ||
    fail "the update didn't sign: $out/updates-codesign.txt"
  local archive=$out/site/updates/update-$next.zip
  ditto -c -k --keepParent "$copy" "$archive"
  local signature
  signature=$(.deps/Sparkle/bin/sign_update --ed-key-file "$scratch/key" -p "$archive")
  python3 -I ../tools/appcast.py local "$out/site/updates/appcast.xml" "$version" "$next" \
    "$scratch/notes.md" "${site}updates/$(basename "$archive")" "$(wc -c < "$archive" | tr -d ' ')" \
    "$signature" > /dev/null || fail "appcast.py local didn't make the feed"
  log="$out/updates-about.log"
  launch +0S
  # Clicks the row's button, which logs where it's drawn as it changes.
  about_click() {
    # shellcheck disable=SC2046 # x and y
    send clickkey $(sed -n "s/^browser: settings update $1 at //p" "$log" | tail -1)
  }
  # Waits up to 5 minutes for the update's stage $1, as a download and Sparkle's checks of the
  # update take.
  stage() {
    for _ in $(seq 3000); do
      grep -q "^browser: update stage $1\$" "$log" && return 0
      kill -0 "$pid" 2> /dev/null || fail "the app exited"
      sleep 0.1
    done
    fail "$2: $(grep '^browser: update stage' "$log" | tr '\n' ';')"
  }
  send menu "About $app_name"
  expect '^browser: settings \(shown\|tab\|drew\) About$' "About ilo didn't show Settings' About tab"
  expect '^browser: settings update check at ' "About's row had no Check for Updates"
  ../tools/window-screenshot.sh "$pid" "$out/updates-about-idle.png" all || fail "no window to capture"
  # With nothing newer in the feed, the check says so within 5 s, as one against the live feed
  # does in under one (#1976).
  mv "$out/site/updates/appcast.xml" "$scratch/appcast.xml"
  python3 -I ../tools/appcast.py local "$out/site/updates/appcast.xml" \
    "$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist")" \
    "$(plutil -extract CFBundleVersion raw "$app/Contents/Info.plist")" "$scratch/notes.md" \
    > /dev/null || fail "appcast.py local didn't make a feed of the build running"
  about_click check
  expect '^browser: update check asked$' "About's Check for Updates didn't ask the updater"
  local tenths=0
  while [ $tenths -lt 50 ] && ! grep -q '^browser: update stage current$' "$log"; do
    kill -0 "$pid" 2> /dev/null || fail "the app exited"
    sleep 0.1
    tenths=$((tenths + 1))
  done
  grep -q '^browser: update stage current$' "$log" ||
    fail "with nothing newer in the feed, About's check didn't say $app_name is up to date in 5 s: \
$(grep '^browser: update' "$log" | tr '\n' ';')"
  current_in=$((tenths / 10)).$((tenths % 10))
  ../tools/window-screenshot.sh "$pid" "$out/updates-about-current.png" all ||
    fail "no window to capture"
  # With the feed gone, the check fails, and Try Again finds the update once it's back.
  rm "$out/site/updates/appcast.xml"
  about_click check
  expect '^browser: update check asked$' "About's Check for Updates didn't ask the updater"
  stage 'failed to check' "with no feed, About's check didn't fail"
  ! grep -q '^browser: update [0-9]* \(shown\|held\)' "$log" ||
    fail "About's check showed Sparkle's alert"
  ../tools/window-screenshot.sh "$pid" "$out/updates-about-failed.png" all ||
    fail "no window to capture"
  mv "$scratch/appcast.xml" "$out/site/updates/appcast.xml"
  about_click check
  stage "ready $version" "Try Again didn't download build $next"
  grep -q "^browser: update stage downloading $version\$" "$log" ||
    fail "the row never said it was downloading"
  ! grep -q '^browser: update [0-9]* \(shown\|held\)' "$log" ||
    fail "the download showed Sparkle's alert"
  expect '^browser: settings update restart at ' "About's row had no Restart to Update"
  ../tools/window-screenshot.sh "$pid" "$out/updates-about-ready.png" all ||
    fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  # Drawn as the update became ready, behind Settings.
  banner_looks
  send move $((bx + bw / 2)) $((by + bh / 2))
  expect '^browser: update banner open$' "the update banner didn't open under the pointer"
  # Posted: the app quits before the driver could say it's done.
  post click $((bx + bw / 2)) $((by + bh / 2))
  expect '^browser: command restart-to-update via sidebar$' "Restart to Update didn't take the click"
  expect '^browser: update banner restarting$' "the update banner didn't say Restarting…"
  expect '^browser: update installs now$' "Restart to Update didn't install the update"
  exec 3>&-
  # Sparkle quits the app, installs the update over it and opens it again, as a new process.
  local old=$pid relaunched=
  for _ in $(seq 1200); do
    kill -0 "$old" 2> /dev/null || break
    sleep 0.1
  done
  kill -0 "$old" 2> /dev/null && fail "Restart to Update didn't quit the app"
  for _ in $(seq 1200); do
    relaunched=$(pgrep -n -f "^$browser" || true)
    [ -n "$relaunched" ] && [ "$relaunched" != "$old" ] && break
    relaunched=
    sleep 0.1
  done
  [ -n "$relaunched" ] || fail "the app didn't come back after Restart to Update"
  [ "$(plutil -extract CFBundleVersion raw "$app/Contents/Info.plist")" = "$next" ] ||
    fail "the app that came back isn't build $next"
  pid=$relaunched
  for _ in $(seq 300); do
    ../tools/window-screenshot.sh "$pid" "$out/updates-relaunched.png" 2> /dev/null && break
    sleep 0.1
  done
  [ -f "$out/updates-relaunched.png" ] || fail "the update's window didn't come up"
  kill "$pid" 2> /dev/null
  for _ in $(seq 100); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.1
  done
}
flatpak_updates_check() {
  local id=app.ilo.browser remote=ilo-updates-check portal= tool
  for tool in flatpak flatpak-builder ostree dbus-daemon; do
    command -v "$tool" > /dev/null || {
      echo "skip: updates. Linux's needs flatpak, flatpak-builder, ostree and dbus-daemon:" \
        "no $tool."
      return 0
    }
  done
  for tool in /usr/libexec/flatpak-portal /usr/lib/flatpak/flatpak-portal; do
    [ -x "$tool" ] && portal=$tool && break
  done
  [ -n "$portal" ] || { echo "skip: updates. No flatpak-portal."; return 0; }
  local origin
  origin=$(flatpak info --user --show-origin "$id" 2> /dev/null || true)
  if [ -n "$origin" ] && [ "$origin" != "$remote" ]; then
    echo "skip: updates. ilo's Flatpak is installed here from $origin, which the run would replace."
    return 0
  fi
  ../tools/flatpak.sh > "$out/updates-flatpak.log" 2>&1 ||
    fail "tools/flatpak.sh didn't build the Flatpak: $out/updates-flatpak.log"
  scratch=$(mktemp -d)
  local repo=$scratch/repo ref=app/$id/x86_64/master
  ostree init --mode=archive-z2 --repo="$repo"
  ostree pull-local --repo="$repo" build/flatpak/repo "$ref" > /dev/null
  flatpak build-update-repo "$repo" > /dev/null
  flatpak remote-delete --user --force "$remote" 2> /dev/null || true
  flatpak remote-add --user --no-gpg-verify "$remote" "file://$repo"
  flatpak install --user -y --noninteractive --reinstall "$remote" "$id" \
    > "$out/updates-install.log" 2>&1 ||
    fail "the Flatpak didn't install from the run's repository: $out/updates-install.log"
  local installed
  installed=$(flatpak info --user "$id" | sed -n 's/^ *Commit: //p')
  # The run's own home and session bus, with Flatpak's own installation.
  local saved_home=$HOME saved_user_dir=${FLATPAK_USER_DIR:-}
  local saved_bus=${DBUS_SESSION_BUS_ADDRESS:-}
  export FLATPAK_USER_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/flatpak"
  export HOME=$scratch/home
  mkdir "$HOME"
  # The Flatpak's profile, where its XDG_CONFIG_HOME puts it (main.rs's profile_dir).
  count_off "$HOME/.var/app/$id/config/$(cat branding/name)"
  local started
  started=$(env -u XDG_DATA_HOME dbus-daemon --session --fork --print-address=1 --print-pid=1 |
    tr '\n' ' ')
  export DBUS_SESSION_BUS_ADDRESS=${started%% *}
  buses=$(echo "$started" | cut -d' ' -f2)
  XDG_DATA_HOME='' flatpak permission-set flatpak updates "$id" yes ||
    fail "Flatpak's permission store didn't take the run's yes: is xdg-desktop-portal installed?"
  "$portal" --poll-timeout=10 --no-idle-exit > "$out/updates-portal.log" 2>&1 &
  buses="$buses $!"
  local saved_procs=$procs
  procs=/app/ilo/chrome
  log="$out/updates.log"
  step=0
  # Under Wayland the driver clicks and types through sway, whose socket the sandbox doesn't
  # otherwise see (drive_wayland.rs).
  set -- --env=BROWSER_DRIVE=1
  [ -z "${SWAYSOCK:-}" ] || set -- "$@" --filesystem="$SWAYSOCK" --env=SWAYSOCK="$SWAYSOCK"
  flatpak run --user "$@" "$id" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: updater started$' || fail "the Flatpak didn't start its updater"
  # A newer build, as a release publishes one.
  flatpak build-commit-from --src-ref="$ref" --subject="The updates run's build" --force \
    "$repo" "$ref" > /dev/null
  flatpak build-update-repo "$repo" > /dev/null
  local next
  next=$(ostree rev-parse --repo="$repo" "$ref")
  # The portal looks every 10 s, then installs it.
  for _ in $(seq 1200); do
    grep -q '^browser: update \(Ready\|Failed\)' "$log" && break
    kill -0 "$pid" 2> /dev/null || fail "the app exited"
    sleep 0.1
  done
  grep -q '^browser: update Installing$' "$log" ||
    fail "ilo didn't install the build Flatpak's portal announced: $out/updates-portal.log"
  grep -q '^browser: update Ready$' "$log" ||
    fail "the new build didn't install: $(grep '^browser: update' "$log" | tail -1)"
  [ "$(flatpak info --user "$id" | sed -n 's/^ *Commit: //p')" = "$next" ] ||
    fail "Flatpak didn't install build $next beside $installed"
  # Before any command of the driver's, which expect looks after.
  await '^browser: update stage ready$' || fail "the build installed didn't make the update ready"
  ../tools/window-screenshot.sh "$pid" "$out/updates-ready.png" || fail "no window to capture"
  local bx by bw bh
  banner_looks
  send move $((bx + bw / 2)) $((by + bh / 2))
  expect '^browser: update banner open$' "the update banner didn't open under the pointer"
  # Posted: the app quits before the driver could say it's done.
  post click $((bx + bw / 2)) $((by + bh / 2))
  expect '^browser: command restart-to-update via sidebar$' "Restart to Update didn't take the click"
  expect '^browser: update banner restarting$' "the update banner didn't say Restarting…"
  for _ in $(seq 300); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.1
  done
  kill -0 "$pid" 2> /dev/null && fail "Restart didn't quit the build running"
  # The new build logs to the portal's output, which started it, and outlives it if it fails. No
  # flatpak ps until its window is up: each one clears the per-app /tmp of the sandbox that quit
  # unless a running one holds its lock, and the new sandbox lets go of it from flatpak run's exec
  # of bwrap until bwrap has bound that /tmp, which it then finds gone (#2688).
  pid=${buses##* }
  log="$out/updates-portal.log"
  await '^browser: UI shown' || {
    grep -q "^bwrap: Can't get type of source .*/\.flatpak/$id/tmp" "$log" &&
      fail "the new build's sandbox found its per-app /tmp gone: Flatpak's race (#2688)"
    fail "the new build's window didn't come up"
  }
  local running= commit=
  # flatpak ps lists a starting instance with PID 0 until bwrap's is known, and cleanup's kill of
  # pid 0 would end the whole run, this script included (exit 143).
  running=$(flatpak ps --columns=application,commit,pid |
    awk -v id="$id" '$1 == id && $3 > 0 { print $2, $3; exit }')
  commit=${running%% *}
  [ -n "$commit" ] && [ "${next#"$commit"}" != "$next" ] ||
    fail "Restart didn't start build $next: running ${commit:-nothing}. $out/updates-portal.log"
  pid=${running##* }
  ../tools/window-screenshot.sh "$pid" "$out/updates-restarted.png" || fail "no window to capture"
  flatpak kill "$id" 2> /dev/null || true
  # Gone before the run ends, which cleanup would otherwise race to kill.
  for _ in $(seq 100); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.1
  done
  kill $buses 2> /dev/null || true
  buses=
  export HOME=$saved_home DBUS_SESSION_BUS_ADDRESS=$saved_bus
  unset FLATPAK_USER_DIR
  [ -z "$saved_user_dir" ] || export FLATPAK_USER_DIR="$saved_user_dir"
  flatpak uninstall --user -y --noninteractive "$id" > /dev/null 2>&1 || true
  flatpak remote-delete --user --force "$remote" 2> /dev/null || true
  rm -rf "$scratch"
  scratch=
  procs=$saved_procs
  echo "ok: ilo's Flatpak, installed at $(echo "$installed" | cut -c1-12), installed the newer" \
    "build $(echo "$next" | cut -c1-12) Flatpak's portal announced, showed the banner at the" \
    "sidebar's foot, collapsed and open under the pointer, in light and dark, and its click said" \
    "Restarting… and started it once the build running had quit." \
    "Windows: $out/updates-ready.png, $out/updates-banner-*.png, $out/updates-restarted.png"
}
updates_check
