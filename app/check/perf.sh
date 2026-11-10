# Measures the browser against #456's targets, the way Ardia's bench measured Ardia, Chrome, Arc
# and Helium (docs/perf/vs-browsers.md in Ardia's repo, its numbers of 2026-10-19 the targets), and
# says each row's median of its samples, with their range, beside its target. Those targets are an
# M5 Pro's, and a hosted runner is a slower machine shared with other jobs, so they only report.
# The gate's rows that compare with Chrome are measured against Chrome on the same runner, and of
# those, memory at 10 and 20 tabs, which moves under 5 % from run to run, fails the run more than
# 10 % over Chrome's.
#
# Its page is Ardia's: 60 boxes, 120 paragraphs and a clock each second, served from the check's
# own site, which puts in its title when it first painted ("<id> fcp <ms>") and, each time it's
# shown, two frames after it became visible ("<id> shown <n> <ms>"), both in Unix ms.
#
# - Launch to the page painted, 5 times, with BROWSER_URL, as a link from another app opens it.
# - Launch to the restored page painted, 5 times, with no URL, its last tab put back.
# - The window on screen, as each of those 10 launches logged it.
# - Then one session, as Ardia's: memory at 1 tab, 30 s after launch; idle CPU and wakeups over
#   the minute after, and again 2 minutes after the page loaded (settled); 19 new tabs opened from
#   ⌘T's bar, ↩ to their page painted; memory at 10 and 20 tabs, 10 s and 15 s after; idle at 20
#   tabs and settled at 20; 16 switches by ⌘1–⌘8, the key to the page shown; 3 new windows, ⌘N to
#   their first frame; and memory back at 1 tab, 30 s after closing the others.
# - Then Google Chrome, where the runner has it, on the same page on the same machine: memory at 1,
#   10 and 20 tabs and idle at 1 and 20, which the gate compares ilo with, and the run says how far
#   ilo is from it on each.
#
# Idle is measured in 10 s windows, six to the minute, and memory is summed over every process of
# the browser, as check/usage.py says for each OS. On Linux and the Mac the shell's clock is a
# process of its own, so each time from a key includes one process start, which the run says.

serve
perf_dir="$out/perf"
# #2317, not for merge: user-mode stack traces for chrome.exe's heap allocations, ilo's and Chrome's
# browser alike, which perf_dissect has UMDH read.
perf_debuggers="/c/Program Files (x86)/Windows Kits/10/Debuggers/x64"
if [ -n "${MSYSTEM:-}" ]; then
  if [ -x "$perf_debuggers/gflags.exe" ]; then
    MSYS2_ARG_CONV_EXCL='*' "$perf_debuggers/gflags.exe" /i chrome.exe +ust || echo "perf: gflags failed"
  else
    echo "perf: no Debugging Tools in $perf_debuggers: $(ls "/c/Program Files (x86)/Windows Kits/10" 2>&1 | tr '\n' ' ')"
  fi
fi
rm -rf "$perf_dir"
mkdir -p "$perf_dir"
profile_dir="$PWD/$perf_dir/profile"
mkdir -p "$profile_dir"
cat > "$out/site/perf.html" << 'HTML'
<!doctype html><meta charset=utf-8><title>loading</title>
<style>
body { font: 14px/1.4 sans-serif; margin: 16px }
.box { display: inline-block; width: 72px; height: 40px; margin: 4px; border-radius: 6px }
</style>
<h1>ilo perf page</h1><p id=clock></p><div id=boxes></div><div id=text></div>
<script>
const id = location.search.slice(1);
for (let i = 0; i < 60; i++) {
  const box = document.createElement("div");
  box.className = "box";
  box.style.background = `hsl(${i * 6} 60% 60%)`;
  boxes.append(box);
}
for (let i = 0; i < 120; i++) {
  const p = document.createElement("p");
  p.textContent = `Paragraph ${i}: the quick brown fox jumps over the lazy dog, again and again.`;
  text.append(p);
}
setInterval(() => clock.textContent = new Date().toLocaleTimeString(), 1000);
// Unix ms as the shell's clock reads it: Date.now() is the system's clock now, where timeOrigin's is
// the one the renderer took at its start, which a VM's clock drifts from, by 70 ms in 10 minutes.
const at = (t) => Math.round(Date.now() - performance.now() + t);
new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    if (entry.name == "first-contentful-paint") {
      document.title = `${id} fcp ${at(entry.startTime)}`;
    }
  }
}).observe({ type: "paint", buffered: true });
let shown = 0;
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState != "visible") {
    return;
  }
  const n = ++shown;
  requestAnimationFrame(() => requestAnimationFrame(() => {
    document.title = `${id} shown ${n} ${Date.now()}`;
  }));
});
</script>
HTML
page="${site}perf.html"

