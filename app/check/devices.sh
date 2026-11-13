# check.sh runs this at 470.
# A page asking for a device, with a click, gets ilo's chooser in its question card (#1649), and
# the browser lives through it, as it didn't Chrome's bubble (#1640). The HID, USB and serial
# requests each show the card, whose × ends them as Chrome's Cancel does: HID's with no device, USB's
# and serial's with NotFoundError. Bluetooth's rejects with NotFoundError where the engine exposes
# it, from the card or before it where the machine has no adapter, and Linux's engine doesn't. With
# BROWSER_CHECK_HID naming a device that's plugged in, as CI's Linux runner's fake one from
# check/uhid.py, HID's lists it, a click on its row grants it, and the site's Site Controls list it,
# whose Remove takes it away. The Mac and Windows have no such device to plug in, so there the HID
# request is cancelled like the others. The page then still answers a click, and the app quits
# cleanly.

serve
log="$out/devices.log"
step=0
rm -f "$saved"
BROWSER_DRIVE=1 BROWSER_URL="${site}devices.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the devices test page didn't draw"
page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
# The page can draw before its load ends, and a click before then may never reach it.
await "^browser: page $page back 0 forward 0 loading 0 ${site}devices.html\$" ||
  fail "the devices test page didn't finish loading"
# A click on the page asks for the next device. Chromium takes the activation the request needs
# from what the page told the browser of its press, which comes on another pipe than the request
# (#2510): a request on a click straight after its press can reach the browser first, which then
# refuses it without a chooser, as the press of a hand, some 100 ms before its release, doesn't. So
# the click is a press, then the release once the browser has the page's title from the press,
# which comes after the activation on the same pipe.
presses=0
ask() {
  presses=$((presses + 1))
  send press $page_point
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: devices. Browser wasn't the active app, so the page ignored the click."
    exit 0
  fi
  expect "^browser: title $page pressed $presses\$" "the page didn't take the press for its $1 request"
  send release $page_point
}
# The card shows for the page's $1 request, which the core reports as $2.
card() {
  expect "^browser: chooser $page [^ ]* $2 " "the page's $1 request didn't show the chooser"
  expect "^browser: chooser $page at " "the $1 chooser's card wasn't drawn"
}
# The card's ×, 7 inside its top right corner and 22 square, as the Cast card's.
close_card() {
  read -r kx ky kw kh <<EOF
$(grep "^browser: chooser $page at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
  send click $((kx + kw - 18)) $((ky + 18))
  expect '^browser: command chooser-close via card$' "the $1 chooser's × didn't take the click"
  expect "^browser: chooser $page ended\$" "× didn't end the $1 request"
}

ask hid
card HID Hid
if [ -n "${BROWSER_CHECK_HID:-}" ]; then
  expect "^browser: chooser $page device $BROWSER_CHECK_HID (" \
    "the HID chooser didn't list $BROWSER_CHECK_HID"
  expect "^browser: chooser row 0 $page at " "the HID chooser's row wasn't drawn"
  ../tools/window-screenshot.sh "$pid" "$out/devices-hid.png" || fail "no window to capture"
  read -r rx ry rw rh <<EOF
$(grep "^browser: chooser row 0 $page at " "$log" | tail -1 | cut -d' ' -f7-)
EOF
  # A row takes a click once it and the card have been on screen a moment, so the clicks before
  # then are the guard's to ignore.
  expect "^browser: chooser $page takes input\$" "the HID chooser never took input"
  for _ in $(seq 20); do
    send click $((rx + rw / 2)) $((ry + rh / 2))
    logged '^browser: command choose-device ' && break
    sleep 0.1
  done
  logged '^browser: command choose-device ' || fail "a click on the HID row didn't choose it"
  expect "^browser: title $page hid chosen\$" "the page didn't get the HID device it was given"
  expect "^browser: chooser $page ended\$" "choosing the device didn't end the HID request"
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type site controls
  expect '^browser: command bar commands "site controls": Site Controls$' \
    "the command bar didn't find Site Controls"
  send key return
  expect '^browser: site controls settled$' "the Site Controls didn't open"
  expect "^browser: site controls .*; devices $BROWSER_CHECK_HID (" \
    "the Site Controls didn't list the HID device the site was given"
  ../tools/window-screenshot.sh "$pid" "$out/devices-site-controls.png" ||
    fail "no window to capture"
  keyed() { grep '^browser: site controls keyed ' "$log" | tail -1 | cut -d' ' -f5-; }
  for _ in $(seq 20); do
    send key tab
    expect '^browser: site controls keyed ' "Tab didn't move the Site Controls' ring"
    [ "$(keyed)" = 'Device(0)' ] && break
  done
  [ "$(keyed)" = 'Device(0)' ] || fail "Tab didn't reach the HID device: the ring is on $(keyed)"
  send key space
  expect '^browser: menu shown .*Remove' "space on the device's row didn't open its menu"
  send choose Remove
  expect "^browser: site controls remove device $BROWSER_CHECK_HID\$" "Remove didn't run"
  for _ in $(seq 100); do
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: site controls http' | tail -1 |
      grep -qv '; devices ' && break
    sleep 0.1
  done
  sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: site controls http' | tail -1 |
    grep -qv '; devices ' || fail "Remove didn't take the device from the Site Controls"
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  hid="lists $BROWSER_CHECK_HID, whose row grants it, and Site Controls' Remove takes it away"
else
  close_card HID
  expect "^browser: title $page hid cancelled\$" "× didn't end the page's HID request with no device"
  hid="ends with no device from ×"
fi
ask usb
card USB Usb
close_card USB
expect "^browser: title $page usb NotFoundError: " "× didn't reject the USB request with NotFoundError"
ask serial
card serial Serial
close_card serial
expect "^browser: title $page serial NotFoundError: " \
  "× didn't reject the serial request with NotFoundError"
# Bluetooth's card shows where there's an adapter, and × ends it.
ask bluetooth
for _ in $(seq 100); do
  logged "^browser: chooser $page [^ ]* Bluetooth " && break
  logged "^browser: title $page bluetooth \(NotFoundError: \|unsupported$\)" && break
  sleep 0.1
done
if logged "^browser: chooser $page [^ ]* Bluetooth "; then
  card Bluetooth Bluetooth
  close_card Bluetooth
fi
expect "^browser: title $page bluetooth \(NotFoundError: \|unsupported$\)" \
  "the page's Bluetooth request didn't reject with NotFoundError"
bluetooth=$(grep "^browser: title $page bluetooth " "$log" | tail -1 | cut -d' ' -f5-)
send click $page_point
expect "^browser: title $page still here\$" "the page didn't answer a click after its requests"
quit
echo "ok: a page's requests for a HID, a USB device and a serial port show the chooser. HID's"
echo "  $hid; USB's and serial's reject with NotFoundError from ×. Bluetooth's says"
echo "  \"$bluetooth\". The page answers a click after them, and the app quits cleanly."
[ -n "${BROWSER_CHECK_HID:-}" ] ||
  echo "skip: granting a HID device. No BROWSER_CHECK_HID: no fake device to plug in here."
