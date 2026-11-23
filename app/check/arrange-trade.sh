# check.sh runs this at 657.
# Arrange's trade (#2980): a thing carried onto another's middle takes its place, and the other
# takes the carried thing's, each at the gap the page had, with nothing else moved. On two flex
# columns with a 20 px gap: one of four things of one height, and one of four of different heights,
# so the room each leaves isn't the other's size. Carried up and carried down, with one thing between
# them, with two, and next to each other, and across the two columns; and between two parents one
# over the other, where the lower place is first in its parent, which the upper room moves (#2990):
# flex columns with a gap, and blocks whose margins collapse through them; and where the upper
# thing is itself the box before the lower thing's parent, or before an ancestor of it.
# After each, ⌘Z with the page focused takes the trade back
# and a click on the page's empty part ends the selection, so each starts from the page as it was.
# Then an arranged page at rest asks for nothing (#2981): each of the three files whose trade
# restored a thing a settle lifted, a frame a pass, turned on from the Site Controls, shown and
# behind another tab.

serve
# The title says, for each column, the things drawn in it from the top, each by its number, or by
# its whole name when it came from the other column, and "packed" when the first is at the column's
# top and each other 20 px under the one before, within a point, or else each one's distance from
# the one before, and where a press landed, once nothing moves.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; height: 100vh; font: 16px sans-serif; background: rgb(243, 245, 246) }
.col { position: absolute; top: 20px; width: 260px; display: flex; flex-direction: column; gap: 20px }
#a { left: 20px }
#b { left: 320px }
section { height: 80px; background: rgb(217, 226, 230) }
#b1 { height: 50px }
#b3 { height: 140px }
#s { position: absolute; left: 620px; top: 20px; width: 260px }
#s > div { display: flex; flex-direction: column; gap: 20px }
#s > div + div { margin-top: 20px }
#c3 { height: 140px }
#m { position: absolute; left: 620px; top: 480px; width: 260px }
#m section { height: 40px; margin: 10px 0 }
#m #m3 { height: 70px }
#n, #o { position: absolute; top: 470px; width: 120px; display: flex; flex-direction: column; gap: 10px }
#n { left: 320px }
#o { left: 460px }
#n > div, #o > div > div { display: flex; flex-direction: column; gap: 10px }
#n section, #o section { height: 30px }
#n1, #o1 { height: 10px !important }
#n2, #o2 { height: 50px !important }
#n3, #o3 { height: 110px !important }
a { position: absolute; left: 20px; width: 60px; height: 30px }
</style>
<a href="arrange-idle.html?m2" style="top: 470px">m2</a><a href="arrange-idle.html?m4" style="top: 510px">m4</a><a href="arrange-idle.html?m12" style="top: 550px">m12</a>
<div class="col" id="a">
<section aria-label="A1"></section><section aria-label="A2"></section><section aria-label="A3"></section><section aria-label="A4"></section>
</div>
<div class="col" id="b">
<section id="b1" aria-label="B1"></section><section aria-label="B2"></section><section id="b3" aria-label="B3"></section><section aria-label="B4"></section>
</div>
<div id="s"><div><section aria-label="C1"></section><section aria-label="C2"></section></div><div><section id="c3" aria-label="C3"></section><section aria-label="C4"></section></div></div>
<div id="m"><div><section aria-label="M1"></section><section aria-label="M2"></section></div><div><section id="m3" aria-label="M3"></section><section aria-label="M4"></section></div></div>
<div id="n"><section id="n1" aria-label="N1"></section><section id="n2" aria-label="N2"></section><div><section id="n3" aria-label="N3"></section><section aria-label="N4"></section></div></div>
<div id="o"><section id="o1" aria-label="O1"></section><section id="o2" aria-label="O2"></section><div><div><section id="o3" aria-label="O3"></section><section aria-label="O4"></section></div></div></div>
<script>
let at = "-";
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
function column(col) {
  const c = col.getBoundingClientRect();
  const things = [...document.querySelectorAll("section")].filter((t) => {
    const r = t.getBoundingClientRect();
    // Not N and O, under column b.
    return r.left + r.width / 2 > c.left && r.left + r.width / 2 < c.right && r.top < 460;
  }).sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top);
  let edge = col.getBoundingClientRect().top - 20;
  const gaps = things.map((t) => {
    const r = t.getBoundingClientRect();
    const gap = Math.round(r.top - edge);
    edge = r.bottom;
    return gap;
  });
  const order = things.map((t) => t.getAttribute("aria-label").slice(t.parentNode === col ? 1 : 0)).join(",");
  return order + " " + (gaps.every((g) => Math.abs(g - 20) <= 1) ? "packed" : gaps.join(","));
}
// Two parents one over the other: their things from the top, and where each starts, from the top
// of the two.
function stacked(s) {
  const top = s.getBoundingClientRect().top;
  const things = [...s.querySelectorAll("section")].sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top);
  return things.map((t) => t.getAttribute("aria-label").slice(1)).join(",") + " " +
    things.map((t) => Math.round(t.getBoundingClientRect().top - top)).join(",");
}
let said;
setInterval(() => {
  // Not while the things move: only where they come to rest.
  if (document.getAnimations().some((x) => x.playState === "running")) return;
  const now = "a " + column(a) + " b " + column(b) + " c " + stacked(s) + " m " + stacked(m) + " n " + stacked(n) + " o " + stacked(o) + " at " + at;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/arrange-trade.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The arrange-trade run'"'"'s Boost. */' > "$boosts/$host.css"

log="$out/arrange-trade.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-trade.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await '^browser: title [0-9]* a 1,2,3,4 packed b 1,2,3,4 packed c 1,2,3,4 0,100,200,360 m 1,2,3,4 10,60,110,190 n 1,2,3,4 0,20,80,200 o 1,2,3,4 0,20,80,200 at -$' || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* a ' "$log" | tail -1 | cut -d' ' -f3)
said() { grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-; }
# Waits for the page to say $1, as a pattern over its title, and fails with $2.
page_says() {
  for _ in $(seq 100); do
    said | grep -q "$1" && return 0
    sleep 0.1
  done
  fail "$2: the page says $(said)"
}

# A click on the page's empty part tells where the page is in the window.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: arrange-trade. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
page_says ' at [0-9-]*,[0-9-]*$' "the page didn't say where the click landed"
landed=$(said | sed -n 's/.* at \([0-9-]*\),\([0-9-]*\)$/\1 \2/p')
left=$((${page_point% *} - ${landed% *}))
top=$((${page_point#* } - ${landed#* }))
page_at() { echo $(($1 + left)) $(($2 + top)); }

if ! [ -e ../core/arrange.cc ] || ! engine_has ../core/arrange.cc ../core/api.cc; then
  exec 3>&-
  quit
  echo "skip: arrange-trade. The engine doesn't have this tree's Arrange core."
  exit 0
fi

# The card's Arrange, the right of the two on its last line, starts the mode and closes the card.
bar_command edit-boost "Edit Boost" "edit boost"
expect "^browser: boost card $tab at " "Edit Boost drew no card"
send layout
read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
send click $((bx + bw * 3 / 4)) $((by + bh - 26))
expect "browser: tab $tab arrange on\$" "the card's Arrange didn't turn the mode on"

# The middle of thing $1 (A1 to B4) in the page, as the fixture places it before any change.
middle() {
  case $1 in
    A*) x=150 tops='20 120 220 320' heights='80 80 80 80' ;;
    B*) x=450 tops='20 90 190 350' heights='50 80 140 80' ;;
    C*) x=750 tops='20 120 220 380' heights='80 80 140 80' ;;
    M*) x=750 tops='490 540 590 670' heights='40 40 70 40' ;;
    N*) x=380 tops='470 490 550 670' heights='10 50 110 30' ;;
    O*) x=520 tops='470 490 550 670' heights='10 50 110 30' ;;
  esac
  set -- ${1#?} $tops $heights
  eval "echo $x \$((\${$(($1 + 1))} + \${$(($1 + 5))} / 2))"
}

