# check.sh runs this at 650.
# A site's Boost (#695): the CSS in its file in the profile's Boosts folder shows on its pages as
# they load, and each save of the file at once; Edit Boost in the command bar opens its card, whose
# Zap hides what the user clicks, now and in the file, until escape in the page ends it; its
# Customize… opens the editor, whose changes show at once in each window on the site and ⌘Z takes
# back, and whose Advanced grows the card for its sliders, which the arrow keys move and Reset puts
# back; a Boost that names the Space's colours reads them with var(), in a private window too,
# with no theme's, and one that names none gives the site none; and with the file gone the page is
# its own again, as with a Boost turned off.

serve
# The page says in its title how #ad and its body look, each time that changes, so the run reads
# what the Boost does to it, and adds "ilo-space" while its root has the Space's colours, which its
# own script can read: a step that expects a title without it shows the site got none. Its icon,
# the site's, is the one the editor's header shows.
printf '%s' '<!doctype html><meta charset="utf-8"><title>Boosted</title>
<link rel="icon" href="icon.png">
<body style="margin:0;background:rgb(51, 51, 51)">
<div id="ad" style="height:100vh;background:#c33;color:#fff">an ad</div>
<script>
const ad = document.getElementById("ad");
let said;
setInterval(() => {
  const space = getComputedStyle(document.documentElement).getPropertyValue("--ilo-space-background");
  const now = "ad " + getComputedStyle(ad).display + " " + getComputedStyle(document.body).backgroundColor +
    (space ? " ilo-space" : "");
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/boost.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
file="$boosts/$host.css"
rm -rf "$boosts" "$profile_dir/Default/Boosts off" "$profile_dir/no-boosts"
mkdir -p "$boosts"
printf '%s\n' 'body { background: rgb(0, 0, 128) !important; }' > "$file"

# A launch the checks don't run writes none of the core's lines, nor any naming the site it shows:
# Chromium writes ERROR lines to stderr in any build, which a desktop can keep, and the core's can
# name a site the user browses, as its Boost's script runs or Zap hides an element (#3731). The
# launch shows this run's site's page and quits once the server has sent it. Each OS's launcher
# hands the app's stderr to the log, Windows' (git-bash.sh) as the Mac's and Linux's.
log="$out/boosts-quiet.log"
(
  # A subshell rather than env -u, which can't run Windows' and the Flatpak's launchers, functions
  # that exec the app themselves. Either way $! is the app's, as quit_app needs.
  unset BROWSER_CHECK BROWSER_DRIVE BROWSER_LOG
  export BROWSER_URL="${site}boost.html"
  case $browser in
    /*) exec "$browser" ;;
    *) "$browser" ;;
  esac
) < /dev/null > "$log" 2>&1 &
pid=$!
served "^GET chromium [^ ]*/boost\.html " \
  "a launch the checks don't run didn't load the site's page: $(tail -5 "$log" | tr '\n' '|')"
await '^browser: UI shown at ' || fail "a launch the checks don't run showed no UI"
quit_app
exited
# Chromium's lines, the core's among them, start with its "[pid:tid:time:LEVEL:file]". The lines
# without, the app's and the core's own fprintf's in any launch, hold times, states and the profile
# directory's name, and #3732 keeps the app's others to the checks.
said=$({ grep '^\[[0-9]*:[0-9]*:[0-9/.]*:[A-Z_]*:' "$log"; grep -F "$host" "$log"; } |
  head -5 | tr '\n' '|') || true
[ -z "$said" ] ||
  fail "a launch the checks don't run wrote Chromium's lines, or the site's host: $said"

log="$out/boosts.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}boost.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# And a launch the checks run, under the driver, still has the core's lines, which they read.
await ':ERROR:[^]]*] browser: ' ||
  fail "a launch under the driver wrote none of the core's lines"
await '^browser: boosts of profile [0-9]* read: 1$' || fail "the app didn't read the profile's Boost"
await "^browser: boost shown $host " || fail "the app didn't give the core the site's Boost"
await '^browser: title [0-9]* ad block rgb(0, 0, 128)$' ||
  fail "the Boost's CSS didn't show on the page as it loaded: $(grep '^browser: title' "$log" | tail -1)"
tab=$(grep '^browser: title [0-9]* ad block' "$log" | tail -1 | cut -d' ' -f3)

# A save in the user's editor shows at once, with no reload.
printf '%s\n' 'body { background: rgb(0, 128, 0) !important; }' > "$file"
# Await, as expect reads only what follows a driver's command, and none is sent yet.
await "^browser: title $tab ad block rgb(0, 128, 0)\$" ||
  fail "a save of the Boost's file didn't show on the page"
../tools/window-screenshot.sh "$pid" "$out/boosted.png" || fail "no window to capture"

# A Boost reads its tab's Space's colours with var(), the ones the app gave the core, and takes the
# new ones as the appearance changes them, with no reload. The Boosts before it named none, so the
# page had none: their titles have no "ilo-space".
await '^browser: boost colors [0-9]* ' || fail "the app gave the core no Space's colours"
system=$(grep '^browser: boost colors ' "$log" | tail -1 | cut -d' ' -f5)
other=dark
[ "$system" = light ] || other=light
# The page's computed background for the last colours the app gave in `look`.
space_background() {
  hex=$(grep "^browser: boost colors [0-9]* $1 " "$log" | tail -1 |
    sed 's/.* background #\([0-9a-f]*\) .*/\1/')
  printf 'rgb(%d, %d, %d)' "0x$(echo "$hex" | cut -c1-2)" "0x$(echo "$hex" | cut -c3-4)" \
    "0x$(echo "$hex" | cut -c5-6)"
}
printf '%s\n' 'body { background: var(--ilo-space-background) !important; }' > "$file"
await "^browser: title $tab ad block $(space_background "$system") ilo-space\$" ||
  fail "the Boost didn't read the Space's colour: $(grep '^browser: title' "$log" | tail -1)"
for look in $other $system; do
  item=Dark
  [ "$look" = dark ] || item=Light
  [ "$look" = "$system" ] && item=Automatic
  send menu "Appearance › $item"
  expect "^browser: boost colors [0-9]* $look " "View › Appearance › $item gave Boosts no colours"
  expect "^browser: title $tab ad block $(space_background "$look") ilo-space\$" \
    "the Boost didn't take the Space's $look colour: $(grep '^browser: title' "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/boost-space-colors-$look.png" ||
    fail "no window to capture"
done
# A private window shows its original's Boosts, and its pages get the colours the app gives its
# Space, which carry nothing of the user's: no theme and dark, the same for everyone.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect '^browser: boost colors [0-9]* dark ' "the private window gave its Boosts no colours"
new_window_bar
send window 2
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
send insert "${site}boost.html"
send key return
expect "^browser: loaded [0-9]* ${site}boost.html\$" \
  "the private window didn't load the boosted page"
private_tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}boost.html\$|\1|p" "$log" | tail -1)
expect "^browser: title $private_tab ad block $(space_background dark) ilo-space\$" \
  "the private page took no Space colours: $(grep "^browser: title $private_tab " "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/boost-space-colors-private.png" all ||
  fail "no window to capture"
send key w cmd shift
expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
send window 1
printf '%s\n' 'body { background: rgb(0, 128, 0) !important; }' > "$file"
expect "^browser: title $tab ad block rgb(0, 128, 0)\$" \
  "the Boost's file kept the Space's colour, or a Boost that names none kept them on the page"

send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: boosts' Zap. Browser wasn't the active app, so the page ignored the click." \
    "The Boost's CSS showed as the page loaded and at once as its file changed."
  exit 0
fi

bar_command edit-boost "Edit Boost" "edit boost"
expect "^browser: boost card $tab opened\$" "Edit Boost opened no card"
expect "^browser: boost card $tab at " "the Boost's card wasn't drawn"
send layout
read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
../tools/window-screenshot.sh "$pid" "$out/boost-card.png" || fail "no window to capture"
# Zap, the right of the card's first two buttons, over Arrange.
send click $((bx + bw * 3 / 4)) $((by + bh - 62))
expect "^browser: zap $tab on\$" "the card's Zap didn't turn Zap on"
expect "browser: tab $tab zap on\$" "the core didn't start Zap on the page"
send click $page_point
expect "browser: tab $tab zapped #ad\$" "the core didn't name what Zap's click was on"
expect "^browser: boost shown $host " "Zap's click didn't reach the core as CSS"
expect "^browser: title $tab ad none rgb(0, 128, 0)\$" "what Zap's click was on stayed on the page"
grep -q '^#ad { display: none !important; } /\* Zap \*/$' "$file" ||
  fail "Zap didn't keep what it hid in the Boost's file: $(cat "$file")"
../tools/window-screenshot.sh "$pid" "$out/zapped.png" || fail "no window to capture"
send key escape
expect "^browser: zap $tab off\$" "escape in the page didn't end Zap"
expect "browser: tab $tab zap off\$" "the core didn't end Zap on the page"
# The card's ×, 7 inside its top right corner and 22 square, closes it, as it covers the tab's row.
send layout
read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
send click $((bx + bw - 18)) $((by + 18))
expect "^browser: boost card $tab closed\$" "the card's × didn't close it"
# A Today tab's menu has the Boost after Mute, in Arc's Customize Page…'s place, and the +'s menu
# after New Note; each opens the card.
post rightclick $(row 0)
expect "^browser: menu shown .*, $(cased 'Mute (disabled), Edit Boost…, Add Split View'), " \
  "the tab's menu didn't offer Edit Boost… after Mute"
expect '^browser: menu drawn ' "the tab's menu didn't draw"
../tools/window-screenshot.sh "$pid" "$out/boost-tab-menu.png" || fail "no window to capture"
send choose "$(cased 'Edit Boost…')"
expect "^browser: boost card $tab opened\$" "the tab menu's Edit Boost… opened no card"
send layout
height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
post click 204 $((height - 25))
plus=$(cased "New Space…, New Folder…, New Split View, New Board, New Note, Edit Boost…")
expect "^browser: menu shown $plus\(, .*\)\{0,1\}\$" "the +'s menu didn't offer Edit Boost…"
expect '^browser: menu drawn ' "the +'s menu didn't draw"
../tools/window-screenshot.sh "$pid" "$out/boost-plus-menu.png" || fail "no window to capture"
send choose "$(cased 'Edit Boost…')"
expect "^browser: boost card $tab opened\$" "the +'s Edit Boost… opened no card"

# A second window on the site, which takes the editor's changes as they're made.
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
new_window_bar
send window 2
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the second window"
send insert "${site}boost.html"
send key return
expect "^browser: loaded [0-9]* ${site}boost.html\$" "the second window didn't load the page"
other=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}boost.html\$|\1|p" "$log" | tail -1)
expect "^browser: title $other ad none rgb(0, 128, 0)\$" "the second window's page wasn't Boosted"
send window 1
# Waits for both windows' pages to say their body is $1, or fails with $2.
both_are() {
  for _ in $(seq 100); do
    [ "$(grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f6-)" = "$1" ] &&
      [ "$(grep "^browser: title $other " "$log" | tail -1 | cut -d' ' -f6-)" = "$1" ] &&
      return 0
    sleep 0.1
  done
  fail "$2: $(grep "^browser: title \($tab\|$other\) " "$log" | tail -2 | tr '\n' ' ')"
}

# Customize…, the left of the two on the last line of the card the +'s Edit Boost… left open,
# beside Arrange, opens the editor in its place. Its clicks are in its points: its arrow 13 wide,
# then the card, whose field is 340 wide and 176 tall, 8.5 in and 48 down.
send layout
read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
send click $((bx + bw / 4)) $((by + bh - 26))
expect "^browser: boost editor for $host\$" "Customize… didn't open the Boost editor"
expect '^browser: boost editor drawn$' "the Boost editor didn't draw itself on screen"
../tools/window-screenshot.sh "$pid" "$out/boost-editor.png" || fail "no window to capture"
# A click in the field, at (0.6, -0.6) of it, places the ground's dot, and both pages take its
# colour at once.
send clickkey 293.5 83.2
# Within a point's rounding, as the window's points can be a fraction off its pixels.
near='ground 0\.60[0-9] -0\.60[0-9]; dragged'
expect "^browser: boost editor set $host $near\$" \
  "a click in the editor's field didn't place the ground's dot"
grep -q "^/\* $near \*/\$" "$file" ||
  fail "the editor didn't keep its change in the Boost's file: $(cat "$file")"
grep -q '^#ad { display: none !important; } /\* Zap \*/$' "$file" ||
  fail "the editor's change took Zap's line away: $(cat "$file")"
for _ in $(seq 100); do
  ground=$(grep "^browser: title $tab " "$log" | tail -1 | cut -d' ' -f6-)
  [ "$ground" != "rgb(0, 128, 0)" ] && break
  sleep 0.1
done
[ "$ground" != "rgb(0, 128, 0)" ] || fail "the editor's ground didn't show on the page"
both_are "$ground" "the second window's page didn't take the editor's ground"
../tools/window-screenshot.sh "$pid" "$out/boost-edited.png" || fail "no window to capture"
# ⌘Z, while the editor has the keyboard, takes it back on both.
send key z cmd
expect '^browser: boost editor undo$' "⌘Z didn't take the editor's change back"
both_are "rgb(0, 128, 0)" "⌘Z left the editor's ground on a page"
# Advanced, at Invert lightness's line's end, grows the card 104 points for three sliders. A click
# at three quarters of Contrast's track, from 76 to 293, sets 125 %, and → a step more, which the
# page takes as a filter on its root.
send clickkey 306 244
expect '^browser: boost editor advanced true$' "Advanced didn't open"
expect '^browser: boost editor height 600$' "the card didn't grow for Advanced's sliders"
send clickkey 251.75 280
expect "^browser: boost editor set $host contrast 125; advanced; dragged\$" \
  "a click on Contrast's track didn't set it where it was"
send key right
expect "^browser: boost editor set $host contrast 130; advanced; dragged\$" \
  "→ didn't move the focused slider a step"
# A click at a quarter of Brightness's track sets 75 %, so the capture shows a fill each way.
send clickkey 143.25 312
expect "^browser: boost editor set $host contrast 130; brightness 75; advanced; dragged\$" \
  "a click on Brightness's track didn't set it where it was"
grep -q '^html { filter: contrast(130%) brightness(75%) !important; }$' "$file" ||
  fail "Contrast and Brightness didn't filter the page in the Boost's file: $(cat "$file")"
../tools/window-screenshot.sh "$pid" "$out/boost-advanced.png" || fail "no window to capture"
# Reset, left of Advanced, puts the three back; Advanced again closes them, and the card shrinks.
send clickkey 222 244
expect "^browser: boost editor set $host advanced; dragged\$" "Reset didn't put the sliders back"
send clickkey 306 244
expect '^browser: boost editor advanced false$' "Advanced didn't close"
expect '^browser: boost editor height 496$' "the card didn't shrink as Advanced closed"
send key escape
expect '^browser: boost editor dismissed$' "escape didn't close the Boost editor"

# Window › View Boosts…, after View Media… as Arc's, with ilo's View Boards… between them, opens
# Library › Boosts, and closes it there.
send menubar Window
expect '^browser: menu bar Window: .*, View Media…, View Boards…, View Boosts…, ' \
  "Window has no View Boosts… after View Media… and View Boards…"
send menu 'View Boosts…'
expect '^browser: command view-boosts via ui$' "Window › View Boosts… didn't run"
expect '^browser: library section boosts$' "View Boosts… didn't open the Library on Boosts"
send menu 'View Boosts…'
expect '^browser: library hidden$' "View Boosts… didn't close the Library"

# With its file gone, the page is its own again, as the core hears for a Boost turned off.
rm -f "$file"
expect "^browser: boost shown $host (0 bytes)\$" "the Boost's file going didn't take its CSS away"
expect "^browser: title $tab ad block rgb(51, 51, 51)\$" "the page kept the Boost's CSS"
exec 3>&-
quit
echo "ok: a site's Boost showed on its page as it loaded and at once as its file changed;" \
  "its var(--ilo-space-background) took the Space's colour, dark and light, and a Boost that" \
  "named none left the page none, and a private window's page took no theme's, dark; Edit" \
  "Boost's card turned Zap on, whose click hid #ad and kept it in the file, and escape ended it;" \
  "the tab's menu and the +'s offered Edit Boost…, which opened the card; Customize… opened the" \
  "editor, whose field coloured the page in both windows, ⌘Z took it back and escape closed it;" \
  "Advanced grew the card, Contrast's and Brightness's filtered the page and Reset put them back;" \
  "Window › View Boosts… opened the Library on Boosts and closed it;" \
  "with the file gone the page was its own again." \
  "Windows: $out/boosted.png, $out/boost-space-colors-dark.png," \
  "$out/boost-space-colors-light.png, $out/boost-space-colors-private.png," \
  "$out/boost-card.png, $out/zapped.png," \
  "$out/boost-tab-menu.png, $out/boost-plus-menu.png, $out/boost-editor.png," \
  "$out/boost-edited.png, $out/boost-advanced.png"
