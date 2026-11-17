# check.sh runs this at 658.
# Arrange's drop (#3003): what a rest lands on, for the carried thing, where the page's columns wrap
# their things at different depths and where a thing already scrolls sideways where it stands. A
# rest on a card's middle in another column trades with the card, however deep either column wraps
# its cards. A rest in a column's gap settles at the nearest card's edge, in a column that wraps its
# cards one box deeper too. One that can't be as narrow as its place is refused. A block that
# scrolls sideways settles and trades between places of its own width, and is refused only in a
# narrower one. On markup with no classes, a card under the rest trades and a column's gap settles,
# though both are plain boxes of plain boxes. A paragraph with a button in it, beside the carried
# card's column, is a thing to trade with, not a track to settle in among its words. After each, ⌘Z takes it back and a click on the page's empty part ends the
# selection, so each starts from the page as it was.

serve
# The title says, for each column, the things drawn in it from the top by name, once nothing
# moves, and where a press landed.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; height: 100vh; font: 12px/14px sans-serif; background: rgb(243, 245, 246) }
.col { position: absolute; top: 20px; display: flex; flex-direction: column; gap: 20px }
#dl { left: 20px; width: 240px }
#dlw { display: flex; flex-direction: column; gap: 20px }
#dr { left: 280px; width: 240px }
#dn { left: 540px; width: 120px }
#dc { left: 20px; top: 400px; width: 240px }
section { box-sizing: border-box; background: rgb(217, 226, 230) }
.card { height: 100px; padding: 10px; display: flex; flex-direction: column; justify-content: space-between }
.card h3, .card p { margin: 0; font-size: 12px; line-height: 14px }
#dn .card { height: 60px }
#dlw .card { min-width: 220px }
#CB { height: 60px; overflow-x: auto; white-space: pre }
#CB p { margin: 0 }
#dj { left: 540px; top: 200px; width: 120px }
#dk { left: 280px; top: 400px; width: 240px }
#dj > div, #dk > div { display: flex; flex-direction: column; gap: 20px }
#dj > div > div, #dk > div > div { box-sizing: border-box; height: 50px; padding: 6px; display: flex;
  flex-direction: column; justify-content: space-between; background: rgb(217, 226, 230) }
#PT { box-sizing: border-box; height: 60px; margin: 0; padding: 6px; background: rgb(217, 226, 230) }
#PT button { font: inherit }
</style>
<div id="dual">
<div class="col" id="dl"><div id="dlw">
<section class="card" aria-label="L1"><h3>Left one</h3><p>Its words</p></section>
<section class="card" aria-label="L2"><h3>Left two</h3><p>Its words</p></section>
<section class="card" aria-label="L3"><h3>Left three</h3><p>Its words</p></section>
</div></div>
<div class="col" id="dr">
<section class="card" aria-label="R1"><h3>Right one</h3><p>Its words</p></section>
<section class="card" aria-label="R2"><h3>Right two</h3><p>Its words</p></section>
<section class="card" aria-label="R3"><h3>Right three</h3><p>Its words</p></section>
</div>
<div class="col" id="dn">
<section class="card" aria-label="N1"><h3>Narrow one</h3></section>
<section class="card" aria-label="N2"><h3>Narrow two</h3></section>
</div>
<div class="col" id="dc">
<section id="CB" aria-label="CB"><p>const line = "a line of code long enough to run out of its box sideways, so it scrolls";</p></section>
<section class="card" aria-label="C2"><h3>Code two</h3><p>Its words</p></section>
</div>
<div class="col" id="dj"><div>
<div aria-label="J1"><div>Jay one</div><div>Its words</div></div>
<div aria-label="J2"><div>Jay two</div><div>Its words</div></div>
</div></div>
<div class="col" id="dk"><div>
<div aria-label="K1"><div>Key one</div><div>Its words</div></div>
<div aria-label="K2"><div>Key two</div><div>Its words</div></div>
</div>
<p id="PT" aria-label="PT">Words, <button>Go</button> and <a href="#">a link</a></p>
</div>
</div>
<script>
let at = "-";
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
const cols = ["dl", "dr", "dn", "dj", "dc", "dk"].map((id) => document.getElementById(id));
// Where the things of each column start, from the top: the code and class-less columns are under
// the left and right ones, and the narrow class-less one under the narrow one.
const band = { dl: [0, 380], dr: [0, 380], dn: [0, 180], dj: [180, 380], dc: [380, 1e4], dk: [380, 1e4] };
function column(col) {
  const c = col.getBoundingClientRect(), [top, bottom] = band[col.id];
  return [...document.querySelectorAll("[aria-label]")].filter((t) => {
    const r = t.getBoundingClientRect();
    return r.left + r.width / 2 > c.left && r.left + r.width / 2 < c.right && r.top >= top && r.top < bottom;
  }).sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top)
    .map((t) => t.getAttribute("aria-label")).join(",");
}
let said;
setInterval(() => {
  // Not while the things move: only where they come to rest.
  if (document.getAnimations().some((x) => x.playState === "running")) return;
  const now = cols.map((col) => col.id + " " + column(col)).join(" ") + " at " + at;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/arrange-drop.html"
before='dl L1,L2,L3 dr R1,R2,R3 dn N1,N2 dj J1,J2 dc CB,C2 dk K1,K2,PT '

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The arrange-drop run'"'"'s Boost. */' > "$boosts/$host.css"

log="$out/arrange-drop.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-drop.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await "^browser: title [0-9]* ${before}at -\$" || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* dl ' "$log" | tail -1 | cut -d' ' -f3)
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
  echo "skip: arrange-drop. Browser wasn't the active app, so the page ignored the click."
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
  echo "skip: arrange-drop. The engine doesn't have this tree's Arrange core."
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

# Carries thing $1, pressed at $2 $3 in the page, to $4 $5, where the drop must preview $6
# (Trades or Settles), or be refused there (Refused), and lets go there. What the page then says
# matches $7: for a refusal, the page as it was. $8 says the case, for the messages. A refusal
# shows no preview: the rest finds the drop doesn't fit, and the step stays Carrying, Refused.
drop() {
  set -- "$1" $(page_at $2 $3) $(page_at $4 $5) "$6" "$7" "$8" "$9"
  send press $2 $3
  send dragto $2 $(($3 + 6))
  expect "^browser: arrange $tab Carrying .* $1\$" "$8: a press on $1 and 6 pt didn't lift it"
  send dragto $4 $5
  if [ "$6" = Refused ]; then
    expect "^browser: arrange $tab Carrying Refused .* $1\$" "$8: the rest didn't refuse it"
  else
    expect "^browser: arrange $tab Preview $6 .* $1\$" "$8: the rest didn't preview $6"
  fi
  send release $4 $5
  expect "^browser: arrange $tab " "$8: letting go didn't change the mode's state"
  # Known under #3003: on the batch's core, letting go where the rest refused lands the drop the
  # move found before the refusal, as arrange's own refusal has it. The run takes that as the core
  # gives it, reports it and takes it back with ⌘Z, so the next case starts from the page as it was.
  # Once the core lands nothing there, the run says so.
  landed=
  if [ "$6" = Refused ]; then
    expect "^browser: arrange $tab Selected " "$8: letting go where it doesn't fit left no selection"
    landed=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
      grep "^browser: arrange $tab Selected \(Settles\|Trades\) " | tail -1)
  fi
  if [ -n "$landed" ]; then
    ../tools/window-screenshot.sh "$pid" "$out/arrange-drop-$9.png" || fail "no window to capture"
    send key z cmd
    page_says "^$before" "$8: ⌘Z didn't take back what letting go landed after the refusal"
    dropped="$dropped $8, but letting go there landed it all the same (#3003), which ⌘Z took back,"
  else
    page_says "^$7" "$8: letting go didn't leave what it previewed"
    ../tools/window-screenshot.sh "$pid" "$out/arrange-drop-$9.png" || fail "no window to capture"
    dropped="$dropped $8,"
    if [ "$6" != Refused ]; then
      send key z cmd
      page_says "^$before" "$8: ⌘Z didn't take it back"
    fi
  fi
  # The selection ends, so its mini bar covers nothing the next press needs. A refusal may have
  # left none.
  case "$(grep "^browser: arrange $tab " "$log" | tail -1)" in
    *" Pointing None"*) ;;
    *)
      send click $(page_at 600 600)
      expect "^browser: arrange $tab Pointing None" "$8: a click on the page's empty part didn't end the selection"
      ;;
  esac
}

