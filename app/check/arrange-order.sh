# check.sh runs this at 659.
# Arrange's order (#2975, #2981): a file's changes are laid out as one order, each in turn, so a
# later change moves what an earlier one placed. Each file is written outside ilo and turned on
# from the Site Controls, the user's own path, and the page says where its things are drawn:
# - a settle beside a thing an earlier trade moved goes beside it where it now is;
# - a column whose things are all moved;
# - a settle whose anchor isn't on the page waits, and the trade after it still places;
# - a settle into another column, and a trade of the thing it settled beside;
# - a row's trade, and a settle beside one of the two;
# - an anchor the site hides takes its thing's place, and comes back when the site shows it.
# Then, in the mode, the mini bar's Put back takes each of a file's three changes back in all six
# orders, and each time the page is the file without the changes taken back.
# Last, a column of 300 things with one settled to its top and one traded: each pass takes
# everything down and lays it all out again, so the page's own PerformanceObserver says the
# longest task while the file is turned on and again after the window's width changes, each three
# times, and the smallest of each three may not reach 50 ms (#3993).

serve
# The page says, once nothing moves, its things in each list by where they're drawn: the column's
# and the side's from the top, the row's from the left, and those hidden. Its button hides Beta, as
# a site's own script might, and shows it again.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; height: 1200px; font: 16px sans-serif; background: rgb(243, 245, 246) }
#col { position: absolute; left: 40px; top: 40px; width: 480px; display: flex; flex-direction: column; gap: 20px }
#side { position: absolute; left: 600px; top: 40px; width: 160px; display: flex; flex-direction: column; gap: 20px }
section, aside { height: 120px; background: rgb(217, 226, 230) }
#row { position: absolute; left: 40px; top: 700px; display: flex; gap: 10px }
#row > span { display: block; height: 40px; background: rgb(217, 226, 230) }
#hide { position: absolute; left: 600px; top: 400px; width: 100px; height: 40px }
</style>
<div id="col">
<section id="a" aria-label="Alpha"><h2>Alpha</h2></section>
<section id="b" aria-label="Beta"><h2>Beta</h2></section>
<section id="c" aria-label="Gamma"><h2>Gamma</h2></section>
<section id="w" aria-label="Wide"><h2>Wide</h2></section>
</div>
<div id="side"><aside id="s" aria-label="Side">Side</aside><aside id="n" aria-label="Note">Note</aside></div>
<div id="row"><span id="r1" style="width: 60px"></span><span id="r2" style="width: 100px"></span><span id="r3" style="width: 140px"></span><span id="r4" style="width: 80px"></span></div>
<button id="hide">Beta</button>
<script>
const ids = ["a", "b", "c", "w", "s", "n", "r1", "r2", "r3", "r4"];
const things = () => ids.map((id) => document.getElementById(id));
const box = (t) => t.getBoundingClientRect();
const drawn = (t) => t.getClientRects().length > 0;
// The row is the band at 700; the column and the side are where their things start across.
const inRow = (t) => Math.abs(box(t).top - 700) < 1;
const list = (pick, by) => things().filter((t) => drawn(t) && pick(t))
  .sort((x, y) => box(x)[by] - box(y)[by]).map((t) => t.id).join(",") || "-";
