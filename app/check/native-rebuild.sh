# check.sh runs this at 846, only when named.
# Probe for #2513, never merged: what the browser's own ad blocker (--ad-block-engine=native, #2111)
# costs a page while it compiles. It has no selfie, but a change to its setup (a list downloaded or
# turned on, a rule in My filters) drops the engine it has, and every request waits for the new one
# (Blocker::use_setup). The tab's page fetches from the check's http server one after another the
# whole run, and says each fetch that took over 40 ms. 6 times a rule goes in My filters, each
# compiling the engine anew; after each a new tab is opened from ⌘T's bar at once, by turns from
# http and from a file, whose navigation the engine is never asked about. Then 6 launches by turns
# with the engine's cache and without, as the launch after a list update has none.
serve
nr_dir="$out/native-rebuild"
nr_profile_was=$profile_dir
profile_dir="$PWD/$nr_dir/profile"
rm -rf "$nr_dir"
mkdir -p "$nr_dir"
printf 'ok\n' > "$out/site/nr.txt"
cat > "$out/site/nr.html" << 'HTML'
<!doctype html><meta charset=utf-8><title>loading</title>
<h1>native rebuild</h1><div id=text></div>
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
if (location.protocol.startsWith("http") && id == "p1") {
  (async () => {
    let n = 0;
    await new Promise((done) => setTimeout(done, 1000));
    document.title = "p1 fetching";
    for (;;) {
      const start = performance.now();
      try { await fetch(`nr.txt?${n++}`, { cache: "no-store" }); } catch (e) {}
      const took = performance.now() - start;
      if (took > 40) document.title = `p1 slow ${n} ${at(start)} ${Math.round(took)}`;
    }
  })();
}
</script>
HTML
nr_page="${site}nr.html"
nr_file="$PWD/$out/site/nr.html"
! windows || nr_file="/$(cygpath -m "$nr_file")"
nr_file="file://$nr_file"
now_ms() { date +%s%3N; }
# A new tab from ⌘T's bar on the page from http, or from a file with $1 file, at once after the
# rule $2 went in, the keys sent before any is answered.
nr_open() {
  n=$((n + 1))
  local kind=$1 url=$nr_page
  [ "$kind" = http ] || url=$nr_file
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for p$n"
  send type "$url?p$n"
  t0=$(now_ms)
  send key return
  expect_late "^browser: title [0-9]* p$n fcp " "p$n didn't paint in its new tab"
  fcp=$(sed -n "s/^browser: title [0-9]* p$n fcp //p" "$log" | tail -1)
  echo "$t0 $2 newtab-$kind $((fcp - t0))" >> "$nr_dir/actions"
  send key w cmd
}
log="$nr_dir/session.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$nr_page?p1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
  --ad-block-engine=native < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await "^browser: title [0-9]* p1 fcp " || fail "the first page didn't paint"
await "^browser: ad block engine \(compiled\|loaded\) in " || fail "the engine didn't load"
await "^browser: title [0-9]* p1 fetching$" || fail "the first page didn't start fetching"
: > "$nr_dir/actions"
: > "$nr_dir/rules"
n=1
# Before any rule: two new tabs with nothing compiling, each way.
for kind in http file http file; do
  nr_open "$kind" none
  sleep 1
done
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Ad Blocking")
expect '^browser: settings tab Ad Blocking$' "a click on Ad Blocking didn't show it"
settings_drew "Ad Blocking"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
for r in 1 2 3 4 5 6; do
  compiled=$(grep -c '^browser: ad block engine compiled in ' "$log" || true)
  send menu Settings…
  expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking for rule $r"
  settings_drew "Ad Blocking"
  send clickkey $(drawn "filter field")
  send insert "127.0.0.1##.probe-$r"
  added=$(now_ms)
  send key return
  send key w cmd
  expect "^browser: ad block filter added 127\.0\.0\.1##\.probe-$r\$" "rule $r didn't go in"
  expect '^browser: settings hidden$' "⌘W didn't hide Settings after rule $r"
  if [ $((r % 2)) = 1 ]; then nr_open http "r$r"; else nr_open file "r$r"; fi
  for _ in $(seq 100); do
    [ "$(grep -c '^browser: ad block engine compiled in ' "$log" || true)" -gt "$compiled" ] && break
    sleep 0.1
  done
  took=$(grep '^browser: ad block engine compiled in ' "$log" | tail -1 | sed 's/[^0-9]*\([0-9]*\) ms$/\1/')
  [ "$(grep -c '^browser: ad block engine compiled in ' "$log" || true)" -gt "$compiled" ] ||
    fail "rule $r compiled no engine in 10 s"
  echo "$added r$r $took" >> "$nr_dir/rules"
  sleep 2
done
quit
echo "probe (#2513): rules, return at, compiled in ms:"
cat "$nr_dir/rules"
echo "probe (#2513): new tabs, return at, after rule, kind, return to first paint ms:"
cat "$nr_dir/actions"
echo "probe (#2513): the first page's fetches over 40 ms, fetch, started at, ms:"
sed -n 's/^browser: title [0-9]* p1 slow //p' "$log"
# Launches by turns with the engine's cache and without, to the first page painted.
: > "$nr_dir/launches"
for l in 1 2 3 4 5 6; do
  way=cached
  if [ $((l % 2)) = 1 ]; then
    way=uncached
    rm -f "$profile_dir/Ad Blocking"/engine-*.dat
  fi
  log="$nr_dir/launch-$l.log"
  step=0
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  t0=$(now_ms)
  BROWSER_URL="$nr_page?l$l" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
    --ad-block-engine=native < /dev/null > "$log" 2>&1 &
  pid=$!
  await "^browser: title [0-9]* l$l fcp " || fail "launch $l's page didn't paint"
  fcp=$(sed -n "s/^browser: title [0-9]* l$l fcp //p" "$log" | tail -1)
  nr_engine=$(grep '^browser: ad block engine \(compiled\|loaded\) in ' "$log" | head -1 |
    sed 's/^browser: ad block engine //')
  echo "$l $way $((fcp - t0)) $nr_engine" >> "$nr_dir/launches"
  quit
done
echo "probe (#2513): launches, way, launch to first paint ms, engine:"
cat "$nr_dir/launches"
profile_dir=$nr_profile_was
echo "ok: probe (#2513) ran; its rows are above and in $nr_dir"
