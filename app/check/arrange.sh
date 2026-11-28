# check.sh runs this at 656.
# Arrange (#2347) on a fixture page of the run's own: from the Boost card's Arrange, a click picks a
# thing and a press, 4 pt and a rest carries it; let go at an edge it settles there, which the mini
# bar's Put back takes back; on another's middle the two trade, and a click then picks each thing
# where it's drawn; where it doesn't fit nothing changes, and with ⌘ it floats, which the mini
# bar's pin turns between staying on screen and scrolling with the page. ⌘Z with the page focused
# takes back the last change, the mini bar's Hide is Zap's rule, which ⌘Z takes back, and its Put
# back on a traded thing takes the trade back. The mini bar goes with Hide and with the mode. Each
# change is saved in the site's .arrange.json as it happens. Last, a settle then a trade of the
# thing it went beside: each thing takes the other's place in the order the settle left (#2975),
# and a press then picks the thing drawn there (#2981).
# A carry to the page's bare background with no key held is no change: only ⌘ floats a thing.
# Reloaded, the page shows the arrangement, and the run measures whether its first frame already
# did, in arrange-first-frame.txt, and says which in its summary rather than failing: a flash there
# is what would bring a renderer hook at document start (#2347's slice 2 plan). Edited outside ilo,
# the file stays off, and Site Controls' Turn On keeps it. A private window shows the profile's
# arrangement but offers no Boost to arrange. The foot bar's Reset comes with the foot bar (#2970).
# No agent step: no tab an agent may act on can be arranged (the core refuses the mode in an
# agent's Space, and giving a tab to an agent ends it), and an agent's call on the user's page is
# refused as not its own, as the mcp run checks.
# It needs the engine with Arrange's core (slice 2): on one without it, it says it skips. It reads
# the core's own lines, which carry Chromium's prefix, and the app's: the mode's state as
# "browser: arrange <tab> <step> <outcome>[ selected] <kind> <name>", and the mini bar's place as
# "browser: mini bar at <x> <y> <w> <h> for <the thing's> in <the page's>".

serve
# The page says in its title where its things are, each time that changes: the column's and the
# side's in their order down the page, those hidden, those floating off both with their position,
# where the last click before the mode landed in the page, the column as its first frame drew it,
# and the space between each two things in the column. Six things: four sections in the column, 120 tall and 20 apart from 40 down, the last too
# wide for the side, which holds two boxes, from 600 across.
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
// A thing is in the column or the side when it is drawn on its track, as a settled or traded one
// is, though the mode takes it out of flow to draw it there; else it floats.
const left = (each) => each.getBoundingClientRect().left;
const on = (thing, track) => Math.abs(left(thing) - left(document.getElementById(track))) < 1;
const floats = (thing) => !on(thing, "col") && !on(thing, "side");
const down = (list) =>
  list.sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top)
    .map((thing) => thing.id).join(",") || "-";
const column = (main) => down(shown().filter((thing) => on(thing, main ? "col" : "side")));
// The space between each two things in the column, down the page, as its gap keeps it (#3030).
const gaps = () => {
  const rs = shown().filter((thing) => on(thing, "col")).map((thing) => thing.getBoundingClientRect())
    .sort((x, y) => x.top - y.top);
  return rs.slice(1).map((r, i) => Math.round(r.top - rs[i].bottom)).join(",") || "-";
};
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
    " float " + floating + " at " + at + " first " + first + " gaps " + gaps();
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
  expect "browser: tab $tab arrange on\$" "the core didn't turn the mode on"
  expect "^browser: boost card $tab closed\$" "the card stayed open over the mode"
}
# Sets $mx, $my, $mw and $mh to the mini bar as the UI last placed it, else fails with $1.
mini_bar() {
  read -r mx my mw mh << EOF
$(grep '^browser: mini bar at ' "$log" | tail -1 | cut -d' ' -f5-8)
EOF
  [ -n "$mh" ] || fail "$1: the UI placed no mini bar"
}
# Fails with $3 unless the mini bar's first place since the last drive step, a drop, is over the
# thing at ($1 $2), its box centred there: the core sends the box at rest, where the thing lands,
# not partway through its landing (#3008).
landed_bar() {
  first=
  for _ in $(seq 100); do
    # The bar is placed a frame after the drop is logged, so no line yet is a wait, not a failure:
    # under set -e grep's status 1 would end the run unsaid (#2974's race).
    first=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: mini bar at ' ||
      true)
    [ -n "$first" ] && break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  [ -n "$first" ] ||
    fail "$3: no mini bar was placed in 10 s after the drop; the log ends: $(tail -5 "$log")"
  read -r ox oy ow oh << EOF
