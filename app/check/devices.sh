# Probe for #2529, on a proof branch only: Enter on a focused button whose click asks for a serial
# port, 50 times. Enter's activation comes on its keydown and the click on the keypress straight
# after, so the request can reach the browser before the activation does, which then refuses it
# with NotFoundError and no chooser. Each press counts as shown (the chooser came, and its × ends
# it) or refused (NotFoundError with no chooser). A click on the page away from the button gives it
# the keyboard and focuses the button again, with no request.

serve
log="$out/devices.log"
step=0
rm -f "$saved"
printf '%s' '<title>loading</title>
<button id=ask style="position:fixed;left:20px;top:20px;width:200px;height:40px">Ask</button>
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
const ask = document.getElementById("ask");
let asked = 0;
onmousedown = e => {
  if (e.target == ask) return;
  e.preventDefault();
  ask.focus();
  document.title = "armed " + asked + " " + (document.activeElement == ask);
};
ask.onclick = () => {
  const n = ++asked;
  document.title = "asking " + n + " " + navigator.userActivation.isActive;
  navigator.serial.requestPort().then(() => "chosen", e => e.name)
    .then(said => document.title = "serial " + n + " " + said);
};
</script>' > "$out/site/enter.html"
BROWSER_DRIVE=1 BROWSER_URL="${site}enter.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the probe page didn't draw"
page=$(grep '^browser: title [0-9]* ready$' "$log" | tail -1 | cut -d' ' -f3)
await "^browser: page $page back 0 forward 0 loading 0 ${site}enter.html\$" ||
  fail "the probe page didn't finish loading"
shown=0
refused=0
others=""
for i in $(seq 50); do
  send click $page_point
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: the Enter probe. Browser wasn't the active app, so the page ignored the click."
    exit 0
  fi
  expect "^browser: title $page armed $((i - 1)) true\$" "the click before press $i didn't focus the button"
  send key return
  pressed=$step
  for _ in $(seq 100); do
    logged "^browser: chooser $page [^ ]* Serial " "$pressed" && break
    logged "^browser: title $page serial $i " "$pressed" && break
    sleep 0.1
  done
  if logged "^browser: chooser $page [^ ]* Serial " "$pressed"; then
    shown=$((shown + 1))
    expect "^browser: chooser $page at " "press $i's chooser card wasn't drawn"
    read -r kx ky kw kh <<EOF
$(grep "^browser: chooser $page at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
    send click $((kx + kw - 18)) $((ky + 18))
    expect "^browser: chooser $page ended\$" "× didn't end press $i's request"
    expect "^browser: title $page serial $i NotFoundError\$" "press $i's request didn't end with NotFoundError"
  elif logged "^browser: title $page serial $i NotFoundError\$" "$pressed"; then
    refused=$((refused + 1))
    echo "probe: press $i refused with no chooser: $(sed -n "/^browser: drive $pressed: /,\$p" "$log" |
      grep "^browser: title $page asking $i " | head -1)"
  else
    said=$(sed -n "/^browser: drive $pressed: /,\$p" "$log" | grep "^browser: title $page " | tail -1)
    others="$others; press $i: ${said:-nothing}"
    asked=$(grep "^browser: title $page \(asking\|serial\) " "$log" | tail -1 | cut -d' ' -f5)
    [ "${asked:-0}" = "$i" ] || fail "Enter $i asked nothing: $said"
  fi
done
quit
echo "probe: Enter on a focused button asking for a serial port, 50 presses: chooser $shown, refused with no chooser $refused${others:+, other$others}"
[ "$refused" = 0 ] || fail "$refused of 50 presses of Enter were refused with no chooser"
echo "ok: 50 presses of Enter on a focused button each showed the serial chooser"