# Sets $stamp to Unix time in ms. Bash 5 has it in EPOCHREALTIME, which Git Bash's is, as a process
# start there takes tens of ms; Linux's sh is dash, so date, and the Mac's bash 3.2 and BSD date have
# neither, so Perl.
if [ -n "${EPOCHREALTIME:-}" ]; then
  perf_clock=bash
elif [ "$(uname)" = Darwin ]; then
  perf_clock=perl
else
  perf_clock=date
fi
perf_stamp() {
  case $perf_clock in
    bash)
      stamp=$EPOCHREALTIME
      stamp_us=${stamp#*[.,]}
      stamp="${stamp%[.,]*}${stamp_us%???}"
      ;;
    perl) stamp=$(perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000') ;;
    *) stamp=$(date +%s%3N) ;;
  esac
}
# Sorts the numbers on standard input, one a line, and says their median, range and count, each
# number as $1 formats it.
perf_stats() {
  sort -n | awk -v f="$1" '{ v[NR] = $1 } END {
    if (!NR) { print "no samples"; exit }
    m = NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2
    printf "median " f " (" f "–" f ", n=%d)", m, v[1], v[NR], NR }'
}
# Says row $1, labelled $2, measured as $3, against its target $4, and keeps it for the summary.
perf_row() {
  echo "perf: $2: $3; target $4"
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$perf_dir/rows.tsv"
}
# Snapshots what the browser's processes use, or those of $snap_procs, as $1.
perf_snap() {
  # shellcheck disable=SC2086 # kinds, or nothing
  python3 -I check/usage.py snap "${snap_procs:-$procs}" "$perf_dir/$1.json" ${2:-} ||
    fail "the browser's usage didn't read"
}
# Waits until $1, in Unix seconds, checking the app is still there.
perf_until() {
  while [ "$(date +%s)" -lt "$1" ]; do
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
}
# The memory now, in $memory: "MiB processes", and by kind of process in $kinds.
perf_memory() {
  perf_snap "memory-$1" kinds
  memory=$(python3 -I check/usage.py memory "$perf_dir/memory-$1.json")
  kinds=$(python3 -I check/usage.py kinds "$perf_dir/memory-$1.json")
}
# #2138, not for merge: the browser process's memory by the OS's own tools, as dissect.py says.
perf_dissect() {
  if [ -n "${MSYSTEM:-}" ] && [ ! -e "$out/vmmap/vmmap64.exe" ]; then
    curl -sSfLo "$out/VMMap.zip" https://download.sysinternals.com/files/VMMap.zip &&
      unzip -oq "$out/VMMap.zip" -d "$out/vmmap" || echo "perf: no VMMap"
  fi
  [ -z "${MSYSTEM:-}" ] || DISSECT_VMMAP=$(cygpath -w "$PWD/$out/vmmap/vmmap64.exe")
  DISSECT_VMMAP=${DISSECT_VMMAP:-} python3 -I check/dissect.py "$perf_dir/memory-$1.json" "$perf_dir" \
    "dissect-$1" || echo "perf: dissect $1 failed"
  # #2317, not for merge: the browser process's NT heap by allocating stack, as UMDH reads it.
  if [ -n "${MSYSTEM:-}" ] && [ -x "$perf_debuggers/umdh.exe" ]; then
    umdh_pid=$(cut -d' ' -f1 "$perf_dir/dissect-$1-browser-pid.txt" 2> /dev/null)
    [ -z "$umdh_pid" ] ||
      _NT_SYMBOL_PATH="srv*$(cygpath -w "$PWD/$out/symbols")*https://msdl.microsoft.com/download/symbols" \
        timeout 900 "$perf_debuggers/umdh.exe" -p:"$umdh_pid" -f:"$(cygpath -w "$PWD/$perf_dir/umdh-$1.txt")" \
        > "$perf_dir/umdh-$1.log" 2>&1 || echo "perf: umdh $1 failed"
  fi
}
# Idle CPU and wakeups over the next minute, in 10 s windows: sets $cpu and $wakeups to their stats.
perf_idle() {
  files=
  for n in 0 1 2 3 4 5 6; do
    [ "$n" = 0 ] || perf_until $(($(date +%s) + 10))
    perf_snap "$1-$n"
    files="$files $perf_dir/$1-$n.json"
  done
  # shellcheck disable=SC2086 # the files, a word each
  python3 -I check/usage.py idle $files > "$perf_dir/$1.txt"
  cpu=$(cut -d' ' -f1 < "$perf_dir/$1.txt" | perf_stats '%.2f')
  wakeups=$(cut -d' ' -f2 < "$perf_dir/$1.txt" | perf_stats '%.1f')
  cpu="$cpu % ($(python3 -I check/usage.py idle-kinds "$perf_dir/$1-0.json" "$perf_dir/$1-6.json") over the minute)"
}
# The Unix ms page $1 said it first painted, in the log since step $2 (all of it with none).
perf_painted() {
  if [ -n "${2:-}" ]; then
    sed -n "/^browser: drive $2: /,\$p" "$log"
  else
    cat "$log"
  fi | sed -n "s/^browser: title [0-9]* $1 fcp //p" | tail -1
}

