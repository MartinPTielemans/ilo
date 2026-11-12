# #2138, not for merge: does ilo load every profile at launch, and what does each cost? For 1, 2 and
# 4 profiles, each with a Space, the last session's tab in the first profile's Space alone: three
# launches each, measured 30 s after launch, with the page restored. Each says the profiles whose
# files the browser's processes hold open, the browser process's memory and the whole tree's, the
# processes by kind, and launch to the window on screen. In the last, the second profile's Space is
# shown once (⌃2), and 30 s on it's measured again. Then a fourth launch, traced with Chromium's
# browser and startup categories, says each profile's loading spans. Results in
# profile-cost/results.txt.

if [ "$(uname)" != Linux ]; then
  echo "skip: profile-cost. #2138's profile numbers are Linux's."
  return 0
fi
serve
pc_dir="$out/profile-cost"
rm -rf "$pc_dir"
mkdir -p "$pc_dir"
printf '%s' '<!doctype html><meta charset=utf-8><title>loading</title><p>profile cost page</p>
<script>
const id = location.search.slice(1);
new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    if (entry.name == "first-contentful-paint") {
      document.title = id + " fcp " + Math.round(Date.now() - performance.now() + entry.startTime);
    }
  }
}).observe({ type: "paint", buffered: true });
</script>' > "$out/site/pc.html"
page="${site}pc.html"
results="$pc_dir/results.txt"
: > "$results"
say() {
  echo "profile-cost: $*"
  echo "$*" >> "$results"
}

# Launches the app on the user data dir $1, logging to $2.log, with Chromium's switches $3, and
# the page $4 if any. Sets $pid, $log and $launched (Unix ms).
pc_launch() {
  log="$pc_dir/$2.log"
  step=0
  launched=$(date +%s%3N)
  if [ -n "${4:-}" ]; then
    # shellcheck disable=SC2086 # the switches, a word each
    BROWSER_DRIVE=1 BROWSER_URL="$4" BROWSER_PROFILE_DIR="$1" "$browser" $3 \
      < "$out/drive" > "$log" 2>&1 &
  else
    # shellcheck disable=SC2086 # the switches, a word each
    BROWSER_DRIVE=1 BROWSER_PROFILE_DIR="$1" "$browser" $3 < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
}
pc_quit() {
  exec 3>&-
  quit
}
# Waits for page $1 to paint after the launch.
pc_painted() {
  for _ in $(seq 300); do
    fcp=$(sed -n "s/^browser: title [0-9]* $1 fcp //p" "$log" | sort -n | tail -1)
    [ "${fcp:-0}" -le "$launched" ] || return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "page $1 didn't paint"
}
# Waits until $1 s after the launch.
pc_until() {
  while [ $(($(date +%s%3N) - launched)) -lt $(($1 * 1000)) ]; do
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
}
# Measures the app as $1, for the user data dir $2.
pc_measure() {
  snap="$pc_dir/$1.json"
  python3 -I check/usage.py snap "$procs" "$snap" kinds || fail "the usage didn't read"
  browser_mib=$(python3 -I -c 'import json, sys
procs = json.load(open(sys.argv[1]))["procs"]
print("%.1f" % (sum(p[3] for p in procs.values() if p[0] == "browser") / 1048576))' "$snap")
  say "$1: memory $(python3 -I check/usage.py memory "$snap" | awk '{ print $1 " MiB in " $2 " processes" }'), browser process $browser_mib MiB"
  say "$1: by kind, MiB: $(python3 -I check/usage.py kinds "$snap")"
  say "$1: $(python3 -I check/profile_cost.py kinds "$snap")"
  python3 -I check/profile_cost.py loaded "$snap" "$2" | while read -r line; do
    say "$1: open files in $line"
  done
}

for n in 1 2 4; do
  dir="$PWD/$pc_dir/data-$n"
  count_off "$dir"
  # The last session: one tab, in the first profile's Space.
  pc_launch "$dir" "setup-$n" "" "$page?setup$n"
  pc_painted "setup$n"
  sleep 3
  pc_quit
  [ -s "$dir/Spaces.json" ] || fail "the setup launch saved no Spaces"
  python3 -I check/profile_cost.py spaces "$dir/Spaces.json" "$n" || fail "the Spaces didn't take more profiles"
  # Makes the new profiles, with what a new profile gets, as a user's are made before a launch.
  pc_launch "$dir" "warm-$n" ""
  pc_until 30
  say "$n profiles, warm launch: $(grep -c '^browser: added profile' "$log") added, Spaces $(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3-)"
  pc_quit
  for round in 1 2 3; do
    name="p$n-r$round"
    pc_launch "$dir" "$name" ""
    pc_painted "setup$n"
    pc_until 30
    on_screen=$(sed -n 's/^browser: window on screen at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
    say "$name: window on screen at $on_screen ms, page painted $((fcp - launched)) ms after the launch"
    pc_measure "$name" "$dir"
    if [ "$round" = 3 ] && [ "$n" -gt 1 ]; then
      before=$(grep -c '^browser: space [0-9]* in' "$log")
      sent=$(date +%s%3N)
      send key 2 ctrl
      for _ in $(seq 100); do
        [ "$(grep -c '^browser: space [0-9]* in' "$log")" -gt "$before" ] && break
        sleep 0.02
      done
      say "$name: ⌃2 to the second profile's Space shown in about $(($(date +%s%3N) - sent)) ms ($(grep '^browser: space [0-9]* in' "$log" | tail -1))"
      launched=$(date +%s%3N)
      pc_until 30
      pc_measure "$name-shown2" "$dir"
    fi
    pc_quit
  done
  pc_launch "$dir" "trace-$n" "--enable-tracing=browser,startup --enable-tracing-format=json --trace-startup-file=$PWD/$pc_dir/trace-$n.json"
  pc_painted "setup$n"
  pc_until 20
  pc_quit
  python3 -I check/profile_cost.py trace "$pc_dir/trace-$n.json" > "$pc_dir/trace-$n.txt" ||
    echo "profile-cost: trace $n didn't read"
  say "trace $n profiles:"
  sed 's/^/  /' "$pc_dir/trace-$n.txt" >> "$results"
  cat "$pc_dir/trace-$n.txt"
done
echo "ok: profile-cost measured, in $results"
