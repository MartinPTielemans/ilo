# Measures on a real Mac what a hosted runner can't, for #456: Speedometer 3.1, three runs in the
# app and three in Chrome for Testing, and the five power rows in each. Each browser has a profile of
# its own, new for the run, and Chrome is the Chrome for Testing CHECK_CHROME names, or else the one
# tools/chrome-for-testing.sh downloads, never the owner's, on the display the app opens on. It runs
# only when named, on the Mac only, with nothing else loading the machine.
#
# Speedometer is WebKit's release/3.1, the version browserbench.org runs, at the commit below,
# served from the check's own site, with one line added after it scores, which puts the score in the
# page's title for the run to read. The power rows are each browser's processes' energy, from
# proc_pid_rusage's ri_energy_nj (check/usage.py), which needs no root: it counts their CPUs' energy,
# not the GPU's or the display's, which only powermetrics reads, as root. Each row is three 20 s
# windows, their median and range: idle in view, on a page of boxes and text with nothing moving;
# idle hidden, the window minimized; scrolling that page down and up, a trackpad's events every 8 ms
# (the driver's in the app, DevTools' in Chrome); switching among 5 tabs twice a second; and a
# 1080p H.264 video playing, looped and muted. It says the machine, macOS, the power source and each
# browser's version, and keeps the rows in build/check/bench/rows.tsv.
#
# Downloads go in ~/.cache/ilo-bench: Speedometer's source at the commit, and the video, Big Buck
# Bunny (CC BY 3.0, the Blender Foundation) as test-videos.co.uk cuts it, checked by its SHA-256.

if [ "$(uname)" != Darwin ]; then
  echo "skip: bench, a real Mac's alone"
  return 0
fi
[ -n "${CHECK_CHROME:-}" ] || CHECK_CHROME=$(../tools/chrome-for-testing.sh) ||
  fail "Chrome for Testing didn't download"
serve
# Absolute, as Chromium's sandboxed helpers take the profile's path: with a relative one each of
# them crashed at its start, until the GPU process's last.
bench_dir="$PWD/$out/bench"
rm -rf "$bench_dir"
mkdir -p "$bench_dir"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/ilo-bench"
mkdir -p "$cache"
log="$bench_dir/fetch.log"
: > "$log"

speedometer_commit=1386415be8fef2f6b6bbdbe1828872471c5d802a
speedometer="$cache/speedometer-$speedometer_commit"
if [ ! -e "$speedometer/.patched" ]; then
  rm -rf "$speedometer" "$speedometer.part"
  mkdir -p "$speedometer.part"
  curl -sSfL "https://codeload.github.com/WebKit/Speedometer/tar.gz/$speedometer_commit" |
    tar -xzf - -C "$speedometer.part" --strip-components 1 ||
    fail "Speedometer's source didn't download"
  # Its summary's title says the score on the page, or Error, once it has one.
  python3 -I - "$speedometer.part/resources/main.mjs" << 'PY' || fail "Speedometer's main.mjs changed"
import sys
path = sys.argv[1]
source = open(path, encoding="utf-8").read()
line = "        document.title = `Speedometer 3.1 ${sectionTitle}`.trimEnd();\n"
assert source.count(line) == 1
said = ('        if (hash === "#summary" && this._hasResults)\n'
        '            document.title = `Speedometer 3.1 score '
        '${document.getElementById("result-number").textContent}`;\n')
open(path, "w", encoding="utf-8").write(source.replace(line, line + said))
PY
  touch "$speedometer.part/.patched"
  mv "$speedometer.part" "$speedometer"
fi
ln -s "$speedometer" "$out/site/speedometer"