let at = "-";
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
document.getElementById("hide").addEventListener("click", () => {
  const b = document.getElementById("b");
  b.style.display = b.style.display ? "" : "none";
});
let said;
setInterval(() => {
  // Not while the things move: only where they come to rest.
  if (document.getAnimations().some((x) => x.playState === "running")) return;
  const now = "col " + list((t) => !inRow(t) && Math.abs(box(t).left - 40) < 1, "top") +
    " side " + list((t) => !inRow(t) && Math.abs(box(t).left - 600) < 1, "top") +
    " row " + list(inRow, "left") +
    " hidden " + (ids.filter((id) => !drawn(document.getElementById(id))).join(",") || "-") + " at " + at;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/arrange-order.html"
own='a,b,c,w side s,n row r1,r2,r3,r4 hidden -'

# The long list's page: 300 things in a column as wide as part of the view, so a width change
# moves every one. Once nothing has moved for half a second it says which of four things is on top,
# the order of the traded two, its view, how many times Mark was pressed, and the longest task the
# browser reported since the last press, by long-animation-frame where it has it. A frame says
# where its time went too: in tasks before it rendered, in rendering, and how long it kept input
# waiting. Arrange's own script isn't among the frame's scripts, as the browser names only the
# page's, so it's in the time, not apart from the rest.
things=
for i in $(seq 0 299); do
  things="$things<section id=\"t$i\" aria-label=\"Thing $i\">Thing $i</section>"
done
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; font: 14px sans-serif; background: rgb(243, 245, 246) }
#list { margin: 40px 0 40px 160px; width: 50vw; display: flex; flex-direction: column; gap: 8px }
#list > section { height: 32px; background: rgb(217, 226, 230) }
#mark { position: fixed; left: 20px; top: 40px; width: 100px; height: 40px }
</style>
<button id="mark">Mark</button>
<div id="list">'"$things"'</div>
<script>
const type = PerformanceObserver.supportedEntryTypes.includes("long-animation-frame") ? "long-animation-frame" : "longtask";
const none = "tasks - render - blocking -";
let longest = 0, parts = none, since = 0, mark = 0;
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) {
    if (e.startTime < since || e.duration <= longest) continue;
    longest = e.duration;
    if (e.blockingDuration === undefined) continue;
    const tasks = e.renderStart ? e.renderStart - e.startTime : e.duration;
    parts = "tasks " + Math.round(tasks) + " render " + Math.round(e.duration - tasks) + " blocking " + Math.round(e.blockingDuration);
  }
}).observe({ type, buffered: true });
document.getElementById("mark").addEventListener("click", () => { since = performance.now(); longest = 0; parts = none; mark++; });
// Not "top": a script declaring window.top again is refused whole, and its title never comes.
const topOf = (id) => document.getElementById(id).getBoundingClientRect().top;
let now, still = 0, said;
setInterval(() => {
  if (document.getAnimations().some((x) => x.playState === "running")) return;
  const first = ["t0", "t100", "t200", "t299"].sort((x, y) => topOf(x) - topOf(y))[0];
  const was = now;
  now = "first " + first + " pair " + (topOf("t100") < topOf("t200") ? "t100,t200" : "t200,t100") +
    " view " + innerWidth + "x" + innerHeight + " mark " + mark;
  if (now !== was) { still = 0; return; }
  if (++still < 5) return;
  const title = "long " + now + " longest " + Math.round(longest) + " " + parts + " by " + type;
  if (title !== said) document.title = said = title;
}, 100);
</script>' > "$out/site/arrange-long.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
arranged="$boosts/$host.arrange.json"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The arrange-order run'"'"'s Boost. */' > "$boosts/$host.css"

log="$out/arrange-order.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-order.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await "^browser: title [0-9]* col $own at -\$" || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* col ' "$log" | tail -1 | cut -d' ' -f3)
said() { grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-; }
# Waits for the page to say its lists are $1, "col" first as its title has them, else fails with $2.
shows() {
  for _ in $(seq 100); do
    case "$(said)" in "col $1 at "*) return 0 ;; esac
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$2: the page says $(said)"
}

# A click on the page's empty part tells where the page is in the window.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: arrange-order. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
for _ in $(seq 100); do
  case "$(said)" in *" at -") sleep 0.1 ;; *) break ;; esac