$(echo "$first" | cut -d' ' -f10-13)
EOF
  [ -n "$oh" ] || fail "$3: the mini bar's line has no box: $first"
  dx=$((ox + ow / 2 - $1)) dy=$((oy + oh / 2 - $2))
  [ ${dx#-} -le 1 ] && [ ${dy#-} -le 1 ] || fail "$3: $first"
}
# Waits for the mini bar placed over the thing at ($1 $2), its box centred there, else fails with
# $3. Each press waits for the place the click before it asked for.
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
# Presses the mini bar's first button, or its last with $1 "last": 10 into it past the bar's 3 pt
# inset. With the real pointer, a hand's click with no move onto the bar first: the window server
# picks the window it goes to, and the bar takes it, not the page under it (#3611). Else the
# driver's pointer goes onto the bar first, then clicks.
press_mini_bar() {
  mini_bar "the mini bar to press $2"
  x=$((mx + 13))
  [ "${1:-}" = last ] && x=$((mx + mw - 13))
  if real_pointer; then
    real click $x $((my + mh / 2))
  else
    send move $x $((my + mh / 2))
    send click $x $((my + mh / 2))
  fi
  expect "^browser: mini bar $2 pressed\$" "a click on the mini bar's $2 didn't press it"
}
# The mini bar went after $1, as the Mac's and Linux's hosts say. Windows' prints no line as it
# hides, so there the run can't see it go.
mini_bar_went() {
  windows && return 0
  expect '^browser: mini bar hidden$' "the mini bar stayed after $1"
}
went="The mini bar went with the mode and with the thing hidden."
! windows || went="Windows' host prints no line as the mini bar hides, so the run didn't see it go."
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
expect "^browser: arrange $tab Selected None selected " "a click in the mode didn't select a thing"
expect "^browser: mini bar \(window \)\{0,1\}shown\$" "the mini bar wasn't shown over the thing selected"
# Nothing of the app's stays on screen once it shows nothing. The command bar that ran Edit Boost
# has gone, and its shadow's window stayed ordered in over the page at alpha 0, round where the
# mini bar shows (#3871). The window server's list, front first, as other apps read it.
if [ "$(uname)" = Darwin ]; then
  ../tools/ax-windows.sh "$pid" > "$out/windows-at-bar.txt" 2>&1 ||
    fail "tools/ax-windows.sh didn't list the app's windows: $(tail -3 "$out/windows-at-bar.txt")"
  if grep -q ' alpha 0 at ' "$out/windows-at-bar.txt"; then
    ours=$(awk '/^window server/ { on = 1; next } /^[^ ]/ { on = 0 } on' \
      "$out/windows-at-bar.txt" | tr -s ' \n' ' ')
    fail "a window of the app's stayed on screen at alpha 0, showing nothing, after the command"\
" bar went and as the mini bar showed; the app's windows, front first:$ours"
  fi
fi
expect '^browser: mini bar drawn over [0-9]*$' "the mini bar didn't draw"
shows "$own" "a click in the mode reached the page"
[ "$(said | sed 's/.* at \([0-9-]*,[0-9-]*\) first .*/\1/')" = "${landed% *},${landed#* }" ] ||
  fail "the page took the mode's click: $(said)"
../tools/window-screenshot.sh "$pid" "$out/arrange-picked.png" || fail "no window to capture"

# Carry and settle: the first carried to the edge under the second settles there, the page making
# room, and the change is saved.
carry $(slot 0) $(slot_edge 1)
expect "^browser: arrange $tab Preview Settles " "a rest at an edge didn't show a settle"
send release $(slot_edge 1)
expect "^browser: arrange $tab Selected Settles selected " "letting go at an edge didn't settle"
landed_bar $(slot 1) "the mini bar wasn't first placed over Alpha where it settled"
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the settle"
shows 'b,a,c,w side s,n hidden - float -' "the settle didn't move the thing under the second"
kept 1 "the settle wasn't saved"
# Each gap as the column had it, under the settled thing too (#3030).
for _ in $(seq 100); do
  case "$(said)" in *" gaps 20,20,20") break ;; esac
  sleep 0.1
