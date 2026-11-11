#!/bin/sh
# Downloads Chrome for Testing for this Mac into ~/.cache/ilo-bench, once, and prints its binary's
# path, which perf and bench run beside the app as CHECK_CHROME (app/check/perf.sh, bench.sh). It's
# Google's build for automation: its own bundle ID and profile, no auto-update, and nothing of the
# owner's Chrome, which the checks never run. By default it's the newest of the engine's milestone
# (engine/pins.ini), so the two browsers differ in their UI and not their Chromium; "stable" takes
# Chrome's stable channel, and a milestone such as 157 its newest.
# Usage: tools/chrome-for-testing.sh [stable | <milestone>]
set -e
[ "$(uname)" = Darwin ] || { echo "chrome-for-testing: the Mac's alone" >&2; exit 1; }
cd "$(dirname "$0")/.."
case "$(uname -m)" in
arm64) platform=mac-arm64 ;;
*) platform=mac-x64 ;;
esac
wanted=${1:-$(sed -n 's/^[[:space:]]*version = \([0-9]*\)\..*/\1/p' engine/pins.ini | head -1)}
feed=https://googlechromelabs.github.io/chrome-for-testing
if [ "$wanted" = stable ]; then
  json=$(curl -sSf "$feed/last-known-good-versions-with-downloads.json")
else
  json=$(curl -sSf "$feed/latest-versions-per-milestone-with-downloads.json")
fi
# The version and its zip's address, a line each.
found=$(printf '%s' "$json" | python3 -I -c '
import json, sys
wanted, platform = sys.argv[1:]
feed = json.load(sys.stdin)
entry = feed["channels"]["Stable"] if wanted == "stable" else feed["milestones"].get(wanted)
if not entry:
    sys.exit("chrome-for-testing: no Chrome for Testing for milestone %s" % wanted)
print(entry["version"])
print(next(d["url"] for d in entry["downloads"]["chrome"] if d["platform"] == platform))
' "$wanted" "$platform")
version=$(echo "$found" | sed -n 1p)
url=$(echo "$found" | sed -n 2p)
dir="${XDG_CACHE_HOME:-$HOME/.cache}/ilo-bench/chrome-for-testing-$version"
binary="$dir/chrome-$platform/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
if [ ! -x "$binary" ]; then
  rm -rf "$dir" "$dir.part"
  mkdir -p "$dir.part"
  curl -sSfL -o "$dir.part/chrome.zip" "$url"
  # ditto keeps the app's signature and its frameworks' links, where unzip may not.
  ditto -x -k "$dir.part/chrome.zip" "$dir.part"
  rm "$dir.part/chrome.zip"
  mv "$dir.part" "$dir"
fi
[ "$(defaults read "${binary%/MacOS/*}/Info" CFBundleIdentifier)" = com.google.chrome.for.testing ] ||
  { echo "chrome-for-testing: $binary isn't Chrome for Testing" >&2; exit 1; }
echo "$binary"