dropped=
# The left column wraps its cards one box deeper than the right does. Cards are 100 tall, 20 apart,
# from 20; the narrow column's are 60 tall; the scrolling block is 60 tall at 400.
drop L1 140 70 400 190 Trades 'dl R2,L2,L3 dr R1,L1,R3 ' 'a wrapped card on a card in the other column trades' 1
drop R2 400 190 140 190 Trades 'dl L1,R2,L3 dr R1,L2,R3 ' 'a card on a wrapped card trades' 2
drop L1 140 70 400 130 Settles 'dl L2,L3 dr R1,L1,R2,R3 ' 'a wrapped card in the gap under the other column'"'"'s card settles' 3
drop R2 400 190 140 130 Settles 'dl L1,R2,L2,L3 dr R1,R3 ' 'a card in the gap of the column that wraps its cards settles' 4
drop L1 140 70 600 90 Refused "$before" 'a card that can'"'"'t be 120 wide is refused in the narrow column' 5
# The block that scrolls sideways where it stands, carried by its empty part under its line.
drop CB 140 445 400 130 Settles 'dl L1,L2,L3 dr R1,CB,R2,R3 dn N1,N2 dj J1,J2 dc C2 ' 'the scrolling block settles in a place of its width' 6
drop CB 140 445 400 190 Trades 'dl L1,L2,L3 dr R1,CB,R3 dn N1,N2 dj J1,J2 dc R2,C2 ' 'the scrolling block trades with a card of its width' 7
drop CB 140 445 600 90 Refused "$before" 'the scrolling block is refused in a narrower place' 8
# Class-less cards, 50 tall, 20 apart, in columns that wrap them one box deeper: the narrow one's
# from 200, the other's from 400, with a paragraph under it beside the box its cards are in. Each
# card is a box of two boxes, and its column's box holds two such.
drop K1 300 425 600 260 Settles 'dl L1,L2,L3 dr R1,R2,R3 dn N1,N2 dj J1,K1,J2 dc CB,C2 dk K2,PT ' \
  'a class-less card in the gap of a class-less column settles' 9
drop K1 300 425 600 295 Trades 'dl L1,L2,L3 dr R1,R2,R3 dn N1,N2 dj J1,K1 dc CB,C2 dk J2,K2,PT ' \
  'a class-less card on a class-less card trades' 10
# The paragraph's middle, right of its line of words and its button. While K1 is lifted, its column
# closes its gap with it (#3028), so the paragraph is 20 pt higher: 560 is its middle either way.
drop K1 300 425 480 560 Trades 'dl L1,L2,L3 dr R1,R2,R3 dn N1,N2 dj J1,J2 dc CB,C2 dk PT,K2,K1 ' \
  'a card on a paragraph with a button in it trades rather than settling among its words' 11

exec 3>&-
quit
echo "ok: Arrange's drop on columns at different depths and with a block that scrolls sideways:${dropped%,}."
