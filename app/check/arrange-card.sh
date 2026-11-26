# check.sh runs this at 652.
# Arrange (#2347) from the Boost card, on the core: the card's Arrange starts the mode on the page
# and closes the card; a carry in the page settles a thing under another, which the core saves and
# the app writes in the site's .arrange.json; escape ends the mode; and the card then lists the
# change, by the page's own name for it, with its ↺, in the Space's look and then in the other.
# The mode's bars and keys are #2414's, and arrange.sh's battery #2419's.

serve
# The page says in its title the look it's shown in and the order of its column, each time that
# changes.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; font: 16px sans-serif; background: rgb(243, 245, 246) }
#col { position: absolute; left: 40px; top: 40px; width: 480px; display: flex; flex-direction: column; gap: 20px }
section { height: 120px; background: rgb(217, 226, 230) }
</style>
<div id="col">
<section id="a" aria-label="Alpha"><h2>Alpha</h2></section>
<section id="b" aria-label="Beta"><h2>Beta</h2></section>
<section id="c" aria-label="Gamma"><h2>Gamma</h2></section>
</div>
<script>
const order = () => [...document.querySelectorAll("section")]
  .sort((x, y) => x.getBoundingClientRect().top - y.getBoundingClientRect().top)
  .map((each) => each.id).join(",");
let at = "-";
let said;
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
setInterval(() => {
  const scheme = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  const now = "scheme " + scheme + " col " + order() + " at " + at;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/arrange-card.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
arranged="$boosts/$host.arrange.json"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The arrange-card run'"'"'s Boost. */' > "$boosts/$host.css"

log="$out/arrange-card.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}arrange-card.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await '^browser: title [0-9]* scheme [a-z]* col a,b,c at -$' || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | cut -d' ' -f3)
# The look the Space starts in, the system's, and the other, which the theme picker turns it to.
look=$(grep "^browser: title $tab scheme " "$log" | tail -1 | cut -d' ' -f5)
other=dark
[ "$look" = light ] || other=light
said() { grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-; }

# A click where the window's page point is tells where the page is in the window.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: arrange-card. Browser wasn't the active app, so the page ignored the click."
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

if ! [ -e ../core/arrange.cc ] || ! engine_has ../core/arrange.cc ../core/api.cc; then
  exec 3>&-
  quit
  echo "skip: arrange-card. The engine has no Arrange core yet, so the card's Arrange turns no" \
    "mode on. The page loaded with the site's Boost."
  exit 0
fi

# Opens the Boost's card and sets $bx, $by, $bw and $bh to it.
open_card() {
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab opened\$" "Edit Boost opened no card"
  expect "^browser: boost card $tab at " "the Boost's card wasn't drawn"
  send layout
  read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
}

# The card's Arrange, the right of the two on its last line, starts the mode and closes the card.
open_card
../tools/window-screenshot.sh "$pid" "$out/arrange-card.png" || fail "no window to capture"
send click $((bx + bw * 3 / 4)) $((by + bh - 26))
expect "^browser: arrange $tab started\$" "the card's Arrange didn't start Arrange"
expect "^browser: arrange $tab on\$" "the card's Arrange didn't ask the core for the mode"
expect "browser: tab $tab arrange on\$" "the core didn't turn the mode on"
expect "^browser: boost card $tab closed\$" "the card stayed open over the mode"

# Alpha carried to Gamma's bottom edge, its last 24 pt, settles after it: 6 pt lifts it, and the
# core places the drop where it's let go, saves it, and the app writes the file. Under Gamma,
# where the page has nothing, it would float (#3017), which the column's order alone can't tell.
# Lifting Alpha moves nothing (#3028): Gamma stays at 320 to 440, so 430 is in its last 24 pt. Had
# Gamma moved up a gap, 430 would be under it, where the page has nothing.
send press $(page_at 280 100)
send dragto $(page_at 280 106)
send dragto $(page_at 280 240)
send dragto $(page_at 280 430)
expect "^browser: arrange $tab Carrying Settles " \
  "a carry to 430, Gamma's bottom edge while Alpha is lifted, didn't show a settle: if Gamma moved up a gap at the lift, 430 is under it"
send release $(page_at 280 430)
expect "^browser: arrange $tab Selected Settles selected section Alpha\$" \
  "letting go at Gamma's bottom edge didn't settle Alpha"
expect "browser: tab $tab arrange saved 1 changes\$" "the core didn't save the settle"
for _ in $(seq 100); do
  [ -s "$arranged" ] && break
  sleep 0.1
done
[ -s "$arranged" ] || fail "the app didn't write the site's arrangement"
for _ in $(seq 100); do
  case "$(said)" in *" col b,c,a "*) break ;; esac
  sleep 0.1
done
case "$(said)" in *" col b,c,a "*) ;; *) fail "Alpha didn't settle under Gamma: the page says $(said)" ;; esac
# The mode's marks on the page, round the thing it just settled.
../tools/window-screenshot.sh "$pid" "$out/arrange-on.png" || fail "no window to capture"
send key escape
expect "browser: tab $tab arrange off\$" "escape in the page didn't end Arrange"

# The card lists the change with its ↺, in the Space's look, then in the other.
open_card
../tools/window-screenshot.sh "$pid" "$out/arrange-list-$look.png" || fail "no window to capture"
send click $((bx + bw - 18)) $((by + 18))
expect "^browser: boost card $tab closed\$" "the card's × didn't close it"
post rightclick 100 112
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, ")" "the Space's menu didn't offer its theme"
send choose "$(cased 'Edit Theme Color…')"
expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
# The first palette, then Light or Dark, as cards.sh's.
send clickkey $(panel_at palette 1)
expect '^browser: colored space [0-9]* [0-9a-f]*$' "the first palette didn't colour the Space"
send clickkey $(panel_at look $other)
expect "^browser: themed space [0-9]* .*appearance=$other" \
  "the theme picker's $other button didn't make the Space's theme $other"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
open_card
../tools/window-screenshot.sh "$pid" "$out/arrange-list-$other.png" || fail "no window to capture"
exec 3>&-
quit
echo "ok: the Boost card's Arrange started the mode on the core and closed the card; a carry" \
  "settled Alpha under Gamma, which the core saved and the app wrote in the site's arrangement;" \
  "escape ended the mode, and the card listed the change, $look and $other." \
  "Windows: $out/arrange-card.png, $out/arrange-on.png, $out/arrange-list-$look.png," \
  "$out/arrange-list-$other.png"
