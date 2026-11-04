# An idle window off Battery Saver isn't woken each minute to measure its pages for freezing
# (#686, engine/patches/ours/chromium-freezing-measure-on-battery-saver.patch). Chrome's freezing
# policy measures pages' CPU and memory on a 60 s timer in resource_attribution's queries.cc, which
# Chromium's startup trace records as a task with a 60 s delay. The window sits on about:blank for
# 75 s, past the first such wake, and its trace must have none; with Battery Saver on, through the
# Local State pref Chrome keeps it in (3 is On), the trace must have one, so the trace would have
# shown it. A profile of the run's own keeps the pref from the runs after it.

profile_dir="$PWD/$out/idle/profile"
# The minute measurements a trace has, tasks that queries.cc's timer posted 60 s ahead, then every
# timer of queries.cc by its delay, for the line.
minute_wakes() {
  python3 -I - "$1" << 'PY'
import collections, json, sys
with open(sys.argv[1]) as trace:
    events = json.load(trace)
events = events["traceEvents"] if isinstance(events, dict) else events
delays = collections.Counter()
for event in events:
    args = event.get("args") or {}
    if event.get("name") != "ThreadControllerImpl::RunTask" or args.get("src_func") != "StartTimer":
        continue
    if not str(args.get("src_file", "")).endswith("resource_attribution/queries.cc"):
        continue
    delays[(args.get("chrome_task_annotator") or {}).get("task_delay_us", 0) // 1000000] += 1
print(delays[60], " ".join(f"{n}x{s}s" for s, n in sorted(delays.items())) or "none")
PY
}
# Runs the app idle for 75 s with the startup trace, and sets $wakes and $timers from its trace.
# $1 names it.
idle_trace() {
  trace="$PWD/$out/idle/$1.json"
  log="$out/idle-$1.log"
  rm -f "$trace"
  BROWSER_URL=about:blank BROWSER_PROFILE_DIR="$profile_dir" "$browser" --trace-startup=toplevel \
    --trace-startup-format=json --trace-startup-duration=75 --trace-startup-file="$trace" \
    < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  # Chromium writes the trace once its 75 s are up.
  for _ in $(seq 150); do
    python3 -I -c 'import json, sys; json.load(open(sys.argv[1]))' "$trace" 2> /dev/null && break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
  [ -s "$trace" ] || fail "the $1 window wrote no trace"
  quit
  counted=$(minute_wakes "$trace") || fail "the $1 window's trace didn't read as JSON"
  wakes=${counted%% *}
  timers=${counted#* }
}

rm -rf "$out/idle"
mkdir -p "$profile_dir"
idle_trace off
off=$wakes off_timers=$timers
[ "$off" = 0 ] ||
  fail "idle off Battery Saver, the window was woken $off times in 75 s to measure its pages ($off_timers)"
python3 -I - "$profile_dir/Local State" << 'PY'
import json, os, sys
local = {}
if os.path.exists(sys.argv[1]):
    with open(sys.argv[1]) as f:
        local = json.load(f)
local.setdefault("performance_tuning", {}).setdefault("battery_saver_mode", {})["state"] = 3
with open(sys.argv[1], "w") as f:
    json.dump(local, f)
PY
idle_trace on
on=$wakes on_timers=$timers
[ "$on" -ge 1 ] ||
  fail "with Battery Saver on, the trace had no minute measurement ($on_timers), so it can't show one's absence"
rm -rf "$profile_dir"
echo "ok: idle for 75 s off Battery Saver the window was woken 0 times to measure its pages" \
  "(queries.cc's timers: $off_timers); with Battery Saver on, $on times ($on_timers)." \
  "Traces: $out/idle/off.json, $out/idle/on.json"