done
landed=$(said | sed -n 's/.* at \([0-9-]*\),\([0-9-]*\)$/\1 \2/p')
[ -n "$landed" ] || fail "the page didn't say where the click landed: $(said)"
left=$((${page_point% *} - ${landed% *}))
top=$((${page_point#* } - ${landed#* }))
page_at() { echo $(($1 + left)) $(($2 + top)); }
# The column's nth place from 0, at its middle.
slot() { page_at 280 $((100 + 140 * $1)); }

if ! [ -e ../core/arrange.cc ] || ! engine_has ../core/arrange.cc ../core/api.cc; then
  exec 3>&-
  quit
  echo "skip: arrange-order. The engine doesn't have this tree's Arrange core."
  exit 0
fi

keyed() { grep '^browser: site controls keyed ' "$log" | tail -1 | cut -d' ' -f5-; }
# Writes the changes $2 outside ilo as the site's file, turns it on from the Site Controls, and
# waits for the page to say $3. $1 says the case, for the messages.
placed() {
  printf '{"version":1,"changes":[%s]}' "$2" > "$arranged"
  expect "^browser: arrangement for $host ([0-9]* bytes) \(New\|Changed\)\$" \
    "$1: the app didn't read the file written outside ilo as one kept off"
  shows "$own" "$1: the file written outside ilo wasn't kept off"
  turn_on "$1"
  shows "$3" "$1"
  ordered="$ordered $1,"
}
# Turns the site's file on from the Site Controls, the user's path. $1 says the case.
turn_on() {
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type site controls
  expect '^browser: command bar commands "site controls": Site Controls$' \
    "the command bar didn't find Site Controls"
  send key return
  expect '^browser: site controls settled$' "the Site Controls didn't open"
  for _ in $(seq 30); do
    send key tab
    expect '^browser: site controls keyed ' "Tab didn't move the Site Controls' ring"
    [ "$(keyed)" = Arrangement ] && break
  done
  [ "$(keyed)" = Arrangement ] || fail "$1: Tab didn't reach the arrangement's Turn On: the ring is on $(keyed)"
  send key return
  expect "^browser: arrangement for $host ([0-9]* bytes) kept true\$" "$1: Turn On didn't keep the file"
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
}
# A thing of the page by its id, and a trade and a settle of them, as Arrange's file has them.
thing() { printf '"name":"%s","role":"region","target":{"path":"#%s"}' "$1" "$1"; }
trade_of() { printf '{"kind":"trade",%s,"with":{%s}}' "$(thing $1)" "$(thing $2)"; }
settle_of() { printf '{"kind":"settle",%s,"%s":{%s}}' "$(thing $1)" "$2" "$(thing $3)"; }

ordered=
placed 'a settle beside a thing an earlier trade moved' \
  "$(trade_of c b),$(settle_of a before c)" 'a,c,b,w side s,n row r1,r2,r3,r4 hidden -'
placed 'a column whose things are all moved' \
  "$(trade_of a b),$(trade_of c w)" 'b,a,w,c side s,n row r1,r2,r3,r4 hidden -'
placed 'a settle beside a thing not on the page, then a trade' \
  "$(settle_of a after gone),$(trade_of c a)" 'c,b,a,w side s,n row r1,r2,r3,r4 hidden -'
placed 'a settle into the side, then a trade of the thing it settled beside' \
  "$(settle_of a after s),$(trade_of n a)" 'b,c,w side s,n,a row r1,r2,r3,r4 hidden -'
placed "a row's trade, then a settle beside one of the two" \
  "$(trade_of r1 r3),$(settle_of r2 after r1)" 'a,b,c,w side s,n row r3,r1,r2,r4 hidden -'
placed 'a settle, then a trade, before the site hides the anchor' \
  "$(settle_of a after b),$(trade_of c w)" 'b,a,w,c side s,n row r1,r2,r3,r4 hidden -'
# The site hides the settle's anchor, and its thing takes the anchor's place; shown, it's back.
send click $(page_at 650 420)
shows 'a,w,c side s,n row r1,r2,r3,r4 hidden b' "a settle whose anchor the site hid didn't take its place"
send click $(page_at 650 420)
shows 'b,a,w,c side s,n row r1,r2,r3,r4 hidden -' "a settle whose anchor the site showed again didn't go back beside it"
../tools/window-screenshot.sh "$pid" "$out/arrange-order-placed.png" || fail "no window to capture"

# The card's Arrange starts the mode and closes the card.
start_arranging() {
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab at " "Edit Boost drew no card"
  send layout
  read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
  send click $((bx + bw * 3 / 4)) $((by + bh - 26))
  expect "browser: tab $tab arrange on\$" "the card's Arrange didn't turn the mode on"
}
# Waits for the mini bar over the thing at ($1 $2), its box centred there, else fails with $3.
bar_over() {
  for _ in $(seq 100); do
    read -r ox oy ow oh << EOF
$(grep '^browser: mini bar at ' "$log" | tail -1 | cut -d' ' -f10-13)
EOF
    if [ -n "$oh" ]; then
      dx=$((ox + ow / 2 - $1)) dy=$((oy + oh / 2 - $2))
      [ ${dx#-} -le 1 ] && [ ${dy#-} -le 1 ] && return 0
    fi
    sleep 0.1
  done
  fail "$3: $(grep '^browser: mini bar at ' "$log" | tail -1)"
}
# Presses the mini bar's last button, Put back, with the pointer on the bar first, as the Mac's
# bar window takes the mouse only where the pointer is. The Mac's host moves a bar already shown
# from the main queue after the UI's update, and the driver's commands run as the run loop's blocks,
# so a move can come before the bar is there; the second comes after a turn of the run loop, which
# drains that queue (a click on Gamma's moved bar went to the page).
put_back() {
  read -r mx my mw mh << EOF
$(grep '^browser: mini bar at ' "$log" | tail -1 | cut -d' ' -f5-8)
EOF
  [ -n "$mh" ] || fail "$1: the UI placed no mini bar"
  send move $((mx + mw - 13)) $((my + mh / 2))
  send move $((mx + mw - 13)) $((my + mh / 2))
  send click $((mx + mw - 13)) $((my + mh / 2))
  expect '^browser: mini bar put back pressed$' "$1: a click on the mini bar's Put back didn't press it"
}
# The file's three changes, and the page with each set of them, by the changes left: 0 settles
# Alpha after Beta, 1 trades Gamma with Beta, 2 settles Wide after Gamma.
three="$(settle_of a after b),$(trade_of c b),$(settle_of w after c)"
with() {
  case "$1" in
    012) echo 'c,w,a,b' ;; 12) echo 'a,c,w,b' ;; 02) echo 'b,a,c,w' ;; 01) echo 'c,a,b,w' ;;
    2) echo 'a,b,c,w' ;; 1) echo 'a,c,b,w' ;; 0) echo 'b,a,c,w' ;; *) echo 'a,b,c,w' ;;
  esac
}
name() { case "$1" in a) echo Alpha ;; b) echo Beta ;; c) echo Gamma ;; w) echo Wide ;; esac; }
# The change Put back on each thing takes back: its own.
change() { case "$1" in a) echo 0 ;; c) echo 1 ;; w) echo 2 ;; esac; }
backs=
for order in acw awc caw cwa wac wca; do
  placed "the three changes for Put back in the order $order" "$three" \
    "$(with 012) side s,n row r1,r2,r3,r4 hidden -"
  start_arranging
  left_now=012
  n=3
  for id in $(echo "$order" | sed 's/./& /g'); do
    column=$(said | sed 's/^col \([^ ]*\) .*/\1/')
    i=$(echo "$column" | tr ',' '\n' | grep -n "^$id\$" | cut -d: -f1)
    [ -n "$i" ] || fail "$order: $(name $id) isn't in the column to select: the page says $(said)"
    send click $(slot $((i - 1)))
    expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section $(name $id)\$" \
      "$order: a click where $(name $id) is drawn didn't select it"
    bar_over $(slot $((i - 1))) "$order: the UI placed no mini bar over $(name $id)"
    put_back "$order: Put back on $(name $id)"
    n=$((n - 1))
    expect "browser: tab $tab arrange saved $n changes\$" "$order: the core didn't save Put back on $(name $id)"
    left_now=$(echo "$left_now" | tr -d "$(change $id)")
    shows "$(with "$left_now") side s,n row r1,r2,r3,r4 hidden -" \
      "$order: Put back on $(name $id) didn't leave the page as the file without it"
  done
  send key escape
  expect "browser: tab $tab arrange off\$" "escape didn't end the mode"
  backs="$backs $order"
