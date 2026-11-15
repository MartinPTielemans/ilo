# check.sh runs this at 654.
# Arrange's mini bar, the window over the thing selected on a page (#2347), drawn by each OS's host.
# First BROWSER_MINI_BAR shows it over a made-up thing selected on the first page that loads, with
# Hide and Put back (selected), the pin as the thing stays on screen (float), and as it scrolls with
# the page (scrolls), each captured over the window with the host's material, corners and shadow.
# Then the core's mode drives it, from the Boost card's Arrange, in the Space's look and the other:
# a click selects a thing at the page's top edge, its bottom edge, its right edge and in its middle,
# and the bar sits over each (under the top one), inside the page; after a resize moves the thing,
# the bar follows it as the pointer comes back over the page. With the page holding the keyboard and
# then the UI, ↑ and ↓ select the box around and inside, the bar's Hide hides the thing, ⌘Z brings
# it back, and escape is Done, which takes the bar away. The foot bar (#2970) says the mode's last
# step: after Inner floats it offers Undo and Reset, and each, pressed with the page focused and
# then the UI, has the core take the float back; its Done ends the mode with either focus.

page='data:text/html;charset=utf-8,<title>ready</title>
<body style="margin:0;height:100vh;background:linear-gradient(%23d9e2e6,%23f3f5f6)">'
shots=
for kind in selected float scrolls; do
  log="$out/mini-bar-$kind.log"
  rm -f "$saved" "$launch_frame"
  BROWSER_MINI_BAR=$kind BROWSER_URL="$page" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  await '^browser: mini bar shown over [0-9]*$' || fail "the mini bar wasn't shown ($kind)"
  await '^browser: mini bar drawn over [0-9]*$' || fail "the mini bar didn't draw ($kind)"
  ../tools/window-screenshot.sh "$pid" "$out/mini-bar-$kind.png" || fail "no window to capture"
  shots="$shots $out/mini-bar-$kind.png"
  quit
done

