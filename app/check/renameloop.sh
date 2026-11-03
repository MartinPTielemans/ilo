# Temporary (#1664), on a probe branch only: the spaces run's last steps over and over. A new
# Space, deleted empty from its dot so the toast slides in and the window switches Space, every
# other round after a second window opened and closed, then Rename Space… from the last Space's
# menu and escape, noting each time the field had ended before escape and how it was left.

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
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
dots_y=$((${content#*x} - 25))
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
rounds=40
windows=1
for round in $(seq $rounds); do
  begun=$((step + 1))
  send menu New Space…
  soft '^browser: command new-space via ui$' || { echo "round $round: no new Space" >> "$report"; break; }
  send type r$round
  send key return
  soft '^browser: create space ' || echo "round $round: the form didn't name the Space" >> "$report"
  if [ $((round % 2)) = 1 ]; then
    send key n cmd
    soft '^browser: window opened [0-9]' || echo "round $round: no second window" >> "$report"
    windows=$((windows + 1))
    send window $windows
    send key w cmd shift
    soft '^browser: window closed ' || echo "round $round: the second window stayed" >> "$report"
    send window 1
  fi
  post rightclick 131 $dots_y
  soft '^browser: menu shown .*Delete Space$' || {
    echo "round $round: no dot's menu" >> "$report"
    send key escape
    continue
  }
  send choose Delete Space
  soft '^browser: toast shown Space deleted' || echo "round $round: no toast" >> "$report"
  post rightclick $name_point
  soft '^browser: menu shown .*Rename Space' || {
    echo "round $round: no name's menu" >> "$report"
    send key escape
    continue
  }
  opened=$((step + 1))
  send choose Rename Space…
  soft '^browser: rename space ' || echo "round $round: Rename Space opened no field" >> "$report"
  send key escape
  if ! soft '^browser: rename cancelled$'; then
    early=$((early + 1))
    how=$(sed -n "/^browser: drive $opened: /,\$p" "$log" |
      grep 'field left\|field.s window\|space name\|menu window\|toast\|settled' | tr '\n' ';')
    echo "round $round: escape found the field gone: $how" >> "$report"
    sed -n "/^browser: drive $begun: /,\$p" "$log" > "$out/renameloop-round-$round.log"
  fi
done
exec 3>&-
quit
echo "ok: renameloop, $early of $rounds rounds had the field end before escape: $(cat "$report")"
