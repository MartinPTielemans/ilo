# check.sh runs this at 800, only when named.
# It builds a check copy of its own (build.sh updates), on the Mac and Windows, so it runs only
# when named.
# Settings › About's Check for updates automatically (#1793), on a check build (build.sh updates)
# whose updater reads a feed the run serves, for updates signed with a key the run makes and
# deletes after. Neither is ever the product's: the run never reads branding's feeds or the owner's
# key. On, the default, About's line says the updater checks every 8 hours. Turned off, the profile
# keeps it, and relaunched with a check due 25 s on, the updater asks the feed nothing past that,
# not at launch and not on its schedule; About's Check for Updates then asks it once. Turned on
# again, the updater schedules its next check from the last, with no relaunch. On the Mac and
# Windows. On Linux, whose Flatpak's updates are Flathub's, About has no such switch.

# Requests for the run's feed the server has had.
update_checks_asked() {
  grep -c '"GET /updates/appcast.xml' "$out/site.log" 2> /dev/null || true
}

# Launches the check build, whose updater last checked $1 seconds ago, logging to
# update-checks-$2.log.
update_checks_launch() {
  log="$out/update-checks-$2.log"
  step=0
  if windows; then
    AGO="$1" powershell.exe -NoProfile -NonInteractive -Command '
      $key = "HKCU:\Software\ilo\Updates (check)"
      New-Item -Path $key -Force | Out-Null
      $last = [DateTime]::UtcNow.AddSeconds(-[int]$env:AGO).ToFileTimeUtc()
      New-ItemProperty -Path $key -Name LastCheck -PropertyType QWord -Force -Value $last |
        Out-Null' || fail "PowerShell couldn't set the check build's last check"
  else
    defaults write "$app_id" SULastCheckTime -date "$(date -u -v-"$1"S '+%Y-%m-%d %H:%M:%S +0000')"
  fi
  BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>update checks</title>' \
    BROWSER_PROFILE_DIR="$profile" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: updater started$' || fail "the check build didn't start its updater"
}

# Settings' About tab, drawn, with its update row's Check.
update_checks_about() {
  send menu "About $app_name"
  expect '^browser: settings \(shown\|tab\|drew\) About$' \
    "About $app_name didn't show Settings' About tab"
  drawn "update check" > /dev/null || fail "About's row had no Check for Updates"
}

