# A page asking for a device, with a click, ends as if the user had cancelled, until ilo has a
# chooser of its own, and the browser lives through it (#1640): Chrome's chooser hung from a
# BrowserView the core's windows don't have, and the first request took the browser down. HID
# answers with no device, USB and serial reject with NotFoundError, and Bluetooth with
# NotFoundError where the engine exposes it: from the chooser, or before it where the machine has
# no Bluetooth adapter, as a check VM or CI runner may not; Linux's engine doesn't expose it. The
# page then still answers a click, and the app quits cleanly.

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
# A click on the page asks for the next device, and the page's title becomes the answer $2, which
# fails with $3.
ask() {
  send click $page_point
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: devices. Browser wasn't the active app, so the page ignored the click."
    exit 0
  fi
  expect "^browser: title $page $1 $2" "$3"
}
ask hid 'cancelled$' "the page's HID request didn't end with no device"
ask usb 'NotFoundError: ' "the page's USB request didn't reject with NotFoundError"
ask serial 'NotFoundError: ' "the page's serial request didn't reject with NotFoundError"
ask bluetooth '\(NotFoundError: \|unsupported$\)' \
  "the page's Bluetooth request didn't reject with NotFoundError"
bluetooth=$(grep "^browser: title $page bluetooth " "$log" | tail -1 | cut -d' ' -f5-)
send click $page_point
expect "^browser: title $page still here\$" "the page didn't answer a click after its requests"
quit
echo "ok: a page's requests for a HID, a USB device and a serial port end as if cancelled, and"
echo "  Bluetooth's says \"$bluetooth\". The page answers a click after them, and the app quits"
echo "  cleanly."