video="$cache/big-buck-bunny-1080p-10s.mp4"
video_sha=e131ad42621442758a3acb899bfbdfeeab0b40c7e2f7c7e66683f58a09a99aee
if [ "$(shasum -a 256 "$video" 2> /dev/null | cut -d' ' -f1)" != "$video_sha" ]; then
  curl -sSfL -o "$video.part" \
    https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/1080/Big_Buck_Bunny_1080_10s_5MB.mp4 ||
    fail "the video didn't download"
  [ "$(shasum -a 256 "$video.part" | cut -d' ' -f1)" = "$video_sha" ] ||
    fail "the video isn't the one the bench checks"
  mv "$video.part" "$video"
fi
ln -s "$video" "$out/site/bench.mp4"

# The page the power rows idle and scroll on, which puts "<id> fcp" in its title once painted.
{
  printf '%s\n' '<!doctype html><meta charset=utf-8><title>loading</title>' \
    '<style>body { font: 14px/1.4 sans-serif; margin: 16px }' \
    '.box { display: inline-block; width: 72px; height: 40px; margin: 4px; border-radius: 6px }</style>' \
    '<h1>ilo bench page</h1>'
  for i in $(seq 0 59); do
    printf '<div class=box style="background: hsl(%d 60%% 60%%)"></div>' $((i * 6))
  done
  for i in $(seq 0 599); do
    printf '<p>Paragraph %d: the quick brown fox jumps over the lazy dog, again and again.</p>\n' "$i"
  done
  printf '%s\n' '<script>new PerformanceObserver(() => document.title = location.search.slice(1) + " fcp")' \
    '.observe({ type: "paint", buffered: true });</script>'
} > "$out/site/bench.html"
printf '%s\n' '<!doctype html><meta charset=utf-8><title>loading</title>' \
  '<style>body { margin: 0; background: #000 } video { width: 100vw; height: 100vh }</style>' \
  '<video src=bench.mp4 autoplay muted loop playsinline></video>' \
  '<script>document.querySelector("video").addEventListener("playing", () =>' \
  '  document.title = location.search.slice(1) + " playing", { once: true });</script>' \
  > "$out/site/bench-video.html"

: > "$bench_dir/rows.tsv"
# Says row $1, labelled $2, measured as $3, against its target $4, and keeps it.
bench_row() {
  echo "bench: $2: $3; target $4"
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$bench_dir/rows.tsv"
}
# Sorts the numbers on standard input and says their median, range and count, as $1 formats each.
bench_stats() {
  sort -n | awk -v f="$1" '{ v[NR] = $1 } END {
    if (!NR) { print "no samples"; exit }
    m = NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2
    printf "median " f " (" f "–" f ", n=%d)", m, v[1], v[NR], NR }'
}
# Snapshots what the processes under $bench_procs use, as $1.
bench_snap() {
  python3 -I check/usage.py snap "$bench_procs" "$bench_dir/$1.json" ||
    fail "the browser's usage didn't read"
}
# Runs $2... while three 20 s windows pass, a snapshot at each end, then says row $1's mW and CPU:
# sets $power and $cpu. $2 runs once a loop until the windows end, so it does a moment's work.
bench_power() {
  name=$1
  shift
  : > "$bench_dir/$name.mw"
  : > "$bench_dir/$name.cpu"
  bench_snap "$name-0"
  start=$(date +%s)
  for n in 1 2 3; do
    while [ "$(date +%s)" -lt $((start + n * 20)) ]; do
      kill -0 "$pid" 2> /dev/null || fail "the browser exited during $name"
      "$@"
    done
    bench_snap "$name-$n"
    python3 -I check/usage.py power "$bench_dir/$name-$((n - 1)).json" "$bench_dir/$name-$n.json" \
      >> "$bench_dir/$name.mw" || fail "the snapshots have no energy"
    python3 -I check/usage.py idle "$bench_dir/$name-$((n - 1)).json" "$bench_dir/$name-$n.json" |
      cut -d' ' -f1 >> "$bench_dir/$name.cpu"
  done
  if [ "$(awk '{ s += $1 } END { print s > 0 }' "$bench_dir/$name.mw")" = 1 ]; then
    power="$(bench_stats '%.1f' < "$bench_dir/$name.mw") mW"
  else
    power="unmeasured: ri_energy_nj read 0 for the browser's processes here"
  fi
  cpu="$(bench_stats '%.2f' < "$bench_dir/$name.cpu") %"
}
idle_moment() { sleep 1; }