# The clock's own cost, which each time from a key includes: on Linux and the Mac a process start.
perf_stamp
clock=$stamp
perf_stamp
clock=$((stamp - clock))

# Launches the app, on page $1 or with none to put back the last, and waits for page $2 to paint:
# sets $painted to the ms from just before the launch, and $on_screen to the window's time.
perf_launch() {
  log="$perf_dir/launch-$3.log"
  perf_stamp
  launched=$stamp
  if [ -n "$1" ]; then
    BROWSER_URL="$1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
  else
    BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
  fi
  pid=$!
  # A tab put back has its last title, an earlier launch's paint, until its page loads.
  fcp=0
  for _ in $(seq 300); do
    fcp=$(sed -n "s/^browser: title [0-9]* $2 fcp //p" "$log" | sort -n | tail -1)
    [ "${fcp:-0}" -le "$launched" ] || break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  [ "${fcp:-0}" -gt "$launched" ] || fail "launch $3's page $2 didn't paint"
  painted=$((fcp - launched))
  on_screen=$(sed -n 's/^browser: window on screen at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
  # The core holds a launch's first page until uBlock Origin is ready (core/ad_block.cc).
  held=$(sed -n 's/^.*browser: ad blocking held a page \([0-9]*\) ms.*$/\1/p' "$log" | head -1)
  quit
}
: > "$perf_dir/link"
: > "$perf_dir/restore"
: > "$perf_dir/on-screen"
: > "$perf_dir/held"
# The first launch makes the profile, which the others find, as a user's, so it isn't counted.
perf_launch "$page?warm" warm warm
for n in 1 2 3 4 5; do
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  perf_launch "$page?link$n" "link$n" "link$n"
  echo "$painted" >> "$perf_dir/link"
  echo "$on_screen" >> "$perf_dir/on-screen"
  echo "${held:-0}" >> "$perf_dir/held"
done
for n in 1 2 3 4 5; do
  perf_launch "" link5 "restore$n"
  echo "$painted" >> "$perf_dir/restore"
  echo "$on_screen" >> "$perf_dir/on-screen"
  echo "${held:-0}" >> "$perf_dir/held"
done
perf_row launch-link "Launch, link to page painted" "$(perf_stats '%.0f' < "$perf_dir/link") ms" \
  "Ardia 413 ms, Chrome 484 ms"
perf_row launch-window "Launch, window on screen" "$(perf_stats '%.0f' < "$perf_dir/on-screen") ms" \
  "Ardia 280 ms, Chrome 424 ms (gate: ≤ Chrome)"
perf_row launch-restore "Launch, restored page painted" \
  "$(perf_stats '%.0f' < "$perf_dir/restore") ms" "Ardia 360 ms, Helium 355 ms, Chrome 438 ms"
perf_row launch-held "Launch, the page held for uBlock Origin" \
  "$(perf_stats '%.0f' < "$perf_dir/held") ms" "none: it's part of the two above"

log="$perf_dir/session.log"
step=0
rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
started=$(date +%s)
BROWSER_DRIVE=1 BROWSER_URL="$page?p1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await "^browser: title [0-9]* p1 fcp " || fail "the session's first page didn't paint"
send click $page_point

# The session, once the page has the active app's keyboard.
perf_session() {
  perf_until $((started + 30))
  perf_memory 1
  perf_row memory-1 "Memory, 1 tab" "${memory% *} MiB, ${memory#* } processes ($kinds)" \
    "Ardia 298 MB, Chrome 370 MB (gate: ≤ Chrome)"
  perf_idle idle-1
  perf_row idle-cpu-1 "Idle CPU, 1 tab" "$cpu" "Ardia 0.16 %, Chrome 0.2 %"
  perf_row idle-wakeups-1 "Idle wakeups, 1 tab" "$wakeups /s" "Ardia 5.27 /s, Chrome 6.77 /s"
  perf_until $((started + 120))
  perf_idle settled-1
  perf_row settled-cpu-1 "Idle CPU, 1 tab, settled" "$cpu" "Ardia 0.05 %"
  perf_row settled-wakeups-1 "Idle wakeups, 1 tab, settled" "$wakeups /s" "Ardia 3.11 /s"

  # Opens page $1 from ⌘T's bar and keeps the ms from ↩ to its first paint.
  perf_open() {
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
    send type "$page?$1"
    perf_stamp
  typed=$stamp
    send key return
    expect_late "^browser: title [0-9]* $1 fcp " "$1 didn't paint in its new tab"
    echo $(($(perf_painted "$1" "$step") - typed)) >> "$perf_dir/newtab"
    loaded=$(date +%s)
  }
  : > "$perf_dir/newtab"
  for n in 2 3 4 5 6 7 8 9 10; do
    perf_open "p$n"
  done
  perf_until $((loaded + 10))
  perf_memory 10
  perf_row memory-10 "Memory, 10 tabs" "${memory% *} MiB, ${memory#* } processes ($kinds)" \
    "Ardia 714 MB, Chrome 692 MB (gate: ≤ Chrome)"
  for n in 11 12 13 14 15 16 17 18 19 20; do
    perf_open "p$n"
  done
  perf_until $((loaded + 15))
  perf_memory 20
  perf_dissect 20
  perf_row memory-20 "Memory, 20 tabs" "${memory% *} MiB, ${memory#* } processes ($kinds)" \
    "Ardia 1082 MB, Chrome 919 MB (gate: ≤ Chrome)"
  perf_row newtab "New tab, ↩ in ⌘T's bar to page painted" \
    "$(perf_stats '%.0f' < "$perf_dir/newtab") ms" "Ardia 54.9 ms, Chrome 54.3 ms (gate: ≤ Chrome)"
  perf_idle idle-20
  perf_row idle-cpu-20 "Idle CPU, 20 tabs" "$cpu" "Ardia 0.45 %, Chrome 0.62 % (gate: ≤ Chrome)"
  perf_row idle-wakeups-20 "Idle wakeups, 20 tabs" "$wakeups /s" \
    "Ardia 13.56 /s, Chrome 30.86 /s (gate: ≤ Chrome)"
  perf_until $((loaded + 120))
  perf_idle settled-20
  perf_row settled-cpu-20 "Idle CPU, 20 tabs, settled" "$cpu" "Ardia 0.31 %"
  perf_row settled-wakeups-20 "Idle wakeups, 20 tabs, settled" "$wakeups /s" "Ardia 11.82 /s"

  # ⌘1 is the newest Today tab, p20, which shows, so each key goes to another tab.
  : > "$perf_dir/switch"
  : > "$perf_dir/switch-core"
  for k in 2 3 4 5 6 7 8 1 2 3 4 5 6 7 8 1; do
    perf_stamp
    pressed=$stamp
    send key "$k" cmd
    expect "^browser: selected [0-9]* in " "⌘$k didn't switch tabs"
    since=$(sed -n "/^browser: drive $step: /,\$p" "$log")
    tab=$(echo "$since" | sed -n 's/^browser: selected \([0-9]*\) in .*/\1/p' | head -1)
    echo "$since" | sed -n 's/^browser: selected [0-9]* in \([0-9.]*\) ms$/\1/p' | head -1 \
      >> "$perf_dir/switch-core"
    expect "^browser: title $tab [^ ]* shown " "tab $tab didn't say it was shown after ⌘$k"
    shown=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
      sed -n "s/^browser: title $tab [^ ]* shown [0-9]* //p" | tail -1)
    echo $((shown - pressed)) >> "$perf_dir/switch"
  done
  perf_row switch "Tab switch, key to page shown" "$(perf_stats '%.0f' < "$perf_dir/switch") ms" \
    "Ardia 14 ms, Chrome 8 ms (gate: 16 ms)"
  perf_row switch-core "Tab switch, key to the core's report" \
    "$(perf_stats '%.1f' < "$perf_dir/switch-core") ms" "none of its own"

  : > "$perf_dir/newwindow"
  for _ in 1 2 3; do
    send key n cmd
    expect_late '^browser: new window first frame at [0-9.]* ms$' "⌘N's window drew no frame"
    sed -n "/^browser: drive $step: /,\$p" "$log" |
      sed -n 's/^browser: new window first frame at \([0-9.]*\) ms$/\1/p' | head -1 >> "$perf_dir/newwindow"
    opened=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
    expect_late '^browser: command bar shown$' "⌘N's window didn't open the command bar"
    send key escape
    expect '^browser: command bar hidden$' "escape didn't close ⌘N's command bar"
    send key w cmd shift
    expect "^browser: window closed $opened\$" "⇧⌘W didn't close ⌘N's window"
  done
  perf_row newwindow "New window, ⌘N to first frame" \
    "$(perf_stats '%.1f' < "$perf_dir/newwindow") ms" "Ardia 79.9 ms, Chrome 100 ms"

  for _ in $(seq 19); do
    send key w cmd
    expect '^browser: today ' "⌘W didn't close a tab"
    # The next tab shows a moment after, and a ⌘W before then would close none.
    expect '^browser: selected [0-9]' "⌘W didn't show another tab"
  done
  [ "$(today | wc -w | tr -d ' ')" = 1 ] || fail "⌘W 19 times didn't leave 1 tab: $(today)"
  perf_until $(($(date +%s) + 30))
  perf_memory closed
  perf_row memory-closed "Memory, back to 1 tab after 20" "${memory% *} MiB, ${memory#* } processes ($kinds)" \
    "Ardia 358 MB, Chrome 406 MB"
}
if [ "$active" = 1 ]; then
  perf_session