# Carries thing $1 onto thing $2's middle and lets go there, which trades them, and waits for the
# page to say $3; then ⌘Z takes the trade back.
trade() {
  set -- "$1" "$2" "$3" $(page_at $(middle "$1")) $(page_at $(middle "$2"))
  send press $4 $5
  send dragto $4 $(($5 + 6))
  expect "^browser: arrange $tab Carrying .* $1\$" "$1 onto $2: a press on $1 and 6 pt didn't lift it"
  send dragto $6 $7
  expect "^browser: arrange $tab Preview Trades .* $1\$" "$1 onto $2: a rest on $2's middle didn't show a trade"
  send release $6 $7
  expect "^browser: arrange $tab Selected Trades selected .* $1\$" "$1 onto $2: letting go on $2's middle didn't trade"
  page_says "$3" "$1 onto $2: the two didn't trade at the column's gap, with the rest where they were"
  ../tools/window-screenshot.sh "$pid" "$out/arrange-trade-$1-$2.png" || fail "no window to capture"
  traded="$traded $1 onto $2,"
  send key z cmd
  page_says '^a 1,2,3,4 packed b 1,2,3,4 packed c 1,2,3,4 0,100,200,360 m 1,2,3,4 10,60,110,190 n 1,2,3,4 0,20,80,200 o 1,2,3,4 0,20,80,200 ' \
    "$1 onto $2: ⌘Z didn't take the trade back"
  # The selection ends, so its mini bar covers nothing the next press needs.
  send click $(page_at 600 600)
  expect "^browser: arrange $tab Pointing None" "$1 onto $2: a click on the page's empty part didn't end the selection"
}

