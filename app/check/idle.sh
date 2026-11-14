# check.sh runs this at 570.
# An idle window off Battery Saver is never woken to measure its pages (#686, #1773). Chrome
# measures them on timers in resource_attribution's queries.cc, which Chromium's startup trace
# records as tasks with their delay: its freezing policy each minute, which now measures only on
# Battery Saver (engine/patches/ours/chromium-freezing-measure-on-battery-saver.patch), and its page
# resource monitor every 2 minutes for UKM nothing reads, which the engine leaves out
# (chromium-no-page-resource-monitor.patch). The window sits on about:blank for 130 s, past each
# one's first wake, and its trace must have no queries.cc timer but one: Chrome's process metrics,
# every 2 minutes too, stay for now, as its urgent discarding on the Mac and Windows reads them
# (#1773). The trace can't tell the two 2-minute timers apart, so a second fails. With Battery Saver
# on, through the Local State pref Chrome keeps it in (3 is On), a trace as long must have the freezing
# policy's, so the trace would have shown one. A profile of the run's own keeps the pref from the
# runs after it. It says how much CPU the browser's processes take idle, in the minute after the trace.

profile_dir="$PWD/$out/idle/profile"
# The tasks of queries.cc's timers a trace has: those posted 60 s ahead, those but the process
# metrics' one 2-minute timer, then all of them by their delay, for the line.
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
# The process metrics' timer, which stays (#1773), isn't counted among them.
others = sum(delays.values()) - min(delays[120], 1)
print(delays[60], others, " ".join(f"{n}x{s}s" for s, n in sorted(delays.items())) or "none")
PY
}
# On Linux, the CPU each of the browser's processes has taken, in clock ticks, by its --type, the
# browser's own as "browser", a line each: "PID TICKS TYPE".
cpu_each() {
  for p in $(pgrep -f -- "$procs"); do
    # Chromium sets its helpers' titles, so their arguments may be one string.
    kind=$(tr '\0' ' ' < "/proc/$p/cmdline" 2> /dev/null | grep -o -e '--type=[^ ]*' | head -1)
    kind=${kind#--type=}
    sed 's/^.*) //' "/proc/$p/stat" 2> /dev/null | awk -v p="$p" -v k="${kind:-browser}" '{ print p, $12 + $13, k }'
  done
}
# Runs the app idle for $2 seconds with the startup trace, and sets $wakes (the minute timer's),
# $all (all but the process metrics') and $timers from its trace, and $idle_cpu. $1 names it.
idle_trace() {
  trace="$PWD/$out/idle/$1.json"
  log="$out/idle-$1.log"
  rm -f "$trace"
  BROWSER_URL=about:blank BROWSER_PROFILE_DIR="$profile_dir" "$browser" --trace-startup=toplevel \
    --trace-startup-format=json --trace-startup-duration="$2" --trace-startup-file="$trace" \
    < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  # Chromium writes the trace once its time is up.
  for _ in $(seq $(($2 + 75))); do
    python3 -I -c 'import json, sys; json.load(open(sys.argv[1]))' "$trace" 2> /dev/null && break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
  [ -s "$trace" ] || fail "the $1 window wrote no trace"
  # The CPU it takes idle, over the minute after, as the trace's recording takes CPU of its own.
  idle_from="$(cpu) $(date +%s)"
  [ ! -d /proc ] || windows || cpu_each > "$out/idle/$1-cpu-from"
  sleep 60 # the idle minute measured
  kill -0 "$pid" 2> /dev/null || gone
  # Where it went, by process kind, the most first.
  idle_where=
  if [ -d /proc ] && ! windows; then
    idle_where=$(cpu_each | awk -v hz="$(getconf CLK_TCK)" 'NR == FNR { from[$1] = $2; next }
      { took[$3] += $2 - from[$1] } END { for (k in took) if (took[k] > 0) printf "%.2f %s\n", took[k] / hz, k }' \
      "$out/idle/$1-cpu-from" - | sort -rn | awk '{ printf "%s%s %s s", (NR > 1 ? ", " : ""), $2, $1 }')
  fi
  idle_cpu=$(echo "$idle_from $(cpu) $(date +%s)" | awk '{
    printf "%.2f s of CPU in %d s (%.2f%% of one core), %d processes", $4 - $1, $6 - $3,
      ($4 - $1) * 100 / ($6 - $3), $5 }')
  quit
  counted=$(minute_wakes "$trace") || fail "the $1 window's trace didn't read as JSON"
  read -r wakes all timers << COUNTED
$counted
COUNTED
}

rm -rf "$out/idle"
mkdir -p "$profile_dir"
idle_trace off 130
off=$all off_timers=$timers off_cpu=$idle_cpu
echo "idle: off Battery Saver, the browser's processes took ${off_cpu:-no measured CPU}${idle_where:+ ($idle_where)}"
[ "$off" = 0 ] ||
  fail "idle off Battery Saver, the window was woken $off times in 130 s to measure its pages ($off_timers)"
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
idle_trace on 130
on=$wakes on_timers=$timers on_cpu=$idle_cpu
echo "idle: on Battery Saver, the browser's processes took ${on_cpu:-no measured CPU}${idle_where:+ ($idle_where)}"
[ "$on" -ge 1 ] ||
  fail "with Battery Saver on, the trace had no minute measurement ($on_timers), so it can't show one's absence"
rm -rf "$profile_dir"
echo "ok: idle for 130 s off Battery Saver the window was woken 0 times to measure its pages, but" \
  "for Chrome's process metrics" \
  "(queries.cc's timers: $off_timers), taking ${off_cpu:-no measured CPU}; with Battery Saver on," \
  "$on times ($on_timers), taking ${on_cpu:-no measured CPU}." \
  "Traces: $out/idle/off.json, $out/idle/on.json"