else
  echo "skip: perf's session. Browser wasn't the active app, so the page didn't get its keys."
fi
exec 3>&-
quit

# Google Chrome, where the runner has it, as GitHub's images do, measured on this machine as ilo was,
# for the gate's rows that compare with it: memory at 1, 10 and 20 tabs and idle at 1 and 20, on the
# same page, its tabs opened from the command line, as a link from another app opens them, a couple
# of seconds apart. A profile of its own, launched once first, as ilo's was.
if [ "$(uname)" = Darwin ]; then
  chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
  chrome_procs="/Applications/Google Chrome.app/Contents"
elif windows; then
  chrome="/c/Program Files/Google/Chrome/Application/chrome.exe"
  chrome_procs="C:/Program Files/Google/Chrome/Application/chrome.exe"
else
  chrome=/opt/google/chrome/chrome
  chrome_procs=/opt/google/chrome/chrome
fi
chrome_profile="$PWD/$perf_dir/chrome-profile"
! windows || chrome_profile="$(cygpath -w "$chrome_profile")"
# Opens page $1 in Chrome, which a running Chrome takes as a new tab.
chrome_open() {
  "$chrome" --user-data-dir="$chrome_profile" --no-first-run --no-default-browser-check \
    --use-mock-keychain --password-store=basic "$page?$1" < /dev/null >> "$perf_dir/chrome.log" 2>&1 &
}
# Quits Chrome as quit_app does the app, then ends what's left of it.
chrome_quit() {
  if windows; then
    # Without /F, taskkill asks its windows to close, as their close button does.
    taskkill //PID "$(cat "/proc/$pid/winpid")" > /dev/null 2>&1 || true
  elif [ "$(uname)" = Darwin ]; then
    osascript -e 'tell application id "com.google.Chrome" to quit' || true
  else
    kill -INT "$pid" 2> /dev/null || true
  fi
  for _ in $(seq 40); do
    perf_snap chrome-left
    python3 -I check/usage.py memory "$perf_dir/chrome-left.json" | grep -q ' 0$' && return 0
    sleep 0.5
  done
  echo "Chrome didn't quit in 20 s, so it's ended"
  python3 -I -c 'import json, sys; print("\n".join(json.load(open(sys.argv[1]))["procs"]))' \
    "$perf_dir/chrome-left.json" | while read -r left; do
    if windows; then taskkill //F //PID "$left" > /dev/null 2>&1; else kill -9 "$left" 2> /dev/null; fi
  done || true
  # A Chrome still there would take the next launch's page as a tab and let that launch end.
  for _ in $(seq 20); do
    perf_snap chrome-left
    python3 -I check/usage.py memory "$perf_dir/chrome-left.json" | grep -q ' 0$' && return 0
    sleep 0.5
  done
  fail "Chrome's processes outlived it: $(python3 -I check/usage.py kinds "$perf_dir/chrome-left.json")"
}
# Launches Chrome on page $1 and sets $pid, or says it didn't stay.
chrome_launch() {
  chrome_open "$1"
  pid=$!
  sleep 5 # the 5 s a Chrome that won't stay takes to go
  kill -0 "$pid" 2> /dev/null
}
if [ "$active" != 1 ]; then
  :