done
case "$(said)" in *" gaps 20,20,20") ;; *) fail "the settle didn't keep the column's 20 pt gaps: the page says $(said)" ;; esac
../tools/window-screenshot.sh "$pid" "$out/arrange-settled.png" || fail "no window to capture"

# Put back, the mini bar's last button on a thing with a change, takes that change back: Alpha,
# settled, goes back to its own place.
send click $(slot 1)
expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section Alpha\$" \
  "a click didn't select Alpha, settled under Beta"
bar_over $(slot 1) "the UI placed no mini bar over Alpha"
press_mini_bar last "put back"
expect "browser: tab $tab arrange saved 0 changes\$" "the core didn't save Put back"
shows "$own" "Put back didn't take the settle back"
kept 0 "Put back didn't take the change back"

# A preview that goes back leaves the carried thing a ghost again, its room kept (#3189): Alpha
# rests on Gamma's middle, which shows a trade, then at the edge under Beta, which shows the settle
# with Alpha landed there, and settles there when let go. Were Alpha left where the trade's preview
# put it, its room would go with it: Gamma would move up to the edge, and Alpha settle after it.
carry $(slot 0) $(slot 2)
expect "^browser: arrange $tab Preview Trades " "a rest on Gamma's middle didn't show a trade"
send dragto $(slot_edge 1)
expect "^browser: arrange $tab Preview Settles " \
  "a rest at the edge under Beta, after a trade's preview on Gamma, didn't show a settle"
shows 'b,a,c,w side s,n hidden - float -' \
  "the settle's preview under Beta, after a trade's preview on Gamma, didn't show Alpha landed there"
send release $(slot_edge 1)
expect "^browser: arrange $tab Selected Settles selected " \
  "letting go at the edge under Beta, after a trade's preview on Gamma, didn't settle"
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the settle after a trade's preview"
shows 'b,a,c,w side s,n hidden - float -' \
  "letting go at the edge under Beta, after a trade's preview on Gamma, didn't settle Alpha there"
send click $(slot 1)
expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section Alpha\$" \
  "a click didn't select Alpha, settled under Beta after a trade's preview"
bar_over $(slot 1) "the UI placed no mini bar over Alpha"
press_mini_bar last "put back"
expect "browser: tab $tab arrange saved 0 changes\$" "the core didn't save Put back"
shows "$own" "Put back didn't take the settle after a trade's preview back"
kept 0 "Put back didn't take the change back"

# A release as the page moves lands where the pointer is (#3228): Alpha rests on Gamma's middle,
# which shows a trade, then goes to the edge under Beta and is let go at once, so the release comes
# in the first frame of the trade's preview going back. Judged on the page as drawn then, it did
# nothing or traded; judged on the page at rest, it settles there as any release there does.
carry $(slot 0) $(slot 2)
expect "^browser: arrange $tab Preview Trades " "a rest on Gamma's middle didn't show a trade"
send dragto $(slot_edge 1)
send release $(slot_edge 1)
expect "^browser: arrange $tab Selected Settles selected section Alpha\$" \
  "letting go at the edge under Beta as a trade's preview on Gamma went back didn't settle Alpha"
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the settle let go as the page moved"
shows 'b,a,c,w side s,n hidden - float -' \
  "letting go at the edge under Beta as a trade's preview on Gamma went back didn't settle Alpha there"
send click $(slot 1)
expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section Alpha\$" \
  "a click didn't select Alpha, settled under Beta as the page moved"
bar_over $(slot 1) "the UI placed no mini bar over Alpha"
press_mini_bar last "put back"
expect "browser: tab $tab arrange saved 0 changes\$" "the core didn't save Put back"
shows "$own" "Put back didn't take the settle let go as the page moved back"
kept 0 "Put back didn't take the change back"

