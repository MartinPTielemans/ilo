# check.sh runs this at 230.
# A Blank Window, Arc's (⌃⌘N on the Mac; New Blank Window in the command bar everywhere): one Space
# of its own in the profile of the Space shown, unsaved and listed nowhere, whose pages load in that
# profile and whose tabs stay in it. A tab's Move to › a Space of that profile moves it out, live, to
# the regular window, and closing the window puts the tabs still in it in the archive. A relaunch
# brings back no Blank Window and none of its tabs (#669).

serve
log="$out/blank.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the test page didn't draw"
await '^browser: window opened [0-9]' || fail "the app didn't say which window opened"
main=$(grep -m1 '^browser: window opened ' "$log" | cut -d' ' -f4)
home=$(space)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: blank. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
archived=$(grep '^browser: archive [0-9]* tabs$' "$log" | tail -1 | cut -d' ' -f3)

# The command bar's New Blank Window, with the page's keyboard, and on the Mac ⌃⌘N.
bar_command new-blank-window "New Blank Window" "blank window"
expect '^browser: blank window opened [0-9]* profile [0-9]*$' "New Blank Window opened no Blank Window"
blank=$(grep '^browser: blank window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: window opened $blank\$" "the Blank Window didn't open"
# It shows a Space of its own, which the Spaces never list.
none_since "^browser: spaces .*" || fail "opening a Blank Window changed the Spaces listed"
# The driver numbers windows in the order they were made, the closed ones too.
send window "$(grep -c '^browser: window opened ' "$log")"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar in the Blank Window"
send insert "${site}notes.txt"
send key return
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the Blank Window didn't load its page"
first=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}notes.txt\$|\1|p" "$log" | tail -1)
expect "^browser: window $blank shows $first\$" "the Blank Window didn't show its page"
expect "^browser: today \(.* \)\{0,1\}$first\( .*\)\{0,1\}\$" "the Blank Window's tab isn't in its own Today"
../tools/window-screenshot.sh "$pid" "$out/blank.png" || fail "no window to capture"
# Its name row's words start on the tab titles' column, within 1 pt, the square glyph before them
# where a Space's icon goes (#669): the first ink of the name, at y 104–120, and of New Tab's, on the
# titles' column at y 160–176, both straight-stemmed capitals, from x 36, against each row's ground.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
if windows; then
  to_bmp "$out/blank.png" "$out/blank.bmp" || fail "PowerShell couldn't read $out/blank.png"
elif [ "$(uname)" = Linux ]; then
  magick "$out/blank.png" BMP3:"$out/blank.bmp" || fail "ImageMagick couldn't read $out/blank.png"
else
  sips -s format bmp "$out/blank.png" --out "$out/blank.bmp" > /dev/null || fail "sips couldn't read $out/blank.png"
fi
inks=$(python3 -I - "$out/blank.bmp" "$content" << 'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
offset, = struct.unpack_from("<I", data, 10)
width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
scale = width / float(sys.argv[2])
stride = (bits * width + 31) // 32 * 4
def at(x, y):
    row = y if height < 0 else abs(height) - 1 - y
    b, g, r = data[offset + row * stride + x * bits // 8:][:3]
    return r + g + b
def first_ink(top, bottom):
    ys = range(int(top * scale), int(bottom * scale))
    ground = at(int(36 * scale), int((top + bottom) / 2 * scale))
    for x in range(int(36 * scale), int(90 * scale)):
        if any(abs(at(x, y) - ground) > 150 for y in ys):
            return x / scale
    return -1
print("%.1f %.1f" % (first_ink(104, 120), first_ink(160, 176)))
PY
)
set -- $inks
awk "BEGIN { d = $1 - $2; exit !(d <= 1 && d >= -1) }" ||
  fail "the Blank Window's name starts at x $1, not on the tab titles' column at x $2"

# A second tab in it, which goes in the archive with the window.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the Blank Window"
send insert "${site}?kept"
send key return
expect "^browser: loaded [0-9]* ${site}?kept\$" "the Blank Window didn't load its second page"
second=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}?kept\$|\1|p" "$log" | tail -1)

# The first tab's menu moves it out to the user's Space, where the regular window shows it, live.
post rightclick $(row 1)
expect '^browser: menu shown .*Move to › ' "the Blank Window's tab menu offers no Move to"
send choose "$(grep '^browser: menu shown ' "$log" | tail -1 | sed 's/.*Move to › \([^,]*\).*/\1/')"
expect "^browser: blank window $blank moved $first to window $main space $home\$" \
  "Move to didn't move the Blank Window's tab to Space $home"
expect "^browser: window $main shows $first\$" "the regular window didn't show the tab moved out"
none_since "^browser: loaded $first " || fail "the tab moved out of the Blank Window loaded again"

# Closing the Blank Window puts the tab still in it in the archive.
send window "$(grep -c '^browser: window opened ' "$log")"
send key w cmd shift
expect "^browser: window closed $blank\$" "⇧⌘W didn't close the Blank Window"
expect "^browser: archive $((${archived:-0} + 1)) tabs\$" \
  "closing the Blank Window didn't put its tab in the archive"
none_since "^browser: tab $first closed\$" || fail "the tab moved out closed with the Blank Window"
send window 1

# A relaunch brings back no Blank Window.
exec 3>&-
quit
log="$out/blank-relaunch.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: window opened [0-9]' || fail "the relaunch opened no window"
send state
none_since '^browser: blank window opened' || fail "the relaunch brought back a Blank Window"
[ "$(grep -c '^browser: window opened ' "$log")" = 1 ] ||
  fail "the relaunch opened $(grep -c '^browser: window opened ' "$log") windows, not the regular one alone"
grep -q "^browser: loaded [0-9]* ${site}?kept\$" "$log" &&
  fail "the relaunch loaded the Blank Window's tab"
exec 3>&-
quit
echo "ok: New Blank Window opens a window with a Space of its own that the Spaces don't list, whose pages stay in it; Move to takes a tab out, live, to the regular window; closing it puts its tabs in the archive; and a relaunch brings none of it back. Windows: $out/blank.png"
