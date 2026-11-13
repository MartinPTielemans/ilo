# check.sh runs this at 999.
# Probe 5 for #2666, as probe 4 with the press's click count (its detail, after md and mu), the
# pointer events (pd pu pc), a drag of the link (ds de) and a move over it with a button down
# (mv<buttons>), to say why a press and release on the link fired no click.
# Probe 4 for #2666, on a proof branch only: little's own link, a 40 by 40 empty anchor in the
# page's top left, ⌥⌘-clicked with the keys down before the pointer comes onto it, and the click
# left to open it. Each try says whether a Little ilo window opened and what the page saw: kd<key>
# for a key down, ov for the pointer coming over the link, md, mu and ck for the press, the release
# and the click, each with where it went (L the link, B elsewhere) and its modifiers (c Ctrl, a Alt).

serve
printf '%s' '<title>probe</title>
<body style="margin:0;height:100vh">
<a id=l href="notes.txt" style="position:fixed;left:0;top:0;width:40px;height:40px"></a>
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
addEventListener("mousedown", (e) => add("md" + at(e) + e.detail), true);
addEventListener("mouseup", (e) => add("mu" + at(e) + e.detail), true);
addEventListener("pointerdown", (e) => add("pd" + at(e)), true);
addEventListener("pointerup", (e) => add("pu" + at(e)), true);
addEventListener("pointercancel", (e) => add("pc"), true);
addEventListener("dragstart", (e) => add("ds"), true);
addEventListener("dragend", (e) => add("de"), true);
let buttons = 0;
link.addEventListener("mousemove", (e) => { if (e.buttons != buttons) { buttons = e.buttons; add("mv" + buttons); } });
addEventListener("click", (e) => add("ck" + at(e)), true);
addEventListener("auxclick", (e) => add("ax" + at(e)), true);
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
opens=0
tries=0
# As little's: a click into the page, a step, then ⌥⌘ down, the pointer onto the link and the press.
for _ in $(seq 20); do
  tries=$((tries + 1))
  send move $page_point
  send click $page_point
  send layout
  send click $((x + 20)) $((y + 20)) alt cmd
  clicked=$step
  opened=""
  for _ in $(seq 50); do
    opened=$(sed -n "/^browser: drive $clicked: /,\$p" "$log" | grep -m1 '^browser: little window opened ' | cut -d' ' -f5)
    [ -n "$opened" ] && break
    sleep 0.1
  done
  send state
  title=$(sed -n "/^browser: drive $clicked: /,\$p" "$log" | grep "^browser: title [0-9]* [0-9]*: " | tail -1 | cut -d' ' -f4-)
  bubble=$(sed -n "/^browser: drive $clicked: /,/^browser: drive $clicked done/p" "$log" | grep -c '^browser: link target window shown$' || true)
  if [ -n "$opened" ]; then
    opens=$((opens + 1))
    echo "probe: try $tries opened little window $opened, bubble before release $bubble | $title"
    send key w cmd
    await "^browser: window closed $opened\$" || fail "⌘W didn't close Little ilo window $opened"
    send window 1 behind
    send window 1
  else
    echo "probe: try $tries opened nothing, bubble before release $bubble | $title"
  fi
done
send move $page_point
exec 3>&-
quit
echo "ok: probe2666 ran $tries tries, $opens opened. Log: $log"
