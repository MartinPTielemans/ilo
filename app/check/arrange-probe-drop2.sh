# check.sh runs this at 947, only when named.
# A probe of Arrange's drop for #3010's review, on an engine without its core: #3010's runtime and
# its fix's, each run in arrange-drop.sh's fixture page's own world, carried and rested by the
# runtime's own ops. The page says where each rest lands and whether its preview fits, in its
# titles; this writes them to arrange-probe-drop2.txt.

serve
cp check/arrange-probe-drop2.html check/arrange-probe-drop2-old.js \
  check/arrange-probe-drop2-new.js "$out/site/"
log="$out/arrange-probe-drop2.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-probe-drop2.html" "$browser" < "$out/drive" > "$log" 2>&1 &
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
grep '^browser: title [0-9]* r ' "$log" | cut -d' ' -f4- | sed 's/ | /\n/g' > "$out/arrange-probe-drop2.txt"
cat "$out/arrange-probe-drop2.txt"
exec 3>&-
quit
echo "ok: the Arrange drop probe said $(wc -l < "$out/arrange-probe-drop2.txt") lines, in arrange-probe-drop2.txt."
