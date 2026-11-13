# check.sh runs this at 860, only when named.
# Only when named, as it takes 12 minutes.
# What each profile costs at launch (#2138, #2500), for 1, 2 and 4 profiles, each with a Space, the
# last session's tab in the first profile's Space and a Today tab in the second's. Two launches
# each, measured 30 s and 60 s after launch, with the page restored (uBlock Origin's process is
# still settling at 30 s): the profiles whose files the browser's processes hold open, the browser
# process's memory and the whole tree's, the processes by kind, launch to the window on screen, and
# when each profile the window doesn't show arrived. In the last, the second profile's Space is
# shown once (⌃2), and 30 s on it's measured again, then the first Space is shown again (⌃1) so the
# next launch restores it. Then a launch traced with Chromium's browser and startup categories says
# each profile's loading spans. Results in profile-cost/results.txt.
#
# What it checks, with the core of #2500: the window comes on screen before the profiles of the
# Spaces it doesn't show have loaded, and each of them arrives after. An extension still starts in
# each profile, its Space never shown, as each profile's uBlock Origin does. ⌃2 sent the moment the
# window is on screen shows the second profile's Space, while its profile may still be loading, and
# its tab's page paints once it's in; what that first switch costs is in the results. With 4
# profiles, a quit the moment the window is on screen, during the late loads, ends clean. Linux's
# numbers only, so it runs only when named.

if [ "$(uname)" != Linux ]; then
  echo "skip: profile-cost. The profiles' launch numbers are Linux's."
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
# An engine without #2500's core loads every profile before the window, and logs no arrivals.
late_loads=
if engine_has ../core/session.cc ../core/profiles.cc ../core/tab.cc ../core/window.cc; then
  late_loads=1