traded=
# One between, carried up and carried down: the lower place's room opened before the upper's
# moved it from under its thing (#2980).
trade A3 A1 '^a 3,2,1,4 packed b 1,2,3,4 packed '
trade A1 A3 '^a 3,2,1,4 packed b 1,2,3,4 packed '
# Two between.
trade A4 A1 '^a 4,2,3,1 packed b 1,2,3,4 packed '
trade A1 A4 '^a 4,2,3,1 packed b 1,2,3,4 packed '
# Next to each other, one room for both places.
trade A2 A1 '^a 2,1,3,4 packed b 1,2,3,4 packed '
trade A1 A2 '^a 2,1,3,4 packed b 1,2,3,4 packed '
# Different heights, the room each leaves not the other's size: one between, and next to each other.
trade B3 B1 '^a 1,2,3,4 packed b 3,2,1,4 packed '
trade B1 B3 '^a 1,2,3,4 packed b 3,2,1,4 packed '
trade B3 B2 '^a 1,2,3,4 packed b 1,3,2,4 packed '
trade B2 B3 '^a 1,2,3,4 packed b 1,3,2,4 packed '
# Across the two columns, side by side, each thing taking the other's place in the other column:
# one with things before and after it, and one first in its column.
trade A2 B3 '^a 1,B3,3,4 packed b 1,2,A2,4 packed '
trade B1 A3 '^a 1,2,B1,4 packed b A3,2,3,4 packed '
# Between two parents one over the other, the lower place first in its parent: the upper parent
# grows by the 60 pt the taller thing brings, and the lower parent, which starts 20 under it, moves
# down with it, so C2 starts there and C4 20 under C2 (#2990).
trade C2 C3 '^a 1,2,3,4 packed b 1,2,3,4 packed c 1,3,2,4 0,100,260,360 '
trade C3 C2 '^a 1,2,3,4 packed b 1,2,3,4 packed c 1,3,2,4 0,100,260,360 '
# The same with 10 pt margins that collapse through both parents, which move a parent's own edge
# rather than what it follows: M2 starts 10 under the taller M3, and M4 10 under M2.
trade M2 M3 '^a 1,2,3,4 packed b 1,2,3,4 packed c 1,2,3,4 0,100,200,360 m 1,3,2,4 10,60,140,190 '
trade M3 M2 '^a 1,2,3,4 packed b 1,2,3,4 packed c 1,2,3,4 0,100,200,360 m 1,3,2,4 10,60,140,190 '
# The upper thing the box just before the lower thing's parent: the 60 pt taller N3 takes N2's
# place, so the parent starts 60 lower, N2 at its top and N4 where it was (#3047). Then with O2
# before an ancestor of O3's parent.
both='c 1,2,3,4 0,100,200,360 m 1,2,3,4 10,60,110,190'
trade N2 N3 "^a 1,2,3,4 packed b 1,2,3,4 packed $both n 1,3,2,4 0,20,140,200 o 1,2,3,4 0,20,80,200 "
trade N3 N2 "^a 1,2,3,4 packed b 1,2,3,4 packed $both n 1,3,2,4 0,20,140,200 o 1,2,3,4 0,20,80,200 "
trade O2 O3 "^a 1,2,3,4 packed b 1,2,3,4 packed $both n 1,2,3,4 0,20,80,200 o 1,3,2,4 0,20,140,200 "
trade O3 O2 "^a 1,2,3,4 packed b 1,2,3,4 packed $both n 1,2,3,4 0,20,80,200 o 1,3,2,4 0,20,140,200 "

send key escape
expect "browser: tab $tab arrange off\$" "escape in the page didn't end Arrange"