elif [ ! -x "$chrome" ]; then
  echo "skip: perf's Chrome on this runner. It has none at $chrome."
else
  snap_procs=$chrome_procs
  : > "$perf_dir/chrome.log"
  if ! chrome_launch warm; then
    echo "skip: perf's Chrome on this runner. It didn't stay open: $(tail -3 "$perf_dir/chrome.log")"
  else
    sleep 15 # Chrome's first 15 s, which warm its caches for the runs measured
    chrome_quit
    chrome_launch c1 || fail "Chrome didn't stay open the second time: $(tail -3 "$perf_dir/chrome.log")"
    started=$(($(date +%s) - 5))
    perf_until $((started + 30))
    perf_memory chrome-1
    perf_row chrome-memory-1 "Chrome here, memory, 1 tab" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)" "ilo's, the gate"
    perf_idle chrome-idle-1
    perf_row chrome-idle-cpu-1 "Chrome here, idle CPU, 1 tab" "$cpu" "ilo's"
    perf_row chrome-idle-wakeups-1 "Chrome here, idle wakeups, 1 tab" "$wakeups /s" "ilo's"
    for n in 2 3 4 5 6 7 8 9 10; do
      chrome_open "c$n"
      perf_until $(($(date +%s) + 2))
    done
    perf_until $(($(date +%s) + 10))
    perf_memory chrome-10
    perf_row chrome-memory-10 "Chrome here, memory, 10 tabs" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)" "ilo's, the gate"
    for n in 11 12 13 14 15 16 17 18 19 20; do
      chrome_open "c$n"
      perf_until $(($(date +%s) + 2))
    done
    perf_until $(($(date +%s) + 15))
    perf_memory chrome-20
    perf_dissect chrome-20
    perf_row chrome-memory-20 "Chrome here, memory, 20 tabs" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)" "ilo's, the gate"
    perf_idle chrome-idle-20
    perf_row chrome-idle-cpu-20 "Chrome here, idle CPU, 20 tabs" "$cpu" "ilo's, the gate"
    perf_row chrome-idle-wakeups-20 "Chrome here, idle wakeups, 20 tabs" "$wakeups /s" "ilo's, the gate"
    chrome_quit
    # ilo against Chrome on this machine, for each of the gate's rows. Memory at 10 and 20 tabs moves
    # under 5 % from run to run, so ilo more than 10 % over Chrome's fails the run. At 1 tab, 30 s
    # after launch, uBlock Origin is still taking in its lists, and ilo's moved by 70 % from one run
    # to the next, and idle moves 2 to 5 times from one 10 s window to the next on a hosted runner,
    # so those only report.
    python3 -I - "$perf_dir" << 'PY' | tee "$perf_dir/gate.txt"
