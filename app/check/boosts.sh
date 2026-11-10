# A site's Boost (#695): the CSS in its file in the profile's Boosts folder shows on its pages as
# they load, and each save of the file at once; Edit Boost in the command bar opens its card, whose
# Zap hides what the user clicks, now and in the file, until escape in the page ends it; and with
# the file gone the page is its own again, as with a Boost turned off.

serve
# The page says in its title how #ad and its body look, each time that changes, so the run reads
# what the Boost does to it.
printf '%s' '<!doctype html><meta charset="utf-8"><title>Boosted</title>
<body style="margin:0;background:rgb(51, 51, 51)">
<div id="ad" style="height:100vh;background:#c33;color:#fff">an ad</div>
<script>
const ad = document.getElementById("ad");
let said;
setInterval(() => {
  const now = "ad " + getComputedStyle(ad).display + " " + getComputedStyle(document.body).backgroundColor;
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

log="$out/boosts.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}boost.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
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
# new ones as the appearance changes them, with no reload.
await '^browser: boost colors [0-9]* ' || fail "the app gave the core no Space's colours"
system=$(grep '^browser: boost colors ' "$log" | tail -1 | cut -d' ' -f5)
other=dark
[ "$system" = light ] || other=light
# The page's computed background for the last colours the app gave in `look`.
space_background() {
  hex=$(grep "^browser: boost colors [0-9]* $1 " "$log" | tail -1 |
    sed 's/.*--ilo-space-background: #\([0-9a-f]*\);.*/\1/')
  printf 'rgb(%d, %d, %d)' "0x$(echo "$hex" | cut -c1-2)" "0x$(echo "$hex" | cut -c3-4)" \
    "0x$(echo "$hex" | cut -c5-6)"
}
printf '%s\n' 'body { background: var(--ilo-space-background) !important; }' > "$file"
await "^browser: title $tab ad block $(space_background "$system")\$" ||
  fail "the Boost didn't read the Space's colour: $(grep '^browser: title' "$log" | tail -1)"
for look in $other $system; do
  item=Dark
  [ "$look" = dark ] || item=Light
  [ "$look" = "$system" ] && item=Automatic
  send menu "Appearance › $item"
  expect "^browser: boost colors [0-9]* $look " "View › Appearance › $item gave Boosts no colours"
  expect "^browser: title $tab ad block $(space_background "$look")\$" \
    "the Boost didn't take the Space's $look colour: $(grep '^browser: title' "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/boost-space-colors-$look.png" ||
    fail "no window to capture"
done
printf '%s\n' 'body { background: rgb(0, 128, 0) !important; }' > "$file"
expect "^browser: title $tab ad block rgb(0, 128, 0)\$" "the Boost's file kept the Space's colour"

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
# Zap, the right of the card's two buttons.
send click $((bx + bw * 3 / 4)) $((by + bh - 26))
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
plus=$(cased "New Space…, New Folder…, New Split View, New Note, Edit Boost…")
expect "^browser: menu shown $plus\(, .*\)\{0,1\}\$" "the +'s menu didn't offer Edit Boost…"
expect '^browser: menu drawn ' "the +'s menu didn't draw"
../tools/window-screenshot.sh "$pid" "$out/boost-plus-menu.png" || fail "no window to capture"
send choose "$(cased 'Edit Boost…')"
expect "^browser: boost card $tab opened\$" "the +'s Edit Boost… opened no card"

# Window › View Boosts…, after View Media… as Arc's, opens Library › Boosts, and closes it there.
send menubar Window
expect '^browser: menu bar Window: .*, View Media…, View Boosts…, ' \
  "Window has no View Boosts… after View Media…"
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
  "its var(--ilo-space-background) took the Space's colour, dark and light; Edit" \
  "Boost's card turned Zap on, whose click hid #ad and kept it in the file, and escape ended it;" \
  "the tab's menu and the +'s offered Edit Boost…, which opened the card;" \
  "Window › View Boosts… opened the Library on Boosts and closed it;" \
  "with the file gone the page was its own again." \
  "Windows: $out/boosted.png, $out/boost-space-colors-dark.png," \
  "$out/boost-space-colors-light.png," \
  "$out/boost-card.png, $out/zapped.png," \
  "$out/boost-tab-menu.png, $out/boost-plus-menu.png"