done
../tools/window-screenshot.sh "$pid" "$out/arrange-order-put-back.png" || fail "no window to capture"

# The long list: its page in the same tab, Mark pressed before the file is turned on and before
# each width change, so each reading is that step's alone. Each is read three times and the bar is
# on the smallest (#3993): whatever else lands in a frame, a window's configures, the Site Controls
# closing, a thread the runner set aside, only adds time and seldom to all three, while a slow pass
# of Arrange's is in each. The smallest hides a cost only the first pass has, so all three are said.
go_from_bar "${site}arrange-long.html"
long() { said | sed -n 's/^long \(.*\) longest [0-9]* tasks .* by [a-z-]*$/\1/p'; }
longest() { said | sed -n 's/^long .* longest \([0-9]*\) tasks .* by [a-z-]*$/\1/p'; }
# Where the longest frame's time went, as the page says it.
parts() { said | sed -n 's/^long .* longest [0-9]* tasks \([0-9-]*\) render \([0-9-]*\) blocking \([0-9-]*\) by [a-z-]*$/tasks \1, rendering \2, input kept waiting \3/p'; }
# Waits for the long page to say $1, up to its view and Mark's count, else fails with $2.
long_shows() {
  for _ in $(seq 100); do
    case "$(long)" in "$1"*) return 0 ;; esac
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$2: the page says $(said)"
}
# Presses Mark and waits for the page to count press $1.
mark() {
  send click $(page_at 70 60)
  for _ in $(seq 100); do
    case "$(long)" in *" mark $1") return 0 ;; esac
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "a click on the long page's Mark didn't count press $1: the page says $(said)"
}
# Both entry types report only tasks of 50 ms or more, so 0 is none reported, not a time.
reading() { if [ "$1" -gt 0 ]; then echo "a task of $1 ms ($2)"; else echo "no task of 50 ms or more"; fi; }
smallest() { printf '%s\n' "$@" | sort -n | head -n 1; }
arranged_long='first t299 pair t200,t100 view '
long_shows 'first t0 pair t100,t200 view ' "the long list's page didn't draw"
# The file three times, each written again outside ilo so the app keeps it off and the page is
# back as the site has it, then turned on: the same two changes, the bytes apart by a space.
on_ms= on_said=
for round in 1 2 3; do
  case $round in 1) pad= ;; 2) pad=' ' ;; *) pad='  ' ;; esac
  printf '{"version":1,%s"changes":[%s]}' "$pad" "$(settle_of t299 before t0),$(trade_of t100 t200)" > "$arranged"
  expect "^browser: arrangement for $host ([0-9]* bytes) \(New\|Changed\)\$" \
    "the long list, file $round: the app didn't read the file written outside ilo as one kept off"
  long_shows 'first t0 pair t100,t200 view ' "the long list, file $round: the file written outside ilo wasn't kept off"
  mark $round
  turn_on "the long list, file $round"
  long_shows "$arranged_long" \
    "the long list's file $round didn't settle Thing 299 to its top and trade Thing 100 with Thing 200"
  on_ms="$on_ms $(longest)"
  on_said="$on_said${on_said:+; }$(reading "$(longest)" "$(parts)")"
