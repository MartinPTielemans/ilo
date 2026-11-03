#!/bin/bash
# The Mac's nightly engine (#871): at 03:00, a launchd agent builds main's Release engine and
# publishes it (tools/mac-engine-publish.sh), when main's core or engine differ from the newest
# engine-mac release, so GitHub's hosted Macs check main on it (.github/workflows/check-mac.yml).
# launchd runs a night the Mac slept through when it wakes. Nothing on GitHub starts it: the Mac
# pulls main.
# It builds in the one engine tree, this checkout's engine/.deps/src, from a worktree of main of its
# own in ~/.cache/ilo-mac-nightly, whose engine/.deps is that one. It skips the night when an engine
# build or setup is running in it, when the tree holds a patch main's series never had, such as a
# branch's being tried, or when the tree's Chromium isn't the one main pins, which the integrator
# takes in with engine/setup.sh update. It never runs the browser, so it never touches
# /Applications/ilo.app, a profile or the Keychain; only gh's login, which uploads the release.
# Usage: mac-nightly.sh install|uninstall|status|run. install starts the agent on this checkout,
# uninstall stops it and removes the worktree, status says whether it's on and how the last night
# went, and run is the night itself, which the agent runs. Its log: ~/Library/Logs/ilo-mac-nightly.log.
set -euo pipefail
label=app.ilo.mac-nightly
agent=$HOME/Library/LaunchAgents/$label.plist
log=$HOME/Library/Logs/ilo-mac-nightly.log
work=$HOME/.cache/ilo-mac-nightly
# The checkout whose engine/.deps holds the tree: this one, or the one install named.
here=$(cd "$(dirname "$0")/.." && pwd)
checkout=${ILO_CHECKOUT:-$here}
# ilo's repository comes from this script's own checkout, main's once installed: the integrator's
# builds leave the engine's checkout on commits that may predate app/branding/repository.
repo=$(cat "$here/app/branding/repository")
tree=$checkout/engine/.deps/src

fail() {
  echo "mac-nightly: $*" >&2
  exit 1
}
skip() {
  echo "mac-nightly: $(date -u +%FT%RZ) skipped: $*"
  exit 0
}

[ "$(uname -sm)" = "Darwin arm64" ] || fail "the Mac's engine is built on a Mac with Apple silicon."

case ${1:-} in
install)
  [ -d "$tree" ] || fail "$checkout has no engine tree in engine/.deps/src. Make it with engine/setup.sh."
  mkdir -p "$(dirname "$agent")" "$(dirname "$log")"
  # The agent runs the worktree's copy of this script, main's, so the commit the checkout is on
  # doesn't matter: the integrator's builds leave it detached on commits from before this script.
  git -C "$checkout" fetch -q "$repo.git" +refs/heads/main:refs/ilo-nightly/main
  git -C "$checkout" cat-file -e refs/ilo-nightly/main:tools/mac-nightly.sh 2>/dev/null ||
    fail "main has no tools/mac-nightly.sh yet."
  [ -d "$work" ] || git -C "$checkout" worktree add -q --detach "$work" refs/ilo-nightly/main
  cat >"$agent" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>$work/tools/mac-nightly.sh</string>
    <string>run</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>ILO_CHECKOUT</key>
    <string>$checkout</string>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>3</integer>
    <key>Minute</key>
    <integer>0</integer>
  </dict>
  <key>Nice</key>
  <integer>10</integer>
  <key>StandardOutPath</key>
  <string>$log</string>
  <key>StandardErrorPath</key>
  <string>$log</string>
</dict>
</plist>
EOF
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$agent"
  echo "mac-nightly: on, at 03:00 each night, on $tree. Its log: $log. Start a night now with: launchctl kickstart gui/$(id -u)/$label"
  ;;
