# Arrange (#2347) on a fixture page of the run's own: from the Boost card's Arrange, a click picks a
# thing and a press, 4 pt and a rest carries it; let go at an edge it settles there, on another's
# middle the two trade, where it doesn't fit nothing changes, and with ⌘ it floats, which the mini
# bar's pin turns between staying on screen and scrolling with the page. The mini bar's Hide is
# Zap's rule and ⌘Z takes it back, its Put back takes a change back, and ⌘Z with the page focused
# takes back the last change. Each change is saved in the site's .arrange.json as it happens. An
# agent's call on the page while the user arranges it is refused, in the activity log. Reloaded,
# the page shows the arrangement, and the run measures whether its first frame already did, in
# arrange-first-frame.txt, and says which in its summary rather than failing: a flash there is what
# would bring a renderer hook at document start (#2347's slice 2 plan). Edited outside ilo, the
# file stays off, and Site Controls' Turn On keeps it. A private window shows the profile's
# arrangement but offers no Boost to arrange. The foot bar's Reset takes the site's arrangement
# away, and the page is its own again.
# It needs the engine with Arrange's core (slice 2): on one without it, it says it skips. The core's
# lines it reads are the app's on #2414 but for the foot bar's, which the foot bar's slice logs as
# the Boost card does its place: "browser: foot bar <tab> <button> at <x> <y> <w> <h>".

serve
# The page says in its title where its things are, each time that changes: the column's and the
# side's in their order down the page, those hidden, those floating with their position, where the
# last click before the mode landed in the page, and the column as its first frame drew it. Six
# things: four sections in the column, 120 tall and 20 apart from 40 down, the last too wide for
# the side, which holds two boxes, from 600 across.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; height: 2400px; font: 16px sans-serif; background: rgb(243, 245, 246) }
#col { position: absolute; left: 40px; top: 40px; width: 480px; display: flex; flex-direction: column; gap: 20px }
#side { position: absolute; left: 600px; top: 40px; width: 160px; display: flex; flex-direction: column; gap: 20px }
section, aside { height: 120px; background: rgb(217, 226, 230) }
#w { min-width: 480px }
</style>
<div id="col">
<section id="a" aria-label="Alpha"><h2>Alpha</h2></section>
<section id="b" aria-label="Beta"><h2>Beta</h2></section>
<section id="c" aria-label="Gamma"><h2>Gamma</h2></section>
<section id="w" aria-label="Wide"><h2>Wide</h2></section>
</div>
<div id="side">
<aside id="s" aria-label="Side">Side</aside>
<aside id="n" aria-label="Note">Note</aside>
</div>
<script>
const ids = ["a", "b", "c", "w", "s", "n"];
const things = () => ids.map((id) => document.getElementById(id));
const shown = () => things().filter((thing) => thing.getClientRects().length);
const floats = (thing) => /fixed|absolute/.test(getComputedStyle(thing).position);
const down = (list) =>
  list.sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top)
    .map((thing) => thing.id).join(",") || "-";
const column = (left) => down(shown().filter((thing) => !floats(thing) &&
  (thing.getBoundingClientRect().left < 560) === left));
let at = "-";
let first = "-";
let said;
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
requestAnimationFrame(() => { first = column(true); });
setInterval(() => {
  const hidden = ids.filter((id) => !shown().some((thing) => thing.id === id)).join(",") || "-";
  const floating = shown().filter(floats)
    .map((thing) => thing.id + ":" + getComputedStyle(thing).position).join(",") || "-";
  const now = "col " + column(true) + " side " + column(false) + " hidden " + hidden +
    " float " + floating + " at " + at + " first " + first;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/arrange.html"
url="${site}arrange.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
css="$boosts/$host.css"
arranged="$boosts/$host.arrange.json"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered only where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The arrange run'"'"'s Boost. */' > "$css"
# The agent the run connects, as the mcp run does, with a token of its own.
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
trap 'cleanup; rm -f "$agents_file"' EXIT

log="$out/arrange.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$url" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
own='a,b,c,w side s,n hidden - float -'
await "^browser: title [0-9]* col $own at - first " || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* col ' "$log" | tail -1 | cut -d' ' -f3)
# What the page last said of itself, after its tab.
said() { grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-; }
# Waits for the page to say its things are where $1 says, its column then "side", "hidden" and
# "float" as its title has them, else fails with $2.
shows() {
  for _ in $(seq 100); do
    case "$(said)" in "col $1 at "*) return 0 ;; esac
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$2: the page says $(said)"
}
# The changes in the site's arrangement, 0 with no file.
changes() {
  python3 -I -c 'import json, sys; print(len(json.load(open(sys.argv[1]))["changes"]))' \
    "$arranged" 2> /dev/null || echo 0
}
# Waits for the site's arrangement to hold $1 changes, else fails with $2.
kept() {
  for _ in $(seq 100); do
    [ "$(changes)" = "$1" ] && return 0
    sleep 0.1
  done
  fail "$2: the site's arrangement holds $(changes) changes, not $1"
}

# A click where the window's page point is tells where the page is in the window, as the page says
# where in it the click landed.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: arrange. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
for _ in $(seq 100); do
  case "$(said)" in *" at -"*) sleep 0.1 ;; *) break ;; esac