# An arranged page at rest asks for nothing (#2981). Arrange's script runs in a world of its own,
# so the page can't count the frames it asks for, but each one runs a pass, and a pass that hides a
# thing and lifts it again fires its toggle events and rewrites its style, which the page sees on
# its own things. The page counts those over 250 ms: from the first frame after its things are
# lifted that saw none, with nothing moving, ending before the hold's passes at 1.5 s, or else from
# the first such frame after them; again 2.5 s after the lift, once they've run; while another tab
# covers it; and from the first such frame after it shows again, past the pass its own title asks
# for. Each is an absence measured over a stated time, as the idle checks' are: the loop it looks
# for fired two toggles a frame, so it gives no frame to start from, and the page says "quiet none".
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; height: 100vh; font: 16px sans-serif; background: rgb(243, 245, 246) }
#col { position: absolute; left: 20px; top: 20px; width: 260px; display: flex; flex-direction: column; gap: 20px }
#col > section { height: 80px; background: rgb(217, 226, 230) }
#row { position: absolute; left: 320px; top: 20px; display: flex; gap: 10px }
#row > span { display: block; height: 40px; background: rgb(217, 226, 230) }
a { position: absolute; left: 20px; width: 60px; height: 30px }
</style>
<div id="col"><section id="a" aria-label="Alpha"></section><section id="b" aria-label="Beta"></section><section id="c" aria-label="Gamma"></section><section id="w" aria-label="Wide"></section></div>
<div id="row"><span id="r1" style="width: 60px"></span><span id="r2" style="width: 100px"></span><span id="r3" style="width: 140px"></span><span id="r4" style="width: 80px"></span></div>
<a href="arrange-idle.html?m2" style="top: 470px">m2</a><a href="arrange-idle.html?m4" style="top: 510px">m4</a><a href="arrange-idle.html?m12" style="top: 550px">m12</a>
<script>
const file = location.search.slice(1);
const things = [...document.querySelectorAll("#col > section, #row > span")];
let seen = 0;
document.addEventListener("toggle", (e) => { if (things.includes(e.target)) seen++; }, true);
new MutationObserver((records) => { for (const r of records) if (things.includes(r.target)) seen++; })
  .observe(document.body, { subtree: true, attributes: true, attributeFilter: ["style", "popover"] });
const lifted = new Promise((done) => {
  const now = () => things.some((t) => t.hasAttribute("popover")) && (done(performance.now()), true);
  if (now()) return;
  const o = new MutationObserver(() => now() && o.disconnect());
  o.observe(document.body, { subtree: true, attributes: true, attributeFilter: ["popover"] });
});
const wait = (ms) => new Promise((done) => setTimeout(done, ms));
// The frames until one whose passes touched nothing, with nothing moving, or "none" by |by|. The
// passes of the load, as its things settle, run past two frames on some runners (43767580159). A
// frame is read after it is drawn, once the passes in its callbacks and observers have run.
const drawn = () => new Promise((done) => requestAnimationFrame(() => setTimeout(done, 0)));
const quiet = async (by) => {
  await drawn();
  for (let n = 1; ; n++) {
    const was = seen;
    await drawn();
    if (seen === was && !document.getAnimations().some((x) => x.playState === "running")) return n;
    if (performance.now() > by) return "none";
  }
};
// Until the page is shown, or hidden.
const until = (hidden) => new Promise((done) => {
  const f = () => { if (document.hidden === hidden) { document.removeEventListener("visibilitychange", f); done(); } };
  document.addEventListener("visibilitychange", f);
  f();
});
// What the page saw of a pass over |ms|, and how long that took, as a timer behind a tab runs late.
const count = async (ms) => { const n = seen, t = performance.now(); await wait(ms); return [seen - n, Math.round(performance.now() - t)]; };
// Where the things are drawn, from the top and from the left, so a file that placed nothing shows.
const order = () => {
  const by = (k) => (x, y) => x.getBoundingClientRect()[k] - y.getBoundingClientRect()[k];
  return things.slice(0, 4).sort(by("top")).map((t) => t.id).join("") + " " + things.slice(4).sort(by("left")).map((t) => t.id).join("");
};
(async () => {
  const at = await lifted;
  // After the load, whose end the core may lay the page out again for. The window stays off the
  // passes at the end of the hold, the runtime pass 1.5 s after its load and the core one 1.55 s
  // after the commit. Both start no earlier than the navigation, 0 on the page clock, and end no
  // later than 1.55 s after the lift, which may come a second or more after the navigation on a
  // slow runner (#3929). So a window that would end past 1450 ms on the page clock starts from a
  // quiet frame after at + 1700 instead, and the title says when it started. A page with no quiet
  // frame by its bound says "quiet none".
  if (document.readyState !== "complete") await new Promise((done) => addEventListener("load", done, { once: true }));
  let settled = await quiet(at + 1200);
  if (performance.now() + 250 > 1450) {
    await wait(Math.max(0, at + 1700 - performance.now()));
    settled = await quiet(at + 2200);
  }
  const from = Math.round(performance.now() - at);
  const [first] = await count(250);
  await wait(Math.max(0, at + 2500 - performance.now()));
  const [late] = await count(250);
  let said = "idle " + file + " " + order() + " quiet " + settled + " shown " + first + " at " + from + " ms late " + late;
  document.title = said;
  await until(true);
  const [hidden, ms] = await count(250);
  document.title = said += " hidden " + hidden + " in " + ms + " ms";
  await until(false);
  const back = await quiet(performance.now() + 1000);
  const [again] = await count(250);
  document.title = said + " again " + again + " quiet " + back;
})();
</script>' > "$out/site/arrange-idle.html"

