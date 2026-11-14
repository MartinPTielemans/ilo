# check.sh runs this at 840, only when named.
# It takes 16 minutes, Chrome's measures beside ilo's, so it runs only when named.
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
# - A new profile's first launch, 3 times, to the page painted with BROWSER_URL and how long uBlock
#   Origin held it, as it compiles the lists it ships. The last is the profile every launch after
#   uses, as a user's.
# - Where uBlock Origin holds a launch's page, 5 launches with no selfie of its filters, then one
#   that stays until it writes the selfie, so the launches after are on it, as a user's.
# - Launch to the page painted, 5 times, with BROWSER_URL, as a link from another app opens it.
# - Launch to the restored page painted, 5 times, with no URL, its last tab put back.
# - The first window on screen, as each of those 10 launches logged it: the launch window, on the
#   Mac and Windows, which shows the last sidebar where the browser window will open before the
#   browser starts, as #242 and Ardia's method time it; on Linux, which has none, the browser window.
#   The browser window on screen too, a row of its own.
# - Then one session, as Ardia's: memory at 1 tab, 30 s after launch; idle CPU and wakeups over
#   the minute after, and again 2 minutes after the page loaded (settled); 19 new tabs opened from
#   ⌘T's bar, ↩ to their page painted; memory at 10 and 20 tabs, 10 s and 15 s after; idle at 20
#   tabs and settled at 20; 16 switches by ⌘1–⌘8, the key to the page shown; 3 new windows, ⌘N to
#   their first frame; and memory back at 1 tab, 30 s after closing the others.
# - Then Google Chrome, where the runner has it, on the same page on the same machine, through
#   DevTools, as it has no driver: 5 launches with a link, to the page painted and the window on
#   screen, then the same session, its tabs opened and shown by DevTools, and the run says how far
#   ilo is from it on each row. Of those, memory at 10 and 20 tabs can fail the run.
# - With CHECK_TRACE_CATEGORIES (check/lib.sh), each browser's 16 switches again at 20 tabs, in a
#   launch of their own, the first 8 a second apart and the last 8 back to back, traced with those
#   through DevTools from the first switch to the last, as in #2377: perf/trace-ilo.json and
#   trace-chrome.json, and a row a switch in traced-switches.tsv, with its span, failing unless
#   each switch to a loaded tab has one. ilo's then go to 4 tabs without their page, unseen 2 hours.
#   The first 3 of the 19 new tabs before them are traced too, in trace-ilo-newtab.json and
#   trace-chrome-newtab.json, for where a new tab's time goes (#2383).
#
# Idle is measured in 10 s windows, six to the minute, and memory is summed over every process of
# the browser, as check/usage.py says for each OS. Each browser's first launch waits for its CRLSet,
# which it then keeps parsed in memory, and each memory sample says whether it was in (#2317). On
# Linux and the Mac the shell's clock is a process of its own, so each time from a key includes one
# process start, which the run says.

