# check.sh runs this at 330.
# A crash leaves a minidump in the profile's Crashpad folder: --crash-after-start crashes the
# browser once it has started, Crashpad's handler is up and the profile's Preferences say its
# session crashed, the handler writes the report, and no Chromium process outlives the crash, the
# handler included. Nor does one outlive a quit.
# A new profile's first launch offers Send Crash Reports at its onboarding's opt-ins step, as the
# run names a collector, check/collector.py, where a prod build alone names the real one. Left off
# there with Esc, nothing goes to the collector: the launch after a crash offers to send that
# crash's report in a toast, and keeps it. Turned on at a new first launch's step, the report goes
# at once, the minidump and Sentry's field saying to keep no IP address, without cookies and with
# no annotation naming the profile's folder or the home, and is deleted once the collector has it.
# One the collector refuses waits for the next launch, and turning the switch off in Settings ›
# Privacy deletes it, which only the Mac's run clicks so far.
# On Windows it runs on the folder check/git-bash.sh gives chrome.exe, as only the prod build's
# lines up by itself.

crashpad="$(dirname "$saved")/Crashpad"
rm -rf "$crashpad"
rm -f "$saved" "$launch_frame"
# The profile's session ended cleanly, as a quit leaves it, so the offer after the crash comes from
# the mark the crash test's launch writes as it starts, not one an earlier try left.
python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    prefs = json.load(f)
prefs.setdefault("profile", {})["exit_type"] = "Normal"
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(prefs, f)
' "$prefs" || fail "the session couldn't be marked clean in $prefs"
# What no annotation may have: the switch that gives the profile, its folder and the home, which
# have the user's name, as Windows writes them there.
profile=$(dirname "$saved")
home=$HOME
! windows || { profile=$(cygpath -w "$profile") && home=$(cygpath -w "$HOME"); }
python3 -I check/collector.py "$out/collector-port" --user-data-dir "$profile" "$home" \
  > "$out/collector.log" 2>&1 &
collector=$!
for _ in $(seq 50); do
  [ -s "$out/collector-port" ] && break
  sleep 0.1
done
[ -s "$out/collector-port" ] || fail "the crash collector didn't start"
collector_url="http://127.0.0.1:$(cat "$out/collector-port")"
reports="$collector_url/api/1/minidump/?sentry_key=check"
page='data:text/html,<title>crash</title>'
# The reports waiting to be sent: Crashpad's pending folder, or on Windows its reports folder, which
# holds them all, and only unsent ones as its handler sends none.
waiting="$crashpad/pending"
! windows || waiting="$crashpad/reports"
pending() {
  find "$waiting" -name '*.dmp' 2>/dev/null | wc -l | tr -d ' '
}
# Waits up to 10 s for $1 reports to be waiting.
await_pending() {
  for _ in $(seq 20); do
    [ "$(pending)" = "$1" ] && return 0
    sleep 0.5
  done
  return 1
}
# Crashes the browser with the crash test, with $1 as the collector, and waits for the report. It
# logs to its own file, which keeps the first launches' logs.
crash() {
  log="$out/crash.log"
  "$browser" --crash-after-start "--crash-upload-url=$1" < /dev/null > "$log" 2>&1 &
  pid=$!
  await '^browser: crash test$' || fail "the app didn't start its crash test"
  for _ in $(seq 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill -0 "$pid" 2>/dev/null && fail "the app didn't crash"
  status=0
  wait "$pid" || status=$?
  [ "$status" -ne 0 ] || fail "the app exited cleanly from its crash test"
  # The handler writes the report as the browser dies, and may still be at it.
  await_pending 1 || fail "Crashpad wrote no minidump in $waiting"
  dump=$(find "$waiting" -name '*.dmp' | head -1)
  head -c 4 "$dump" | grep -q '^MDMP$' || fail "$dump isn't a minidump"
  for _ in $(seq 20); do
    pgrep -f "$procs" > /dev/null || break
    sleep 0.5
  done
  pgrep -f "$app/.*chrome_crashpad_handler" > /dev/null &&
    fail "Crashpad's handler outlived the crash"
  pgrep -f "$procs" > /dev/null && fail "Chromium processes outlived the crash"
  return 0
}
# Launches the browser driven, with $1 as the collector, and waits for its page.
launch() {
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" "--crash-upload-url=$1" < "$out/drive" \
    > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
}
# Launches a new profile, whose saved Spaces are gone, logging to $1, and goes on through its
# onboarding to the opt-ins step, which offers crash reports as a collector is named. Its import
# home is empty, so the import never lists this desk's browsers and leaves its step out.
mkdir -p "$out/crash-home"
import_home="$PWD/$out/crash-home"
! windows || import_home=$(cygpath -w "$import_home")
first_launch() {
  log=$1
  step=0
  rm -f "$saved"
  BROWSER_DRIVE=1 BROWSER_ONBOARDING=light BROWSER_IMPORT_HOME="$import_home" \
    "$browser" "--crash-upload-url=$reports" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: onboarding step intro settled$' || fail "the first launch didn't play the intro"
  steps=$(grep -m1 '^browser: onboarding steps ' "$log" | cut -d' ' -f4-)
  case "$steps" in
    *" opt-ins") ;;
    *) fail "the first launch's steps were '$steps', with no opt-ins step for crash reports" ;;
  esac
  for each in $steps; do
    case $each in
      intro) send key a && how=key ;;
      opt-ins) break ;;
      *) send key escape && how=escape ;;
    esac
    expect "^browser: onboarding left $each via $how\$" "Esc didn't leave the first launch's $each"
  done
  expect '^browser: onboarding step opt-ins settled$' "the first launch's opt-ins step didn't come"
}

