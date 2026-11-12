# Peek grows from the link and shrinks back into it showing only the card: at a point inside its
# resting frame, every frame shows the window's own page there until the card reaches it, and
# again once the card has left it as it closes, never Peek's page laid out where it rests (#2519).
# It films opens and closes (tools/window-frames.sh, which keeps every picture) and reads
# each frame with check/peek_motion.py, which takes where the motion is from the dim the frame
# shows over the window's page. Its frames: build/check/peek-frames/open-1, close-1 and on.
# Where the page shows under the preview's window, on Windows and Linux; the Mac's page is in the
# preview's window and scales with its card, and preview.sh records it.

if [ "$(uname)" = Darwin ]; then
  echo "skip: peek-motion. The Mac's Peek page is in its own window, scaling with the card."
  exit 0
fi
serve
# The window's page is one green, its top 40% a link to another site, localhost, and Peek's page
# one magenta, so a frame says whose page shows at a point. The titles say "ready" once drawn.
target="http://localhost:$port/peek-motion-target.html"
printf '%s' "<title>loading</title>
<body style=\"margin:0;height:100vh;background:rgb(0,176,0)\">
<a href=\"$target\" style=\"position:fixed;left:0;top:0;width:100%;height:40%;font:24px system-ui;color:rgb(0,90,0)\">
A link to another site</a>
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"ready\"));</script>" \
  > "$out/site/peek-motion.html"
printf '%s' "<title>loading</title><body style=\"margin:0;height:100vh;background:rgb(224,0,224)\">
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"peek ready\"));</script>" \
  > "$out/site/peek-motion-target.html"
log="$out/peek-motion.log"
frames="$out/peek-frames"
step=0
rm -f "$saved"
rm -rf "$frames"
BROWSER_DRIVE=1 BROWSER_URL="${site}peek-motion.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the green page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: peek-motion. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The motion is what it reads, and the app keeps still under Reduce Motion, as with Windows'
# animations off, which GitHub's runner starts with. Turned on only on a CI VM, as the setting is
# the system's, and off again after.
animations=
if windows && [ -n "${GITHUB_ACTIONS:-}" ] && [ "$(last 'reduce motion')" = on ]; then
  windows_animations 1 || fail "PowerShell didn't turn Windows' animations on"
  animations=off
  expect '^browser: reduce motion off$' "Windows' animations turned on didn't bring the app's motion back"
fi
if [ "$(last 'reduce motion')" = on ]; then
  exec 3>&-
  quit
  echo "skip: peek-motion. The system's animations are off, so the app keeps still and Peek doesn't move."
  exit 0
fi
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
width=${content%x*}
height=${content#*x}

# Films the window's content into $frames/$1 until stop_film.
film() {
  stop="$frames/$1.stop"
  ../tools/window-frames.sh "$pid" 60 "$stop" 0 0 "$width" "$height" "$frames/$1-flat" \
    "$frames/$1" > "$frames/$1.txt" 2> "$frames/$1-capture.log" &
  capture=$!
  for _ in $(seq 600); do
    grep -q '^capturing' "$frames/$1.txt" 2> /dev/null && return 0
    kill -0 "$capture" 2> /dev/null || break
    sleep 0.1
  done
  why=$(tail -1 "$frames/$1-capture.log")
  case "$why" in
  "Reads only"* | "Copies the screen only"*)
    exec 3>&-
    quit
    echo "skip: peek-motion, which can't film the window here: $why"
    exit 0
    ;;
  esac
  fail "the window's filming didn't start: $why"
}
stop_film() {
  touch "$stop"
  wait "$capture" || fail "the window's filming failed: $(tail -1 "${stop%.stop}-capture.log")"
}
mkdir -p "$frames"

# Opens the link in Peek with ⇧-click, from the green page's Today tab, which stays, and closes it
# with Esc, filming each motion into $frames/open-$1 and close-$1.
round() {
  film "open-$1"
  send click 600 200 shift
  expect "^browser: link preview [1-9][0-9]* $target\$" "⇧-click on the link didn't open Peek"
  p=$(grep "^browser: link preview [1-9][0-9]* $target\$" "$log" | tail -1 | cut -d' ' -f4)
  expect '^browser: link preview shown$' "Peek's window didn't show"
  expect '^browser: link preview settled$' "Peek didn't settle"
  stop_film
  expect "^browser: title $p peek ready\$" "Peek's page didn't draw"
  rest=$(grep '^browser: link preview page at ' "$log" | tail -1 | sed 's/^browser: link preview page at //')
  [ -n "$rest" ] || fail "Peek didn't say where its page rests"
  film "close-$1"
  send key escape
  expect '^browser: link preview hidden$' "Esc didn't close Peek"
  expect "^browser: tab $p closed\$" "Esc didn't close Peek's page"
  stop_film
}

# A screen copy can take longer than a motion (Windows' took 260 ms at a time on GitHub's runner),
# so it opens and closes Peek again until a frame has caught each motion mid-way, five times at
# most, and fails at once on a frame that shows Peek's resting frame outside its card.
for n in 1 2 3 4 5; do
  round $n
  # shellcheck disable=SC2086 # numbers: x y w h from x y
  python3 -I check/peek_motion.py "$frames" "$width" $(echo "$rest" | sed 's/ from / /') \
    > "$out/peek-motion.txt" 2>&1 && status=0 || status=$?
  [ $status = 2 ] && ! grep -q "isn't drawn blended" "$out/peek-motion.txt" || break
done
[ "$(today)" = "$a" ] || fail "Peek opened a tab: Today is \"$(today)\""
exec 3>&-
quit
[ -z "$animations" ] || windows_animations 0 || fail "PowerShell didn't turn Windows' animations off again"
case $status in
0) ;;
2) if windows; then
     fail "the frames can't say where Peek's card is: $(grep -v '^open-\|^close-' "$out/peek-motion.txt" | tail -1)"
   fi
   echo "skip: peek-motion, as the frames can't say where the card is here: $(grep -v '^open-\|^close-' "$out/peek-motion.txt" | tail -1)"
   exit 0 ;;
*) fail "Peek's resting frame shows outside its card as it moves: $(grep 'FAIL' "$out/peek-motion.txt" | tr '\n' ' ')" ;;
esac
echo "ok: Peek grows from the link and shrinks back into it with only its card on screen: each frame shows the window's page at a point of Peek's resting frame until the card reaches it and once it has left. Frames: build/check/peek-frames, read in build/check/peek-motion.txt"
