# Temporary (#1664), on a probe branch only: Rename Space… from the Space's menu, then escape,
# many times over, noting each time the field had ended before escape and how it was left.

[ "$(uname)" = Darwin ] || {
  echo "skip: renameloop, which probes the Mac's menu window."
  exit 0
}
serve
log="$out/renameloop.log"
report="$out/renameloop.txt"
: > "$report"
step=0
printf '%s' "<title>loading</title><p style=\"font:24px system-ui\">Rename loop</p>
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = 'ready'));</script>" \
  > "$out/site/loop.html"
BROWSER_DRIVE=1 BROWSER_URL="${site}loop.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the loop's page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: renameloop. Browser wasn't the active app."
  exit 0
fi
name_point="100 112"
# Waits up to 3 s for $1 since the last command, without failing.
soft() {
  for _ in $(seq 30); do
    logged "$1" && return 0
    sleep 0.1
  done
  return 1
}
early=0
rounds=60
for round in $(seq $rounds); do
  # Every other round the pointer rests on the name after the menu, as the check's does, and in
  # the others it's moved off it first.
  post rightclick $name_point
  soft '^browser: menu shown .*Rename Space' || {
    echo "round $round: no menu" >> "$report"
    send key escape
    continue
  }
  opened=$((step + 1))
  send choose Rename Space…
  soft '^browser: rename space ' || echo "round $round: Rename Space opened no field" >> "$report"
  [ $((round % 2)) = 0 ] && send move 600 400
  send key escape
  if ! soft '^browser: rename cancelled$'; then
    early=$((early + 1))
    how=$(sed -n "/^browser: drive $opened: /,\$p" "$log" |
      grep 'field left\|name field left\|space name\|menu window\|became\|resign' | tr '\n' ';')
    echo "round $round: escape found the field gone: $how" >> "$report"
  fi
done
exec 3>&-
quit
echo "ok: renameloop, $early of $rounds rounds had the field end before escape: $(cat "$report")"