done
landed=$(said | sed -n 's/.* at \([0-9-]*\),\([0-9-]*\) first .*/\1 \2/p')
[ -n "$landed" ] || fail "the page didn't say where the click landed: $(said)"
left=$((${page_point% *} - ${landed% *}))
top=$((${page_point#* } - ${landed#* }))
# The window's point at ($1, $2) in the page.
page_at() { echo $(($1 + left)) $(($2 + top)); }
# The page's things by their place: the column's nth from 0 and the side's, each at its middle.
slot() { page_at 280 $((100 + 140 * $1)); }
side_slot() { page_at 680 $((100 + 140 * $1)); }
# The edge under the column's nth thing, and under the side's first.
slot_edge() { page_at 280 $((170 + 140 * $1)); }
side_edge() { page_at 680 170; }

if ! [ -e ../core/arrange.cc ] || ! engine_has ../core/arrange.cc ../core/api.cc; then
  exec 3>&-
  quit
  echo "skip: arrange. The engine has no Arrange core yet (#2347's slice 2), so the card's Arrange" \
    "turns no mode on. The page loaded with the site's Boost."
  exit 0
fi

# The card's Arrange, beside Customize… on its last line, starts the mode on the page and closes
# the card.
start_arranging() {
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab opened\$" "Edit Boost opened no card"
  expect "^browser: boost card $tab at " "the Boost's card wasn't drawn"
  send layout
  read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
  send click $((bx + bw * 3 / 4)) $((by + bh - 26))
  expect "^browser: arrange $tab started\$" "the card's Arrange didn't start Arrange"
  expect "^browser: arrange $tab on\$" "the card's Arrange didn't ask the core for the mode"
  expect "^browser: boost card $tab closed\$" "the card stayed open over the mode"
}
# Sets $mx, $my, $mw and $mh to the mini bar's window, which is the bar and its shadow's margin
# round it, the bar 30 tall, else fails with $1.
mini_bar() {
  send layout
  child=$(since | grep -m1 '^browser: layout child .*x\(30\|110\)$' | sed 's/^browser: layout child //')
  read -r mx my mw mh << EOF
$(echo "$child" | tr ',x' '  ')
EOF
  [ -n "$mh" ] || fail "$1: the window has no mini bar"
  margin=$(((mh - 30) / 2))
}
# Presses the mini bar's first button, or its last with $1 "last": 10 into it from the bar's inset.
press_mini_bar() {
  mini_bar "the mini bar to press $2"
  if [ "${1:-}" = last ]; then
    send click $((mx + mw - margin - 13)) $((my + mh / 2))
  else
    send click $((mx + margin + 13)) $((my + mh / 2))
  fi
  expect "^browser: mini bar $2 pressed\$" "a click on the mini bar's $2 didn't press it"
}
# Carries from ($1 $2) to ($3 $4) in steps, with $5 held if it's given: 4 pt lifts it, and the
# drop shows once the pointer rests, which the core says.
carry() {
  send press $1 $2 ${5:-}
  send dragto $1 $(($2 + 6)) ${5:-}
  expect "^browser: arrange $tab Carrying " "a press and 6 pt didn't lift the thing"
  send dragto $((($1 + $3) / 2)) $((($2 + $4) / 2)) ${5:-}
  send dragto $3 $4 ${5:-}
}

start_arranging
../tools/window-screenshot.sh "$pid" "$out/arrange-on.png" || fail "no window to capture"

# Pick: a click selects the thing under it, and the mini bar shows over it with Hide alone.
send click $(slot 0)
expect "^browser: arrange $tab Selected None selected\$" "a click in the mode didn't select a thing"
expect "^browser: mini bar shown over $tab\$" "the mini bar wasn't shown over the thing selected"
expect '^browser: mini bar drawn over [0-9]*$' "the mini bar didn't draw"
shows "$own" "a click in the mode reached the page"
[ "$(said | sed 's/.* at \([0-9-]*,[0-9-]*\) first .*/\1/')" = "${landed% *},${landed#* }" ] ||
  fail "the page took the mode's click: $(said)"
../tools/window-screenshot.sh "$pid" "$out/arrange-picked.png" || fail "no window to capture"

# An agent's call on the page while the user arranges it is refused, with why, in the activity
# log. The client is the bridge on two pipes, as in the mcp run.
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved switch didn't start the agents server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi
rm -f "$out/arrange-mcp-in" "$out/arrange-mcp-out"
mkfifo "$out/arrange-mcp-in" "$out/arrange-mcp-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/arrange-mcp-in" \
    "$out/arrange-mcp-out" 2>> "$out/arrange-mcp-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/arrange-mcp-in" > "$out/arrange-mcp-out" \
    2>> "$out/arrange-mcp-bridge.log" &
fi
exec 4> "$out/arrange-mcp-in" 5< "$out/arrange-mcp-out"
# The bridge's next line, within 30 s, as the mcp run reads it.
next_answer() {
  perl -e 'alarm 30; exec @ARGV' sh -c 'read -r line && printf "%s\n" "$line"' <&5
}
# Sends the request $1 and prints its answer.
ask() {
  printf '%s\n' "$1" >&4
  next_answer || fail "the browser didn't answer $1"
}
# Calls the tool $1 with the arguments $2 as request $3 and prints its answer: its text, after
# "error: " if it's an error.
call() {
  ask "{\"jsonrpc\":\"2.0\",\"id\":$3,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" |
    python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
print(("error: " if result["isError"] else "") + result["content"][0]["text"])'
}
ask '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
  > "$out/arrange-mcp-initialize.json"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4
arranging='The user is arranging this page.'
for tool in read_page press_key; do
  case $tool in
    read_page) refused=$(call read_page "{\"tab\":$tab}" 1) ;;
    press_key) refused=$(call press_key "{\"tab\":$tab,\"key\":\"Tab\"}" 2) ;;
  esac
  [ "$refused" = "error: $arranging" ] || fail "$tool on the page being arranged answered: $refused"
  await "^browser: agent activity mcp-client $tab $tool refused: $arranging\$" ||
    fail "the refused $tool didn't reach the activity log"
done
grep -q "^browser: tab text $tab " "$log" && fail "the core read the page being arranged"
exec 4>&- 5<&-

# Carry and settle: the first carried to the edge under the second settles there, the page making
# room, and the change is saved.
carry $(slot 0) $(slot_edge 1)
expect "^browser: arrange $tab Preview Settles\$" "a rest at an edge didn't show a settle"
send release $(slot_edge 1)
expect "^browser: arrange $tab Selected Settles selected\$" "letting go at an edge didn't settle"
shows 'b,a,c,w side s,n hidden - float -' "the settle didn't move the thing under the second"
kept 1 "the settle wasn't saved"
../tools/window-screenshot.sh "$pid" "$out/arrange-settled.png" || fail "no window to capture"

# Trade: the third let go on the first's middle, and the two trade places.
carry $(slot 2) $(slot 0)
expect "^browser: arrange $tab Preview Trades\$" "a rest on a thing's middle didn't show a trade"
send release $(slot 0)
expect "^browser: arrange $tab Selected Trades selected\$" "letting go on a thing's middle didn't trade"
shows 'c,a,b,w side s,n hidden - float -' "the trade didn't swap the two"
kept 2 "the trade wasn't saved"

# A fit refusal: the wide section at the side's edge, where it doesn't fit, and nothing changes.
carry $(slot 3) $(side_edge)
expect "^browser: arrange $tab Preview Refused\$" "a rest where the thing doesn't fit didn't refuse it"
../tools/window-screenshot.sh "$pid" "$out/arrange-refused.png" || fail "no window to capture"
send release $(side_edge)
expect "^browser: arrange $tab Selected " "letting go where the thing doesn't fit left no selection"
shows 'c,a,b,w side s,n hidden - float -' "the refused drop changed the page"
[ "$(changes)" = 2 ] || fail "the refused drop was saved: $(changes) changes"

# A float: the note carried with ⌘ held stays where it's let go, over the page, and the mini bar's
# pin, its first button, turns it between staying on screen and scrolling with the page.
carry $(side_slot 1) $(page_at 300 760) cmd
expect "^browser: arrange $tab Preview Floats\$" "a rest with ⌘ held didn't show a float"
send release $(page_at 300 760) cmd
expect "^browser: arrange $tab Selected Floats selected\$" "letting go with ⌘ held didn't float"
kept 3 "the float wasn't saved"
for _ in $(seq 100); do
  case "$(said)" in *" float n:"*) break ;; esac
  sleep 0.1
