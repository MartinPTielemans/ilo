# check.sh runs this at 946, only when named.
# A probe for #3010, on an engine without its core: arrange-trade's idle step, run in the page's own
# world on main's runtime, #3010's and the order model's, over m2, m4, m12 and two trades between
# columns of different heights. The page says each in its titles; this writes them to
# arrange-probe-idle.txt.

serve
cp check/arrange-probe-idle.html check/arrange-probe-idle-main.js check/arrange-probe-idle-f3010.js \
  check/arrange-probe-idle-order.js "$out/site/"
log="$out/arrange-probe-idle.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-probe-idle.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* probe start' || fail "the probe page didn't start"
probe=$(grep '^browser: title [0-9]* probe start' "$log" | tail -1 | cut -d' ' -f3)
# Waits up to 60 s for the probe's title $1.
titled() {
  for _ in $(seq 600); do
    grep -q "^browser: title $probe $1\$" "$log" && return 0
    kill -0 "$pid" 2>/dev/null || return 1
    sleep 0.1
  done
  return 1
}
# For each runtime's each file, a tab over the probe's while its page counts what it sees hidden.
for i in $(seq 15); do
  titled "probe hide $i" ||
    fail "the probe page didn't ask for a tab over it ($i): its last title was $(grep '^browser: title ' "$log" | tail -1)"
  open_from_bar example.org
  titled "probe hidden-done $i" || fail "the probe page didn't say it had been hidden ($i)"
  send key w cmd
  expect "^browser: selected $probe\$" "closing the tab over the probe didn't show it again ($i)"
done
titled 'probe done [0-9]*' ||
  fail "the probe page didn't say it was done: its last title was $(grep '^browser: title ' "$log" | tail -1)"
grep '^browser: title [0-9]* r ' "$log" | cut -d' ' -f4- | sed 's/ | /\n/g' > "$out/arrange-probe-idle.txt"
cat "$out/arrange-probe-idle.txt"
exec 3>&-
quit
echo "ok: the Arrange idle probe said $(wc -l < "$out/arrange-probe-idle.txt") lines, in arrange-probe-idle.txt."
