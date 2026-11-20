#!/bin/bash
# Whether macOS starts the signed app $1, or the app in the DMG $1. macOS checks an app's profile
# and keys as its main executable starts, and kills one whose profile doesn't list its certificate,
# isn't for its App ID or doesn't grant a key it claims, before its first line: an app nobody can
# open. With BROWSER_PREWARM the app ends at its first line (main.rs), as an update's prewarm does,
# so it reads no Keychain item and opens no window. tools/release.sh runs it on the app before
# packing it, and release.yml on the DMG.
# Usage: tools/start-signed.sh <app or dmg>
set -euo pipefail
target=$1
mounted=
detach() {
  [ -z "$mounted" ] || hdiutil detach -quiet "$mounted" || true
}
trap detach EXIT
case $target in
  *.dmg)
    mounted=$(mktemp -d)
    hdiutil attach -quiet -readonly -nobrowse -noautoopen -mountpoint "$mounted" "$target"
    app=$(find "$mounted" -maxdepth 1 -name '*.app' | head -1)
    [ -n "$app" ] || { echo "$target holds no app." >&2; exit 1; }
    ;;
  *) app=$target ;;
esac
binary=$app/Contents/MacOS/$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist")
keys=$(codesign -d --entitlements - --xml "$app" 2> /dev/null | plutil -convert xml1 -o - - |
  sed -n 's/^	<key>\(.*\)<\/key>$/\1/p' | tr '\n' ' ')
profile=none
if [ -f "$app/Contents/embedded.provisionprofile" ]; then
  profile=$(security cms -D -i "$app/Contents/embedded.provisionprofile" 2> /dev/null |
    plutil -extract Name raw -o - - 2> /dev/null || echo unreadable)
fi

# A minute is far more than its first line needs; past it the app has gone on, so this build
# has no BROWSER_PREWARM.
started=$(date +%s)
status=0
BROWSER_PREWARM=1 perl -e 'alarm shift; exec @ARGV or die "$!\n"' 60 "$binary" < /dev/null || status=$?
if [ "$status" != 0 ]; then
  case $status in
    137) why="killed as it started, which is how macOS refuses a profile or a key" ;;
    142) why="still running after a minute, so it went past BROWSER_PREWARM" ;;
    *) why="exit $status" ;;
  esac
  echo "macOS didn't start $app: $why." >&2
  # What macOS said, which names the key or the profile it refused.
  log show --style compact --start "$(date -r "$started" '+%Y-%m-%d %H:%M:%S')" \
    --predicate 'process == "kernel" AND eventMessage CONTAINS[c] "AMFI" OR process == "amfid"' \
    2> /dev/null | tail -20 >&2 || true
  exit 1
fi
echo "macOS started $app, with the profile \"$profile\" and the keys ${keys% }, and it ended at its first line."