done
pinned=$(said | sed -n 's/.* float n:\([a-z]*\) at .*/\1/p')
case "$pinned" in
  fixed) unpinned=absolute ;;
  absolute) unpinned=fixed ;;
  *) fail "the float isn't over the page: $(said)" ;;
esac
../tools/window-screenshot.sh "$pid" "$out/arrange-floats.png" || fail "no window to capture"
press_mini_bar first pin
shows "c,a,b,w side s hidden - float n:$unpinned" "the pin didn't turn the float"
press_mini_bar first pin
shows "c,a,b,w side s hidden - float n:$pinned" "the pin didn't turn the float back"

# ⌘Z with the page focused takes back the last change, the float.
send key z cmd
shows 'c,a,b,w side s,n hidden - float -' "⌘Z didn't take back the float"
kept 2 "⌘Z's step back wasn't saved"

# Put back, the mini bar's last button on a thing with a change, takes that change back.
send click $(slot 1)
expect "^browser: arrange $tab Selected Settles selected\$" "a click didn't select the settled thing"
press_mini_bar last "put back"
kept 1 "Put back didn't take the change back"

# Hide, the mini bar's first button on a thing with no change, is Zap's rule, which ⌘Z takes back.
send click $(slot 3)
expect "^browser: arrange $tab Selected None selected\$" "a click didn't select the wide section"
press_mini_bar first hide
for _ in $(seq 100); do
  case "$(said)" in *" hidden w "*) break ;; esac
  sleep 0.1