fi

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
# Waits for page $1 to paint after $2 (Unix ms, the launch's by default), and sets $fcp to when.
pc_painted() {
  for _ in $(seq 300); do
    fcp=$(sed -n "s/^browser: title [0-9]* $1 fcp //p" "$log" | sort -n | tail -1)
    [ "${fcp:-0}" -le "${2:-$launched}" ] || return 0
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
# Waits for the window to be on screen, and sets $on_screen to when, in ms since the app started.
pc_on_screen() {
  await '^browser: window on screen at' || fail "the window didn't come on screen"
  on_screen=$(sed -n 's/^browser: window on screen at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
}
# Sends ⌃$1 and waits for the Space it shows, and sets $sent to when it was sent (Unix ms).
pc_space() {
  local before
  before=$(grep -c '^browser: space [0-9]* in' "$log" || true)
  sent=$(date +%s%3N)
  send key "$1" ctrl
  for _ in $(seq 100); do
    if [ "$(grep -c '^browser: space [0-9]* in' "$log" || true)" -gt "$before" ]; then
      return 0
    fi
    sleep 0.02
  done
  fail "⌃$1 showed no Space"
}
# The Unix ms the core said profile $1 arrived at, or nothing. The core's lines carry Chromium's
# prefix.
pc_arrived() {
  sed -n "s/.*browser: profile $1 arrived at \([0-9]*\)$/\1/p" "$log" | head -1
}
# Says when each of the n-1 late profiles arrived, as $1, and with #2500's core checks that each
# arrived, after the window came on screen.
pc_arrivals() {
  local shown_at at line
  [ "$n" -gt 1 ] || return 0
  shown_at=$(grep -n '^browser: window on screen at' "$log" | head -1 | cut -d: -f1)
  for i in $(seq $((n - 1))); do
    if grep -q "browser: profile Profile $i couldn't load" "$log"; then
      fail "Profile $i couldn't load"
    fi
    at=$(pc_arrived "Profile $i")
    if [ -z "$at" ]; then
      [ -z "$late_loads" ] || fail "Profile $i didn't arrive"
      continue
    fi
    say "$1: Profile $i arrived $((at - launched)) ms after the launch"
    line=$(grep -n "browser: profile Profile $i arrived at" "$log" | head -1 | cut -d: -f1)
    [ "$line" -gt "$shown_at" ] || fail "Profile $i arrived before the window came on screen"
  done
  [ -n "$late_loads" ] || say "$1: the engine loads every profile before the window"
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
  python3 -I check/profile_cost.py spaces "$dir/Spaces.json" "$n" ||
    fail "the Spaces didn't take more profiles"
  # Makes the new profiles, with what a new profile gets, as a user's are made before a launch.
  pc_launch "$dir" "warm-$n" ""
  pc_until 20
  say "$n profiles, warm launch: $(grep -c '^browser: added profile' "$log" || true) added, Spaces $(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3-)"
  pc_quit
  for round in 1 2; do
    name="p$n-r$round"
    pc_launch "$dir" "$name" ""
    pc_painted "setup$n"
    pc_until 30
    pc_on_screen
    say "$name: window on screen at $on_screen ms, page painted $((fcp - launched)) ms after the launch"
    pc_arrivals "$name"
    pc_measure "$name" "$dir"
    pc_until 60
    pc_measure "$name-60s" "$dir"
    extensions=$(python3 -I check/profile_cost.py count "$pc_dir/$name-60s.json" extension)
    [ "$extensions" = "$n" ] ||
      fail "$name: $extensions extension processes at 60 s for $n profiles, not one a profile"
    if [ "$round" = 2 ] && [ "$n" -gt 1 ]; then
      # The second profile's Space, its profile loaded long since.
      pc_space 2
      say "$name: ⌃2 to the second profile's Space shown in about $(($(date +%s%3N) - sent)) ms ($(grep '^browser: space [0-9]* in' "$log" | tail -1))"
      pc_painted "other$n" "$sent"
      say "$name: its tab's page painted $((fcp - sent)) ms after ⌃2"
      launched=$(date +%s%3N)
      pc_until 30
      pc_measure "$name-shown2" "$dir"
      pc_space 1
      sleep 2
    fi
    pc_quit
  done
  if [ "$n" -gt 1 ]; then
    # The second profile's Space the moment the window is on screen, its profile maybe still
    # loading: the Space shows at once, and its tab's page once the profile is in.
    name="p$n-first-switch"
    pc_launch "$dir" "$name" ""
    pc_on_screen
    pc_space 2
    shown=$(date +%s%3N)
    say "$name: ⌃2 sent $((sent - launched)) ms after the launch, window on screen at $on_screen ms; the Space shown in about $((shown - sent)) ms ($(grep '^browser: space [0-9]* in' "$log" | tail -1))"
    pc_painted "other$n" "$sent"
    say "$name: its tab's page painted $((fcp - sent)) ms after ⌃2"
    at=$(pc_arrived "Profile 1")
    [ -z "$at" ] || say "$name: Profile 1 arrived $((at - sent)) ms after ⌃2"
    pc_arrivals "$name"
    pc_space 1
    sleep 2
    pc_quit
  fi
  if [ "$n" = 4 ]; then
    # A quit while the late profiles load ends clean: exited checks the status, Crashpad's reports
    # and that no helper is left.
    pc_launch "$dir" "quit-$n" ""
    pc_on_screen
    pc_quit
    say "quit-$n: quit at the window on screen, $(grep -c 'browser: profile .* arrived at' "$log" || true) of $((n - 1)) late profiles arrived first, ended clean"
  fi
  pc_launch "$dir" "trace-$n" "--enable-tracing=browser,startup --enable-tracing-format=json --trace-startup-file=$PWD/$pc_dir/trace-$n.json"
  # Tracing slows the launch, so the paint isn't waited for: the profiles load long before 25 s.
  pc_until 25
  pc_quit
  python3 -I check/profile_cost.py trace "$pc_dir/trace-$n.json" > "$pc_dir/trace-$n.txt" ||
    echo "profile-cost: trace $n didn't read"
  say "trace $n profiles:"
  sed 's/^/  /' "$pc_dir/trace-$n.txt" >> "$results"
  cat "$pc_dir/trace-$n.txt"
done
echo "ok: profile-cost measured and checked, in $results"
