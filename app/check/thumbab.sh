# check.sh runs this at 991, only when named.
# Proof only (#2671): this checkout's engine against the one check/thumbab-other names, built from
# the same core, on the same runner, in rounds with the order alternated. Each launch, traced with
# CHECK_TRACE_CATEGORIES, makes 8 tab switches a second apart over 4 tall pages, then sits while the
# page's clock ticks 8 times, the small redraw. In the first round it also captures a light page's
# and a dark page's scrollbar thumb, idle, hovered and dragged.

serve
dir="$out/thumbab"
rm -rf "$dir"
mkdir -p "$dir"
cat > "$out/site/thumb.html" << 'HTML'
<!doctype html><meta charset=utf-8><title>loading</title>
<script>if (location.search.startsWith("?d")) document.write('<meta name=color-scheme content=dark>');</script>
<style>
body { font: 14px/1.4 sans-serif; margin: 16px; height: 4360px }
body.dark { background: #161616; color: #e8e8e8 }
</style>
<h1>A tall page</h1><p id=clock></p>
<script>
const id = location.search.slice(1);
if (id.startsWith("d")) document.body.className = "dark";
const title = (t) => { document.title = `${id} ${t}`; performance.mark(document.title); };
let ticks = 0;
setInterval(() => {
  clock.textContent = new Date().toLocaleTimeString();
  // A hidden tab's clock ticks too, so only the page shown says so.
  if (++ticks == 8 && document.visibilityState == "visible") title(`ticks ${Date.now()}`);
}, 1000);
const twice = (f) => requestAnimationFrame(() => requestAnimationFrame(f));
twice(() => title("ready"));
let shown = 0;
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState == "visible") {
    ticks = 0;
    const n = ++shown;
    twice(() => title(`shown ${n} ${Date.now()}`));
  }
});
let scrolled = false;
addEventListener("scroll", () => { if (!scrolled) { scrolled = true; twice(() => title("scrolled")); } });
</script>
HTML
thumb_page="${site}thumb.html"

# The engines: this checkout's, as build/Browser has it, and the other, beside it with the same app.
this_engine=$(cd ../engine && ./fetch.sh commit | cut -c1-12)
other_engine=$(cat check/thumbab-other 2> /dev/null || true)
engines=$this_engine
if [ -n "$other_engine" ] && [ "$other_engine" != "$this_engine" ]; then
  other_dir="$PWD/$dir/engine-$other_engine"
  repo=$(git -C ../engine config -f prebuilt.ini prebuilt.repository)
  url=$repo/releases/download/engine-linux-$other_engine/Chromium-linux-x64.tar.xz
  mkdir -p "$other_dir.part"
  curl -fsSL --retry 5 -o "$dir/other.tar.xz" "$url" || fail "couldn't download engine-linux-$other_engine"
  sha=$(curl -fsSL --retry 5 "$url.sha256")
  echo "$sha  $dir/other.tar.xz" | sha256sum -c --status || fail "engine-linux-$other_engine doesn't match its sha256"
  tar -xf "$dir/other.tar.xz" -C "$other_dir.part"
  rm "$dir/other.tar.xz"
  rm -rf build/BrowserOther
  cp -al "$other_dir.part" build/BrowserOther
  cp -R build/Browser/libBrowser.so build/Browser/mcp-bridge build/Browser/ublock build/BrowserOther/
  engines="$engines $other_engine"
fi
echo "thumbab: engines $engines"
this_app=$app

# The thumb of a page in the pane, in the window: the pane's top left is (228, 10), and a page 4392
# px tall has its 9 px thumb 18 px from the pane's top, 131 tall, at x 1029 in the page.
thumb_point="1261 93"
away_point="640 400"
# The same, as fractions of a 1280 by 800 capture, for pixel.
thumb_x=$(awk 'BEGIN { print 1261.5 / 1280 }')
thumb_y=$(awk 'BEGIN { print 93.5 / 800 }')

# Waits until the colour at the thumb's middle in a capture differs from "$1" and then stays, the
# hover's fade done, for up to 5 s.
thumb_changed() {
  last=$1
  for _ in $(seq 25); do
    ../tools/window-screenshot.sh "$pid" "$dir/poll.png" > /dev/null 2>&1 || continue
    now=$(pixel "$dir/poll.png" "$thumb_x" "$thumb_y")
    [ "$now" != "$1" ] && [ "$now" = "$last" ] && return 0
    last=$now
    sleep 0.2
  done
  return 1
}

