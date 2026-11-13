# check.sh runs this at 245.
# With Alt held, a press on a link and a move of a pixel and back with the button down still
# click it, and only a drag past Blink's drag distance selects the link's text instead (#2717).
# The engine's patch for it holds the selection back until the pointer is that far. ilo's ⌥-click
# and ⌥⌘-click open links, so a hand that moved a pixel lost them. The link is 40 by 40 in the
# page's top left, pressed with ⌥⌘ (Ctrl+Alt off the Mac). The page cancels each click, so nothing
# opens. It counts what reached it per press in its title: md, mu and ck for the press, release and
# click, ss for selectstart, and "end" for the move off the link after the release, which closes
# the press. Only real input holds keys down before a press, which the Mac's and Linux's drivers
# can't. There the run says it skipped: the Mac's case rests on the source, which has no platform
# branch on this path but the drag distance's size.

serve
printf '%s' '<title>alt-click</title>
<body style="margin:0;height:100vh">
<a id=l href="notes.txt" style="position:fixed;left:0;top:0;width:40px;height:40px"></a>
<script>
let n = 0, seen = [], open = false;
const link = document.getElementById("l");
const say = () => { document.title = "p" + n + ": " + seen.join(" "); };
const add = (s) => { if (open) { seen.push(s); say(); } };
addEventListener("mousedown", (e) => {
  if (link.contains(e.target)) { n++; seen = []; open = true; }
  add("md" + (e.altKey ? "a" : ""));
}, true);
addEventListener("mouseup", () => add("mu"), true);
addEventListener("click", (e) => { add("ck"); e.preventDefault(); }, true);
addEventListener("selectstart", () => add("ss"), true);
addEventListener("mousemove", (e) => {
  if (!e.buttons && open && !link.contains(e.target)) {
    add("end");
    open = false;
  }
}, true);
</script>' > "$out/site/alt-click.html"

log="$out/alt-click.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}alt-click.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* alt-click$' || fail "the page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: alt-click. Browser wasn't the active app."
  exit 0
fi
send layout
frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  grep -o 'WebContentsViewCocoa [0-9,]*' | head -1 | cut -d' ' -f2)
x=$((${frame%%,*} + 20))
y=$((${frame#*,} + 20))
# One press: ⌥⌘ down, the button down on the link, then the moves to $1, …, with the button down,
# then the release where the last move ended and the pointer off the link. $seen is what the page
# saw.
pressed=0
alt_press() {
  pressed=$((pressed + 1))
  send move $page_point
  send press $x $y alt cmd
  if ! logged '^browser: drive: real press'; then
    exec 3>&-
    quit
    echo "skip: alt-click. The driver posts its presses here, with no keys down before them."
    exit 0
  fi
  at=$x
  for to in "$@"; do
    send dragto $to $y alt cmd
    at=$to
  done
  send release $at $y alt cmd
  send move $page_point
  await "^browser: title [0-9]* p$pressed: .* end\$" ||
    fail "press $pressed: the page didn't say the press ended, after it was let go and the pointer left the link: $(grep '^browser: title ' "$log" | tail -1)"
  seen=$(grep "^browser: title [0-9]* p$pressed: " "$log" | tail -1 | cut -d' ' -f4-)
}
for _ in 1 2 3 4; do
  alt_press $((x + 1)) $x
  case " $seen " in
    *" mda "*" ck "*) ;;
    *) fail "press $pressed: ⌥⌘ on a link, moved a pixel and back with the button down, didn't click it. The page saw: $seen" ;;
  esac
done
# Past the drag distance, 4 px (3 on the Mac), it selects the link's text and doesn't click.
for _ in 1 2; do
  alt_press $((x + 6)) $((x + 12))
  case " $seen " in
    *" ck "*) fail "press $pressed: ⌥⌘-drag on a link past the drag distance clicked it. The page saw: $seen" ;;
    *" mda "*" ss "*) ;;
    *) fail "press $pressed: ⌥⌘-drag on a link past the drag distance selected nothing. The page saw: $seen" ;;
  esac
done
exec 3>&-
quit
echo "ok: with ⌥⌘ held, a press on a link moved a pixel and back clicks it, 4 of 4, and a drag past the drag distance selects its text without a click, 2 of 2."
