# check.sh runs this at 950, only when named.
# A spike for #3073, never merged: what an Easel drawn as a page of its own costs. It opens a plain
# page from ⌘T's bar and then a full Easel (240 notes, 48 pictures, 16 links, 80 strokes over 6000
# by 4000 points), each ↩ to first paint; pans the Easel down and back with the trackpad; zooms it
# to a quarter and back over 2 s, re-rastered each frame and then as one scaled layer; types 20
# letters into a note, each key to the frame that draws it, and composes 日本語 with the scripted
# input method; switches 16 times between the Easel and the plain page, each key to the page shown;
# and reads the browser's memory before and after each opens. Each line goes to easel-probe.txt.

serve
cp check/easel-probe.html check/easel-plain.html "$out/site/"
dir="$out/easel-probe"
rm -rf "$dir"
mkdir -p "$dir"
log="$dir/session.log"
results="$dir/easel-probe.txt"
: > "$results"
step=0
rm -f "$saved" "$launch_frame"

if [ "$(uname)" = Darwin ]; then
  ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
else
  ms() { date +%s%3N; }
fi
row() {
  echo "easel-probe: $*"
  echo "$*" >> "$results"
}
# The title of page $1 that starts with $2, logged since the last command.
titled() {
  sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n "s/^browser: title [0-9]* $1 $2 //p" | tail -1
}
# Waits for page $1's title to start with $2 and says what follows.
await_title() {
  expect_late "^browser: title [0-9]* $1 $2 " "page $1 never said \"$2\"; its last title was $(grep "^browser: title [0-9]* $1 " "$log" | tail -1)"
  titled "$1" "$2"
}
memory() {
  python3 -I check/usage.py snap "$procs" "$dir/$1.json" kinds || fail "the browser's usage didn't read"
  python3 -I check/usage.py memory "$dir/$1.json"
}
# Opens page $1 with query $2 from ⌘T's bar and says the ms from ↩ to its first paint.
open_page() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for $2"
  send type "$site$1?$2"
  typed=$(ms)
  send key return
  painted=$(await_title "$2" fcp)
  echo $((painted - typed))
}

BROWSER_DRIVE=1 BROWSER_URL="${site}easel-plain.html?p1" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await "^browser: title [0-9]* p1 fcp " || fail "the first plain page didn't paint"
send click $page_point
sleep 5
m1=$(memory m1)
row "memory, one plain page: $m1 (MiB, processes)"
row "open plain page, ↩ to first paint: $(open_page easel-plain.html p2) ms"
send click $page_point
sleep 5
m2=$(memory m2)
row "memory, two plain pages: $m2"
typed=$(ms)
row "open Easel, ↩ to first paint: $(open_page easel-probe.html easel) ms"
ready=$(await_title easel ready)
row "open Easel, ↩ to every picture decoded and drawn: $((ready - typed)) ms"
row "frames at rest, 1 s: $(await_title easel rest)"
send click $page_point
sleep 5
m3=$(memory m3)
row "memory, two plain pages and the Easel: $m3"

send key p
send scroll $page_point 30 60
row "pan down, 60 trackpad events of 30 pt: $(await_title easel "pan 1")"
send key p
send scroll $page_point -30 60
row "pan back up: $(await_title easel "pan 2")"
send key z
row "zoom to 0.25 and back in 2 s, re-rastered: $(await_title easel 'zoom 3 crisp')"
send key x
row "zoom to 0.25 and back in 2 s, one scaled layer: $(await_title easel 'zoom 4 hinted')"
send key t
await_title easel typing > /dev/null
send type abcdefghijklmnopqrst
expect_late '^browser: title [0-9]* easel typed 20 ' "the note didn't take 20 letters: $(grep '^browser: title [0-9]* easel ' "$log" | tail -1)"
row "typing, key event to the start of the frame that draws it: $(titled easel 'typed 20')"
step=$((step + 1))
echo "ime on" >&3
await "^browser: drive $step done" || fail "the driver didn't finish \"ime on\""
if grep -q "^browser: drive $step: can't do that" "$log"; then
  row "composing: this driver has no scripted input method"
else
  send type nihongo
  send key space
  send key return
  row "composing nihongo, space, return: $(await_title easel composed)"
  send ime off
fi

# 16 switches between the Easel (⌘1, the newest Today tab) and the plain page before it (⌘2).
: > "$dir/to-easel"
: > "$dir/to-plain"
for k in 2 1 2 1 2 1 2 1 2 1 2 1 2 1 2 1; do
  pressed=$(ms)
  send key "$k" cmd
  if [ "$k" = 1 ]; then name=easel; file=to-easel; else name=p2; file=to-plain; fi
  shown=$(await_title "$name" shown)
  echo $((${shown##* } - pressed)) >> "$dir/$file"
  sed -n "/^browser: drive $step: /,\$p" "$log" | sed -n 's/^browser: selected [0-9]* in \([0-9.]*\) ms$/\1/p' |
    head -1 >> "$dir/$file-core"
done
median() { sort -n | awk '{ v[NR] = $1 } END { if (!NR) { print "none"; exit } m = NR % 2 ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2; printf "median %s (%s–%s, n=%d)", m, v[1], v[NR], NR }'; }
row "switch to the Easel, key to page shown: $(median < "$dir/to-easel") ms; the core's: $(median < "$dir/to-easel-core") ms"
row "switch to the plain page, key to page shown: $(median < "$dir/to-plain") ms; the core's: $(median < "$dir/to-plain-core") ms"
row "the key's clock costs one process start: $(a=$(ms); b=$(ms); echo $((b - a))) ms"
exec 3>&-
quit
echo "ok: the Easel probe said $(wc -l < "$results") lines, in easel-probe/easel-probe.txt."
