# Measures the browser against #456's targets, the way Ardia's bench measured Ardia, Chrome, Arc
# and Helium (docs/perf/vs-browsers.md in Ardia's repo, its numbers of 2026-10-19 the targets), and
# says each row's median of its samples, with their range, beside its target. It fails on none:
# the targets are an M5 Pro's, and a hosted runner is a slower machine shared with other jobs, so
# the numbers say where ilo stands and how they move from run to run, not whether it's beaten.
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
#
# Idle is measured in 10 s windows, six to the minute, and memory is summed over every process of
# the browser, as check/usage.py says for each OS. The shell's clock is a process of its own, so
# each time from a key includes one process start, which the run says.

serve
perf_dir="$out/perf"
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
const at = (t) => Math.round(performance.timeOrigin + t);
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
    document.title = `${id} shown ${n} ${at(performance.now())}`;
  }));
});
</script>
HTML
page="${site}perf.html"

# Unix time in ms. BSD date has no %N, so the Mac's is Perl's.
perf_now() {
  if [ "$(uname)" = Darwin ]; then
    perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'
  else
    date +%s%3N
  fi
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
perf_snap() {
  python3 -I check/usage.py snap "$procs" "$perf_dir/$1.json" || fail "the browser's usage didn't read"
}
# Waits until $1, in Unix seconds, checking the app is still there.
perf_until() {
  while [ "$(date +%s)" -lt "$1" ]; do
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
}
# The memory now, in $memory: "MiB processes".
perf_memory() {
  perf_snap "memory-$1"
  memory=$(python3 -I check/usage.py memory "$perf_dir/memory-$1.json")
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
}
# The Unix ms page $1 said it first painted, in the log since step $2 (all of it with none).
perf_painted() {
  sed -n "/^browser: drive ${2:-0}: /,\$p" "$log" | sed -n "s/^browser: title [0-9]* $1 fcp //p" | tail -1
}

# The clock's own cost, a process start, which each time from a key includes.
clock_from=$(perf_now)
clock_to=$(perf_now)
clock=$((clock_to - clock_from))

# Launches the app, on page $1 or with none to put back the last, and waits for page $2 to paint:
# sets $painted to the ms from just before the launch, and $on_screen to the window's time.
perf_launch() {
  log="$perf_dir/launch-$3.log"
  launched=$(perf_now)
  if [ -n "$1" ]; then
    BROWSER_URL="$1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
  else
    BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
  fi
  pid=$!
  await "^browser: title [0-9]* $2 fcp " || fail "launch $3's page $2 didn't paint"
  painted=$(($(perf_painted "$2") - launched))
  on_screen=$(sed -n 's/^browser: window on screen at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
  quit
}
: > "$perf_dir/link"
: > "$perf_dir/restore"
: > "$perf_dir/on-screen"
# The first launch makes the profile, which the others find, as a user's, so it isn't counted.
perf_launch "$page?warm" warm warm
for n in 1 2 3 4 5; do
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  perf_launch "$page?link$n" "link$n" "link$n"
  echo "$painted" >> "$perf_dir/link"
  echo "$on_screen" >> "$perf_dir/on-screen"
done
for n in 1 2 3 4 5; do
  perf_launch "" link5 "restore$n"
  echo "$painted" >> "$perf_dir/restore"
  echo "$on_screen" >> "$perf_dir/on-screen"
done
perf_row launch-link "Launch, link to page painted" "$(perf_stats '%.0f' < "$perf_dir/link") ms" \
  "Ardia 413 ms, Chrome 484 ms"
perf_row launch-window "Launch, window on screen" "$(perf_stats '%.0f' < "$perf_dir/on-screen") ms" \
  "Ardia 280 ms, Chrome 424 ms (gate: ≤ Chrome)"
perf_row launch-restore "Launch, restored page painted" \
  "$(perf_stats '%.0f' < "$perf_dir/restore") ms" "Ardia 360 ms, Helium 355 ms, Chrome 438 ms"

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
  perf_row memory-1 "Memory, 1 tab" "${memory% *} MiB, ${memory#* } processes" \
    "Ardia 298 MB, Chrome 370 MB (gate: ≤ Chrome)"
  perf_idle idle-1
  perf_row idle-cpu-1 "Idle CPU, 1 tab" "$cpu %" "Ardia 0.16 %, Chrome 0.2 %"
  perf_row idle-wakeups-1 "Idle wakeups, 1 tab" "$wakeups /s" "Ardia 5.27 /s, Chrome 6.77 /s"
  perf_until $((started + 120))
  perf_idle settled-1
  perf_row settled-cpu-1 "Idle CPU, 1 tab, settled" "$cpu %" "Ardia 0.05 %"
  perf_row settled-wakeups-1 "Idle wakeups, 1 tab, settled" "$wakeups /s" "Ardia 3.11 /s"

  # Opens page $1 from ⌘T's bar and keeps the ms from ↩ to its first paint.
  perf_open() {
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
    send type "$page?$1"
    typed=$(perf_now)
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
  perf_row memory-10 "Memory, 10 tabs" "${memory% *} MiB, ${memory#* } processes" \
    "Ardia 714 MB, Chrome 692 MB (gate: ≤ Chrome)"
  for n in 11 12 13 14 15 16 17 18 19 20; do
    perf_open "p$n"
  done
  perf_until $((loaded + 15))
  perf_memory 20
  perf_row memory-20 "Memory, 20 tabs" "${memory% *} MiB, ${memory#* } processes" \
    "Ardia 1082 MB, Chrome 919 MB (gate: ≤ Chrome)"
  perf_row newtab "New tab, ↩ in ⌘T's bar to page painted" \
    "$(perf_stats '%.0f' < "$perf_dir/newtab") ms" "Ardia 54.9 ms, Chrome 54.3 ms (gate: ≤ Chrome)"
  perf_idle idle-20
  perf_row idle-cpu-20 "Idle CPU, 20 tabs" "$cpu %" "Ardia 0.45 %, Chrome 0.62 % (gate: ≤ Chrome)"
  perf_row idle-wakeups-20 "Idle wakeups, 20 tabs" "$wakeups /s" \
    "Ardia 13.56 /s, Chrome 30.86 /s (gate: ≤ Chrome)"
  perf_until $((loaded + 120))
  perf_idle settled-20
  perf_row settled-cpu-20 "Idle CPU, 20 tabs, settled" "$cpu %" "Ardia 0.31 %"
  perf_row settled-wakeups-20 "Idle wakeups, 20 tabs, settled" "$wakeups /s" "Ardia 11.82 /s"

  # ⌘1 is the newest Today tab, p20, which shows, so each key goes to another tab.
  : > "$perf_dir/switch"
  : > "$perf_dir/switch-core"
  for k in 2 3 4 5 6 7 8 1 2 3 4 5 6 7 8 1; do
    pressed=$(perf_now)
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
  done
  [ "$(today | wc -w | tr -d ' ')" = 1 ] || fail "⌘W 19 times didn't leave 1 tab: $(today)"
  perf_until $(($(date +%s) + 30))
  perf_memory closed
  perf_row memory-closed "Memory, back to 1 tab after 20" "${memory% *} MiB, ${memory#* } processes" \
    "Ardia 358 MB, Chrome 406 MB"
}
if [ "$active" = 1 ]; then
  perf_session
else
  echo "skip: perf's session. Browser wasn't the active app, so the page didn't get its keys."
fi
exec 3>&-
quit
echo "ok: perf measured $(wc -l < "$perf_dir/rows.tsv" | tr -d ' ') rows against #456's targets," \
  "failing on none; each time from a key includes the shell's clock, $clock ms. Rows:" \
  "$perf_dir/rows.tsv, usage snapshots: $perf_dir/"
