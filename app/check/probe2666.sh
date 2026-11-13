# check.sh runs this at 999.
# Probe 8 for #2666, on a proof branch only: does a move with the button held, between a press on
# a link and its release, take the click away when Alt is held, as Blink's link selection would?
# A 40 by 40 link in the page's top left, pressed with real input, then moved with the button held
# (not at all, a pixel, or a pixel and back), then released, with Ctrl+Alt (the driver's alt), Ctrl
# alone (cmd) or Alt alone (winalt). The page cancels each click, so nothing opens, and counts per
# try what reached it: md, mu, ck for the press, release and click, mv for a held move, ss for
# selectstart, and "end" for the move off the link after the release, which closes the try.

serve
printf '%s' '<title>probe</title>
<body style="margin:0;height:100vh">
<a id=l href="notes.txt" style="position:fixed;left:0;top:0;width:40px;height:40px"></a>
<script>
let n = 0, seen = [], open = false;
const link = document.getElementById("l");
const say = () => { document.title = "t" + n + ": " + seen.join(" "); };
const add = (s) => { if (open) { seen.push(s); say(); } };
addEventListener("mousedown", (e) => {
  if (link.contains(e.target)) { n++; seen = []; open = true; }
  add("md" + (e.ctrlKey ? "c" : "") + (e.altKey ? "a" : "") + e.detail);
}, true);
addEventListener("mouseup", (e) => add("mu"), true);
addEventListener("click", (e) => { add("ck"); e.preventDefault(); }, true);
addEventListener("auxclick", (e) => { add("ax"); e.preventDefault(); }, true);
addEventListener("selectstart", () => add("ss"), true);
addEventListener("dragstart", () => add("ds"), true);
addEventListener("mousemove", (e) => {
  if (e.buttons) {
    add("mv" + e.clientX + "," + e.clientY);
  } else if (open && !link.contains(e.target)) {
    add("end");
    open = false;
  }
}, true);
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
lx=$((x + 20))
ly=$((y + 20))
try=0
for keys in "alt cmd" "cmd" "winalt"; do
  for moved in none pixel back; do
    for _ in 1 2 3 4; do
      try=$((try + 1))
      send move $page_point
      send click $page_point
      send press $lx $ly $keys
      case $moved in
        pixel) send dragto $((lx + 1)) $ly $keys ;;
        back)
          send dragto $((lx + 1)) $ly $keys
          send dragto $lx $ly $keys
          ;;
      esac
      send release $lx $ly $keys
      send move $page_point
      await "^browser: title [0-9]* t$try: .* end\$" ||
        fail "try $try ($keys, moved $moved): the page didn't say the try ended: $(grep '^browser: title ' "$log" | tail -1)"
      title=$(grep "^browser: title [0-9]* t$try: " "$log" | tail -1 | cut -d' ' -f4-)
      case " $title " in
        *" ck "*) clicked=1 ;;
        *) clicked=0 ;;
      esac
      echo "probe: try $try keys '$keys' moved $moved clicked $clicked | $title"
    done
  done
done
send move $page_point
exec 3>&-
quit
echo "ok: probe2666 ran $try tries. Log: $log"