arranged="$boosts/$host.arrange.json"
keyed() { grep '^browser: site controls keyed ' "$log" | tail -1 | cut -d' ' -f5-; }
# Writes file $1, the changes $2, outside ilo; turns it on from the Site Controls; opens its page;
# and has the page count what it sees while it's idle, shown and behind two.html's tab.
idle() {
  printf '{"version":1,"changes":[%s]}' "$2" > "$arranged"
  expect "^browser: arrangement for $host ([0-9]* bytes) \(New\|Changed\)\$" \
    "$1: the app didn't read the file written outside ilo as one kept off"
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
  send click $(page_at 50 $3)
  await "^browser: title $tab idle $1 .* late [0-9]*\$" "$1: the page didn't say what it saw while idle: it says $(said)"
  case "$(said)" in
    "idle $1 abcw r1r2r3r4 "*) fail "$1: the file placed nothing: the page says $(said)" ;;
  esac
  page=$tab
  open_from_bar "http://localhost:$port/two.html"
  over=$tab
  tab=$page
  await "^browser: title $tab idle $1 .* hidden [0-9]* in [0-9]* ms\$" \
    "$1: the page didn't say what it saw behind two.html's tab: it says $(said)"
  tab=$over
  send key w cmd
  tab=$page
  expect "^browser: selected $tab\( in .*\)*\$" "$1: ⌘W on two.html's tab didn't show the page again"
  await "^browser: title $tab idle $1 .* again [0-9]* quiet [0-9a-z]*\$" "$1: the page didn't say what it saw shown again: it says $(said)"
  case "$(said)" in
    *" quiet "[0-9]*" shown 0 at "[0-9]*" ms late 0 hidden 0 in "*" again 0 quiet "[0-9]*) ;;
    *) fail "$1: the arranged page didn't rest: it saw passes while nothing changed: $(said)" ;;
  esac
  rested="$rested $1 ($(said | cut -d' ' -f3-4)),"
}
# A thing of the idle page by its id, and a trade and a settle of them, as Arrange's file has them.
thing() { printf '"name":"%s","role":"region","target":{"path":"#%s"}' "$1" "$1"; }
trade_of() { printf '{"kind":"trade",%s,"with":{%s}}' "$(thing $1)" "$(thing $2)"; }
settle_of() { printf '{"kind":"settle",%s,"%s":{%s}}' "$(thing $1)" "$2" "$(thing $3)"; }
rested=
# The three of probe 3's twenty-one files that looped (43759327155, 43761139000): a trade, and a
# settle beside one of the two it traded. Each page is opened from a link at the page's left, at
# the height given.
idle m2 "$(trade_of c b),$(settle_of a before c)" 485
idle m4 "$(settle_of a after b),$(trade_of c b),$(settle_of w after c)" 525
idle m12 "$(trade_of r1 r3),$(settle_of r2 after r1)" 565

exec 3>&-
quit
echo "ok: Arrange's trade put each of the two at the other's place, at the column's gap, with the" \
  "rest where they were:${traded%,}. An arranged page whose trade restores a thing a settle lifted" \
  "saw no pass while nothing changed, shown, behind another tab and shown again:${rested%,}."