echo "bench: on $(sysctl -n hw.model), $(sysctl -n machdep.cpu.brand_string), macOS $(sw_vers -productVersion), $(pmset -g batt | head -1 | sed "s/^Now drawing from //;s/'//g")"
ilo_version=$(defaults read "$app/Contents/Info" CFBundleShortVersionString 2> /dev/null || echo unknown)
engine_version=$(for plist in "$app"/Contents/Frameworks/*.framework/Resources/Info.plist; do
  defaults read "${plist%.plist}" CFBundleShortVersionString 2> /dev/null && break
done || echo unknown)
chrome_version=$("$CHECK_CHROME" --version 2> /dev/null || echo unknown)
echo "bench: ilo $ilo_version on Chromium $engine_version, beside $chrome_version"

# The app, on page $1 with the driver, in the bench's own profile.
ilo_profile="$bench_dir/ilo-profile"
mkdir -p "$ilo_profile"
step=0
ilo_launch() {
  log="$bench_dir/ilo-$2.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$1" BROWSER_PROFILE_DIR="$ilo_profile" "$browser" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
}
# Waits up to $2 s for the app's log to match $1, failing with $3.
ilo_await() {
  for _ in $(seq $(($2 * 10))); do
    grep -q "$1" "$log" && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$3"
}

speedometer_url="${site}speedometer/index.html?startAutomatically=true"
: > "$bench_dir/ilo-speedometer"
for n in 1 2 3; do
  ilo_launch "$speedometer_url" "speedometer-$n"
  ilo_await "^browser: title [0-9]* Speedometer 3.1 score " 1200 "Speedometer $n didn't score in the app"
  score=$(sed -n 's/^browser: title [0-9]* Speedometer 3.1 score //p' "$log" | head -1)
  echo "bench: ilo's Speedometer $n: $score"
  case $score in [0-9]*) echo "$score" >> "$bench_dir/ilo-speedometer" ;; esac
  quit 30
done
bench_row speedometer "Speedometer 3.1, ilo" "$(bench_stats '%.2f' < "$bench_dir/ilo-speedometer")" \
  "Ardia 59.1"

bench_procs=$procs
ilo_launch "${site}bench.html?b1" power
ilo_await "^browser: title [0-9]* b1 fcp" 30 "the bench page didn't paint"
send click $page_point
sleep 30
bench_power ilo-idle idle_moment
bench_row power-idle "Power, idle, window in view, ilo" "$power, CPU $cpu" "Ardia 3.5 mW"
send minimize
sleep 10
bench_power ilo-hidden idle_moment
bench_row power-hidden "Power, idle, window hidden, ilo" "$power, CPU $cpu" "Ardia 3.9 mW"
send minimize
sleep 5
ilo_scroll() {
  send scroll 600 400 8 250
  send scroll 600 400 -8 250
}
bench_power ilo-scroll ilo_scroll
bench_row power-scroll "Power, scrolling, ilo" "$power, CPU $cpu" "Ardia 28.2 mW"
for n in 2 3 4 5; do
  send key t cmd
  ilo_await '^browser: command bar shown$' 5 "⌘T didn't open the command bar"
  send type "${site}bench.html?b$n"
  send key return
  ilo_await "^browser: title [0-9]* b$n fcp" 30 "b$n didn't paint in its new tab"
done
sleep 10
switch_key=0
ilo_switch() {
  switch_key=$((switch_key % 5 + 1))
  send key "$switch_key" cmd
  sleep 0.5
}
bench_power ilo-switch ilo_switch
bench_row power-switch "Power, switching tabs, ilo" "$power, CPU $cpu" "Ardia 136 mW"
send key t cmd
ilo_await '^browser: command bar shown$' 5 "⌘T didn't open the command bar"
send type "${site}bench-video.html?v1"
send key return
ilo_await "^browser: title [0-9]* v1 playing" 30 "the video didn't play in the app"
sleep 5
bench_power ilo-video idle_moment
bench_row power-video "Power, playing a video, ilo" "$power, CPU $cpu" "Ardia 82 mW"
quit 30
rm -rf "$ilo_profile"

# Chrome for Testing, through DevTools, as it has no driver: on page $1, in the bench's own profile,
# 40 points into the app's display, which it checks before anything else.
chrome_dir="$bench_dir/chrome-profile"
chrome_procs=${CHECK_CHROME%/MacOS/*}
chrome_id=$(defaults read "$chrome_procs/Info" CFBundleIdentifier)
[ "$chrome_id" = com.google.chrome.for.testing ] ||
  fail "CHECK_CHROME is $chrome_id, not Chrome for Testing, which keeps nothing of a user's"
chrome_display=$(display_bounds)
chrome_at=$(echo "$chrome_display" | awk '{ print $1 + 40 "," $2 + 40 }')
bench_chrome() {
  log="$bench_dir/chrome-$2.log"
  rm -f "$chrome_dir/DevToolsActivePort"
  "$CHECK_CHROME" --user-data-dir="$chrome_dir" --no-first-run --no-default-browser-check \
    --use-mock-keychain --password-store=basic --remote-debugging-port=0 \
    --window-position="$chrome_at" --window-size=1280,800 "$1" \
    < /dev/null >> "$bench_dir/chrome.log" 2>&1 &
  pid=$!
  tries=0
  # Its port, then the browser's path, with no newline after.
  until grep -q '^/devtools/browser/' "$chrome_dir/DevToolsActivePort" 2> /dev/null; do
    kill -0 "$pid" 2> /dev/null || fail "Chrome exited at its launch: $(tail -3 "$bench_dir/chrome.log")"
    tries=$((tries + 1))
    [ "$tries" -le 600 ] || fail "Chrome's DevTools didn't listen within 60 s of its launch"
    sleep 0.1
  done
  socket="ws://127.0.0.1:$(sed -n 1p "$chrome_dir/DevToolsActivePort")$(sed -n 2p "$chrome_dir/DevToolsActivePort")"
  rm -f "$bench_dir/chrome-cdp"
  mkfifo "$bench_dir/chrome-cdp"
  python3 -I -u check/cdp.py --socket "$socket" < "$bench_dir/chrome-cdp" > "$log" 2>&1 &
  helper=$!
  exec 5> "$bench_dir/chrome-cdp"
  echo titles >&5
  echo window >&5
  chrome_await "^cdp: window " 30 "Chrome's DevTools didn't say where its window is"
  at=$(grep '^cdp: window ' "$log" | tail -1 | cut -d' ' -f3-)
  if ! echo "$chrome_display $at" | awk '{ exit !($5 >= $1 && $6 >= $2 && $5 + $7 <= $1 + $3 &&
    $6 + $8 <= $2 + $4) }'; then
    bench_chrome_quit
    fail "Chrome's window opened at $at, not inside the app's display, $chrome_display"
  fi
}
# Waits up to $2 s for Chrome's log to match $1, failing with $3.
chrome_await() {
  for _ in $(seq $(($2 * 10))); do
    grep -q "$1" "$log" && return 0
    kill -0 "$pid" 2> /dev/null || fail "Chrome exited, waiting for: $3"
    sleep 0.1
  done
  fail "$3"
}
bench_chrome_quit() {
  osascript -e "tell application id \"$chrome_id\" to quit" || true
  for _ in $(seq 40); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.5
  done
  kill -0 "$pid" 2> /dev/null && fail "Chrome didn't quit in 20 s"
  exec 5>&-
  wait "$helper" 2> /dev/null || true
  for _ in $(seq 20); do
    pgrep -f "$chrome_procs" > /dev/null || return 0
    sleep 0.5
  done
  fail "Chrome's processes outlived it"
}

: > "$bench_dir/chrome-speedometer"
for n in 1 2 3; do
  bench_chrome "$speedometer_url" "speedometer-$n"
  chrome_await "^cdp: title [^ ]* Speedometer 3.1 score " 1200 "Speedometer $n didn't score in Chrome"
  score=$(sed -n 's/^cdp: title [^ ]* Speedometer 3.1 score //p' "$log" | head -1)
  echo "bench: Chrome's Speedometer $n: $score"
  case $score in [0-9]*) echo "$score" >> "$bench_dir/chrome-speedometer" ;; esac
  bench_chrome_quit
done
bench_row chrome-speedometer "Speedometer 3.1, Chrome for Testing" \
  "$(bench_stats '%.2f' < "$bench_dir/chrome-speedometer")" "ilo's"

bench_procs=$chrome_procs
bench_chrome "${site}bench.html?b1" power
chrome_await "^cdp: title [^ ]* b1 fcp" 30 "the bench page didn't paint in Chrome"
first=$(sed -n 's/^cdp: title \([^ ]*\) b1 fcp$/\1/p' "$log" | head -1)
sleep 30
bench_power chrome-idle idle_moment
bench_row chrome-power-idle "Power, idle, window in view, Chrome for Testing" "$power, CPU $cpu" "ilo's"
echo "state minimized" >&5
chrome_await "^cdp: window minimized" 10 "Chrome's window didn't minimize"
sleep 10
bench_power chrome-hidden idle_moment
bench_row chrome-power-hidden "Power, idle, window hidden, Chrome for Testing" "$power, CPU $cpu" "ilo's"
echo "state normal" >&5
chrome_await "^cdp: window normal" 10 "Chrome's window didn't come back"
sleep 5
chrome_scroll() {
  since=$(wc -l < "$log")
  echo "scroll $first 600 400 8 250" >&5
  echo "scroll $first 600 400 -8 250" >&5
  for _ in $(seq 100); do
    [ "$(tail -n +$((since + 1)) "$log" | grep -c '^cdp: scrolled')" -ge 2 ] && return 0
    sleep 0.1
  done
  fail "Chrome didn't scroll"
}
bench_power chrome-scroll chrome_scroll
bench_row chrome-power-scroll "Power, scrolling, Chrome for Testing" "$power, CPU $cpu" "ilo's"
for n in 2 3 4 5; do
  echo "open ${site}bench.html?b$n" >&5
  chrome_await "^cdp: title [^ ]* b$n fcp" 30 "b$n didn't paint in Chrome's new tab"
done
targets=$(for n in 1 2 3 4 5; do sed -n "s/^cdp: title \([^ ]*\) b$n fcp$/\1/p" "$log" | head -1; done)
sleep 10
chrome_switch() {
  switch_key=$((switch_key % 5 + 1))
  echo "activate $(echo "$targets" | sed -n "${switch_key}p")" >&5
  sleep 0.5
}
bench_power chrome-switch chrome_switch
bench_row chrome-power-switch "Power, switching tabs, Chrome for Testing" "$power, CPU $cpu" "ilo's"
echo "open ${site}bench-video.html?v1" >&5
chrome_await "^cdp: title [^ ]* v1 playing" 30 "the video didn't play in Chrome"
sleep 5
bench_power chrome-video idle_moment
bench_row chrome-power-video "Power, playing a video, Chrome for Testing" "$power, CPU $cpu" "ilo's"
bench_chrome_quit
rm -rf "$chrome_dir"
log="$bench_dir/fetch.log"