done
by=$(said | sed 's/.* by //')
read -r vw vh << VIEW
$(long | sed -n 's/.* view \([0-9]*\)x\([0-9]*\) .*/\1 \2/p')
VIEW
# The window as wide as the view was, as mini-bar.sh does, so the view narrows by the sidebar; then
# 100 narrower, twice, each a pass over every thing.
resized_ms= resized_said= widths=$vw was=$vw wide=$vw
for round in 1 2 3; do
  mark $((3 + round))
  send resize "$wide" $((vh + 40))
  for _ in $(seq 100); do
    case "$(long)" in "$arranged_long${was}x"*) sleep 0.1 ;; "$arranged_long"*) break ;; *) sleep 0.1 ;; esac
  done
  case "$(long)" in
    "$arranged_long${was}x"*) fail "the long page's view didn't change width after resize $round: the page says $(said)" ;;
    "$arranged_long"*" mark $((3 + round))") ;;
    *) fail "the long list's arrangement didn't hold after resize $round: the page says $(said)" ;;
  esac
  resized_ms="$resized_ms $(longest)"
  resized_said="$resized_said${resized_said:+; }$(reading "$(longest)" "$(parts)")"
  was=$(long | sed 's/.* view \([0-9]*\)x.*/\1/')
  widths="$widths to $was"
  wide=$((wide - 100))
done
on_least=$(smallest $on_ms)
resized_least=$(smallest $resized_ms)
echo "arrange-order: 300 things, by $by. Turning the file on, first to third: $on_said; the smallest $on_least ms." \
  "The width from $widths, first to third: $resized_said; the smallest $resized_least ms."
[ "$on_least" -lt 50 ] ||
  fail "turning on a file for 300 things made a task of 50 ms or more each of three times, the smallest $on_least ms"
[ "$resized_least" -lt 50 ] ||
  fail "a width change with a file for 300 things made a task of 50 ms or more each of three times, the smallest $resized_least ms"
../tools/window-screenshot.sh "$pid" "$out/arrange-order-long.png" || fail "no window to capture"

exec 3>&-
quit
echo "ok: Arrange laid each file out as one order, each change in turn:${ordered%,}. Put back took" \
  "each of three changes back in every order, the page each time the file without them:$backs." \
  "Over 300 things the smallest of three readings stayed under 50 ms: $on_least ms turning the file on," \
  "$resized_least ms after a width change, where 0 is no task of 50 ms or more."