uninstall)
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  rm -f "$agent"
  if [ -d "$work" ]; then
    # The link to the tree goes first, so nothing removes what's behind it.
    rm -f "$work/engine/.deps"
    git -C "$work" worktree remove --force "$work"
  fi
  git -C "$checkout" update-ref -d refs/ilo-nightly/main 2>/dev/null || true
  echo "mac-nightly: off. The engine tree and its build stay as they are."
  ;;
status)
  if launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1; then
    echo "mac-nightly: on, at 03:00 each night, on $tree."
  else
    echo "mac-nightly: off."
  fi
  [ ! -e "$log" ] || grep '^mac-nightly: ' "$log" | tail -3
  ;;
run)
  echo "mac-nightly: $(date -u +%FT%RZ) started"
  [ -d "$work" ] || fail "it isn't installed. Install it with tools/mac-nightly.sh install."
  [ -d "$tree" ] || skip "there's no engine tree in $tree."
  # The integrator's builds and the prod updater use the same tree, run as engine/build.sh, the
  # memory guard engine-build.sh around it, or ninja on its out directories.
  if pgrep -f 'engine/(build|setup)[.]sh|engine-build[.]sh|ninja -C [.]deps/src/out/' >/dev/null; then
    skip "an engine build or setup is running in the tree."
  fi
  # Main alone, so the checkout gets no one else's branches.
  git -C "$work" fetch -q "$repo.git" +refs/heads/main:refs/ilo-nightly/main
  if [ "$(git -C "$work" rev-parse HEAD)" != "$(git -C "$work" rev-parse refs/ilo-nightly/main)" ]; then
    git -C "$work" checkout -q -f --detach refs/ilo-nightly/main
    [ -e "$work/tools/mac-nightly.sh" ] || fail "main has no tools/mac-nightly.sh yet."
    # Main's copy of this script, which the checkout may just have changed, runs the night.
    ILO_CHECKOUT=$checkout exec /bin/bash "$work/tools/mac-nightly.sh" run
  fi
  main=$(git -C "$work" rev-parse HEAD)
  # The commit of the newest engine-mac release in main's history, in prebuilt.ini's repository.
  released=$("$work/engine/fetch.sh" commit 2>/dev/null || true)
  if [ -n "$released" ] && git -C "$work" diff --quiet "$released" HEAD -- core engine \
    ':(exclude)engine/linux' ':(exclude)engine/windows' ':(exclude)*.md'; then
    skip "main (${main:0:12}) has the core and engine of engine-mac-${released:0:12}."
  fi
  # A patch the tree holds that no commit of main ever had is someone's work in progress, which
  # main's series would take out.
  kept=$tree/.browser_core-series
  if [ -f "$kept/series" ]; then
    while read -r name; do
      blob=$(git -C "$work" hash-object "$kept/$name")
      had=$(git -C "$work" log --format= --raw --no-abbrev HEAD -- "engine/patches/$name" |
        awk '{ print $4 }')
      grep -qx "$blob" <<<"$had" || skip "the tree holds $name as no commit of main has it."
    done <"$kept/series"
  fi
  version=$(git -C "$work" config -f engine/pins.ini chromium.version)
  built=$(sed -n 's/^[A-Z]*=//p' "$tree/chrome/VERSION" | paste -sd. -)
  [ "$built" = "$version" ] ||
    skip "the tree has Chromium $built and main pins $version, which engine/setup.sh update takes in."
  [ -e "$work/engine/.deps" ] || ln -s "$checkout/engine/.deps" "$work/engine/.deps"
  # build.sh links the worktree's core into the tree. The checkout's goes back after, as the
  # integrator's scripts expect it, even when the night fails.
  trap 'ln -sfn "$checkout/core" "$tree/browser_core"' EXIT
  # Hours, with the Mac idle, which would otherwise sleep halfway.
  caffeinate -i "$work/engine/build.sh"
  "$work/tools/mac-engine-publish.sh"
  echo "mac-nightly: $(date -u +%FT%RZ) published engine-mac-${main:0:12}"
  ;;
*) echo "usage: mac-nightly.sh install|uninstall|status|run" >&2 && exit 2 ;;
esac