serve
# Four things on the page: one at its top edge, one at its bottom edge, one at its right edge, and
# Outer, a tenth of the page in, around Inner, so a resize moves them, and a text field under Outer.
# The title says the look it's shown in, the page's size, whether Inner shows, the last key the page
# saw, the field's text, the engine it's in, and where a press landed.
printf '%s' '<!doctype html><meta charset="utf-8"><title>-</title>
<style>
body { margin: 0; font: 16px sans-serif; background: rgb(243, 245, 246) }
section { position: absolute; background: rgb(217, 226, 230) }
#top { left: 30%; top: 0; width: 260px; height: 70px }
#bottom { left: 30%; bottom: 0; width: 260px; height: 70px }
#right { right: 0; top: 40%; width: 140px; height: 90px }
#outer { left: 10%; top: 140px; width: 380px; height: 220px; padding: 30px; box-sizing: border-box }
#inner { position: static; width: 100%; height: 100%; background: rgb(190, 205, 212) }
#field { position: absolute; left: 10%; top: 400px; width: 160px }
</style>
<section id="top" aria-label="Top"></section>
<section id="bottom" aria-label="Bottom"></section>
<section id="right" aria-label="Right"></section>
<section id="outer" aria-label="Outer"><section id="inner" aria-label="Inner"></section></section>
<input id="field" aria-label="Field">
<script>
let at = "-";
let key = "-";
addEventListener("keydown", (event) => { key = event.key; }, true);
let said;
addEventListener("mousedown", (event) => { at = Math.round(event.clientX) + "," + Math.round(event.clientY); });
setInterval(() => {
  const shows = getComputedStyle(inner).display == "none" ? "hidden" : "shown";
  const scheme = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  const now = "scheme " + scheme + " view " + innerWidth + "x" + innerHeight + " inner " + shows + " key " + key + " field " + (field.value || "-") +
    " engine " + (/Chrome\//.test(navigator.userAgent) ? "chromium" : "safari") + " at " + at;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/mini-bar.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
# Arrange is offered where the site has a Boost: one with no rule of its own.
printf '%s\n' '/* The mini-bar run'"'"'s Boost. */' > "$boosts/$host.css"

log="$out/mini-bar-mode.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_LOG=keys BROWSER_DRIVE=1 BROWSER_URL="${site}mini-bar.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await '^browser: title [0-9]* scheme [a-z]* view [0-9]*x[0-9]* inner shown key - field - engine chromium at -$' || fail "the fixture page didn't draw"
tab=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | cut -d' ' -f3)
said() { grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f4-; }
# Waits for the page to say $1, as a pattern over its title, and fails with $2.
page_says() {
  for _ in $(seq 100); do
    said | grep -q "$1" && return 0
    sleep 0.1
  done
  fail "$2: the page says $(said)"
}

# A click where the window's page point is tells where the page is in the window.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "ok: the mini bar showed and drew over a thing selected, with Hide and Put back, and the" \
    "pin either way. Windows:$shots"
  echo "skip: the mini bar in the mode. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
page_says ' at [0-9-]*,[0-9-]*$' "the page didn't say where the click landed"
landed=$(said | sed -n 's/.* at \([0-9-]*\),\([0-9-]*\)$/\1 \2/p')
left=$((${page_point% *} - ${landed% *}))
top=$((${page_point#* } - ${landed#* }))
page_at() { echo $(($1 + left)) $(($2 + top)); }
view() { said | sed -n 's/^scheme [a-z]* view \([0-9]*\)x\([0-9]*\) .*/\1 \2/p'; }
# Whether $1 and $2 are within a point of each other, as the page's rounding leaves them.
near() { [ $(($1 - $2)) -le 1 ] && [ $(($2 - $1)) -le 1 ]; }

if ! [ -e ../core/arrange.cc ] || ! engine_has ../core/arrange.cc ../core/api.cc; then
  exec 3>&-
  quit
  echo "ok: the mini bar showed and drew over a thing selected, with Hide and Put back, and the" \
    "pin either way. Windows:$shots"
  echo "skip: the mini bar in the mode. The engine has no Arrange core yet."
  exit 0
fi

# The card's Arrange, the right of the two on its last line, starts the mode and closes the card.
arrange() {
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab at " "$1: Edit Boost drew no card"
  send layout
  read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
  send click $((bx + bw * 3 / 4)) $((by + bh - 26))
  expect "browser: tab $tab arrange on\$" "$1: the card's Arrange didn't turn the mode on"
}

# The bar's place as the UI last asked for it, after line $1 of the log: "<x> <y> <w> <h> for <the
# thing's> in <the page's>", in points from the top left of the window's content.
bar_after() {
  for _ in $(seq 50); do
    placed=$(sed -n "$(($1 + 1)),\$p" "$log" | grep '^browser: mini bar at ' | tail -1 |
      sed 's/^browser: mini bar at //')
    [ -n "$placed" ] && return 0
    sleep 0.1
  done
  fail "$2: the UI asked for no mini bar after $3. Lines since: $(sed -n "$(($1 + 1)),\$p" "$log" | grep '^browser: \(arrange\|mini bar\)' | tail -8)"
}
# Waits for the bar to be back over Inner, where $inner_bar has it, after $1: the shell moves it as
# it next draws, after the core's line.
bar_back() {
  for _ in $(seq 50); do
    placed=$(grep '^browser: mini bar at ' "$log" | tail -1 | sed 's/^browser: mini bar at //')
    [ "$placed" = "$inner_bar" ] && return 0
    sleep 0.1
  done
  fail "the mini bar didn't go back over Inner ($inner_bar) after $1, but stayed at $placed. Lines: $(grep '^browser: \(arrange\|mini bar\)' "$log" | tail -6)"
}
# Fails unless the bar in $placed is inside the page, mini-bar.edge (8) in, and $1 of the thing:
# over (its bottom 8 over the thing's top, its right end at the thing's, unless that would leave the
# page), or under, for what happened, $2.
bar_is() {
  set -- "$1" "$2" $placed
  x=$3 y=$4 w=$5 h=$6 sx=$8 sy=$9 sw=${10} sh=${11} px=${13} py=${14} pw=${15} ph=${16}
  [ "$x" -ge $((px + 7)) ] && [ $((x + w)) -le $((px + pw - 7)) ] &&
    [ "$y" -ge $((py + 7)) ] && [ $((y + h)) -le $((py + ph - 7)) ] ||
    fail "$2: the mini bar ($x $y ${w}x$h) went past the page ($px $py ${pw}x$ph), 8 inside its edges"
  case $1 in
    over) [ $((y + h)) -le $((sy - 7)) ] && [ $((y + h)) -ge $((sy - 9)) ] ;;
    under) [ "$y" -ge $((sy + sh + 7)) ] && [ "$y" -le $((sy + sh + 9)) ] ;;
  esac || fail "$2: the mini bar ($x $y ${w}x$h) isn't 8 $1 the thing selected ($sx $sy ${sw}x$sh)"
  if near $((x + w)) $((sx + sw)); then
    :
  elif [ $((sx + sw)) -gt $((px + pw - 8)) ] && near $((x + w)) $((px + pw - 8)); then
    :
  else
    fail "$2: the mini bar's right end ($((x + w))) is neither at the thing's ($((sx + sw))) nor 8 inside the page's"
  fi
}
# Selects the thing named $1 with a click at its page point $2 $3, which the core says, and sets
# $placed to the bar over it.
select_thing() {
  n=$(wc -l < "$log")
  send click $(page_at "$2" "$3")
  expect "^browser: arrange $tab Selected .* selected .* $1\$" "a click on $1 in Arrange didn't select it"
  bar_after "$n" "selecting $1" "the click on $1"
}
# Clicks the bar's Hide, its only button over a thing with no change, in its middle.
hide() {
  set -- $placed
  # The pointer onto the bar first, as the Mac's window lets the mouse through but on the bar where
  # the pointer is, as the toast's does.
  send move $(($1 + $3 / 2)) $(($2 + $4 / 2))
  send click $(($1 + $3 / 2)) $(($2 + $4 / 2))
  expect 'browser: tab [0-9]* arrange hid ' "the mini bar's Hide didn't hide the thing selected"
  page_says ' inner hidden ' "the mini bar's Hide didn't hide Inner"
}

# The foot bar's lines since the last step, for a failure's message.
foot_lines() { since | grep '^browser: \(arrange [0-9]* \)\?foot bar' | tail -6; }
# Carries Inner with the free key held to the page's middle, where it floats over the page and the
# foot bar offers Undo and Reset, for $1.
float_inner() {
  set -- "$1" $(page_at $((vw2 / 10 + 190)) 250) $(page_at $((vw2 / 2)) $((vh2 / 2 + 60)))
  send press $2 $3 cmd
  send dragto $2 $(($3 + 6)) cmd
  expect "^browser: arrange $tab Carrying " "$1: a press on Inner and 6 pt didn't lift it"
  send dragto $4 $5 cmd
  send release $4 $5 cmd
  expect "^browser: arrange $tab Selected Floats selected" "$1: letting go with the free key held didn't float Inner"
  expect "^browser: arrange $tab foot bar: Inner .* | Undo on, Reset on\$" \
    "$1: the foot bar didn't say Inner floats and offer Undo and Reset. Its lines: $(foot_lines)"
  expect '^browser: foot bar drawn over [0-9]*$' "$1: the foot bar didn't draw"
}
# Clicks the foot bar's button $1 (arrange-undo, arrange-reset or arrange-done) in its middle, where
# it last drew in the window, for $2.
press_foot_bar() {
  set -- "$1" "$2" $(grep "^browser: foot bar $1 at " "$log" | tail -1 | cut -d' ' -f6-)
  [ -n "${6:-}" ] || fail "$2: the foot bar never drew its $1. Its lines: $(foot_lines)"
  send move $(($3 + $5 / 2)) $(($4 + $6 / 2))
  send click $(($3 + $5 / 2)) $(($4 + $6 / 2))
  expect "^browser: foot bar $1 pressed\$" "$2: a click on the foot bar's $1 at $3 $4 ${5}x$6 didn't press it"
}
# Undo, then Reset, on the foot bar, each after a float of Inner and then $2, which leaves the
# keyboard where $1 says: each has the core take the float back, and the bar goes back to its line
# and Done. The float is captured with the bar, in the look $3.
foot_bar_steps() {
  float_inner "the float before the foot bar's Undo $1"
  ../tools/window-screenshot.sh "$pid" "$out/mini-bar-foot-$3.png" || fail "no window to capture"
  pinned="$pinned $out/mini-bar-foot-$3.png"
  $2
  press_foot_bar arrange-undo "Undo $1"
  expect "^browser: arrange $tab foot bar: [^|]*\$" "Undo $1 left the foot bar offering it. Its lines: $(foot_lines)"
  float_inner "the float before the foot bar's Reset $1"
  $2
  press_foot_bar arrange-reset "Reset $1"
  expect "browser: tab $tab arrange saved 0 changes\$" "Reset $1 didn't have the core take the float back"
  expect "^browser: arrange $tab foot bar: [^|]*\$" "Reset $1 left the foot bar offering it. Its lines: $(foot_lines)"
}
# The page keeps the keyboard through a press on the foot bar, which never takes it: ↑ is the mode's
# from the page.
page_keeps_keys() {
  send key up
  expect 'browser: \[keys\] .* before the page: arrange$' "↑ after a press on the foot bar didn't go through the page"
}
# The UI has the keyboard from a click on the sidebar.
ui_keys() { send click $sidebar_point; }

look=$(said | cut -d' ' -f2)
arrange "the Space's look"
read -r vw vh << EOF
$(view)
EOF
pinned=
# Bottom is clicked 7 into its top, over the foot bar, which covers the page's bottom centre from
# foot-bar.bottom (16) up for its height (40).
for each in "Top $((vw * 3 / 10 + 130)) 35 under" "Bottom $((vw * 3 / 10 + 130)) $((vh - 63)) over" \
  "Right $((vw - 70)) $((vh * 4 / 10 + 45)) over" "Inner $((vw / 10 + 190)) 250 over"; do
  set -- $each
  select_thing "$1" "$2" "$3"
  bar_is "$4" "$1, at the page's $(echo "$1" | tr '[:upper:]' '[:lower:]')"
  ../tools/window-screenshot.sh "$pid" "$out/mini-bar-mode-$1.png" || fail "no window to capture"
  pinned="$pinned $out/mini-bar-mode-$1.png"
done

# With the page holding the keyboard, as the click on Inner left it: ↑ selects Outer, ↓ Inner again,
# each kept from the page for the mode.
inner_bar=$placed
send key up
expect 'browser: \[keys\] .* before the page: arrange$' "↑ with the page focused went to the page"
expect "^browser: arrange $tab Selected .* Outer\$" "↑ with the page focused didn't select the box around Inner"
send key down
expect 'browser: \[keys\] .* before the page: arrange$' "↓ with the page focused went to the page"
expect "^browser: arrange $tab Selected .* Inner\$" "↓ with the page focused didn't select Inner again"
bar_back "↓ with the page focused"
hide
send key z cmd
expect 'browser: \[keys\] .* before the page: arrange$' "⌘Z with the page focused went to the page"
page_says ' inner shown ' "⌘Z with the page focused didn't bring back the Inner that Hide hid"

# After a resize that moves Inner, a tenth of the page in, the bar follows it once the pointer is
# back over the page, which has the core say where it is now.
select_thing Inner $((vw / 10 + 190)) 250
send resize "$vw" $((vh + 40))
for _ in $(seq 50); do
  [ "$(view)" != "$vw $vh" ] && break
  sleep 0.1
done
read -r vw2 vh2 << EOF
$(view)
EOF
[ "$vw2" != "$vw" ] || fail "the resize didn't change the page's width ($vw)"
n=$(wc -l < "$log")
send move $(page_at $((vw2 / 10 + 190)) 200)
send move $(page_at $((vw2 / 10 + 200)) 210)
bar_after "$n" "the resize" "the resize and the pointer back over the page"
bar_is over "Inner, after the resize"
# Inner is Outer's padding inside it, so 30 past a tenth of the page.
set -- $placed
near $(($6 - ${11})) $((vw2 / 10 + 30)) ||
  fail "after the resize the bar is over a thing $(($6 - ${11})) into the page, not Inner, now $((vw2 / 10 + 30)) in"
../tools/window-screenshot.sh "$pid" "$out/mini-bar-mode-resized.png" || fail "no window to capture"
pinned="$pinned $out/mini-bar-mode-resized.png"

# The foot bar's Undo and Reset with the page focused, which a press on the bar leaves it.
send click $(page_at $((vw2 / 10 + 190)) 250)
foot_bar_steps "with the page focused" : "$look"
page_keeps_keys

# Escape with the page focused is Done, and the bar goes.
send click $(page_at $((vw2 / 10 + 190)) 250)
send key escape
expect 'browser: \[keys\] .* before the page: zap or arrange$' "escape with the page focused went to the page"
expect "browser: tab $tab arrange off\$" "escape with the page focused didn't end Arrange"
expect '^browser: arrange foot bar none$' "the foot bar stayed after escape ended Arrange"

# The same with the UI holding the keyboard, from a click on the sidebar, in the other look. The
# keys reach the shell's bindings, never the page. The sidebar has the keyboard before the Space's
# menu, as on sway the theme picker hides as it shows after a right-click with the page focused
# (#2949).
send click $sidebar_point
post rightclick 100 112
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, ")" "the Space's menu didn't offer its theme"
send choose "$(cased 'Edit Theme Color…')"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
send clickkey "$(swatch_x 0)" $swatch_y
expect '^browser: colored space [0-9]* [0-9a-f]*$' "the first swatch didn't colour the Space"
other=dark other_at="231.7 49"
[ "$look" != dark ] || other=light other_at="191.3 49"
send clickkey $other_at
expect "^browser: themed space [0-9]* .*appearance=$other" "the theme picker didn't make the Space's theme $other"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
arrange "the other look"
select_thing Inner $((vw2 / 10 + 190)) 250
inner_bar=$placed
../tools/window-screenshot.sh "$pid" "$out/mini-bar-mode-$other.png" || fail "no window to capture"
pinned="$pinned $out/mini-bar-mode-$other.png"
send click $sidebar_point
send key up
expect "^browser: arrange $tab Selected .* Outer\$" "↑ with the UI focused didn't select the box around Inner"
none_since 'before the page: arrange$' || fail "↑ with the UI focused went through the page"
send key down
expect "^browser: arrange $tab Selected .* Inner\$" "↓ with the UI focused didn't select Inner again"
bar_back "↓ with the UI focused"
hide
send key z cmd
page_says ' inner shown ' "⌘Z with the UI focused didn't bring back the Inner that Hide hid"
send key escape
expect "browser: tab $tab arrange off\$" "escape with the UI focused didn't end Arrange"
# The foot bar's Undo and Reset with the UI focused, then its Done, which ends the mode.
arrange "the foot bar with the UI focused"
send click $(page_at $((vw2 / 10 + 190)) 250)
expect "^browser: arrange $tab Selected .* Inner\$" "a click on Inner didn't select it for the foot bar"
foot_bar_steps "with the UI focused" ui_keys "$other"
ui_keys
press_foot_bar arrange-done "Done with the UI focused"
expect "browser: tab $tab arrange off\$" "the foot bar's Done with the UI focused didn't end Arrange"
expect '^browser: arrange foot bar none$' "the foot bar stayed after its Done"
# Done with the page focused, after a click on Inner, ends it too.
arrange "Done with the page focused"
send click $(page_at $((vw2 / 10 + 190)) 250)
expect "^browser: arrange $tab Selected .* Inner\$" "a click on Inner didn't select it for the foot bar's Done"
press_foot_bar arrange-done "Done with the page focused"
expect "browser: tab $tab arrange off\$" "the foot bar's Done with the page focused didn't end Arrange"
expect '^browser: arrange foot bar none$' "the foot bar stayed after its Done"
grep -q 'before the page: \(zap or \)\?arrange$' "$log" || fail "no key went through the page in the run"

# On the Mac, Japanese input in a field on the page while Arrange is on: the composition takes ↓
# and escape, as the input method does with any key while it composes, and the mode has them again
# once it's committed. The click on the field before the mode gives it the page's focus, which the
# mode's own click on it keeps.
composing=
if [ "$(uname)" = Darwin ]; then
  send click $(page_at $((vw2 / 10 + 80)) 412)
  arrange "Japanese input"
  send click $(page_at $((vw2 / 10 + 80)) 412)
  expect "^browser: arrange $tab Selected .* Field\$" "a click on the field in Arrange didn't select it"
  send state
  in_browser RenderWidgetHostViewCocoa "after the click on the field in Arrange"
  n=$(wc -l < "$log")
  send ime on
  send type nihongo
  page_says ' field nihongo ' "the field didn't show the composition with Arrange on"
  send key down
  send key escape
  page_says ' field - ' "escape in the composition didn't cancel it"
  sed -n "$((n + 1)),\$p" "$log" | grep -q "before the page: \(zap or \)\?arrange\$\|^browser: arrange $tab Selected\|^browser: tab $tab arrange off\$" &&
    fail "Arrange took a key from the composition: $(sed -n "$((n + 1)),\$p" "$log" | grep 'arrange' | tail -4)"
  send type nihongo
  send key space
  page_says ' field 日本語 ' "space didn't convert the composition with Arrange on"
  send key return
  send ime off
  page_says ' field 日本語 ' "return didn't commit the composition with Arrange on"
  send key escape
  expect 'browser: \[keys\] .* before the page: zap or arrange$' "escape after the composition went to the page"
  expect "browser: tab $tab arrange off\$" "escape after the composition didn't end Arrange"
  composing=1
fi

exec 3>&-
quit

# Safari's engine, on the Mac from macOS 14, shows a page the core's Arrange can't reach: the card
# offers no Arrange there, and ↑, ↓, ⌘Z and escape are the page's with the page focused, and the
# UI's with the UI focused, as with the mode off. The checks' streaming host (main.rs) opens the
# page there from its first load.
safari=
if [ "$(uname)" = Darwin ] && [ "$(sw_vers -productVersion | cut -d. -f1)" -ge 14 ]; then
  forget_engine_sites
  log="$out/mini-bar-safari.log"
  step=0
  rm -f "$saved" "$launch_frame"
  BROWSER_LOG=keys BROWSER_DRIVE=1 BROWSER_STREAMING_TEST_HOST="$host" BROWSER_URL="${site}mini-bar.html" \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: title [0-9]* scheme [a-z]* view [0-9]*x[0-9]* inner shown key - field - engine [a-z]* at -$' ||
    fail "the fixture page didn't draw with the streaming host"
  grep -q ' engine chromium at -$' "$log" && fail "the streaming host's page opened in Chromium, not Safari's engine"
  tab=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | cut -d' ' -f3)
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab offers no Arrange in Safari's engine\$" \
    "the Boost's card offered Arrange over Safari's engine's page"
  expect "^browser: boost card $tab at " "Edit Boost drew no card over Safari's engine's page"
  ../tools/window-screenshot.sh "$pid" "$out/mini-bar-safari-card.png" || fail "no window to capture"
  # The page keeps the keyboard as the card opens, and escape there is the page's with neither Zap
  # nor Arrange on, so the card stays open through the keys below, as it would over Chromium's.
  send click $page_point
  page_says ' at [0-9-]*,[0-9-]*$' "the click didn't reach Safari's engine's page"
  send state
  in_browser WKWebView "after a click on Safari's engine's page"
  for each in "up ArrowUp" "down ArrowDown" "z,cmd z" "escape Escape"; do
    chord=${each% *} seen=${each#* }
    n=$(wc -l < "$log")
    send key ${chord%,*} $(echo "$chord" | grep -o 'cmd$')
    page_says " key $seen " "Safari's engine's page didn't see ${chord%,*}"
    # Safari's engine's page asks the app only about what AppKit offers as a key equivalent, which
    # escape isn't, so escape has no line; one that went anywhere but the page is a miss.
    sed -n "$((n + 1)),\$p" "$log" | grep 'before the page: ' | grep -qv 'before the page: page$' &&
      fail "${chord%,*} in Safari's engine's page went elsewhere: $(sed -n "$((n + 1)),\$p" "$log" | grep 'before the page' | tail -2)"
  done
  send click $sidebar_point
  n=$(wc -l < "$log")
  for chord in up down "z cmd" escape; do
    send key $chord
  done
  sed -n "$((n + 1)),\$p" "$log" | grep -q 'before the page: ' &&
    fail "a key with the UI focused went through Safari's engine's page: $(sed -n "$((n + 1)),\$p" "$log" | grep 'before the page' | tail -2)"
  grep -q "^browser: tab $tab arrange \|^browser: arrange $tab " "$log" &&
    fail "Arrange ran over Safari's engine's page: $(grep 'arrange' "$log" | tail -3)"
  exec 3>&-
  quit
  forget_engine_sites
  safari=1
fi
echo "ok: the mini bar showed and drew over a thing selected, with Hide and Put back, and the pin" \
  "either way. Windows:$shots"
echo "ok: in the mode the bar sat over things at the page's top, bottom and right edges and in its" \
  "middle, inside the page, and followed Inner after a resize; ↑, ↓, the bar's Hide, ⌘Z and escape" \
  "worked with the page focused and with the UI, in $look and $other, and so did the foot bar's" \
  "Undo, Reset and Done. Windows:$pinned"
[ -z "$composing" ] || echo "ok: Japanese input in a field on the page kept ↓ and escape while it" \
  "composed with Arrange on, and the mode had escape again once it committed."
[ -z "$safari" ] || echo "ok: over Safari's engine's page the Boost's card offered no Arrange, and" \
  "↑, ↓, ⌘Z and escape went to the page with it focused and to the UI with the UI. Windows:" \
  "$out/mini-bar-safari-card.png"
