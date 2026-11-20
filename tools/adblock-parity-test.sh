#!/bin/bash
# Proves app/check/adblock-parity.sh's reading of a beacon that DevTools never heard end, as a page
# that sends it while it leaves gives none (#3602). The gate's own Python runs on 45 made-up sites
# against a baseline that blocked the beacon's host on the last. The beacon passes only with a
# line of the app's log for it. One line accounts for one beacon, and only for a beacon: no line,
# two beacons for one line, a line taken by a beacon DevTools heard blocked, and a request that
# isn't a beacon each leave it let through. It changes nothing here. Check
# scripts' lints job runs it.
# Usage: tools/adblock-parity-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
gate=$scratch/gate.py
# The Python between the line that hands it the pings and its PY.
awk '/"\$pings" "\$\{ADBLOCK_PARITY_BASELINE/ { on = 1; next } on && /^PY$/ { exit } on' \
  "$here/app/check/adblock-parity.sh" > "$gate"
grep -q 'blocked_by_app_log' "$gate" ||
  { echo "adblock-parity-test: didn't find the gate's Python in adblock-parity.sh"; exit 1; }
beacon=https://privacyportal.onetrust.com/request/v1/consentreceipts
failed=0

# 45 sites a second apart, each loaded, the last with the beacon's host blocked in the baseline,
# and one site whose blocker's style sheet hid something, as the gate wants one.
mkdir -p "$scratch/hidden"
: > "$scratch/starts.txt" : > "$scratch/videos.txt" : > "$scratch/contents.txt"
printf 'site\tloaded\tload_ms\trequests\tblocked\tstand_ins\tblocked_hosts\tallowed_hosts' > "$scratch/baseline.tsv"
printf '\tbot_check\tvideo\tmissed\tsplit_blocked\ttext\timages\tcover\tfailed\n' >> "$scratch/baseline.tsv"
for n in $(seq 45); do
  url=https://site$n.example/
  echo "$((n * 1000)) $url 500 $url" >> "$scratch/starts.txt"
  hosts=
  [ "$n" = 45 ] && hosts=privacyportal.onetrust.com
  printf '%s\t1\t500\t1\t1\t0\t%s\t\t0\t\t0\t\t\t\t\t\n' "$url" "$hosts" >> "$scratch/baseline.tsv"
done
echo '{"missed": [], "blocker": ["div.ad"], "page": []}' > "$scratch/hidden/https___site1_example_.json"

# Runs the gate on the requests $2 (JSON lines, each "sent" event's id its number) and the app's
# log lines $3, then expects it to pass or fail ($1).
try() {
  local want=$1 events=$2 lines=$3 what=$4 got status
  printf '%s' "$events" > "$scratch/requests.jsonl"
  printf '%s' "$lines" > "$scratch/pings.txt"
  status=0
  python3 -I "$gate" "$scratch/requests.jsonl" "$scratch/starts.txt" "$scratch/videos.txt" \
    "$scratch/hidden" "$scratch/contents.txt" "$scratch/sites.tsv" "$scratch/pings.txt" \
    "$scratch/baseline.tsv" > "$scratch/result.txt" 2>&1 || status=$?
  case $status in 0) got=pass ;; 1) got=fail ;; *) got="error ($status)" ;; esac
  if [ "$got" = "$want" ]; then
    echo "ok: $what: $got"
  else
    echo "adblock-parity-test: $what: $got, not $want. The gate said: $(cat "$scratch/result.txt")"
    failed=1
  fi
}

# A request sent on the last site's watch: $1 its id, $2 a beacon or not, then its later event.
sent() {
  local ping=
  [ "$2" = ping ] && ping=', "ping": true'
  printf '{"event": "sent", "id": "%s", "url": "%s", "at": 45050%s}\n' "$1" "$beacon" "$ping"
}
blocked() { printf '{"event": "failed", "id": "%s", "error": "net::ERR_BLOCKED_BY_CLIENT"}\n' "$1"; }
line="$beacon"$'\n'

try fail "$(sent 1 ping)"$'\n' "" "a beacon with no end and no line"
try pass "$(sent 1 ping)"$'\n' "$line" "a beacon with no end and its line"
try pass "$(sent 1 ping)"$'\n'"$(blocked 1)"$'\n' "" "a beacon DevTools heard blocked, no line"
try fail "$(sent 1 ping)"$'\n'"$(sent 2 ping)"$'\n' "$line" "two beacons with no end and one line"
try pass "$(sent 1 ping)"$'\n'"$(sent 2 ping)"$'\n' "$line$line" "two beacons with no end, two lines"
# The host's other beacon, heard blocked, excuses it as main's reading does, so the gate passes;
# the line still goes to that one, leaving the beacon with no end let through in the table.
try pass "$(sent 1 ping)"$'\n'"$(blocked 1)"$'\n'"$(sent 2 ping)"$'\n' "$line" \
  "a line taken by the beacon DevTools heard blocked, another with no end"
row=$(tail -1 "$scratch/sites.tsv")
if [ -n "$(printf '%s' "$row" | cut -f17)" ] ||
  [ "$(printf '%s' "$row" | cut -f8)" != privacyportal.onetrust.com ]; then
  echo "adblock-parity-test: the beacon with no end isn't left let through: $row"
  failed=1
fi
try fail "$(sent 1 fetch)"$'\n' "$line" "a request that isn't a beacon, with no end and a line"
# The table names the one the log accounts for apart from those DevTools heard blocked.
try pass "$(sent 1 ping)"$'\n' "$line" "the table's column"
if ! tail -1 "$scratch/sites.tsv" | grep -q "	privacyportal.onetrust.com/request/v1/consentreceipts$"; then
  echo "adblock-parity-test: the last site's blocked_by_app_log doesn't name the beacon:" \
    "$(tail -1 "$scratch/sites.tsv")"
  failed=1
fi
grep -q "blocked by the app's log alone" "$scratch/result.txt" ||
  { echo "adblock-parity-test: the summary doesn't count it apart: $(cat "$scratch/result.txt")"; failed=1; }
exit $failed
