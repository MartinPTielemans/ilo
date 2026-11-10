#!/bin/bash
# Runs the spike on the app check.sh runs, app/build/Browser, which app/setup.sh and app/build.sh
# make on the engine fetch.sh downloads, with its sandbox on, under Xvfb and openbox. It leaves
# results.json and chrome.log in OUT (default ./out).
# Usage: run.sh [OUT]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/out}
chrome=$here/../../app/build/Browser/chrome
if [ ! -x "$chrome" ]; then
  echo "run.sh: build the app first: cd app && ./setup.sh && ./build.sh" >&2
  exit 1
fi
# shellcheck disable=SC2016 # expanded by the session's shell
xvfb-run -a -s "-screen 0 1280x800x24" sh -c 'openbox & python3 -I "$1" "$2" "$3"' \
  sh "$here/spike.py" "$chrome" "$out"
