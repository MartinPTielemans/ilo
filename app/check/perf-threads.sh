# check.sh runs this at 846, only when named.
# What a thread costs the browser while it waits, and what kept threads cost a launch, on the Mac,
# measured as perf measures (check/lib.sh's perf_ helpers), on perf's page:
# - A thread waiting on the user, its knot moving, at 1 tab and at 20: idle CPU and wakeups by kind
#   of process over a minute, beside the same tabs with no thread, measured first.
# - The same thread with the sidebar hidden, where it waits as a knot at the window's edge.
# - Launches with 10, 100 and 1,000 kept threads, 5 each, seeded by tools/seed-kept-threads.py into
#   the check's own profile before each, as a launch removes all but 10: link to page painted and
#   the launch window on screen.
# A build that draws no waiting knot logs no line for it, and one that draws no edge knot shows no
# child window for it in the driver's layout (nothing logs whether it shows, #4091); that leg says
# "skip: ...: not in this build" and counts as a skip: a leg that measured nothing never passes.
# Its threads run on a stand-in for Claude Code's claude, in a home of the run's own, which every
# launch here gets with a login shell that gives the stand-in's folder alone, so the app never finds
# or runs the user's claude. The stand-in answers the app's sign-in and model asks, and a thread's
# turn by asking the user a question through the browser's own MCP server, which holds the thread
# waiting until the app ends it. Measure on a quiet Mac, on its built-in screen.

[ "$(uname)" = Darwin ] || {
  echo "skip all: perf-threads. Its stand-in claude and the knots it measures are the Mac's."
  exit 0
}
serve
perf_dir="$out/perf-threads"
rm -rf "$perf_dir"
mkdir -p "$perf_dir"
on_cpu=$(runner_cpu)
perf_page
echo "perf: on $on_cpu"
# The line a build with the waiting knot logs as it draws one.
knot_moving() { echo "^browser: task card $1 knot moving\$"; }
# Whether the window has a child window, which with the sidebar hidden and in and a thread waiting is
# the tab at its edge, as the driver's layout lists it.
edge_knot_shown() {
  send layout
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q '^browser: layout child'
}

home="$PWD/$out/threads-home"
rm -rf "$home"
mkdir -p "$home/bin"
python=$(python3 -I -c 'import sys; print(sys.executable)')
# The login shell the app asks for its PATH, which answers the stand-in's folder and the system's.
cat > "$home/login-shell" << EOF
#!/bin/sh
printf '%s' '$home/bin:/usr/bin:/bin:/usr/sbin:/sbin'
EOF
cat > "$home/claude.py" << 'PY'
import json, os, subprocess, sys
args = sys.argv[1:]
def say(message):
    print(json.dumps(message), flush=True)
if args[:2] == ["auth", "status"]:
    say({"loggedIn": True, "authMethod": "check"})
elif "-p" in args and "--mcp-config" in args:
    # A thread's turn: its first message, then a question to the user through the browser's
    # server, as the config names it, which holds the thread waiting until its input closes.
    with open(args[args.index("--mcp-config") + 1], encoding="utf-8") as file:
        server = next(iter(json.load(file)["mcpServers"].values()))
    sys.stdin.readline()
    bridge = subprocess.Popen([server["command"], *server.get("args", [])],
                              env={**os.environ, **server.get("env", {})},
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    def send(message):
        bridge.stdin.write((json.dumps({"jsonrpc": "2.0", **message}) + "\n").encode())
        bridge.stdin.flush()
    send({"id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18",
          "capabilities": {}, "clientInfo": {"name": "check", "version": "1"}}})
    for line in bridge.stdout:
        if json.loads(line).get("id") == 1:
            break
    send({"method": "notifications/initialized"})
    say({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "ask-1",
         "name": "ask", "input": {}}]}})
    send({"id": 2, "method": "tools/call",
          "params": {"name": "ask", "arguments": {"question": "Which seat?"}}})
    for line in sys.stdin:
        pass
    bridge.terminate()
# A kept thread's removal (`purge`) has nothing of the stand-in's to remove, and its model ask
# (-p with no MCP config) answers nothing, so the app keeps its default.
PY
printf '#!/bin/sh\nexec "%s" -I "%s" "$@"\n' "$python" "$home/claude.py" > "$home/bin/claude"
chmod +x "$home/login-shell" "$home/bin/claude"
# Threads on, run on claude, as the app saves them.
printf '{"threads":true,"other_apps":false,"agents":[],"tasks":{"default":"claude"}}' \
  > "$profile_dir/Agents"