# Trade: the third let go on the first's middle, and the two trade places.
carry $(slot 2) $(slot 0)
expect "^browser: arrange $tab Preview Trades " "a rest on a thing's middle didn't show a trade"
send release $(slot 0)
expect "^browser: arrange $tab Selected Trades selected " "letting go on a thing's middle didn't trade"
landed_bar $(slot 0) "the mini bar wasn't first placed over Gamma where it traded to"
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the trade"
shows 'c,b,a,w side s,n hidden - float -' "the trade didn't swap the two"
kept 1 "the trade wasn't saved"
../tools/window-screenshot.sh "$pid" "$out/arrange-traded.png" || fail "no window to capture"

# After a trade a click picks the thing drawn where it lands, by name, the two traded too. If one
# doesn't, Arrange presses the wrong thing after a trade the user made, which is more than #2975.
picks() {
  send click $(slot $1)
  expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section $2\$" \
    "after a trade, a click on the column's thing $1 down didn't select $2 drawn there"
}
picks 3 Wide
picks 2 Alpha
picks 1 Beta
picks 0 Gamma

# A fit refusal: the wide section at the side's edge, where it doesn't fit. The rest finds the drop
# doesn't fit and shows no preview, the step staying Carrying, Refused, and letting go there lands
# nothing: the thing stays selected where it was and nothing is saved (#3003).
carry $(slot 3) $(side_edge)
expect "^browser: arrange $tab \(Preview\|Carrying Refused\) " \
  "a rest at the side's edge showed no drop and no refusal"
../tools/window-screenshot.sh "$pid" "$out/arrange-refused.png" || fail "no window to capture"
preview=$(grep "^browser: arrange $tab \(Preview\|Carrying Refused\) " "$log" | tail -1 |
  cut -d' ' -f5)
[ "$preview" = Refused ] || fail "a rest where the thing doesn't fit didn't refuse it: $preview"
send release $(side_edge)
expect "^browser: arrange $tab Selected " "letting go where the thing doesn't fit left no selection"
# The core logs a release's save before its selection, so once Selected is in, a drop the release
# landed is in the log too.
landed=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
  grep "^browser: arrange $tab Selected \(Settles\|Trades\) " | tail -1)
[ -z "$landed" ] || fail "letting go where the rest refused Wide landed a drop all the same: $landed"
shows 'c,b,a,w side s,n hidden - float -' "the refused drop changed the page"
[ "$(changes)" = 1 ] || fail "the refused drop was saved: $(changes) changes"

# The page's bare background, where it has no block, is no place: carried there with no key held,
# the note goes back where it was, and the foot bar says so rather than naming ⌘ (#3017).
carry $(side_slot 1) $(page_at 300 760)
for _ in $(seq 50); do
  carried=$(grep -E "^browser: arrange $tab (Pointing|Carrying|Preview|Selected) " "$log" | tail -1 |
    cut -d' ' -f4-5)
  [ "$carried" = "Carrying None" ] && break
  sleep 0.1
done
[ "$carried" = "Carrying None" ] || fail "a carry to the bare page with no key held wasn't no change: $carried"
# The foot bar's last line, as an earlier step of the carry may have said it.
for _ in $(seq 50); do
  foot=$(grep "^browser: arrange $tab foot bar: " "$log" | tail -1 | cut -d' ' -f6-)
  case "$foot" in "Let go where it goes"*) break ;; esac
  sleep 0.1
done
case "$foot" in
  "Let go where it goes"*) ;;
  *) fail "the foot bar didn't say a drop on the bare page goes nowhere: it says $foot" ;;
esac
send release $(page_at 300 760)
expect "^browser: arrange $tab Selected None selected complementary Note\$" \
  "letting go on the bare page with no key held didn't leave the note as it was"
shows 'c,b,a,w side s,n hidden - float -' "letting go on the bare page with no key held moved the note"
[ "$(changes)" = 1 ] || fail "letting go on the bare page with no key held was saved: $(changes) changes"

# A float: the note carried with ⌘ held stays where it's let go, over the page, and the mini bar's
# pin, its first button, turns it between staying on screen and scrolling with the page.
# ⌘ stays down from the press to the release, as a hand's does: Windows' real input lets a
# command's keys go as it ends (#3109), and the page then reads the carry with no key held.
send flags cmd
carry $(side_slot 1) $(page_at 300 760) cmd
# A float shows no preview, as the thing carried is already where it lands: the core says Floats
# from the lift on.
# The core's steps only: the foot bar's lines say what it shows.
carried=$(grep -E "^browser: arrange $tab (Pointing|Carrying|Preview|Selected) " "$log" | tail -1 |
  cut -d' ' -f4-5)