update_checks_check() {
  if [ "$(uname)" = Linux ]; then
    # The run's own app: About draws its rows, and none is the switch.
    log="$out/update-checks.log"
    step=0
    BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>update checks</title>' "$browser" \
      < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await_load
    send menu "About $app_name"
    expect '^browser: settings \(shown\|tab\|drew\) About$' \
      "About $app_name didn't show Settings' About tab"
    drawn "switch whats new" > /dev/null || fail "About didn't draw its rows"
    grep -q '^browser: settings switch automatic update checks at ' "$log" &&
      fail "About on Linux has a switch for automatic update checks, which are Flathub's"
    ../tools/window-screenshot.sh "$pid" "$out/update-checks-linux.png" || fail "no window to capture"
    exec 3>&-
    quit
    echo "ok: About on Linux has no Check for updates automatically, as Flathub updates the" \
      "Flatpak. Window: $out/update-checks-linux.png"
    return 0
  fi
  serve
  scratch=$(mktemp -d)
  mkdir "$scratch/profile" "$out/site/updates"
  local public build version
  # The run's own key, in its temp dir, which cleanup deletes if the run fails.
  public=$(python3 -I ../tools/appcast.py key "$scratch/key")
  UPDATE_FEED="${site}updates/appcast.xml" UPDATE_PUBLIC_KEY="$public" \
    ./build.sh updates > "$out/update-checks-build.log" 2>&1 ||
    fail "build.sh updates didn't make the check build: $out/update-checks-build.log"
  # The feed's one build is the one before this, which the updater never offers, so each check
  # is a request and no more. Never downloaded, so its files are nowhere and its signature nothing.
  build=$(git rev-list --count HEAD)
  version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
  local signature os file items=
  signature=$(python3 -I -c 'import base64; print(base64.b64encode(bytes(64)).decode())')
  for os in macos:zip windows-x64:exe; do
    file=ilo-$((build - 1)).${os#*:}
    items="$items<item><title>$version</title><sparkle:version>$((build - 1))</sparkle:version>"
    items="$items<sparkle:shortVersionString>$version</sparkle:shortVersionString>"
    items="$items<enclosure url=\"https://127.0.0.1/$file\" length=\"1\""
    items="$items type=\"application/octet-stream\" sparkle:os=\"${os%%:*}\""
    items="$items sparkle:edSignature=\"$signature\"/></item>"
  done
  printf '%s\n' '<?xml version="1.0" encoding="utf-8"?>' \
    '<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">' \
    "<channel><title>$version</title>$items</channel></rss>" > "$out/site/updates/appcast.xml"
  saved_app=$app saved_browser=$browser saved_procs=$procs
  local profile
  if windows; then
    app="$PWD/build/Updates"
    procs="$(cygpath -m "$app/chrome.exe")"
    profile=$(cygpath -w "$scratch/profile")
  else
    app="$PWD/build/Updates.app"
    browser="$app/Contents/MacOS/Browser"
    procs="$app/Contents"
    app_id=dev.browser.app.updates
    defaults delete "$app_id" > /dev/null 2>&1 || true
    profile=$scratch/profile
  fi

  # On, as a first launch has it; off, kept in the profile.
  update_checks_launch 0 on
  update_checks_about
  local at
  at=$(drawn "switch automatic update checks") ||
    fail "About has no Check for updates automatically"
  ../tools/window-screenshot.sh "$pid" "$out/update-checks-on.png" all || fail "no window to capture"
  # shellcheck disable=SC2086 # x and y
  send clickkey $at
  expect '^browser: automatic update checks off$' "the switch didn't turn automatic checks off"
  expect '^browser: update checks automatic off$' "the updater wasn't told checks are off"
  ../tools/window-screenshot.sh "$pid" "$out/update-checks-off.png" all || fail "no window to capture"
  exec 3>&-
  quit
  [ -e "$scratch/profile/no-automatic-update-checks" ] ||
    fail "the profile didn't keep automatic checks off"

  # Relaunched off, with a check due 25 s after the launch: nothing goes to the feed past it.
  local before asked
  before=$(update_checks_asked)
  update_checks_launch 28775 off
  grep -q '^browser: update checks automatic off$' "$log" ||
    fail "the launch didn't start the updater with checks off"
  # Past the due time, with Sparkle's timer leeway of up to 15 s.
  sleep 50
  kill -0 "$pid" 2> /dev/null || gone
  asked=$(update_checks_asked)
  [ "$asked" = "$before" ] ||
    fail "with automatic checks off, the updater asked the feed $((asked - before)) times on its own"
  grep -q '^browser: update check in ' "$log" &&
    fail "with automatic checks off, the updater scheduled a check: $(grep '^browser: update check in ' "$log")"
  # Check for Updates…, from About, asks it once.
  update_checks_about
  # shellcheck disable=SC2046 # x and y
  send clickkey $(drawn "update check")
  expect '^browser: update check asked$' "About's Check for Updates didn't ask the updater"
  for _ in $(seq 300); do
    [ "$(update_checks_asked)" -ge $((before + 1)) ] && break
    sleep 0.1
  done
  [ "$(update_checks_asked)" = $((before + 1)) ] ||
    fail "Check for Updates with automatic checks off asked the feed $(($(update_checks_asked) - before)) times, not once"
  ../tools/window-screenshot.sh "$pid" "$out/update-checks-asked.png" all ||
    fail "no window to capture"
  # On again, the updater schedules its next check from the last, with no relaunch.
  at=$(drawn "switch automatic update checks") ||
    fail "About has no Check for updates automatically"
  # shellcheck disable=SC2086 # x and y
  send clickkey $at
  expect '^browser: automatic update checks on$' "the switch didn't turn automatic checks on again"
  expect '^browser: update checks automatic on$' "the updater wasn't told checks are on"
  for _ in $(seq 300); do
    logged '^browser: update check in [0-9]* s$' && break
    sleep 0.1
  done
  logged '^browser: update check in [0-9]* s$' ||
    fail "turned on again, the updater didn't schedule its next check"
  exec 3>&-
  quit
  [ -e "$scratch/profile/no-automatic-update-checks" ] &&
    fail "the profile kept automatic checks off after they were turned on"

  if windows; then
    powershell.exe -NoProfile -NonInteractive -Command \
      'Remove-Item -Path "HKCU:\Software\ilo\Updates (check)" -Recurse -Force -ErrorAction SilentlyContinue' ||
      true
  else
    defaults delete "$app_id" > /dev/null 2>&1 || true
    app_id=dev.browser.app
  fi
  rm -rf "$scratch"
  scratch=
  app=$saved_app browser=$saved_browser procs=$saved_procs
  echo "ok: About's Check for updates automatically turned off, and the profile kept it. Relaunched" \
    "off with a check due, the updater asked the feed nothing in 50 s; About's Check for Updates" \
    "asked it once. On again, it scheduled its next check: $(grep '^browser: update check in ' "$log" | tail -1)." \
    "Windows: $out/update-checks-on.png, $out/update-checks-off.png, $out/update-checks-asked.png"
}
update_checks_check