serve
# Windows turns the display off after the runner's idle minutes, which DevTools' input never resets
# as the driver's does, and Chrome then takes its window as hidden, whose new tab reports no paint
# (#2385), so the display stays on.
! windows || powercfg -change -monitor-timeout-ac 0 || echo "perf: the display's timeout stays"
perf_dir="$out/perf"
engine_switch=
# perf-native sets engine=native for the browser's own ad blocker (#2111), which holds no page.
if [ -n "${engine:-}" ] && [ "$engine" != ublock ]; then
  perf_dir="$out/perf-$engine"
  engine_switch=--ad-block-engine=$engine
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
      performance.mark(document.title);
    }
  }
}).observe({ type: "paint", buffered: true });
// Each stamp is also a mark, which a trace with blink.user_timing has on its own clock, to pair the
// trace's events with the run's Unix ms.
const stamped = (n) => requestAnimationFrame(() => requestAnimationFrame(() => {
  document.title = `${id} shown ${n} ${Date.now()}`;
  performance.mark(document.title);
}));
let shown = 0;
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState == "visible") {
    stamped(++shown);
  }
});
// A page loaded again in its tab as it's shown, as a tab without its page gets it back, has no
// visibilitychange, so it's stamped shown 0. A new tab's page, which perf times to its paint, isn't.
if (document.visibilityState == "visible" && performance.getEntriesByType("navigation")[0]?.type != "navigate") {
  stamped(0);
}
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
# The CRLSet, Google's list of revoked certificates, comes from the component updater a minute or
# more after a profile's first launch, and from then on the browser keeps it parsed, about 27 MiB
# for the set of 2026-11 (#2317). So each browser's warm launch, not measured, checks for updates
# 10 s in rather than a minute and waits for its set, and each memory sample says whether the set
# was in, as a slow download would otherwise make one browser's figure the smaller by its size.
# Chromium logs nothing at the parse, but at VLOG 1 its component installer logs each component's
# version and folder as it hands it over, at launch or once downloaded, and the CRLSet's policy
# then reads and parses it at once.
crlset_wait=300
# The flags that have a browser log that to $1, with --component-updater=fast-update if $2 is fast.
# Logging to a file still puts errors on stderr, as the checks read them there.
crlset_flags() {
  crlset_file=$1
  ! windows || crlset_file="$(cygpath -w "$crlset_file")"
  echo "${2:+--component-updater=fast-update }--enable-logging --vmodule=component_installer=1" \
    "--log-file=$crlset_file"
}
# The version of the CRLSet that log $1 says was handed over, with its file in user data folder $2.
crlset_in() {
  [ -f "$1" ] || return 0
  v=$(tr -d '\r' < "$1" |
    sed -n 's|^.*Component ready, version \([0-9][0-9]*\) in .*CertificateRevocation[/\\]\1$|\1|p' | tail -1)
  [ -z "$v" ] || [ ! -f "$2/CertificateRevocation/$v/crl-set" ] || echo "$v"
}
# Waits, until $crlset_wait s after $3 in Unix seconds, for the CRLSet of $4, which logs to $1 with
# user data folder $2, checking $pid is still there.
crlset_await() {
  until [ -n "$(crlset_in "$1" "$2")" ]; do
    if [ "$(date +%s)" -ge $(($3 + crlset_wait)) ]; then
      echo "perf: $4 CRLSet never arrived in the $crlset_wait s its first launch waited"
      return 0
    fi
    kill -0 "$pid" 2> /dev/null || fail "$4 first launch ended while it waited for its CRLSet"
    sleep 1
  done
  echo "perf: $4 CRLSet $(crlset_in "$1" "$2") arrived $(($(date +%s) - $3)) s into its first launch"
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
# The memory now, in $memory: "MiB processes", and by kind of process in $kinds; whether the
# browser had its CRLSet in, which $crlset_log and $crlset_dir say, in $settled.
perf_memory() {
  perf_snap "memory-$1" kinds
  memory=$(python3 -I check/usage.py memory "$perf_dir/memory-$1.json")
  kinds=$(python3 -I check/usage.py kinds "$perf_dir/memory-$1.json")
  crlset_in "$crlset_log" "$crlset_dir" > "$perf_dir/crlset-$1"
  if [ -s "$perf_dir/crlset-$1" ]; then
    settled="; CRLSet $(cat "$perf_dir/crlset-$1") in"
  else
    settled="; not settled: $crlset_who CRLSet never arrived"
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
# sets $painted to the ms from just before the launch, $on_screen to the first window's time, and
# $browser_on_screen to the browser window's.
perf_launch() {
  log="$perf_dir/launch-$3.log"
  perf_stamp
  launched=$stamp
  if [ -n "$1" ]; then
    # shellcheck disable=SC2046,SC2086 # the flags, a word each
    BROWSER_URL="$1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" $engine_switch \
      $([ "$3" != warm ] || crlset_flags "$PWD/$perf_dir/crlset-warm.log" fast) \
      < /dev/null > "$log" 2>&1 &
  else
    BROWSER_PROFILE_DIR="$profile_dir" "$browser" $engine_switch < /dev/null > "$log" 2>&1 &
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
  browser_on_screen=$(sed -n 's/^browser: window on screen at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
  # Linux logs the launch window's line with no window behind it (platform/linux/host.cc).
  on_screen=$browser_on_screen
  if [ "$(uname)" = Darwin ] || windows; then
    on_screen=$(sed -n 's/^browser: launch window at \([0-9.]*\) ms$/\1/p' "$log" | head -1)
  fi
  # The core holds a launch's first page until uBlock Origin is ready (core/ad_block.cc).
  held=$(sed -n 's/^.*browser: ad blocking held a page \([0-9]*\) ms.*$/\1/p' "$log" | head -1)
  [ "$3" != warm ] || crlset_await "$PWD/$perf_dir/crlset-warm.log" "$profile_dir" $((launched / 1000)) "ilo's"
  quit
}
: > "$perf_dir/link"
: > "$perf_dir/restore"
: > "$perf_dir/on-screen"
: > "$perf_dir/browser-on-screen"
: > "$perf_dir/held"
# A new profile's first launch, 3 times: uBlock Origin compiles the lists it ships. The last profile
# is the one every launch after uses, as a user's.
: > "$perf_dir/new"
: > "$perf_dir/new-held"
for n in 1 2 3; do
  rm -rf "$profile_dir"
  mkdir -p "$profile_dir"
  perf_launch "$page?new$n" "new$n" "new$n"
  echo "$painted" >> "$perf_dir/new"
  echo "${held:-0}" >> "$perf_dir/new-held"
done
perf_row launch-new "Launch, a new profile's first, link to page painted" \
  "$(perf_stats '%.0f' < "$perf_dir/new") ms, held $(perf_stats '%.0f' < "$perf_dir/new-held") ms" \
  "none: once a profile"
# uBlock Origin's selfie (#2381), all in this block and lib.sh's, which go whole with uBlock
# Origin (#2111).
# A launch takes its filters in from its selfie, the filters it compiled, or with none from its
# lists. It writes the selfie 53 s after its lists last changed, in the session then, and takes it
# out as a list changes, so a launch after a list update has none, as has each launch while no
# session has lasted the 53 s. Those are 5 launches that quit seconds in; then one stays until the
# selfie is written, and the launches after it are on it, a user's.
if [ -n "$held" ]; then
  : > "$perf_dir/unselfied"
  : > "$perf_dir/unselfied-held"
  for n in 1 2 3 4 5; do
    ! ublock_selfie ||
      fail "uBlock Origin wrote its selfie before launch unselfied$n, in one that quit seconds in"
    rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
    perf_launch "$page?unselfied$n" "unselfied$n" "unselfied$n"
    echo "$painted" >> "$perf_dir/unselfied"
    echo "${held:-0}" >> "$perf_dir/unselfied-held"
  done
  unselfied=$(perf_stats '%.0f' < "$perf_dir/unselfied")
  perf_row launch-unselfied "Launch with no uBlock Origin selfie, link to page painted" \
    "$unselfied ms, held $(perf_stats '%.0f' < "$perf_dir/unselfied-held") ms" \
    "none: after a list update, until a session lasts 53 s"
  selfie_launch "$page?selfie" "$perf_dir/launch-selfie.log"
  echo "perf: uBlock Origin wrote its selfie $selfie_at s into a launch that stayed;" \
    "the launches below are on it"
  selfied=", on uBlock Origin's selfie"
fi
for n in 1 2 3 4 5; do
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  perf_launch "$page?link$n" "link$n" "link$n"
  echo "$painted" >> "$perf_dir/link"
  echo "$on_screen" >> "$perf_dir/on-screen"
  echo "$browser_on_screen" >> "$perf_dir/browser-on-screen"
  echo "${held:-0}" >> "$perf_dir/held"
done
for n in 1 2 3 4 5; do
  perf_launch "" link5 "restore$n"
  echo "$painted" >> "$perf_dir/restore"
  echo "$on_screen" >> "$perf_dir/on-screen"
  echo "$browser_on_screen" >> "$perf_dir/browser-on-screen"
  echo "${held:-0}" >> "$perf_dir/held"
done
perf_row launch-link "Launch, link to page painted${selfied:-}" \
  "$(perf_stats '%.0f' < "$perf_dir/link") ms" "Ardia 413 ms, Chrome 484 ms"
perf_row launch-window "Launch, window on screen" "$(perf_stats '%.0f' < "$perf_dir/on-screen") ms" \
  "Ardia 280 ms, Chrome 424 ms (gate: ≤ Chrome)"
perf_row launch-browser-window "Launch, browser window on screen" \
  "$(perf_stats '%.0f' < "$perf_dir/browser-on-screen") ms" "none: on Linux it's the row above"
perf_row launch-restore "Launch, restored page painted${selfied:-}" \
  "$(perf_stats '%.0f' < "$perf_dir/restore") ms" "Ardia 360 ms, Helium 355 ms, Chrome 438 ms"
perf_row launch-held "Launch, the page held for uBlock Origin${selfied:-}" \
  "$(perf_stats '%.0f' < "$perf_dir/held") ms" "none: it's part of the two above"

log="$perf_dir/session.log"
# #2316, not for merge: Chromium's memory-infra, a detailed dump every 20 s, for ilo.
trace_result="$PWD/$perf_dir/trace-ilo.json"
trace_config="$PWD/$perf_dir/trace-ilo.config"
! windows || trace_result=$(cygpath -w "$trace_result")
printf '{"trace_config":{"included_categories":["disabled-by-default-memory-infra"],"excluded_categories":["*"],"record_mode":"record-until-full","trace_buffer_size_in_kb":400000,"memory_dump_config":{"triggers":[{"mode":"detailed","periodic_interval_ms":20000}]}},"startup_duration":245,"result_file":"%s"}\n' \
  "$(printf '%s' "$trace_result" | sed 's/\\/\\\\/g')" > "$trace_config"
! windows || trace_config=$(cygpath -w "$trace_config")
step=0
rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
started=$(date +%s)
crlset_log="$PWD/$perf_dir/crlset-session.log"
crlset_dir=$profile_dir
crlset_who="ilo's"
# shellcheck disable=SC2046,SC2086 # the flags, a word each
BROWSER_DRIVE=1 BROWSER_URL="$page?p1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" $engine_switch \
  $(crlset_flags "$crlset_log") --trace-config-file="$trace_config" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await "^browser: title [0-9]* p1 fcp " || fail "the session's first page didn't paint"
send click $page_point

# Opens page $1 from ⌘T's bar and keeps the ms from ↩ to its first paint in the file $2.
perf_open() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
  send type "$page?$1"
  perf_stamp
  typed=$stamp
  send key return
  expect_late "^browser: title [0-9]* $1 fcp " "$1 didn't paint in its new tab"
  echo $(($(perf_painted "$1" "$step") - typed)) >> "$perf_dir/$2"
  loaded=$(date +%s)
}
# 16 switches by ⌘1–⌘8 at 20 tabs, keeping the ms from the key to the page shown in the file $1,
# to the core's report in $1-core and each key's Unix ms in $1-keys, and a line a switch in $1-rows
# for check/switch_table.py. ⌘1 is the newest Today tab, which shows, so each key goes to another
# tab. With $2, the first 8 wait 1 s after their page shows, so each switch's work, and the hover
# card's picture of the page it left after it, stand alone in a trace; the last 8 are back to back.
perf_switches() {
  : > "$perf_dir/$1"
  : > "$perf_dir/$1-core"
  : > "$perf_dir/$1-keys"
  : > "$perf_dir/$1-rows"
  i=0
  for k in 2 3 4 5 6 7 8 1 2 3 4 5 6 7 8 1; do
    i=$((i + 1))
    perf_stamp
    pressed=$stamp
    send key "$k" cmd
    echo "$pressed" >> "$perf_dir/$1-keys"
    expect "^browser: selected [0-9]* in " "⌘$k didn't switch tabs"
    since=$(sed -n "/^browser: drive $step: /,\$p" "$log")
    tab=$(echo "$since" | sed -n 's/^browser: selected \([0-9]*\) in .*/\1/p' | head -1)
    echo "$since" | sed -n 's/^browser: selected [0-9]* in \([0-9.]*\) ms$/\1/p' | head -1 \
      >> "$perf_dir/$1-core"
    expect "^browser: title $tab [^ ]* shown " "tab $tab didn't say it was shown after ⌘$k"
    title=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
      sed -n "s/^browser: title $tab \([^ ]* shown [0-9]* [0-9]*\)$/\1/p" | tail -1)
    shown=${title##* }
    echo $((shown - pressed)) >> "$perf_dir/$1"
    echo "loaded $step ${title% *} $pressed $shown" | sed 's/ shown / /' >> "$perf_dir/$1-rows"
    [ -z "${2:-}" ] || [ "$i" -gt 8 ] || sleep 1
  done
}
# The session, once the page has the active app's keyboard.
perf_session() {
  perf_until $((started + 30))
  perf_memory 1
  perf_row memory-1 "Memory, 1 tab" "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" \
    "Ardia 298 MB, Chrome 370 MB (gate: ≤ Chrome)"
  perf_idle idle-1
  perf_row idle-cpu-1 "Idle CPU, 1 tab" "$cpu" "Ardia 0.16 %, Chrome 0.2 %"
  perf_row idle-wakeups-1 "Idle wakeups, 1 tab" "$wakeups /s" "Ardia 5.27 /s, Chrome 6.77 /s"
  perf_until $((started + 120))
  perf_idle settled-1
  perf_row settled-cpu-1 "Idle CPU, 1 tab, settled" "$cpu" "Ardia 0.05 %"
  perf_row settled-wakeups-1 "Idle wakeups, 1 tab, settled" "$wakeups /s" "Ardia 3.11 /s"

  : > "$perf_dir/newtab"
  for n in 2 3 4 5 6 7 8 9 10; do
    perf_open "p$n" newtab
  done
  perf_until $((loaded + 10))
  perf_memory 10
  perf_row memory-10 "Memory, 10 tabs" "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" \
    "Ardia 714 MB, Chrome 692 MB (gate: ≤ Chrome)"
  for n in 11 12 13 14 15 16 17 18 19 20; do
    perf_open "p$n" newtab
  done
  perf_until $((loaded + 15))
  perf_memory 20
  perf_row memory-20 "Memory, 20 tabs" "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" \
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

  perf_switches switch
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
  perf_row memory-closed "Memory, back to 1 tab after 20" "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" \
    "Ardia 358 MB, Chrome 406 MB"
}
if [ "$active" = 1 ]; then
  perf_session
else
  echo "skip: perf's session. Browser wasn't the active app, so the page didn't get its keys."
fi
exec 3>&-
quit

# With CHECK_TRACE_CATEGORIES, each browser's 16 switches again at 20 tabs, in a launch of their own,
# as tracing slows what it times and the session's rows stay untraced. DevTools traces from just
# before the first switch to just after the last, so the buffer holds them all, where a whole
# session's would overflow. Each switch is a row of perf/traced-switches.tsv, with its span from the
# trace, and the run fails unless each switch to a loaded tab has one: the trace is what this leg
# is for.
traced_tsv="$perf_dir/traced-switches.tsv"
# Traces into perf/$1 what the command after does, through the DevTools client logging to $2.
perf_traced() {
  perf_trace_file=$1 perf_trace_log=$2
  shift 2
  since=$(wc -l < "$perf_trace_log")
  echo "trace start $trace_categories" >&5
  perf_cdp_await "$perf_trace_log" "$since" '^cdp: tracing$' "DevTools didn't start tracing"
  "$@"
  since=$(wc -l < "$perf_trace_log")
  echo "trace end $perf_dir/$perf_trace_file" >&5
  perf_cdp_await "$perf_trace_log" "$since" "^cdp: traced $perf_dir/$perf_trace_file" \
    "DevTools didn't write $perf_trace_file"
  ! tail -n +$((since + 1)) "$perf_trace_log" | grep -q "^cdp: traced .* lossy$" ||
    echo "perf: $perf_trace_file: the browser dropped some of the trace"
}
# Traces the command after into perf/$1 with perf_traced, then adds its switches, kept in perf/$4,
# to the table as browser $3's, with the browser's log $5.
perf_trace() {
  perf_trace_browser=$3 perf_trace_rows=$4 perf_trace_app=$5
  perf_trace_at=$1 perf_trace_via=$2
  shift 5
  perf_traced "$perf_trace_at" "$perf_trace_via" "$@"
  [ -s "$traced_tsv" ] || printf 'browser\tkind\tpage\tshown\tkey_ms\tshown_ms\ttab_state\tspan_ms\tselected_ms\tlet_go_ms\n' > "$traced_tsv"
  python3 -I check/switch_table.py "$perf_trace_browser" "$perf_dir/$perf_trace_file" \
    "$perf_dir/$perf_trace_rows" ${perf_trace_app:+"$perf_trace_app"} >> "$traced_tsv" 2> "$perf_dir/traced-missing" ||
    fail "$perf_trace_file lacks spans ($trace_categories): $(tr '\n' ';' < "$perf_dir/traced-missing")"
}
# Waits up to 60 s for a line matching $3 in the DevTools log $1 after its line $2, failing with $4.
perf_cdp_await() {
  for _ in $(seq 600); do
    tail -n +$(($2 + 1)) "$1" | grep -q "$3" && return 0
    kill -0 "$pid" 2> /dev/null || fail "the browser exited, waiting for: $4"
    sleep 0.1
  done
  fail "$4: $(tail -3 "$1")"
}
# Waits up to 10 s for a line matching $1 in the log since this step, without failing the run.
perf_saw() {
  for _ in $(seq 100); do
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "$1" && return 0
    sleep 0.1
  done
  return 1
}
# The 16 switches, then ilo's alone, as DevTools can't take a page from Chrome's tab: unseen 2
# hours, each tab but the one shown lets go of its page at the next switch, ⌘2's, then ⌘3–⌘6 to 4
# tabs without it, each from the key to its page shown again, in perf/traced-unloaded and their rows
# with the 16's. They're rows beside the switches, so a step that doesn't come says so rather than
# failing the run.
perf_traced_ilo() {
  perf_switches traced-switch spaced
  : > "$perf_dir/traced-unloaded"
  send age 7200
  perf_stamp
  pressed=$stamp
  send key 2 cmd
  perf_saw 'browser: idle tabs checked$' ||
    { echo "perf: unseen 2 hours, ⌘2 didn't check for idle tabs, so no switches to tabs without their page"; return 0; }
  perf_saw "^browser: title [0-9]* t19 shown " &&
    title=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n "s/^browser: title [0-9]* \(t19 shown [0-9]* [0-9]*\)$/\1/p" | tail -1) &&
    echo "check $step ${title% *} $pressed ${title##* }" | sed 's/ shown / /' >> "$perf_dir/traced-switch-rows"
  for k in 3 4 5 6; do
    # Not $page, the perf page's address, which Chrome's launches after take.
    unloaded=t$((21 - k))
    perf_stamp
    pressed=$stamp
    send key "$k" cmd
    perf_saw "^browser: selected [0-9]* in " || { echo "perf: ⌘$k didn't switch to $unloaded"; return 0; }
    tab=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n 's/^browser: selected \([0-9]*\) in .*/\1/p' | head -1)
    # The core's line, after Chromium's prefix. A page loaded as it's shown stamps shown 0.
    if ! perf_saw "browser: tab $tab has its page" || ! perf_saw "^browser: title $tab $unloaded shown 0 "; then
      echo "perf: tab $tab, $unloaded by ⌘$k, didn't get its page back and say it was shown"
      return 0
    fi
    shown=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n "s/^browser: title $tab $unloaded shown 0 //p" | tail -1)
    echo $((shown - pressed)) >> "$perf_dir/traced-unloaded"
    echo "unloaded $step $unloaded 0 $pressed $shown" >> "$perf_dir/traced-switch-rows"
  done
}
# Rows $1, the traced switches to the page shown, and $1-span, the spans of browser $3's switches to
# loaded tabs in the table, labelled $2.
perf_traced_rows() {
  perf_row "$1" "$2, traced, to page shown" "$(perf_stats '%.0f' < "$perf_dir/$1") ms" "untraced's"
  perf_row "$1-span" "$2, traced, Chrome's span, tab strip to frame shown" \
    "$(awk -F '\t' -v b="$3" '$1 == b && $2 == "loaded" { print $8 }' "$traced_tsv" | perf_stats '%.1f') ms" \
    "Chrome 8 ms (gate: 16 ms)"
}
if [ -n "${CHECK_TRACE_CATEGORIES:-}" ] && [ "$active" = 1 ]; then
  log="$perf_dir/traced.log"
  cdp_log="$perf_dir/traced-cdp.log"
  step=0
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  scratch=$(mktemp -d)
  start_with_cdp "$page?t1" "$profile_dir" ""
  await "^browser: title [0-9]* t1 fcp " || fail "the traced launch's first page didn't paint"
  send click $page_point
  [ "$active" = 1 ] || fail "Browser wasn't the active app for the traced switches"
  : > "$perf_dir/traced-newtab"
  # The first 3 new tabs, traced on their own into perf/trace-ilo-newtab.json, for where a new
  # tab's ms go (#2383).
  perf_traced_opens() {
    for n in 2 3 4; do
      perf_open "t$n" traced-newtab
    done
  }
  perf_traced trace-ilo-newtab.json "$cdp_log" perf_traced_opens
  for n in $(seq 5 20); do
    perf_open "t$n" traced-newtab
  done
  perf_until $((loaded + 15))
  perf_trace trace-ilo.json "$cdp_log" ilo traced-switch-rows "$log" perf_traced_ilo
  perf_traced_rows traced-switch "Tab switch" ilo
  [ ! -s "$perf_dir/traced-unloaded" ] || perf_row traced-unloaded \
    "Tab switch to a tab without its page, traced, key to page shown" \
    "$(perf_stats '%.0f' < "$perf_dir/traced-unloaded") ms" "none of its own"
  exec 3>&- 5>&-
  quit
  kill "$helper" 2> /dev/null || true
  helper=
  rm -rf "$scratch"
  scratch=