# The first launch offers crash reports, and Esc goes on with them off.
first_launch "$out/crash-first.log"
../tools/window-screenshot.sh "$pid" "$out/crash-opt-ins.png" || fail "no window to capture"
send key escape
expect '^browser: onboarding left opt-ins via escape$' "Esc didn't leave the opt-ins step"
exec 3>&-
quit

crash "$reports"
bytes=$(wc -c < "$dump" | tr -d ' ')

# Off, as it starts, the launch after the crash offers to send its report, and sends nothing.
log="$out/crash-off.log"
launch "$reports"
await '^browser: crash report offered$' || fail "the launch after the crash didn't offer its report"
await "^browser: toast shown $app_name quit unexpectedly\. \[send, don't send\]\$" ||
  fail "the offer's toast didn't show"
await '^browser: toast settled$' || fail "the offer's toast didn't settle"
../tools/window-screenshot.sh "$pid" "$out/crash-offer.png" || fail "no window to capture"
# Whole, its plate inside the toast's window, as the archive's note is in the restore run (#1462).
await '^browser: toast plate .* in [0-9]* [0-9]*$' ||
  fail "the offer's toast didn't say where its plate is"
plate=$(grep '^browser: toast plate' "$log" | tail -1)
set -- $(echo "${plate#browser: toast plate }" | sed 's/ in / /')
[ "$1" -ge 0 ] && [ "$2" -ge 0 ] && [ $(($1 + $3)) -le "$5" ] && [ $(($2 + $4)) -le "$6" ] ||
  fail "the offer's toast runs past its window, which cuts it: $plate"
exec 3>&-
quit
[ -s "$out/collector.log" ] && fail "a report was sent while Send Crash Reports was off"
[ "$(pending)" = 1 ] || fail "the report didn't wait while Send Crash Reports was off"

# Turned on at a new first launch's opt-ins step, it sends the report waiting and deletes it. Their
# row takes a click on its switch, where the app says it's drawn once the step has come, as
# onboarding.sh clicks the agents' (#1286).
first_launch "$out/crash-on.log"
expect '^browser: onboarding crash-reports at ' "the opt-ins step didn't draw the crash reports' switch"
send click $(grep '^browser: onboarding crash-reports at ' "$log" | tail -1 | cut -d' ' -f5-6)
expect '^browser: onboarding crash-reports on$' \
  "a click on the crash reports' row didn't turn them on"
await '^browser: crash report sent$' || fail "the report wasn't sent once the step turned them on"
send key escape
expect '^browser: onboarding left opt-ins via escape$' "Esc didn't leave the opt-ins step"
grep -q '^refused' "$out/collector.log" &&
  fail "the collector refused the report: $(cat "$out/collector.log")"
grep -q "^report [0-9]* /api/1/minidump/?sentry_key=check\$" "$out/collector.log" ||
  fail "the collector didn't get the report"
await_pending 0 || fail "the report sent wasn't deleted"
exec 3>&-
quit

# A report the collector refuses waits, and Send Crash Reports turned off in Settings deletes it.
crash "$reports"
log="$out/crash-refused.log"
launch "$collector_url/refuse/"
await '^browser: crash report not sent$' || fail "the refused report wasn't tried"
[ "$(pending)" = 1 ] || fail "the refused report didn't wait"
send click $page_point
# The Mac's driven clicks need the app active; the other drivers' reach Settings where it drew, as
# preview's switch has them.
if [ "$(uname)" != Darwin ] || [ "$active" = 1 ]; then
  send key , cmd
  expect '^browser: settings shown General$' "⌘, didn't show Settings"
  settings_drew General
  send clickkey $(drawn "toolbar Privacy")
  expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
  settings_drew Privacy
  ../tools/window-screenshot.sh "$pid" "$out/privacy.png" || fail "no window to capture"
  click_in_sight "switch send crash reports"
  expect '^browser: send crash reports off$' "a click on the switch didn't turn crash reports off"
  expect '^browser: crash reports deleted 1$' "turning crash reports off didn't delete the report"
  [ "$(pending)" = 0 ] || fail "the report not sent outlived turning crash reports off"
  turned_off=" Turned off in Settings › Privacy, it deleted that one."
  windows="$out/crash-opt-ins.png, $out/crash-offer.png, $out/privacy.png"
else
  echo "skip: crash's Settings switch. Browser wasn't the active app."
  turned_off=
  windows="$out/crash-opt-ins.png, $out/crash-offer.png"
fi
exec 3>&-
quit
kill "$collector" 2>/dev/null || true
annotations=$(grep -m1 '^annotations ' "$out/collector.log" | cut -d' ' -f2-)
echo "ok: a new profile's first launch offered Send Crash Reports at its opt-ins step; left off" \
  "there, the crash left a minidump of $bytes bytes, which the next launch offered to send and" \
  "kept, and turned on at a new first launch's step, it was sent and deleted once the collector" \
  "had it," \
  "with Sentry's field to keep no IP address and annotations naming neither the profile nor the" \
  "home: $annotations." \
  "One the collector refused waited.$turned_off Windows: $windows"
