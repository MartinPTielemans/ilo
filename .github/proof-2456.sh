#!/bin/bash
# Proof only (#2456): the released ilo, main's ilo and Google Chrome, one after the other on this
# runner and so from the same address, each with a new profile: our own page asking to share the
# screen with no click, a budgeting app's sign-in page, and a store's guest checkout up to its first
# form. Usage: proof-2456.sh MAIN_APP, the folder of main's build/Browser.
set -u
out=$RUNNER_TEMP/proof
mkdir -p "$out"
here=$(cd "$(dirname "$0")" && pwd)
port=9456

site=$(mktemp -d)
cat > "$site/index.html" << 'EOF'
<!doctype html><title>loading</title><h1>Share probe</h1>
<script>
document.title = "asking";
navigator.mediaDevices.getDisplayMedia({video: true}).then(
  () => document.title = "shared",
  e => document.title = "refused " + e.name + ": " + e.message);
setTimeout(() => { if (document.title == "asking") document.title = "pending: no answer in 3 s"; }, 3000);
</script>
EOF
python3 -I -m http.server 8456 --bind 127.0.0.1 --directory "$site" > "$out/server.log" 2>&1 &
own=http://127.0.0.1:8456/

openbox > /dev/null 2>&1 &

# Runs the browser the arguments name, with DevTools on $port, through proof-2456.py.
try() {
  label=$1
  shift
  "$@" > "$out/$label.log" 2>&1 &
  pid=$!
  timeout 900 python3 -I "$here/proof-2456.py" "$port" "$out" "$label" "$own" || echo "$label: driver exit $?" | tee -a "$out/summary"
  kill "$pid" 2> /dev/null
  flatpak kill app.ilo.browser 2> /dev/null
  sleep 3
  pkill -f "remote-debugging-port=$port" 2> /dev/null
  sleep 2
}

try released flatpak run --env=BROWSER_URL=$own app.ilo.browser --remote-debugging-port=$port
flatpak info --user app.ilo.browser | sed -n 's/^ *\(Version\|Commit\):/released \1:/p' | tee -a "$out/summary"
profile=$(mktemp -d)
BROWSER_URL=$own BROWSER_PROFILE_DIR=$profile try main "$1/chrome" --remote-debugging-port=$port
chrome_profile=$(mktemp -d)
try chrome /opt/google/chrome/chrome --user-data-dir="$chrome_profile" --no-first-run \
  --no-default-browser-check --remote-debugging-port=$port "$own"
/opt/google/chrome/chrome --version | tee -a "$out/summary"
exit 0
