# check.sh runs this at 948, only when named.
# A probe for #2990, on an engine without the fix: trades across two parents one over the other, on
# main's runtime and the fix's, in the page's own world, carried by the runtime's own ops. The
# page says where each thing lands, in its titles; this writes them to arrange-probe-stack.txt.

serve
cp check/arrange-probe-stack.html check/arrange-probe-stack-before.js \
  check/arrange-probe-stack-after.js check/arrange-probe-stack-order.js "$out/site/"
log="$out/arrange-probe-stack.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-probe-stack.html" "$browser" < "$out/drive" > "$log" 2>&1 &
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
grep '^browser: title [0-9]* r ' "$log" | cut -d' ' -f4- | sed 's/ | /\n/g' > "$out/arrange-probe-stack.txt"
cat "$out/arrange-probe-stack.txt"
exec 3>&-
quit
echo "ok: the Arrange drop probe said $(wc -l < "$out/arrange-probe-stack.txt") lines, in arrange-probe-stack.txt."