# Runs the browser, with the NAME=value words before it, in the run's own home, login shell and
# PATH, as every launch here is. Only ever run in the background: it execs.
threads_browser() {
  exec env HOME="$home" ZDOTDIR="$home" SHELL="$home/login-shell" \
    PATH="$home/bin:/usr/bin:/bin:/usr/sbin:/sbin" BROWSER_PROFILE_DIR="$profile_dir" "$@"
}
# Waits up to $2 tenths of a second for a line matching $1 in the run's log.
threads_shows() {
  for _ in $(seq "$2"); do
    grep -q "$1" "$log" && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  return 1
}
# A driven launch, logged as $1, on perf's page as $2.
threads_launch() {
  log="$perf_dir/$1.log"
  step=0
  rm -f "$saved" "$launch_frame"
  threads_browser BROWSER_DRIVE=1 BROWSER_URL="$page?$2" "$browser" < "$out/drive" > "$log" \
    2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: title [0-9]* $2 fcp " || fail "$1's first page didn't paint"
  loaded=$(date +%s)
  send click $page_point
}
# Opens perf's page as $1 in a new tab from ⌘T's bar.
threads_open() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
  send type "$page?$1"
  send key return
  expect_late "^browser: title [0-9]* $1 fcp " "$1 didn't paint in its new tab"
  loaded=$(date +%s)
}
# Asks a thread from ⌘T, and waits until it waits on the user: $thread is its agent.
threads_ask() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for a thread"
  send type "Pick a seat"
  send key tab
  expect '^browser: command bar asks an agent$' "⇥ didn't make what was typed a thread"
  # A launch looks for no CLI: the ask's first showing in a run does (Agents::task_ask_shown).
  await "^browser: task agent claude is SignedIn at $home/bin/claude\$" ||
    fail "the ask didn't find the stand-in signed in: $(grep '^browser: task agent claude ' "$log")"
  send key return
  expect '^browser: command bar runs a task on claude$' "↩ didn't run the thread on the stand-in"
  expect_late '^browser: task task-[0-9]* on claude asked from space ' "the thread didn't start"
  thread=$(sed -n 's/^browser: task \(task-[0-9]*\) on claude asked from space .*/\1/p' "$log" |
    tail -1)
  expect_late "^browser: task card $thread Waiting " \
    "the thread didn't wait on the user: $(grep "^browser: task card $thread " "$log" | tail -1)"
}
# Hides the sidebar, or shows it, by ⌘S: $1 is the state it ends in.
threads_sidebar() {
  send key s cmd
  expect "^browser: sidebar $1\$" "⌘S didn't leave the sidebar $1"
}
# Quits, and clears what the session kept, so the next launch starts with no thread.
threads_quit() {
  quit
  rm -rf "$profile_dir/Kept tasks" "$profile_dir/Kept threads"
}

# Which knots the build draws: a short launch with a thread waiting, then the sidebar hidden.
threads_launch probe probe
threads_ask
knot=
edge=
! threads_shows "$(knot_moving "$thread")" 30 || knot=1
threads_sidebar hidden
! edge_knot_shown || edge=1
threads_sidebar shown
threads_quit
measured=
skipped=
[ -n "$knot" ] || skipped="${skipped}the waiting knot, "
[ -n "$edge" ] || skipped="${skipped}the edge knot, "
[ -n "$knot" ] ||
  echo "skip: the waiting knot: not in this build, which logs no $(knot_moving task-N)"
[ -n "$edge" ] ||
  echo "skip: the edge knot: not in this build, whose window shows no child window for it"