fi

# Google Chrome, where the runner has it, as GitHub's images do, measured on this machine as ilo was,
# for each of the gate's rows that compares with it, on the same page. Chrome has no driver, so
# DevTools opens its tabs in front, as a link from another app does, a couple of seconds apart, and
# shows them through the tab strip, as ⌘1–⌘8 do; ilo's are timed from ↩ in ⌘T's bar and from the
# key. A profile of its own, launched once first, as ilo's was.
# CHECK_CHROME names another, such as Chrome for Testing on the owner's Mac, whose own Chrome a run
# never launches or quits: off CI, there's none without it.
chrome_id=com.google.Chrome
if [ -n "${CHECK_CHROME:-}" ]; then
  chrome=$CHECK_CHROME
  chrome_procs=${chrome%/MacOS/*}
  [ "$(uname)" != Darwin ] || chrome_id=$(defaults read "$chrome_procs/Info" CFBundleIdentifier)
  [ "$(uname)" = Darwin ] || chrome_procs=$chrome
elif [ -z "${CI:-}" ]; then
  chrome=none
elif [ "$(uname)" = Darwin ]; then
  chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
  chrome_procs="/Applications/Google Chrome.app/Contents"
elif windows; then
  chrome="/c/Program Files/Google/Chrome/Application/chrome.exe"
  chrome_procs="C:/Program Files/Google/Chrome/Application/chrome.exe"
else
  chrome=/opt/google/chrome/chrome
  chrome_procs=/opt/google/chrome/chrome
fi
chrome_dir="$PWD/$perf_dir/chrome-profile"
chrome_profile=$chrome_dir
! windows || chrome_profile="$(cygpath -w "$chrome_profile")"
chrome_launches=0
# On the Mac, Chrome opens 40 points into the display the app does, and each launch checks it's
# there: on a Mac with another display, the main one may be playing something (AGENTS.md).
chrome_at=
if [ "$(uname)" = Darwin ] && [ "$chrome" != none ]; then
  chrome_display=$(display_bounds)
  chrome_at=$(echo "$chrome_display" | awk '{ print $1 + 40, $2 + 40 }')
fi
# Launches Chrome on page $1, with the arguments after, and check/cdp.py on the DevTools port it
# picks, which logs its pages' titles to $log, as the app's log has its own, and takes commands on
# fd 5. Sets $pid and $launched, the Unix ms just before, or says Chrome didn't stay.
chrome_launch() {
  chrome_url="$page?$1"
  shift
  chrome_launches=$((chrome_launches + 1))
  log="$perf_dir/chrome-$chrome_launches.log"
  rm -f "$chrome_dir/DevToolsActivePort"
  perf_stamp
  launched=$stamp
  # shellcheck disable=SC2046 # the place, two words, or none
  "$chrome" --user-data-dir="$chrome_profile" --no-first-run --no-default-browser-check \
    --use-mock-keychain --password-store=basic --remote-debugging-port=0 \
    $([ -z "$chrome_at" ] || echo "--window-position=${chrome_at% *},${chrome_at#* }" \
      --window-size=1280,800) "$@" "$chrome_url" < /dev/null >> "$perf_dir/chrome.log" 2>&1 &
  pid=$!
  # Its port, then the browser's path, with no newline after, once DevTools listens, which a
  # runner's first launch of it, from a cold disk, took 15.5 s to (#2385).
  tries=0
  until grep -q '^/devtools/browser/' "$chrome_dir/DevToolsActivePort" 2> /dev/null; do
    kill -0 "$pid" 2> /dev/null || return 1
    tries=$((tries + 1))
    [ "$tries" -le 600 ] || fail "Chrome's DevTools didn't listen within 60 s of its launch"
    sleep 0.1
  done
  chrome_socket="ws://127.0.0.1:$(tr -d '\r' < "$chrome_dir/DevToolsActivePort" | sed -n 1p)$(
    tr -d '\r' < "$chrome_dir/DevToolsActivePort" | sed -n 2p)"
  case $chrome_socket in
  ws://127.0.0.1:[0-9]*/devtools/browser/*) ;;
  *) fail "Chrome's DevToolsActivePort didn't say where it listens: $chrome_socket" ;;
  esac
  rm -f "$perf_dir/chrome-cdp"
  mkfifo "$perf_dir/chrome-cdp"
  # A native program can't read Git Bash's fifo, so cat passes cdp.py its commands, as lib.sh's
  # start_with_cdp does.
  cat "$perf_dir/chrome-cdp" | python3 -I -u check/cdp.py --socket "$chrome_socket" > "$log" 2>&1 &
  exec 5> "$perf_dir/chrome-cdp"
  echo titles >&5
  if [ -n "$chrome_at" ]; then
    echo window >&5
    chrome_await "^cdp: window " "Chrome's DevTools didn't say where its window is"
    at=$(grep '^cdp: window ' "$log" | tail -1 | cut -d' ' -f3-)
    if ! echo "$chrome_display $at" | awk '{ exit !($5 >= $1 && $6 >= $2 && $5 + $7 <= $1 + $3 &&
      $6 + $8 <= $2 + $4) }'; then
      chrome_quit
      fail "Chrome's window opened at $at, not inside the app's display, $chrome_display"
    fi
  fi
}
# Waits for a line matching $1 in Chrome's log after its line $3, or any, failing with $2.
chrome_await() {
  for _ in $(seq 300); do
    tail -n +$((${3:-0} + 1)) "$log" | grep -q "$1" && return 0
    kill -0 "$pid" 2> /dev/null || fail "Chrome exited, waiting for: $2"
    sleep 0.1
  done
  fail "$2"
}
# The Unix ms page $1's title says it $2 ("fcp" or "shown"), first after Chrome's log line $3.
chrome_stamp() {
  tail -n +$((${3:-0} + 1)) "$log" | sed -n "s/^cdp: title [^ ]* $1 $2 \([0-9]* \)\{0,1\}//p" | head -1
}
# Chrome's histogram $1's sum, once it has one, or none after 5 s.
chrome_histogram() {
  for _ in $(seq 50); do
    since=$(wc -l < "$log")
    echo "histogram $1" >&5
    chrome_await "^cdp: histogram $1 " "Chrome's DevTools didn't answer for $1" "$since"
    sum=$(tail -n +$((since + 1)) "$log" | sed -n "s/^cdp: histogram $1 \([0-9]*\) [0-9]*$/\1/p" | head -1)
    [ -z "$sum" ] || { echo "$sum"; return; }
    sleep 0.1
  done
}
# Opens page $1 in a new tab in front, as a link from another app does, and keeps the ms from just
# before, as cdp.py stamps it, to its first paint in the file $2, as the app's are kept from ↩ in
# ⌘T's bar.
chrome_open() {
  since=$(wc -l < "$log")
  echo "open $page?$1" >&5
  chrome_await "^cdp: title [^ ]* $1 fcp " "Chrome's $1 didn't paint in its new tab" "$since"
  typed=$(tail -n +$((since + 1)) "$log" | sed -n 's/^cdp: opening at \([0-9]*\)$/\1/p' | head -1)
  echo $(($(chrome_stamp "$1" fcp "$since") - typed)) >> "$perf_dir/$2"
  loaded=$(date +%s)
}
# The app's 16 switches by ⌘1–⌘8 at 20 tabs, to the same pages, $1<n>: ⌘2–⌘8 its tabs opened before
# the last, 19 to 13, and ⌘1 the last, 20, which shows first, twice. Chrome's are through the tab
# strip, keeping the ms from just before, as cdp.py stamps it, to the page shown in the file $2, each
# Unix ms before in $2-keys and a line a switch in $2-rows. With $3, spaced as the app's.
chrome_switches() {
  : > "$perf_dir/$2"
  : > "$perf_dir/$2-keys"
  : > "$perf_dir/$2-rows"
  i=0
  for k in 2 3 4 5 6 7 8 1 2 3 4 5 6 7 8 1; do
    i=$((i + 1))
    n=$((21 - k))
    [ "$k" != 1 ] || n=20
    tab=$(sed -n "s/^cdp: title \([^ ]*\) $1$n fcp .*/\1/p" "$log" | head -1)
    since=$(wc -l < "$log")
    echo "activate $tab" >&5
    chrome_await "^cdp: title $tab $1$n shown " "Chrome's $1$n didn't say it was shown" "$since"
    pressed=$(tail -n +$((since + 1)) "$log" | sed -n "s/^cdp: activating $tab at \([0-9]*\)$/\1/p" | head -1)
    count=$(tail -n +$((since + 1)) "$log" | sed -n "s/^cdp: title $tab $1$n shown \([0-9]*\) .*/\1/p" | head -1)
    shown=$(chrome_stamp "$1$n" shown "$since")
    echo "$pressed" >> "$perf_dir/$2-keys"
    echo $((shown - pressed)) >> "$perf_dir/$2"
    echo "loaded - $1$n $count $pressed $shown" >> "$perf_dir/$2-rows"
    [ -z "${3:-}" ] || [ "$i" -gt 8 ] || sleep 1
  done
}
# Quits Chrome as quit_app does the app, then ends what's left of it after $1 s, or 20.
chrome_quit() {
  # cdp.py ends as its commands do, and as Chrome closes its socket.
  exec 5>&-
  if windows; then
    # Without /F, taskkill asks its windows to close, as their close button does.
    taskkill //PID "$(cat "/proc/$pid/winpid")" > /dev/null 2>&1 || true
  elif [ "$(uname)" = Darwin ]; then
    osascript -e "tell application id \"$chrome_id\" to quit" || true
  else
    kill -INT "$pid" 2> /dev/null || true
  fi
  for _ in $(seq $((${1:-20} * 2))); do
    perf_snap chrome-left
    python3 -I check/usage.py memory "$perf_dir/chrome-left.json" | grep -q ' 0$' && return 0
    sleep 0.5
  done
  echo "Chrome didn't quit in ${1:-20} s, so it's ended"
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
if [ "$active" != 1 ]; then
  :
elif [ ! -x "$chrome" ]; then
  if [ -n "${CI:-}" ] || [ -n "${CHECK_CHROME:-}" ]; then
    echo "skip: perf's Chrome on this runner. It has none at $chrome."
  else
    echo "skip: perf's Chrome. Off CI, CHECK_CHROME names the one to run, as the owner's own never is."
  fi
else
  snap_procs=$chrome_procs
  : > "$perf_dir/chrome.log"
  # The 5 s a Chrome that won't stay takes to go, in its first 15 s, which warm its caches for the
  # launches measured.
  # It also checks for updates 10 s in and waits for its CRLSet, as ilo's warm launch does.
  # shellcheck disable=SC2046 # the flags, a word each
  if ! chrome_launch warm $(crlset_flags "$PWD/$perf_dir/crlset-chrome-warm.log" fast) ||
    ! sleep 15 || ! kill -0 "$pid" 2> /dev/null; then
    echo "skip: perf's Chrome on this runner. It didn't stay open: $(tail -3 "$perf_dir/chrome.log")"
  else
    crlset_await "$PWD/$perf_dir/crlset-chrome-warm.log" "$chrome_dir" $((launched / 1000)) \
      "Chrome's"
    chrome_quit
    # Five launches with a link, as the app's: to the page painted, and to the window on screen,
    # which Chrome times from its main to its window shown (Startup.BrowserWindowDisplay), and from
    # its process's start to its main, as the app's log times its own from its process's start.
    : > "$perf_dir/chrome-link"
    : > "$perf_dir/chrome-on-screen"
    for n in 1 2 3 4 5; do
      chrome_launch "cl$n" || fail "Chrome didn't stay open for launch $n: $(tail -3 "$perf_dir/chrome.log")"
      chrome_await "^cdp: title [^ ]* cl$n fcp " "Chrome's launch $n didn't paint its page"
      echo $(($(chrome_stamp "cl$n" fcp) - launched)) >> "$perf_dir/chrome-link"
      shown=$(chrome_histogram Startup.BrowserWindowDisplay)
      [ -n "$shown" ] || fail "Chrome's launch $n timed no window shown"
      main=$(chrome_histogram Startup.LoadTime.ProcessCreateToApplicationStart)
      [ -n "$main" ] || fail "Chrome's launch $n timed no start to its main"
      echo $((main + shown)) >> "$perf_dir/chrome-on-screen"
      chrome_quit
    done
    perf_row chrome-launch-link "Chrome here, launch, link to page painted" \
      "$(perf_stats '%.0f' < "$perf_dir/chrome-link") ms" "ilo's"
    perf_row chrome-launch-window "Chrome here, launch, window on screen" \
      "$(perf_stats '%.0f' < "$perf_dir/chrome-on-screen") ms" "ilo's, the gate"

    # Then the app's session, as the app's: the same rows at the same times, its tabs opened by
    # DevTools, as Chrome has no driver.
    # Its memory samples say whether its CRLSet was in, from the session's own log.
    crlset_log="$PWD/$perf_dir/crlset-chrome.log"
    crlset_dir=$chrome_dir
    crlset_who="Chrome's"
    # shellcheck disable=SC2046 # the flags, a word each
    chrome_launch c1 $(crlset_flags "$crlset_log") ||
      fail "Chrome didn't stay open for its session: $(tail -3 "$perf_dir/chrome.log")"
    chrome_await "^cdp: title [^ ]* c1 fcp " "Chrome's session's first page didn't paint"
    started=$((launched / 1000))
    perf_until $((started + 30))
    perf_memory chrome-1
    perf_row chrome-memory-1 "Chrome here, memory, 1 tab" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" "ilo's, the gate"
    # cdp.py asks for the titles each 50 ms, which idle mustn't count.
    echo "titles off" >&5
    perf_idle chrome-idle-1
    perf_row chrome-idle-cpu-1 "Chrome here, idle CPU, 1 tab" "$cpu" "ilo's"
    perf_row chrome-idle-wakeups-1 "Chrome here, idle wakeups, 1 tab" "$wakeups /s" "ilo's"
    perf_until $((started + 120))
    perf_idle chrome-settled-1
    perf_row chrome-settled-cpu-1 "Chrome here, idle CPU, 1 tab, settled" "$cpu" "ilo's"
    perf_row chrome-settled-wakeups-1 "Chrome here, idle wakeups, 1 tab, settled" "$wakeups /s" "ilo's"
    echo titles >&5
    # A couple of seconds apart, as before Chrome's new tabs were timed.
    : > "$perf_dir/chrome-newtab"
    for n in 2 3 4 5 6 7 8 9 10; do
      chrome_open "c$n" chrome-newtab
      perf_until $((typed / 1000 + 2))
    done
    perf_until $(($(date +%s) + 10))
    perf_memory chrome-10
    perf_row chrome-memory-10 "Chrome here, memory, 10 tabs" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" "ilo's, the gate"
    for n in 11 12 13 14 15 16 17 18 19 20; do
      chrome_open "c$n" chrome-newtab
      perf_until $((typed / 1000 + 2))
    done
    perf_until $(($(date +%s) + 15))
    perf_memory chrome-20
    perf_row chrome-memory-20 "Chrome here, memory, 20 tabs" \
      "${memory% *} MiB, ${memory#* } processes ($kinds)$settled" "ilo's, the gate"
    perf_row chrome-newtab "Chrome here, new tab, cdp.py's stamp to page painted" \
      "$(perf_stats '%.0f' < "$perf_dir/chrome-newtab") ms" "ilo's, the gate"
    echo "titles off" >&5
    perf_idle chrome-idle-20
    perf_row chrome-idle-cpu-20 "Chrome here, idle CPU, 20 tabs" "$cpu" "ilo's, the gate"
    perf_row chrome-idle-wakeups-20 "Chrome here, idle wakeups, 20 tabs" "$wakeups /s" "ilo's, the gate"
    perf_until $((loaded + 120))
    perf_idle chrome-settled-20
    perf_row chrome-settled-cpu-20 "Chrome here, idle CPU, 20 tabs, settled" "$cpu" "ilo's"
    perf_row chrome-settled-wakeups-20 "Chrome here, idle wakeups, 20 tabs, settled" "$wakeups /s" "ilo's"
    echo titles >&5
    chrome_switches c chrome-switch
    perf_row chrome-switch "Chrome here, tab switch, cdp.py's stamp to page shown" \
      "$(perf_stats '%.0f' < "$perf_dir/chrome-switch") ms" "ilo's, the gate"
    chrome_quit
    # The traced switches, in a launch of Chrome's own, as ilo's, its trace perf/trace-chrome.json.
    if [ -n "${CHECK_TRACE_CATEGORIES:-}" ]; then
      chrome_launch t1 || fail "Chrome didn't stay open for its traced switches: $(tail -3 "$perf_dir/chrome.log")"
      chrome_await "^cdp: title [^ ]* t1 fcp " "Chrome's traced launch didn't paint its page"
      : > "$perf_dir/chrome-traced-newtab"
      chrome_traced_opens() {
        for n in $(seq "$1" "$2"); do
          chrome_open "t$n" chrome-traced-newtab
          perf_until $((typed / 1000 + 2))
        done
      }
      # As ilo's, its first 3 new tabs traced on their own, into perf/trace-chrome-newtab.json.
      perf_traced trace-chrome-newtab.json "$log" chrome_traced_opens 2 4
      chrome_traced_opens 5 20
      perf_until $((loaded + 15))
      perf_trace trace-chrome.json "$log" chrome chrome-traced-switch-rows "" \
        chrome_switches t chrome-traced-switch spaced
      perf_traced_rows chrome-traced-switch "Chrome here, tab switch" chrome
      chrome_quit
    fi
    # ilo against Chrome on this machine, for each of the gate's rows. Memory at 10 and 20 tabs moves
    # under 5 % from run to run, so ilo more than 10 % over Chrome's fails the run. At 1 tab, 30 s
    # after launch, uBlock Origin is still taking in its lists, and ilo's moved by 70 % from one run
    # to the next, and idle moves 2 to 5 times from one 10 s window to the next on a hosted runner,
    # so those only report. A memory row where either browser's sample hadn't its CRLSet in isn't a
    # fair comparison either way, so it reports that and doesn't decide.
    python3 -I - "$perf_dir" << 'PY' | tee "$perf_dir/gate.txt"
import json, os, statistics, sys

d = sys.argv[1]
def memory(name):
    with open(os.path.join(d, f"memory-{name}.json")) as f:
        return sum(p[3] for p in json.load(f)["procs"].values()) / 1048576
def unsettled(*samples):
    for who, name in samples:
        with open(os.path.join(d, f"crlset-{name}")) as f:
            if not f.read().strip():
                yield f"{who} CRLSet never arrived"
def idle(name, column):
    with open(os.path.join(d, f"{name}.txt")) as f:
        return statistics.median(float(line.split()[column]) for line in f)
def median(name):
    with open(os.path.join(d, name)) as f:
        return statistics.median(float(line) for line in f if line.strip())
def sets(n):
    return list(unsettled(("ilo's", n), ("Chrome's", f"chrome-{n}")))
for label, ilo, chrome, unit, fails, apart in [
    ("launch, link to page painted", median("link"), median("chrome-link"), "ms", False, []),
    ("launch, window on screen", median("on-screen"), median("chrome-on-screen"), "ms", False, []),
    # ilo's from the shell's clock, whose process start is in it, Chrome's from cdp.py's.
    ("new tab to page painted, ilo's with its clock's start", median("newtab"),
     median("chrome-newtab"), "ms", False, []),
    ("tab switch to page shown, ilo's with its clock's start", median("switch"),
     median("chrome-switch"), "ms", False, []),
    ("idle CPU, 1 tab, settled", idle("settled-1", 0), idle("chrome-settled-1", 0), "%", False, []),
    ("idle CPU, 20 tabs, settled", idle("settled-20", 0), idle("chrome-settled-20", 0), "%", False,
     []),
    ("idle wakeups, 1 tab, settled", idle("settled-1", 1), idle("chrome-settled-1", 1), "/s", False,
     []),
    ("idle wakeups, 20 tabs, settled", idle("settled-20", 1), idle("chrome-settled-20", 1), "/s",
     False, []),
    ("memory, 1 tab", memory("1"), memory("chrome-1"), "MiB", False, sets("1")),
    ("memory, 10 tabs", memory("10"), memory("chrome-10"), "MiB", True, sets("10")),
    ("memory, 20 tabs", memory("20"), memory("chrome-20"), "MiB", True, sets("20")),
    ("idle CPU, 1 tab", idle("idle-1", 0), idle("chrome-idle-1", 0), "%", False, []),
    ("idle CPU, 20 tabs", idle("idle-20", 0), idle("chrome-idle-20", 0), "%", False, []),
    ("idle wakeups, 1 tab", idle("idle-1", 1), idle("chrome-idle-1", 1), "/s", False, []),
    ("idle wakeups, 20 tabs", idle("idle-20", 1), idle("chrome-idle-20", 1), "/s", False, []),
]:
    by = f"{(ilo / chrome - 1) * 100:+.0f} %" if chrome else "Chrome none"
    if apart:
        missed = "not settled: " + ", ".join(apart)
    elif fails and chrome and ilo > chrome * 1.1:
        missed = "missed"
    else:
        missed = "fails over +10 %" if fails else "reports"
    print(f"perf: gate, {label}: ilo {ilo:.2f} {unit}, Chrome here {chrome:.2f} {unit} ({by}), {missed}")
PY
  fi
  snap_procs=
fi
missed=$(grep ', missed$' "$perf_dir/gate.txt" 2> /dev/null | sed 's/^perf: gate, //; s/: ilo .*(\(.*\)), missed$/ \1/' |
  tr '\n' ';' | sed 's/;$//; s/;/; /g')
[ -z "$missed" ] || fail "ilo took more than 10 % over Chrome on this runner: $missed (#456)"
apart=$(grep -E '^perf: gate, memory, (10|20) tabs: .*, not settled: ' "$perf_dir/gate.txt" 2> /dev/null |
  sed 's/^perf: gate, memory, \([0-9]*\) tabs: .*, not settled: /\1 tabs, /' | tr '\n' ';' |
  sed 's/;$//; s/;/; /g')
[ -z "$apart" ] || echo "skip: the gate on memory, as a browser's CRLSet hadn't arrived: $apart"
within="ilo's memory at 10 and 20 tabs within 10 % of Chrome's here"
[ -z "$apart" ] || within="ilo's memory within 10 % of Chrome's where both had their CRLSet"
echo "ok: perf measured $(wc -l < "$perf_dir/rows.tsv" | tr -d ' ') rows against #456's targets," \
  "and $within; each time from a key" \
  "includes the shell's clock ($perf_clock), $clock ms. Rows:" \
  "$perf_dir/rows.tsv, usage snapshots: $perf_dir/"
