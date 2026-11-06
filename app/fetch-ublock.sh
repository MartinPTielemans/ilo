#!/bin/bash
# Downloads the uBlock Origin ublock.ini names, checked against its hash, into .deps/ublock, adds
# our key to its manifest so its id doesn't depend on where the app is, takes out the timers that
# wake an idle browser and the reload as it first starts, has it say when it's ready to block, for the
# core to hold the first pages until then (below), and prints its folder.
# Nothing in the download runs here: it's unzipped into a folder of its own and only read.
set -euo pipefail
cd "$(dirname "$0")"

fail() {
  echo "fetch-ublock: $*" >&2
  exit 1
}

# What starts each file this changes, as GPL 3 asks of a changed file.
changed="// Changed by ilo's app/fetch-ublock.sh: its housekeeping doesn't wake an idle browser, it doesn't reload as it first starts, and it says when it's ready to block."

# Puts $3, which may be several lines or none, in place of the line $2 of uBlock Origin's file $1,
# and fails unless that line is there once, so a new release is read again before it ships.
edit() {
  local file=$dir.part/uBlock0.chromium/$1
  [ "$(grep -cxF -- "$2" "$file")" = 1 ] || fail "$1 no longer has the line: $2"
  # Through the environment, as macOS's awk takes no newline in a -v value.
  old="$2" new="$3" changed="$changed" awk '
    NR == 1 && $0 != ENVIRON["changed"] { print ENVIRON["changed"] }
    $0 == ENVIRON["old"] { if (ENVIRON["new"] != "") print ENVIRON["new"]; next }
    { print }' "$file" > "$file.new"
  mv "$file.new" "$file"
}

# Git for Windows has sha256sum but no shasum, which the Mac has alone.
sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

version=$(git config -f ublock.ini ublock.version)
url=$(git config -f ublock.ini ublock.url)
sha256=$(git config -f ublock.ini ublock.sha256)
key=$(git config -f ublock.ini ublock.key)

dir=$PWD/.deps/ublock/$sha256
if [ ! -f "$dir/manifest.json" ] || ! grep -qF "\"key\": \"$key\"" "$dir/manifest.json" ||
  [ "$(head -n 1 "$dir/js/tab.js")" != "$changed" ] ||
  [ "$(head -n 1 "$dir/js/start.js")" != "$changed" ]; then
  mkdir -p .deps/ublock
  rm -rf "$dir" "$dir.part" "$dir.zip"
  echo "fetch-ublock: downloading uBlock Origin $version" >&2
  curl -fL --progress-bar -o "$dir.zip" "$url"
  echo "$sha256  $dir.zip" | sha256 -c --status ||
    { rm -f "$dir.zip"; fail "the download doesn't match ublock.ini's sha256."; }
  mkdir "$dir.part"
  unzip -q "$dir.zip" -d "$dir.part"
  rm "$dir.zip"
  manifest=$dir.part/uBlock0.chromium/manifest.json
  [ "$(head -n 1 "$manifest")" = "{" ] || fail "the manifest doesn't start as uBlock Origin's did."
  awk -v key="$key" 'NR == 1 { print; print "  \"key\": \"" key "\","; next } { print }' \
    "$manifest" > "$dir.part/manifest.json"
  mv "$dir.part/manifest.json" "$manifest"
  # Its housekeeping timers re-armed themselves, so an idle browser woke every 30 s after a request
  # with no tab, every 10 minutes for each tab and every 15 for each profile, to free what a tab
  # closed or discarded unseen left behind (#252). Nothing the browser uses needs them: the lists
  # update on their own weekly alarm, and the cache and the counts save once after a change. So
  # each runs once after what gives it work: the tabless requests' pages 30 s after them, a tab's
  # context 10 minutes after it's made, and the page stores' sweep 5 s after a tab closes.
  edit js/traffic.js '        gcTimer.on(30011);' '        reset();'
  edit js/tab.js '            this.gcTimer.on(gcPeriod);' ''
  edit js/tab.js '        pageStoreJanitorTimer.on(pageStoreJanitorPeriod);' ''
  edit js/tab.js '    const pageStoreJanitorPeriod = { min: 15 };' \
    '    const pageStoreJanitorPeriod = { sec: 5 };'
  edit js/tab.js '    pageStoreJanitorTimer.on(pageStoreJanitorPeriod);' \
    '    µb.pageStoreJanitorSoon = ( ) => { pageStoreJanitorTimer.on(pageStoreJanitorPeriod); };'
  edit js/tab.js '        super.onClosed(tabId);' \
    $'        super.onClosed(tabId);\n        µb.pageStoreJanitorSoon();'
  # On Chromium it reloaded itself as it first started in a profile, for Chrome's managed storage,
  # which can be empty then (uBlock-issues#1547). The core gives it its managed settings before it
  # loads, and the reload unloaded it under the first pages, as Chromium then resets every frame's
  # URL loader factories, dropping a subresource they were asking for (#954).
  edit js/start.js "    if ( lastVersionInt === 0 && vAPI.webextFlavor.soup.has('chromium') ) {" \
    '    if ( false ) {'
  # Chromium sends a request only to the listeners there are as it starts, and uBlock Origin starts
  # with the first pages and is ready seconds later, so the core holds a page until it says it's
  # ready (#744): it listens to an event only ChromeOS's kiosk apps are sent (core/ad_block.cc).
  edit js/traffic.js '            vAPI.net.setSuspendableListener(onBeforeRequest);' \
    $'            vAPI.net.setSuspendableListener(onBeforeRequest);\n            chrome.runtime.onRestartRequired.addListener(( ) => { });'
  mv "$dir.part/uBlock0.chromium" "$dir"
  rm -rf "$dir.part"
  # Earlier versions go.
  find .deps/ublock -mindepth 1 -maxdepth 1 ! -name "$sha256" -exec rm -rf {} +
fi
echo "$dir"
