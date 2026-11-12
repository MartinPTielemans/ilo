# Probe for #2502, never merged: what uBlock Origin's selfie write costs a user. Two legs, each on a
# profile of its own: "new", a new profile's first launch, and "update", a launch whose lists were
# compiled by one before that quit before the selfie, as after a list update. Each leg launches on
# 3 tabs and from 30 s opens new tabs from ⌘T's bar, by turns of the page from http and from a
# file, until 10 s after the selfie is in both stores, sampling every thread's CPU each 250 ms on
# Linux.
serve
cost_dir="$out/selfie-cost"
rm -rf "$cost_dir"
mkdir -p "$cost_dir"
cat > "$out/site/cost.html" << 'HTML'
<!doctype html><meta charset=utf-8><title>loading</title>
<h1>selfie cost</h1><div id=text></div>
<script>
const id = location.search.slice(1);
for (let i = 0; i < 120; i++) {
  const p = document.createElement("p");
  p.textContent = `Paragraph ${i}: the quick brown fox jumps over the lazy dog, again and again.`;
  text.append(p);
}
const at = (t) => Math.round(Date.now() - performance.now() + t);
new PerformanceObserver((list) => {
  for (const entry of list.getEntries()) {
    if (entry.name == "first-contentful-paint") document.title = `${id} fcp ${at(entry.startTime)}`;
  }
}).observe({ type: "paint", buffered: true });
let shown = 0;
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState == "visible") {
    const n = ++shown;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      document.title = `${id} shown ${n} ${Date.now()}`;
    }));
  }
});
</script>
HTML
cost_page="${site}cost.html"
# The same page from a file, whose requests uBlock Origin's listeners (http and https alone) never
# see: a new tab of it held as the http one is would be held by the browser, not the extension.
cost_file="$PWD/$out/site/cost.html"
! windows || cost_file="/$(cygpath -m "$cost_file")"
cost_file="file://$cost_file"
now_ms() { date +%s%3N; }
# A new tab from ⌘T's bar on the page from http, or from a file with $1 file, closed once painted.
cost_open() {
  n=$((n + 1))
  local kind=${1:-http} url=$cost_page
  [ "$kind" = http ] || url=$cost_file
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for p$n"
  send type "$url?p$n"
  t0=$(now_ms)
  send key return
  expect_late "^browser: title [0-9]* p$n fcp " "p$n didn't paint in its new tab"
  fcp=$(sed -n "s/^browser: title [0-9]* p$n fcp //p" "$log" | tail -1)
  echo "$t0 newtab-$kind $((fcp - t0))" >> "$cost_dir/$leg-actions"
  [ "$n" -le 3 ] || send key w cmd
}
cost_switch() {
  k=$((2 + i % 2))
  t0=$(now_ms)
  send key "$k" cmd
  expect "^browser: selected [0-9]* in " "⌘$k didn't switch tabs"
  tab=$(sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n 's/^browser: selected \([0-9]*\) in .*/\1/p' | head -1)
  expect "^browser: title $tab [^ ]* shown " "tab $tab didn't say it was shown after ⌘$k"
  shown=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
    sed -n "s/^browser: title $tab [^ ]* shown [0-9]* \([0-9]*\)$/\1/p" | tail -1)
  echo "$t0 switch $((shown - t0))" >> "$cost_dir/$leg-actions"
}
cost_leg() {
  leg=$1
  log="$cost_dir/$leg.log"
  step=0
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  launched=$(now_ms)
  BROWSER_DRIVE=1 BROWSER_URL="$cost_page?p1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: title [0-9]* p1 fcp " || fail "the leg's first page didn't paint"
  send click $page_point
  rm -f "$cost_dir/stop"
  cost_sampler=
  if [ "$(uname)" = Linux ]; then
    python3 -I check/selfie_cost.py sample "$cost_dir/$leg-samples" "$procs" "$cost_dir/stop" &
    cost_sampler=$!
  fi
  : > "$cost_dir/$leg-actions"
  n=1
  i=0
  cost_open
  cost_open
  while [ $(($(now_ms) - launched)) -lt 30000 ]; do
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.5
  done
  selfie_ms=
  end=
  while [ $(($(now_ms) - launched)) -lt 180000 ]; do
    now=$(now_ms)
    if [ -z "$selfie_ms" ] && [ $((i % 3)) = 0 ] && ublock_selfie; then
      selfie_ms=$now
      end=$((now + 10000))
    fi
    [ -z "$end" ] || [ "$now" -lt "$end" ] || break
    i=$((i + 1))
    if [ $((i % 2)) = 0 ]; then cost_open http; else cost_open file; fi
    sleep 0.2
  done
  touch "$cost_dir/stop"
  [ -z "$cost_sampler" ] || wait "$cost_sampler"
  quit
  [ -n "$selfie_ms" ] || fail "uBlock Origin wrote no selfie in leg $leg's 180 s"
  if [ -n "$cost_sampler" ]; then
    python3 -I check/selfie_cost.py report "$cost_dir/$leg-samples" "$cost_dir/$leg-actions" \
      "$launched" "$selfie_ms" "$leg"
  else
    echo "probe (#2502) $leg: the selfie in both stores $(((selfie_ms - launched) / 1000)) s after launch"
    while read -r at what took; do
      echo "probe (#2502) $leg: action at $(((at - launched) / 100))e-1 s: $what $took ms"
    done < "$cost_dir/$leg-actions"
  fi
}
profile_dir="$PWD/$cost_dir/new-profile"
mkdir -p "$profile_dir"
cost_leg new
# The update leg's profile: a launch that stays until uBlock Origin compiled its lists, then quits
# before its selfie.
profile_dir="$PWD/$cost_dir/update-profile"
mkdir -p "$profile_dir"
log="$cost_dir/compile.log"
step=0
compiled_at=$(now_ms)
BROWSER_URL="$cost_page?p1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
pid=$!
for _ in $(seq 60); do
  cat "$profile_dir/Default/Local Extension Settings/oghkpboejbohceocapipmnjifoklonij"/*.log 2> /dev/null |
    grep -a -q 'compiled/ublock-filters' && break
  kill -0 "$pid" 2> /dev/null || gone
  sleep 1
done
sleep 5
quit
! ublock_selfie || fail "the compile launch wrote a selfie, $(($(now_ms) - compiled_at)) ms in"
echo "probe (#2502): the update leg's lists compiled by a launch that quit $(($(now_ms) - compiled_at)) ms in, with no selfie"
cost_leg update