done
case "$(said)" in *" hidden w "*) ;; *) fail "Hide didn't hide the thing: $(said)" ;; esac
grep -q '/\* Zap \*/$' "$css" || fail "Hide didn't keep Zap's rule in the Boost: $(cat "$css")"
send key z cmd
for _ in $(seq 100); do
  case "$(said)" in *" hidden - "*) break ;; esac
  sleep 0.1
done
case "$(said)" in *" hidden - "*) ;; *) fail "⌘Z didn't take back the Hide: $(said)" ;; esac
grep -q '/\* Zap \*/$' "$css" && fail "⌘Z left Zap's rule in the Boost: $(cat "$css")"

# Escape is Done.
send key escape
expect "^browser: arrange $tab off\$" "escape didn't end the mode"
kept_column=$(said | sed 's/^col \([^ ]*\) .*/\1/')

# Reloaded, the page places the arrangement as it loads. The run measures, and says, whether the
# first frame already showed it or the site's own layout.
send key r cmd
expect "^browser: loaded $tab $url\$" "⌘R didn't reload the page"
for _ in $(seq 100); do
  case "$(said)" in "col $kept_column side s,n hidden - float - at - first "*) break ;; esac
  sleep 0.1
done
case "$(said)" in
  "col $kept_column side s,n hidden - float - at - first "*) ;;
  *) fail "the reloaded page didn't place the arrangement: $(said)" ;;
esac
first=$(said | sed 's/.* first //')
if [ "$first" = "$kept_column" ]; then
  first_frame="its first frame already showed it"
else
  first_frame="its first frame showed the site's own column, $first, a flash"
fi
echo "$first_frame" > "$out/arrange-first-frame.txt"

# Edited outside ilo, the file stays off: the page reloads to its own layout, and Site Controls'
# Turn On keeps it as it is now.
cp "$arranged" "$out/arrange-kept.json"
python3 -I -c '
import json, sys
path = sys.argv[1]
with open(path) as f:
    file = json.load(f)
with open(path, "w") as f:
    f.write(json.dumps(file, indent=3) + "\n\n")' "$arranged"
