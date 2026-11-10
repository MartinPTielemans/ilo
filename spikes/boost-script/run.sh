#!/bin/bash
# Runs the spike on the Linux engine fetch.sh downloads, the newest one in this checkout's history,
# with its sandbox on, and leaves results.json and chrome.log in OUT (default ./out).
# Usage: run.sh [OUT]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$here/out}
engine=$("$here/../../engine/fetch.sh")
python3 -I "$here/spike.py" "$engine/chrome" "$out"
