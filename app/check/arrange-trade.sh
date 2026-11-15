# check.sh runs this at 657.
# Arrange's trade (#2980): a thing carried onto another's middle takes its place, and the other
# takes the carried thing's, each at the gap the page had, with nothing else moved. On two flex
# columns with a 20 px gap: one of four things of one height, and one of four of different heights,
# so the room each leaves isn't the other's size. Carried up and carried down, with one thing between
# them, with two, and next to each other, and across the two columns. After each, ⌘Z with the page focused takes the trade back
# and a click on the page's empty part ends the selection, so each starts from the page as it was.

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
</style>
<div class="col" id="a">
<section aria-label="A1"></section><section aria-label="A2"></section><section aria-label="A3"></section><section aria-label="A4"></section>
</div>
<div class="col" id="b">
<section id="b1" aria-label="B1"></section><section aria-label="B2"></section><section id="b3" aria-label="B3"></section><section aria-label="B4"></section>
</div>
<script>
let at = "-";
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
function column(col) {
  const c = col.getBoundingClientRect();
  const things = [...document.querySelectorAll("section")].filter((t) => {
    const r = t.getBoundingClientRect();
    return r.left + r.width / 2 > c.left && r.left + r.width / 2 < c.right;
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
let said;
setInterval(() => {
  // Not while the things move: only where they come to rest.
  if (document.getAnimations().some((x) => x.playState === "running")) return;
  const now = "a " + column(a) + " b " + column(b) + " at " + at;
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
await '^browser: title [0-9]* a 1,2,3,4 packed b 1,2,3,4 packed at -$' || fail "the fixture page didn't draw"
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
  page_says '^a 1,2,3,4 packed b 1,2,3,4 packed ' "$1 onto $2: ⌘Z didn't take the trade back"
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

exec 3>&-
quit
echo "ok: Arrange's trade put each of the two at the other's place, at the column's gap, with the" \
  "rest where they were:${traded%,}."
