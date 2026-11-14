# check.sh runs this at 540.
# Chrome's Battery Saver reads the computer's own battery as the OS tells of its changes, and never
# wakes an idle browser to look (core/platform/battery.h). On Linux, where Chromium reads none, the
# core reads UPower's display device on its PropertiesChanged signal (#1771): the run's stand-in
# UPower, check/upower-server.py on a bus of the run's own that the app takes for the system bus,
# says 50% discharging, then 19%, and Battery Saver turns on; charging, it turns off. A change to a
# property Battery Saver doesn't read, such as the discharge rate, isn't read again for, so the app
# reads the device once at launch and once for each of those two changes. On Windows, where
# Chromium reads the battery each minute, the core samples on Windows' power-setting notices
# (#1772): Windows says the power source and charge once as they're registered, and a startup trace
# of the window idle for 70 s, past the minute, has no task from Chromium's timer
# (timer_sampling_event_source.cc). The Mac's own sampler already waits for IOKit's notices.

log="$out/battery-os.log"
step=0
# The tasks a trace has from Chromium's battery timer, then all its tasks.
timer_tasks() {
  python3 -I - "$1" << 'PY'
import json, sys
with open(sys.argv[1]) as trace:
    events = json.load(trace)
events = events["traceEvents"] if isinstance(events, dict) else events
tasks = [event for event in events if event.get("name") == "ThreadControllerImpl::RunTask"]
timer = [task for task in tasks if str((task.get("args") or {}).get("src_file", "")).endswith(
    "timer_sampling_event_source.cc")]
print(len(timer), len(tasks))
PY
}
if windows && ! engine_has ../core/platform/win/battery.cc ../core/battery_saver.cc; then
  echo "skip: battery-os. The engine was built before the core sampled on Windows' notices."
elif windows; then
  mkdir -p "$out/battery-os"
  trace="$PWD/$out/battery-os/trace.json"
  rm -f "$trace" "$saved" "$launch_frame"
  BROWSER_URL=about:blank "$browser" --trace-startup=toplevel --trace-startup-format=json \
    --trace-startup-duration=70 --trace-startup-file="$(app_path "$trace")" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  await "browser: battery sampled on Windows' notice\$" ||
    fail "the battery wasn't sampled on Windows' notice"
  # Chromium writes the trace once its time is up.
  for _ in $(seq 145); do
    python3 -I -c 'import json, sys; json.load(open(sys.argv[1]))' "$(app_path "$trace")" \
      2> /dev/null && break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
  [ -s "$trace" ] || fail "the window wrote no trace"
  quit
  counted=$(timer_tasks "$(app_path "$trace")") || fail "the trace didn't read as JSON"
  timer=${counted% *} tasks=${counted#* }
  [ "$tasks" -gt 0 ] || fail "the trace had no tasks, so it can't show the timer's absence"
  [ "$timer" = 0 ] ||
    fail "idle for 70 s the window was woken $timer times by Chromium's battery timer"
  echo "ok: on Windows the battery was sampled on Windows' notice, and idle for 70 s the window" \
    "was woken 0 times by Chromium's battery timer, among $tasks tasks." \
    "Trace: $out/battery-os/trace.json"
elif [ "$(uname)" != Linux ]; then
  echo "skip: battery-os. The Mac's own sampler already reads the battery on IOKit's notices."
elif ! command -v dbus-daemon > /dev/null ||
  ! python3 -I -c 'from gi.repository import Gio' 2> /dev/null; then
  echo "skip: battery-os, which needs dbus-daemon and python3-gi for its UPower."
elif ! engine_has ../core/platform/linux/upower.cc ../core/battery_saver.cc; then
  echo "skip: battery-os. The engine was built before the core read UPower."
else
  started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
  upower_bus=$(echo "$started" | cut -d' ' -f1)
  buses=$(echo "$started" | cut -d' ' -f2)
  [ -n "$upower_bus" ] || fail "dbus-daemon didn't start the run's bus"
  upower="$out/upower-server.log"
  rm -f "$out/upower-in"
  mkfifo "$out/upower-in"
  DBUS_SYSTEM_BUS_ADDRESS=$upower_bus python3 -I check/upower-server.py < "$out/upower-in" \
    > "$upower" 2>&1 &
  buses="$buses $!"
  exec 6> "$out/upower-in"
  for _ in $(seq 50); do
    grep -q '^ready$' "$upower" && break
    sleep 0.1
  done
  grep -q '^ready$' "$upower" || fail "the stand-in UPower didn't take its name: $(cat "$upower")"
  upower_reads() {
    grep -c '^read$' "$upower" || true
  }

  rm -f "$saved" "$launch_frame"
  DBUS_SYSTEM_BUS_ADDRESS=$upower_bus BROWSER_URL=about:blank "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  await 'browser: UPower says 50% on battery$' || fail "the app didn't read UPower's 50% on battery"
  ! grep -q 'browser: battery saver on' "$log" || fail "Battery Saver turned on at 50%"
  launched=$(upower_reads)
  echo "rate 3" >&6
  echo "set 19 2" >&6
  await 'browser: UPower says 19% on battery$' || fail "the app didn't read UPower's 19% on battery"
  await 'browser: battery saver on at 19%$' || fail "Battery Saver didn't turn on at 19% on battery"
  echo "set 19 1" >&6
  await 'browser: UPower says 19% plugged in$' || fail "the app didn't read UPower's charging"
  await 'browser: battery saver off$' || fail "Battery Saver stayed on, charging"
  quit
  exec 6>&-
  kill $buses 2> /dev/null || true
  [ "$launched" = 1 ] && [ "$(upower_reads)" = 3 ] ||
    fail "the app read UPower's device $launched times at launch and $(upower_reads) in all, not 1 and 3"
  echo "ok: on Linux the app read UPower's display device at launch and on its two changes Battery" \
    "Saver reads, not the rate's: at 50% discharging Battery Saver stayed off, at 19% it turned on," \
    "and charging it turned off."
fi