[ "$carried" = "Carrying Floats" ] || fail "a carry with ⌘ held wasn't a float: $carried"
send release $(page_at 300 760) cmd
expect "^browser: arrange $tab Selected Floats selected " "letting go with ⌘ held didn't float"
landed_bar $(page_at 300 760) "the mini bar wasn't first placed over the float where it was let go"
expect "browser: tab $tab arrange saved 2 changes\$" "the core didn't save the float"
kept 2 "the float wasn't saved"
# The float's pin is the core's, as the foot bar's line for it says, and the page then draws it
# so. The page's title is sampled every 100 ms, and while carried the thing is drawn fixed, so the
# title just after the drop can still be the carry's.
# Sets $pin to the position the foot bar's last line since the drive step gives the float, the
# words it said in $words, and the press that turns it in $turn, else fails with $1.
pin_said() {
  expect "^browser: arrange $tab foot bar: Note \(stays on screen\|scrolls with the page\)" "$1"
  case "$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: arrange $tab foot bar: Note " |
    tail -1)" in
    *"stays on screen"*) pin=fixed words="stays on screen" turn=ScrollsWithPage ;;
    *) pin=absolute words="scrolls with the page" turn=StaysOnScreen ;;
  esac
}
pin_said "the foot bar didn't say how the float is pinned"
pinned=$pin pinned_words=$words
shows "c,b,a,w side s hidden - float n:$pinned" "the page doesn't draw the float as the core pins it"
../tools/window-screenshot.sh "$pid" "$out/arrange-floats.png" || fail "no window to capture"
# ⌘ goes once the float's lines since its release are read, before the mini bar is pressed.
send flags
# The pin pressed: the UI asks the core to turn it, the core's answer comes, then the page.
turn_pin() {
  local ask=$turn
  press_mini_bar first pin
  expect "^browser: arrange $tab $ask\$" "$1: the UI didn't ask the core for $ask"
  case $ask in
    StaysOnScreen) pin=fixed words="stays on screen" turn=ScrollsWithPage ;;
    *) pin=absolute words="scrolls with the page" turn=StaysOnScreen ;;
  esac
  expect "^browser: arrange $tab foot bar: Note $words" "$1: the core didn't answer $ask"
  shows "c,b,a,w side s hidden - float n:$pin" "$1"
}
turn_pin "the pin didn't turn the float"
unpinned=$pin unpinned_words=$words
turn_pin "the pin didn't turn the float back"
[ "$pin" = "$pinned" ] || fail "the pin turned twice didn't come back to $pinned"

# ⌘Z with the page focused takes back one step at a time: each pin, then the float.
send key z cmd
expect "browser: tab $tab arrange saved 2 changes\$" "⌘Z with the page focused didn't reach the mode"
expect "^browser: arrange $tab foot bar: Note $unpinned_words" "⌘Z didn't take back the second pin in the core"
shows "c,b,a,w side s hidden - float n:$unpinned" "⌘Z didn't take back the second pin"
send key z cmd
expect "browser: tab $tab arrange saved 2 changes\$" "the second ⌘Z didn't reach the mode"
expect "^browser: arrange $tab foot bar: Note $pinned_words" "⌘Z didn't take back the first pin in the core"
shows "c,b,a,w side s hidden - float n:$pinned" "⌘Z didn't take back the first pin"
send key z cmd
expect "browser: tab $tab arrange saved 1 changes\$" "the third ⌘Z didn't reach the mode"
shows 'c,b,a,w side s,n hidden - float -' "⌘Z didn't take back the float"
kept 1 "⌘Z's step back wasn't saved"

# Hide, the mini bar's first button on a thing with no change, is Zap's rule, which ⌘Z takes back.
send click $(slot 3)
expect "^browser: arrange $tab Selected None selected " "a click didn't select the wide section"
bar_over $(slot 3) "the UI placed no mini bar over the wide section"
press_mini_bar first hide
expect "browser: tab $tab arrange hid " "the core didn't hand Hide to Zap"
mini_bar_went "Hide hid the thing it was over"
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