# A session at $1 tabs of perf's page: idle with no thread, then with one waiting, then with it
# waiting at the window's edge, each where the build draws its knot.
threads_session() {
  threads_launch "session-$1" "s$1-1"
  n=2
  while [ $n -le "$1" ]; do
    threads_open "s$1-$n"
    n=$((n + 1))
  done
  perf_until $((loaded + 30))
  perf_idle "none-$1"
  perf_row "none-cpu-$1" "Idle CPU, $1 tabs, no thread" "$cpu" "the thread rows' baseline"
  perf_row "none-wakeups-$1" "Idle wakeups, $1 tabs, no thread" \
    "$wakeups /s ($wake_kinds over the minute)" "the thread rows' baseline"
  threads_ask
  if [ -n "$knot" ]; then
    perf_until $(($(date +%s) + 10))
    perf_idle "waiting-$1"
    perf_row "waiting-cpu-$1" "Idle CPU, $1 tabs, a thread waiting, its knot moving" "$cpu" \
      "none of its own: beside no thread's"
    perf_row "waiting-wakeups-$1" "Idle wakeups, $1 tabs, a thread waiting, its knot moving" \
      "$wakeups /s ($wake_kinds over the minute)" "none of its own: beside no thread's"
  fi
  if [ -n "$edge" ]; then
    threads_sidebar hidden
    perf_until $(($(date +%s) + 10))
    perf_idle "edge-$1"
    perf_row "edge-cpu-$1" "Idle CPU, $1 tabs, sidebar hidden, a thread waiting at the edge" \
      "$cpu" "none of its own: beside no thread's"
    perf_row "edge-wakeups-$1" \
      "Idle wakeups, $1 tabs, sidebar hidden, a thread waiting at the edge" \
      "$wakeups /s ($wake_kinds over the minute)" "none of its own: beside no thread's"
  fi
  threads_quit
}
if [ -n "$knot$edge" ]; then
  threads_session 1
  threads_session 20
  [ -z "$knot" ] || measured="${measured}the waiting knot at 1 and 20 tabs, "
  [ -z "$edge" ] || measured="${measured}the edge knot at 1 and 20 tabs, "
fi

# Kept threads at launch. A warm launch first, not measured, that stays the minute a profile's first
# session takes to settle its filters, so each launch after is on the same profile, as a user's.
log="$perf_dir/warm.log"
rm -f "$saved" "$launch_frame"
threads_browser BROWSER_URL="$page?warm" "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
await "^browser: title [0-9]* warm fcp " || fail "the warm launch's page didn't paint"
perf_until $(($(date +%s) + 60))
threads_quit
# Seeds $1 kept threads into the check's profile, through a new folder in it the tool fills.
threads_seed() {
  rm -rf "$profile_dir/Kept tasks" "$profile_dir/Kept threads" "$profile_dir/seed"
  python3 -I ../tools/seed-kept-threads.py "$profile_dir/seed" "$1" > /dev/null ||
    fail "seed-kept-threads.py didn't seed $1 kept threads"
  mv "$profile_dir/seed/Kept tasks" "$profile_dir/seed/Kept threads" "$profile_dir/" ||
    fail "the $1 kept threads didn't move into the profile"
  rmdir "$profile_dir/seed"
}
for count in 10 100 1000; do
  : > "$perf_dir/kept-$count-link"
  : > "$perf_dir/kept-$count-window"
  kept=$count
  [ $kept -le 10 ] || kept=10
  for n in 1 2 3 4 5; do
    threads_seed $count
    id="k$count-$n"
    log="$perf_dir/kept-$id.log"
    rm -f "$saved" "$launch_frame"
    perf_stamp
    launched=$stamp
    threads_browser BROWSER_URL="$page?$id" "$browser" < /dev/null > "$log" 2>&1 &
    pid=$!
    await "^browser: title [0-9]* $id fcp " || fail "launch $id's page didn't paint"
    fcp=$(sed -n "s/^browser: title [0-9]* $id fcp //p" "$log" | tail -1)
    echo $((fcp - launched)) >> "$perf_dir/kept-$count-link"
    sed -n 's/^browser: launch window at \([0-9.]*\) ms$/\1/p' "$log" | head -1 \
      >> "$perf_dir/kept-$count-window"
    # It read them all: it brings back 10 and removes the rest, through the stand-in.
    await "^browser: $kept tasks of a run before are kept\$" ||
      fail "launch $id didn't bring back $kept of its $count kept threads"
    [ $count -le 10 ] || await "^browser: kept tasks: $((count - 10)) removed, 0 couldn't be\$" ||
      fail "launch $id didn't remove the $((count - 10)) kept threads past 10"
    threads_quit
  done
  perf_row "kept-$count-link" "Launch with $count kept threads, link to page painted" \
    "$(perf_stats '%.0f' < "$perf_dir/kept-$count-link") ms" "none of its own: beside 10's"
  perf_row "kept-$count-window" "Launch with $count kept threads, window on screen" \
    "$(perf_stats '%.1f' < "$perf_dir/kept-$count-window") ms" "none of its own: beside 10's"
done
measured="${measured}launches with 10, 100 and 1,000 kept threads"
rm -rf "$home"
echo "ok: perf-threads measured ${measured}${skipped:+; skipped ${skipped%, }}"
