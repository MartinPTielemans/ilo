# check.sh runs this at 790, only when named.
# It builds a check copy of its own (build.sh updates), on the Mac and Windows, so it runs only
# when named.
# Settings › About's Get nightly builds (docs/ux-spec.md › Updates), on a nightly check build
# (APP_TRACK=nightly, build.sh updates) whose updater reads one of two feeds the run serves, a
# release feed and a nightly one, for updates signed with a key the run makes and deletes after.
# Neither is ever the product's: the run never reads branding's feeds or the owner's key.
# About names the build Nightly, with the switch on under its version, and its check reads the
# nightly feed. Turned off, the row says the build stays until a release is newer, and the check
# reads the release feed, whose newest build is older than this one, which the updater doesn't
# offer: it never goes back a build. The choice outlives a relaunch, and turned on again, the
# updater checks the nightly feed at once.
# On the Mac and Windows. A Flatpak follows the branch it's installed from, so Linux has no row.

nightly_check() {
  if [ "$(uname)" = Linux ]; then
    echo "skip: nightly. A Flatpak follows the branch it's installed from, so Linux has no switch."
    return 0
  fi
  serve
  scratch=$(mktemp -d)
  mkdir "$scratch/profile" "$out/site/updates" "$out/site/nightly"
  local public build older version
  # The run's own key, in its temp dir, which cleanup deletes if the run fails.
  public=$(python3 -I ../tools/appcast.py key "$scratch/key")
  APP_TRACK=nightly UPDATE_FEED="${site}updates/appcast.xml" \
    UPDATE_FEED_NIGHTLY="${site}nightly/appcast.xml" UPDATE_PUBLIC_KEY="$public" \
    ./build.sh updates > "$out/nightly-build.log" 2>&1 ||
    fail "build.sh updates didn't make the nightly check build: $out/nightly-build.log"
  # The check build's number, as build.sh and build.rs count it.
  build=$(git rev-list --count HEAD)
  older=$((build - 1))
  version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
  # The release feed's newest build is the one before this, as a release is behind the nightly
  # that came after it, and the nightly feed's is this one. Neither is ever downloaded, so their
  # files are nowhere and their signatures nothing.
  nightly_feed "$out/site/updates/appcast.xml" "$older"
  nightly_feed "$out/site/nightly/appcast.xml" "$build"
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

  nightly_launch first
  nightly_about
  await '^browser: settings about Nightly ' || fail "About didn't name the build Nightly"
  # shellcheck disable=SC2046 # x and y
  send clickkey $(drawn "update check")
  expect '^browser: update check asked$' "About's Check for Updates didn't ask the updater"
  nightly_read nightly "$build" "with nightly builds on, the check didn't read the nightly feed" 1
  ../tools/window-screenshot.sh "$pid" "$out/nightly-on.png" all || fail "no window to capture"
  local at
  at=$(drawn "switch nightly builds") || fail "About has no Get nightly builds switch"
  # shellcheck disable=SC2086 # x and y
  send clickkey $at
  expect '^browser: nightly builds off$' "the switch didn't turn nightly builds off"
  # shellcheck disable=SC2046 # x and y
  send clickkey $(drawn "update check")
  nightly_read updates "$older" "with nightly builds off, the check didn't read the release feed" 1
  ../tools/window-screenshot.sh "$pid" "$out/nightly-off.png" all || fail "no window to capture"
  exec 3>&-
  quit
  [ "$(cat "$scratch/profile/update-track")" = release ] ||
    fail "the profile didn't keep nightly builds off: $(cat "$scratch/profile/update-track")"

  # Relaunched, it's still off, and on again it checks the nightly feed with no click of Check.
  nightly_launch relaunched
  nightly_about
  # shellcheck disable=SC2046 # x and y
  send clickkey $(drawn "update check")
  nightly_read updates "$older" "relaunched, the check didn't read the release feed" 2
  at=$(drawn "switch nightly builds") || fail "About has no Get nightly builds switch"
  # shellcheck disable=SC2086 # x and y
  send clickkey $at
  expect '^browser: nightly builds on$' "the switch didn't turn nightly builds on again"
  nightly_read nightly "$build" "turned on again, the updater didn't check the nightly feed" 2
  ../tools/window-screenshot.sh "$pid" "$out/nightly-on-again.png" all ||
    fail "no window to capture"
  exec 3>&-
  quit
  [ "$(cat "$scratch/profile/update-track")" = nightly ] ||
    fail "the profile didn't keep nightly builds on: $(cat "$scratch/profile/update-track")"

  if windows; then
    nightly_registry remove
  else
    defaults delete "$app_id" > /dev/null 2>&1 || true
    app_id=dev.browser.app
  fi
  rm -rf "$scratch"
  scratch=
  app=$saved_app browser=$saved_browser procs=$saved_procs
  echo "ok: the nightly check build's About named it Nightly, with Get nightly builds on, and its" \
    "check read the nightly feed. Off, the check read the release feed, whose build $older it" \
    "didn't offer to build $build, and a relaunch kept it off. On again, the updater read the" \
    "nightly feed at once. Windows: $out/nightly-on.png, $out/nightly-off.png," \
    "$out/nightly-on-again.png"
}
# The feed at $1, whose one build, for the Mac and for Windows, is $2.
nightly_feed() {
  local signature
  signature=$(python3 -I -c 'import base64; print(base64.b64encode(bytes(64)).decode())')
  local os file items=
  for os in macos:zip windows-x64:exe; do
    file=ilo-$2.${os#*:}
    items="$items<item><title>$version</title><sparkle:version>$2</sparkle:version>"
    items="$items<sparkle:shortVersionString>$version</sparkle:shortVersionString>"
    items="$items<enclosure url=\"https://127.0.0.1/$file\" length=\"1\""
    items="$items type=\"application/octet-stream\" sparkle:os=\"${os%%:*}\""
    items="$items sparkle:edSignature=\"$signature\"/></item>"
  done
  printf '%s\n' '<?xml version="1.0" encoding="utf-8"?>' \
    '<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">' \
    "<channel><title>$version</title>$items</channel></rss>" > "$1"
}
# Launches the check build, whose updater last checked a moment ago, so it checks only when asked,
# logging to nightly-$1.log.
nightly_launch() {
  log="$out/nightly-$1.log"
  step=0
  if windows; then
    nightly_registry now
  else
    defaults write "$app_id" SULastCheckTime -date "$(date -u '+%Y-%m-%d %H:%M:%S +0000')"
  fi
  BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>nightly</title>' \
    BROWSER_PROFILE_DIR="$profile" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: updater started$' || fail "the nightly check build didn't start its updater"
}
# Settings' About tab, with its update row's Check.
nightly_about() {
  send menu "About $app_name"
  expect '^browser: settings \(shown\|tab\|drew\) About$' \
    "About $app_name didn't show Settings' About tab"
  drawn "update check" > /dev/null || fail "About's row had no Check for Updates"
}
# Since the last command, the updater read the feed $1, the run's read $4 of it, whose build $2 is
# no newer than this one, and said so, and the row says it's up to date. Fails with $3.
nightly_read() {
  nightly_logged "^browser: update feed ${site}$1/appcast.xml\$" ||
    fail "$3: it read $(grep '^browser: update feed' "$log" | tail -1)"
  # The server's log is the whole run's.
  for _ in $(seq 100); do
    [ "$(grep -c "\"GET /$1/appcast.xml" "$out/site.log")" -ge "$4" ] && break
    sleep 0.1
  done
  [ "$(grep -c "\"GET /$1/appcast.xml" "$out/site.log")" -ge "$4" ] ||
    fail "$3: the server wasn't asked for it"
  local none="^browser: update none: reason [0-9]*, feed's newest $2, running $build\$"
  ! windows || none="^browser: update: none newer than build $build\$"
  nightly_logged "$none" || fail "$3: $(grep '^browser: update' "$log" | tail -3 | tr '\n' ';')"
  nightly_logged '^browser: update stage current$' || fail "$3: the row didn't say it's up to date"
}
# Waits up to 30 s for a line matching $1 logged since the last command, as a check can take.
nightly_logged() {
  for _ in $(seq 300); do
    logged "$1" && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  return 1
}
# Sets the Windows check build's last check to now, or removes its settings, under the key its
# updater keeps apart from ilo's (platform/windows/updates.cc).
nightly_registry() {
  WHAT="$1" powershell.exe -NoProfile -NonInteractive -Command '
    $key = "HKCU:\Software\ilo\Updates (check)"
    if ($env:WHAT -eq "remove") {
      Remove-Item -Path $key -Recurse -Force -ErrorAction SilentlyContinue
      exit 0
    }
    New-Item -Path $key -Force | Out-Null
    New-ItemProperty -Path $key -Name LastCheck -PropertyType QWord -Force `
      -Value ([DateTime]::UtcNow.ToFileTimeUtc()) | Out-Null' ||
    fail "PowerShell couldn't set the check build's last check"
}
nightly_check