cmp -s "$arranged" "$out/arrange-kept.json" && fail "the edit outside ilo left the file as it was"
send key r cmd
expect "^browser: loaded $tab $url\$" "⌘R didn't reload the page"
shows "$own" "the file edited outside ilo was placed"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type site controls
expect '^browser: command bar commands "site controls": Site Controls$' \
  "the command bar didn't find Site Controls"
send key return
expect '^browser: site controls settled$' "the Site Controls didn't open"
../tools/window-screenshot.sh "$pid" "$out/arrange-changed.png" || fail "no window to capture"
keyed() { grep '^browser: site controls keyed ' "$log" | tail -1 | cut -d' ' -f5-; }
for _ in $(seq 20); do
  send key tab
  expect '^browser: site controls keyed ' "Tab didn't move the Site Controls' ring"
  [ "$(keyed)" = Arrangement ] && break
done
[ "$(keyed)" = Arrangement ] || fail "Tab didn't reach the arrangement's Turn On: the ring is on $(keyed)"
send key space
expect '^browser: site controls turned on the arrangement$' "Turn On didn't run"
expect "^browser: arrangement for $host turned on\$" "Turn On didn't keep the arrangement"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
send key r cmd
expect "^browser: loaded $tab $url\$" "⌘R didn't reload the page"
shows "$kept_column side s,n hidden - float -" "the arrangement turned on wasn't placed"

# A private window's page shows the profile's arrangement, and its command bar offers no Boost.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
send insert "$url"
send key return
expect "^browser: loaded [0-9]* $url\$" "the private window didn't load the page"
private=$(sed -n "s|^browser: loaded \([0-9]*\) $url\$|\1|p" "$log" | tail -1)
[ "$private" != "$tab" ] || fail "the private window's page loaded in the regular tab"
for _ in $(seq 100); do
  grep "^browser: title $private " "$log" | tail -1 |
    grep -q " col $kept_column side s,n hidden - float - " && break
  sleep 0.1
done
grep "^browser: title $private " "$log" | tail -1 | grep -q " col $kept_column side s,n " ||
  fail "the private window's page wasn't arranged: $(grep "^browser: title $private " "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/arrange-private.png" || fail "no window to capture"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the private window"
send type "edit boost"
typed=$step
send key escape
expect '^browser: command bar hidden$' "escape didn't close the private window's command bar"
none_since 'command bar commands "edit boost": Edit Boost' "$typed" ||
  fail "the private window's command bar offered Edit Boost"
send key w cmd shift
expect '^browser: private session ended ' "⇧⌘W didn't close the private window"

# Reset, on the foot bar, takes the site's arrangement away: the file goes, and the page is its own.
start_arranging
send layout
read -r rx ry rw rh << EOF
$(grep "^browser: foot bar $tab Reset at " "$log" | tail -1 | cut -d' ' -f7-)
EOF
[ -n "$rh" ] || fail "the foot bar didn't offer Reset"
send click $((rx + rw / 2)) $((ry + rh / 2))
shows "$own" "Reset didn't put the page back"
for _ in $(seq 100); do
  [ -e "$arranged" ] || break
  sleep 0.1
done
[ -e "$arranged" ] && fail "Reset left the site's arrangement: $(cat "$arranged")"
send key escape
expect "^browser: arrange $tab off\$" "escape didn't end the mode"

exec 3>&-
quit
rm -f "$agents_file"
trap cleanup EXIT
echo "ok: from the Boost card's Arrange, a click picked a thing with the page taking none, and a" \
  "carry settled it at an edge, traded it on a thing's middle, was refused where it didn't fit," \
  "and with ⌘ floated it, whose pin turned it to scroll with the page and back. ⌘Z took the float" \
  "back, Put back the settle, and Hide was Zap's rule, which ⌘Z took back. Each change was saved" \
  "in $host.arrange.json. An agent's read_page and press_key on the page were refused as the user" \
  "arranged it, in the activity log. Reloaded, the page placed the arrangement, and $first_frame." \
  "Edited outside ilo it stayed off, and Site Controls' Turn On kept it. A private window's page" \
  "was arranged, with no Boost offered, and the foot bar's Reset gave the page its own layout." \
  "Windows: $out/arrange-on.png, $out/arrange-picked.png, $out/arrange-settled.png," \
  "$out/arrange-refused.png, $out/arrange-floats.png, $out/arrange-changed.png," \
  "$out/arrange-private.png"