# Captures page $1's thumb idle, hovered and dragged, as $2-$1-<state>.png.
thumb_looks() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $1"
  send type "$thumb_page?$1"
  send key return
  expect_late "^browser: title [0-9]* $1 ready$" "$1 didn't draw"
  send move $away_point
  ../tools/window-screenshot.sh "$pid" "$dir/$2-$1-idle.png" || fail "no window to capture"
  idle=$(pixel "$dir/$2-$1-idle.png" "$thumb_x" "$thumb_y")
  send move $thumb_point
  thumb_changed "$idle" || echo "thumbab: $2 $1: the thumb's middle stayed $idle under the pointer"
  ../tools/window-screenshot.sh "$pid" "$dir/$2-$1-hovered.png" || fail "no window to capture"
  hovered=$(pixel "$dir/$2-$1-hovered.png" "$thumb_x" "$thumb_y")
  send press $thumb_point
  thumb_changed "$hovered" || echo "thumbab: $2 $1: the thumb's middle stayed $hovered pressed"
  # In steps, as a hand moves it. Under X11 the driver's sent motion doesn't move a held thumb, so
  # the capture is of it held where it was pressed, which the run says.
  for y in 113 153 193 243 293; do
    send dragto 1261 $y
  done
  for _ in $(seq 30); do
    grep -q "^browser: title [0-9]* $1 scrolled$" "$log" && break
    sleep 0.1
  done
  grep -q "^browser: title [0-9]* $1 scrolled$" "$log" ||
    echo "thumbab: $2 $1: the drag didn't scroll the page; captured held where pressed"
  ../tools/window-screenshot.sh "$pid" "$dir/$2-$1-dragged.png" || fail "no window to capture"
  send release 1261 293
  send move $away_point
}

# One launch on engine $1 in round $2.
thumb_round() {
  if [ "$1" = "$this_engine" ]; then app=$this_app; else app="$PWD/build/BrowserOther"; fi
  browser="$app/chrome"
  procs="$app/chrome"
  for _ in $(seq 200); do
    pgrep -f "$PWD/build/Browser" > /dev/null || break
    sleep 0.1
  done
  profile="$PWD/$dir/profile-$1-$2"
  mkdir -p "$profile/Default"
  log="$dir/$1-$2.log"
  cdp_log="$dir/$1-$2-cdp.log"
  step=0
  scratch=$(mktemp -d)
  start_with_cdp "$thumb_page?t1" "$profile" ""
  await "^browser: title [0-9]* t1 ready$" || fail "$1: the first page didn't draw"
  send click $page_point
  for n in 2 3 4; do
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send type "$thumb_page?t$n"
    send key return
    expect_late "^browser: title [0-9]* t$n ready$" "t$n didn't draw"
  done
  send move $away_point
  since=$(wc -l < "$cdp_log")
  echo "trace start $trace_categories" >&5
  perf_wait "$since" '^cdp: tracing$' "DevTools didn't start tracing"
  # ⌘1 is the newest tab, t4, which shows, so each key goes to another.
  for n in 2 3 4 1 2 3 4 1; do
    send key "$n" cmd
    expect_late "^browser: title [0-9]* t[0-9] shown " "⌘$n showed no page"
    sleep 1
  done
  expect_late "^browser: title [0-9]* t[0-9] ticks " "the page's clock didn't tick 8 times"
  since=$(wc -l < "$cdp_log")
  echo "trace end $PWD/$dir/trace-$1-$2.json" >&5
  perf_wait "$since" "^cdp: traced " "DevTools didn't write the trace"
  [ "$2" != 1 ] || { thumb_looks l "$1"; thumb_looks d "$1"; }
  exec 3>&- 5>&-
  quit
  kill "$helper" 2> /dev/null || true
  rm -rf "$scratch" "$profile"
  echo "thumbab: round $2, engine $1: $dir/trace-$1-$2.json"
}
perf_wait() {
  for _ in $(seq 600); do
    tail -n +$(($1 + 1)) "$cdp_log" | grep -q "$2" && return 0
    kill -0 "$pid" 2> /dev/null || fail "the browser exited, waiting for: $3"
    sleep 0.1
  done
  fail "$3: $(tail -3 "$cdp_log")"
}

rounds=${THUMBAB_ROUNDS:-3}
set -- $engines
for round in $(seq "$rounds"); do
  order="$*"
  # Alternate which goes first: the reverse of the first round's order on even rounds.
  [ $((round % 2)) = 1 ] || order=$(echo "$*" | awk '{ for (i = NF; i > 0; i--) printf "%s ", $i }')
  for engine in $order; do
    thumb_round "$engine" "$round"
  done
done
app=$this_app
browser="$app/chrome"
procs="$app/chrome"
echo "ok: proof only (#2671): engines $engines, $rounds rounds, traces and looks in $dir"
