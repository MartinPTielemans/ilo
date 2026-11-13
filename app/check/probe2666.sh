# check.sh runs this at 999.
# Probe for #2666, on a proof branch only: what a page sees of a click whose keys go down before
# the pointer comes onto a link. The page's title says, for each try, the events in order: kd<key>
# for a key down, ov for the pointer coming over the link, and md, mu and ck for the press, the
# release and the click, each with the element it went to (L the link, B elsewhere) and the
# modifiers it carried (c Ctrl, a Alt). The page cancels each click, so nothing opens.

serve
printf '%s' '<title>probe</title>
<body style="margin:0;height:100vh">
<a id=l href="notes.txt" style="position:absolute;left:0;top:0;width:300px;height:200px;background:#dde">link</a>
<script>
let n = 0, armed = true, seen = [];
const link = document.getElementById("l");
const at = (e) => (link.contains(e.target) ? "L" : "B") + (e.ctrlKey ? "c" : "") + (e.altKey ? "a" : "");
const add = (s) => {
  if (armed) { n++; seen = []; armed = false; }
  seen.push(s);
  document.title = n + ": " + seen.join(" ");
};
addEventListener("mousemove", (e) => { if (!link.contains(e.target)) armed = true; }, true);
addEventListener("keydown", (e) => { if (!e.repeat) add("kd" + e.key); }, true);
link.addEventListener("mouseover", () => add("ov"));
addEventListener("mousedown", (e) => add("md" + at(e)), true);
addEventListener("mouseup", (e) => add("mu" + at(e)), true);
addEventListener("click", (e) => { add("ck" + at(e)); e.preventDefault(); }, true);
addEventListener("auxclick", (e) => { add("ax" + at(e)); e.preventDefault(); }, true);
</script>' > "$out/site/probe2666.html"

log="$out/probe2666.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}probe2666.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* probe$' || fail "the probe page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: probe2666. Browser wasn't the active app."
  exit 0
fi
send layout
frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
x=${frame%%,*}
y=${frame#*,}
tries=0
for mods in "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt" \
  "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt" "alt cmd" "cmd" "alt"; do
  tries=$((tries + 1))
  # Off the link, which arms the page for the next try, with no keys held.
  send move $page_point
  send click $((x + 150)) $((y + 100)) $mods
  await "^browser: title [0-9]* $tries: .*mu" || echo "probe: $mods try $tries: no release seen"
  send state
  title=$(grep "^browser: title [0-9]* $tries: " "$log" | tail -1 | cut -d' ' -f4-)
  echo "probe: $mods | $title"
done
send move $page_point
# The same, the pointer resting on the link before the keys, as a control.
for mods in "alt cmd" "cmd" "alt"; do
  tries=$((tries + 1))
  send move $page_point
  send move $((x + 150)) $((y + 100))
  send click $((x + 150)) $((y + 100)) $mods
  await "^browser: title [0-9]* $tries: .*mu" || echo "probe: rested $mods try $tries: no release seen"
  send state
  title=$(grep "^browser: title [0-9]* $tries: " "$log" | tail -1 | cut -d' ' -f4-)
  echo "probe: rested $mods | $title"
done
send move $page_point
exec 3>&-
quit
echo "ok: probe2666 ran $tries tries. Log: $log"
