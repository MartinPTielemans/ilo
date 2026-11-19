# check.sh runs this at 9990, only when named.
# Probe for #3527, on a probe branch that never merges. With ⌘L's bar open, it sends a shortcut that
# shows another tab beneath the bar. It then asks two things: whether the input method reaches the
# bar (insert), and whether plain keys still do (type). It prints a "probe:" line for each case,
# and fails only when the setup does.
plain='data:text/html,<title>plain ready</title><p>plain'
# One line: the driver's commands are a line each.
field='data:text/html,<title>loading</title><textarea id=text autofocus></textarea><script>requestAnimationFrame(() => setTimeout(() => document.title = "field ready")); text.oninput = () => document.title = "field has " + text.value;</script>'
log="$out/probe-3527.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$plain" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* plain ready$' || fail "the plain page didn't draw"
first=$(grep -m1 '^browser: title [0-9]* plain ready$' "$log" | cut -d' ' -f3)

# Sends a command, as send does, and says whether the driver could do it.
can() {
  step=$((step + 1))
  echo "$*" >&3
  await "^browser: drive $step done" || fail "the driver didn't finish \"$*\""
  ! grep -q "^browser: drive $step: can't do that" "$log"
}
# Waits up to 3 s for a line matching $1 since command $2, without failing.
soft() {
  for _ in $(seq 30); do
    since "$2" | grep -q "$1" && return 0
    sleep 0.1
  done
  return 1
}
kind() {
  case " $fields " in *" $1 "*) echo field ;; *) echo plain ;; esac
}
open_tab() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the bar"
  send insert "$1"
  send key return
  expect "^browser: title [0-9]* $2\$" "the new tab's page didn't draw"
  opened_tab=$(since | grep -m1 "^browser: title [0-9]* $2\$" | cut -d' ' -f3)
}
# Not in $( ): send counts the steps in this shell.
open_tab "$field" 'field ready'
second=$opened_tab
fields=$second
open_tab "$plain" 'plain ready'
third=$opened_tab
echo "probe: tabs $first plain, $second field, $third plain"

n=0
# probe <name> [<key words>]: ⌘L, the shortcut if any, then the input method and plain keys.
probe() {
  name=$1
  shift
  n=$((n + 1))
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the bar ($name)"
  from=$(grep '^browser: selected [0-9]*' "$log" | tail -1 | cut -d' ' -f3)
  opened=$step
  shown=$from
  follows=-
  if [ $# -gt 0 ]; then
    send key "$@"
    if soft '^browser: selected [0-9]*' "$step"; then
      shown=$(since | grep '^browser: selected [0-9]*' | tail -1 | cut -d' ' -f3)
    fi
    follows=no
    soft '^browser: command bar follows the tab shown$' "$step" && follows=yes
    soft '^browser: command bar hidden$' "$step" && follows=hidden
  fi
  can insert "im$n" && im=yes || im=no
  imdrew=no
  soft "^browser: command bar drew \".*im$n\"\$" "$step" && imdrew=yes
  impage=no
  since "$opened" | grep -q "^browser: title [0-9]* field has .*im$n" && impage=yes
  typed=$step
  send type "kb$n"
  keys=no
  soft "^browser: command bar drew \".*kb$n\"\$" "$typed" && keys=yes
  keyspage=no
  since "$opened" | grep -q "^browser: title [0-9]* field has .*kb$n" && keyspage=yes
  can insert "again$n" && again=yes || again=no
  echo "probe: $name: from $from ($(kind "$from")) to $shown ($(kind "$shown")), bar follows $follows;" \
    "input method $im, bar drew it $imdrew, page got it $impage;" \
    "plain keys reach the bar $keys, page got them $keyspage; input method after the keys $again"
  echo "probe: $name lines:"
  since "$opened" | grep -E '^browser: (probe|selected|command|drive [0-9]+: |drive.s input)' |
    grep -v -E 'drive [0-9]+ done' | sed 's/^/probe:   /'
  send key escape
  soft '^browser: command bar hidden$' "$step" || echo "probe: $name: escape left the bar open"
}

# The bar with no shortcut, on each kind of page.
probe "no shortcut"
send key 1 cmd
probe "no shortcut, from tab 1"
# ⌥⌘↓ and ⌥⌘↑, the step in keys.sh.
for start in 1 2 3; do
  send key "$start" cmd
  probe "⌥⌘↓ from tab $start" down alt cmd
  send key "$start" cmd
  probe "⌥⌘↑ from tab $start" up alt cmd
done
# ⌃Tab and ⌃⇧Tab.
for start in 1 2 3; do
  send key "$start" cmd
  probe "⌃Tab from tab $start" tab ctrl
  send key "$start" cmd
  probe "⌃⇧Tab from tab $start" tab ctrl shift
done
# ⌘1–⌘3, each from each.
for start in 1 2 3; do
  for to in 1 2 3; do
    [ "$start" != "$to" ] || continue
    send key "$start" cmd
    probe "⌘$to from tab $start" "$to" cmd
  done
done
exec 3>&-
quit
echo "ok: probe done"
