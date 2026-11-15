# check.sh runs this at 480.
# A Bluetooth device a page pairs with asks the user in the page's card, not in Chrome's dialogs
# (#1701), which Chrome shows only on Linux and Windows. CI's runners have no Bluetooth, so the
# driver's pair command asks as "Check device" would, through the core's own prompt
# (bc_tab_pair_for_testing). For a PIN the card's field has the keyboard, and ↩ pairs with the PIN
# as typed; for a passkey the card's × cancels; for a plain question its Pair agrees, with no PIN.

log="$out/pair.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL=about:blank "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: selected [0-9]' || fail "the app showed no tab"
pair_tab=$(grep '^browser: selected [0-9]' "$log" | tail -1 | cut -d' ' -f3)
# Where the card is: x y width height.
pair_at() {
  grep "^browser: pair $pair_tab at " "$log" | tail -1 | cut -d' ' -f5-
}

send pair "$pair_tab" pin
expect "^browser: pair $pair_tab Check device Pin\$" "the core's PIN prompt didn't reach the app"
expect "^browser: pair $pair_tab at " "the PIN prompt's card wasn't drawn"
expect '^browser: pair pin field focused$' "the PIN field didn't take the keyboard"
send type a1b2
../tools/window-screenshot.sh "$pid" "$out/pair-pin.png" || fail "no window to capture"
send key return
expect '^browser: command pair via card$' "↩ in the PIN field didn't pair"
expect "browser: tab $pair_tab paired a1b2\$" "the page's pairing didn't get the PIN as typed"
expect "^browser: pair $pair_tab ended\$" "pairing didn't take the card away"

send pair "$pair_tab" passkey
expect "^browser: pair $pair_tab Check device Passkey(\"123456\")\$" \
  "the core's passkey prompt didn't reach the app"
expect "^browser: pair $pair_tab at " "the passkey prompt's card wasn't drawn"
../tools/window-screenshot.sh "$pid" "$out/pair-passkey.png" || fail "no window to capture"
# The card's ×, 7 inside its top right corner and 22 square, as the chooser's.
read -r pair_x pair_y pair_w pair_h << EOF
$(pair_at)
EOF
send click $((pair_x + pair_w - 18)) $((pair_y + 18))
expect '^browser: command pair-close via card$' "the passkey card's × didn't take the click"
expect "browser: tab $pair_tab pairing cancelled\$" "× didn't cancel the page's pairing"
expect "^browser: pair $pair_tab ended\$" "× didn't take the card away"

send pair "$pair_tab" confirm
expect "^browser: pair $pair_tab Check device Confirm\$" "the core's question didn't reach the app"
expect "^browser: pair $pair_tab at " "the question's card wasn't drawn"
# Pair is the card's last row, 12 inside its bottom and left edges and 28 high.
read -r pair_x pair_y pair_w pair_h << EOF
$(pair_at)
EOF
send click $((pair_x + 32)) $((pair_y + pair_h - 13 - 14))
expect '^browser: command pair via card$' "the question card's Pair didn't take the click"
expect "browser: tab $pair_tab paired\$" "Pair didn't agree to the page's pairing"
expect "^browser: pair $pair_tab ended\$" "Pair didn't take the card away"
quit
echo "ok: a Bluetooth device's PIN prompt shows in the card, whose field takes the keyboard and"
echo "  pairs with the PIN as typed on ↩; × cancels a passkey prompt, and Pair agrees to a plain"
echo "  question, each taking the card away. Captures: $out/pair-pin.png, $out/pair-passkey.png"
