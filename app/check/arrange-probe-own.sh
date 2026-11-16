# check.sh runs this at 948, only when named.
# A probe for #3003, on an engine without the core: a rest in the carried thing's own column's gap,
# on #3010 and #3040 merged and on that with the gap a track's, in arrange-drop.sh's fixture page's
# own world, carried by the runtime's own ops. The page says where each rest lands, in its
# titles; this writes them to arrange-probe-own.txt.

serve
cp check/arrange-probe-own.html check/arrange-probe-own-merged.js \
  check/arrange-probe-own-settles.js "$out/site/"
log="$out/arrange-probe-own.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-probe-own.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* drop start' || fail "the drop probe page didn't start"
for _ in $(seq 1200); do
  grep -q '^browser: title [0-9]* drop done' "$log" && break
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.1
done
grep -q '^browser: title [0-9]* drop done' "$log" ||
  fail "the drop probe page didn't say it was done in 120 s: its last title was $(grep '^browser: title ' "$log" | tail -1)"
grep '^browser: title [0-9]* r ' "$log" | cut -d' ' -f4- | sed 's/ | /\n/g' > "$out/arrange-probe-own.txt"
cat "$out/arrange-probe-own.txt"
exec 3>&-
quit
echo "ok: the Arrange drop probe said $(wc -l < "$out/arrange-probe-own.txt") lines, in arrange-probe-own.txt."