# Put back on Gamma, one of the two traded, takes the trade back, both places.
send click $(slot 0)
expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section Gamma\$" \
  "a click didn't select Gamma, which the trade moved"
bar_over $(slot 0) "the UI placed no mini bar over Gamma"
press_mini_bar last "put back"
expect "browser: tab $tab arrange saved 0 changes\$" "the core didn't save Put back"
shows "$own" "Put back on Gamma didn't take the trade back"
kept 0 "Put back didn't take the trade back"

# Escape is Done, and the mini bar goes with the mode.
send click $(slot 1)
expect "^browser: arrange $tab Selected [A-Z][a-z]* selected section Beta\$" \
  "a click didn't select Beta"
expect '^browser: mini bar at ' "the UI placed no mini bar over Beta"
send key escape
expect "^browser: arrange $tab off\$" "escape didn't end the mode"
expect "browser: tab $tab arrange off\$" "the core didn't end the mode"
mini_bar_went "escape ended the mode"

# A settle, then a trade of the thing it went beside: the arrangement is an order, so Gamma and Beta
# swap places in the order the settle left, and Alpha stays where it went (#2975). Each thing is
# styled once a pass, so a press where Wide is drawn picks Wide (#2981).
start_arranging
carry $(slot 0) $(slot_edge 1)
expect "^browser: arrange $tab Preview Settles " "a rest at an edge didn't show a settle"
send release $(slot_edge 1)
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the settle"
shows 'b,a,c,w side s,n hidden - float -' "the settle didn't move the thing under the second"
carry $(slot 2) $(slot 0)
expect "^browser: arrange $tab Preview Trades " "a rest on a thing's middle didn't show a trade"
send release $(slot 0)
expect "browser: tab $tab arrange saved 2 changes\$" "the core didn't save the trade"
shows 'c,a,b,w side s,n hidden - float -' \
  "the trade of Gamma with Beta after the settle didn't swap the two where the settle left them (#2975)"
kept 2 "the settle and the trade weren't saved"
../tools/window-screenshot.sh "$pid" "$out/arrange-2975.png" || fail "no window to capture"
send press $(slot 3)
for _ in $(seq 50); do
  pointed=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: arrange $tab Pointing " |
    tail -1 | cut -d' ' -f6-)
  [ -n "$pointed" ] && break
  sleep 0.1
done
send release $(slot 3)
[ "$pointed" = "section Wide" ] ||
  fail "after the settle and the trade, a press where Wide is drawn picked ${pointed:-nothing}, not Wide (#2981)"
send key escape
expect "^browser: arrange $tab off\$" "escape didn't end the mode"
expect "browser: tab $tab arrange off\$" "the core didn't end the mode"
# The press where Wide is drawn picked it, so the page's title has it floated until the page writes
# the title again for the mode's end, which can come after the core's line (#3909).
expect "^browser: title $tab col [^ ]* side .* float - at " \
  "with the mode ended, the page's title still had a thing floated"
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
first=$(said | sed 's/.* first \([^ ]*\) gaps .*/\1/')
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
new_window_bar
send window 2
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

exec 3>&-
quit
echo "ok: from the Boost card's Arrange, a click picked a thing with the page taking none, and a" \
  "carry settled it at an edge, which Put back took back, traded two on a thing's middle, after" \
  "which a click picked each thing where it was drawn, was carried where it didn't fit, and with" \
  "⌘ floated one, whose pin turned it to scroll with the page and back. ⌘Z took the float back," \
  "Hide was Zap's rule, which ⌘Z took back, and Put back on a traded thing took the trade back." \
  "$went Each change was saved in $host.arrange.json. A settle then a trade of the thing it" \
  "went beside swapped the two in the settle's order, and a press after picked Wide. Letting go" \
  "where Wide didn't fit landed nothing and saved nothing. Reloaded, the page" \
  "placed the arrangement, and" \
  "$first_frame." \
  "Edited outside ilo it stayed off, and Site Controls' Turn On kept it. A private window's page" \
  "was arranged, with no Boost offered." \
  "Windows: $out/arrange-on.png, $out/arrange-picked.png, $out/arrange-settled.png," \
  "$out/arrange-traded.png, $out/arrange-refused.png, $out/arrange-floats.png," \
  "$out/arrange-2975.png, $out/arrange-changed.png," \
  "$out/arrange-private.png"
