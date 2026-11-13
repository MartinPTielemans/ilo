# check.sh runs this at 999.
# Probe 2 for #2666, on a proof branch only: what a page sees of a click whose keys go down before
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
# The page counted the click that made it active as its first try.
tries=1
# Each try: a click off the link, then at once a click on it, its keys down first.
for mods in "" "cmd" "alt" "alt cmd" "" "cmd" "alt" "alt cmd" "" "cmd" "alt" "alt cmd" \
  "" "cmd" "alt" "alt cmd" "" "cmd" "alt" "alt cmd" "" "cmd" "alt" "alt cmd"; do
  tries=$((tries + 1))
  send move $((x + 500)) $((y + 300))
  send click $page_point
  send click $((x + 150)) $((y + 100)) $mods
  await "^browser: title [0-9]* $tries: .*muL" || echo "probe: after a click, ${mods:-plain} try $tries: no release on the link seen"
  send state
  title=$(grep "^browser: title [0-9]* $tries: " "$log" | tail -1 | cut -d' ' -f4-)
  echo "probe: after a click, ${mods:-plain} | $title"
done
send move $page_point
exec 3>&-
quit
echo "ok: probe2666 ran $tries tries. Log: $log"