import json, os, statistics, sys

d = sys.argv[1]
def memory(name):
    with open(os.path.join(d, f"memory-{name}.json")) as f:
        return sum(p[3] for p in json.load(f)["procs"].values()) / 1048576
def idle(name, column):
    with open(os.path.join(d, f"{name}.txt")) as f:
        return statistics.median(float(line.split()[column]) for line in f)
for label, ilo, chrome, unit, fails in [
    ("memory, 1 tab", memory("1"), memory("chrome-1"), "MiB", False),
    ("memory, 10 tabs", memory("10"), memory("chrome-10"), "MiB", True),
    ("memory, 20 tabs", memory("20"), memory("chrome-20"), "MiB", True),
    ("idle CPU, 1 tab", idle("idle-1", 0), idle("chrome-idle-1", 0), "%", False),
    ("idle CPU, 20 tabs", idle("idle-20", 0), idle("chrome-idle-20", 0), "%", False),
    ("idle wakeups, 1 tab", idle("idle-1", 1), idle("chrome-idle-1", 1), "/s", False),
    ("idle wakeups, 20 tabs", idle("idle-20", 1), idle("chrome-idle-20", 1), "/s", False),
]:
    by = f"{(ilo / chrome - 1) * 100:+.0f} %" if chrome else "Chrome none"
    missed = "missed" if fails and chrome and ilo > chrome * 1.1 else "fails over +10 %" if fails else "reports"
    print(f"perf: gate, {label}: ilo {ilo:.2f} {unit}, Chrome here {chrome:.2f} {unit} ({by}), {missed}")
PY
  fi
  snap_procs=
fi
missed=$(grep ', missed$' "$perf_dir/gate.txt" 2> /dev/null | sed 's/^perf: gate, //; s/: ilo .*(\(.*\)), missed$/ \1/' |
  tr '\n' ';' | sed 's/;$//; s/;/; /g')
[ -z "$missed" ] || fail "ilo took more than 10 % over Chrome on this runner: $missed (#456)"
echo "ok: perf measured $(wc -l < "$perf_dir/rows.tsv" | tr -d ' ') rows against #456's targets," \
  "and ilo's memory at 10 and 20 tabs within 10 % of Chrome's here; each time from a key" \
  "includes the shell's clock ($perf_clock), $clock ms. Rows:" \
  "$perf_dir/rows.tsv, usage snapshots: $perf_dir/"
