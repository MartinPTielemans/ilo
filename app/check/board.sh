# check.sh runs this at 690.
# A Board (#3073). New Board's key makes one in the Space's profile, a pinned tab whose row's name
# is there to type over; ↩ keeps the name in the Board's folder and gives the page the keyboard,
# where the first note's field takes the keys typed next. The page saves the note a second later, in
# its own Board's folder alone, and no log line says what the note or the name is. Library › Boards
# lists it. Renamed while open, opened from the command bar by the start of its name, unpinned,
# archived and reopened, it's still only "(a Board)" in the log, never its name, and the reopened
# page reads its file: a change keeps the note. Reloaded, and left for a web page, it leaves the
# browser running. A Board whose file holds markup in a note's text and a javascript: link runs
# neither. A picture waiting in a Board's arrivals lands as a card when the Board opens, and leaves
# the list once the card is saved; one that comes while its tab isn't showing lands when it's shown,
# a step from the first, both captured. A Board's file over 4 MiB, which the host doesn't read, says
# so and is never written over, captured, as is one holding [] rather than a Board. A private window
# has no New Board, and a Board's address there shows "This Board isn't here.", captured. The Board
# holding [] shows its file in the system's file manager from its note's button, last of the steps;
# after it, that the page the bar's ↩ loaded over a Board had the keyboard is judged. Japanese
# input as the first keys is the hosted Mac's.

log="$out/board.log"
step=0
rm -f "$saved" "$launch_frame"
# Showing the file on Linux, with no file manager on the bus, falls to xdg-open, which tries
# $BROWSER before the runner's own browsers: true ends it there, so none stays on the display.
opener=${BROWSER:-}
[ "$(uname)" != Linux ] || opener=true
BROWSER=$opener BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>ready</title>' "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load

# New Board, from its key.
send key e ctrl shift
expect '^browser: command new-board via ' "⌃⇧E didn't run New Board"
expect '^browser: new board [0-9a-f]\{32\} tab [0-9]*$' \
  "New Board made no Board: $(grep '^browser: new board ' "$log" | tail -1)"
made=$(grep '^browser: new board [0-9a-f]\{32\} tab ' "$log" | tail -1)
id=$(echo "$made" | cut -d' ' -f4)
board=$(echo "$made" | cut -d' ' -f6)
expect "^browser: name board $board\$" "the new Board's row didn't offer its name to type over"
folder=$(find "$profile_dir" -type d -path "*/Boards/$id" | head -1)
[ -n "$folder" ] || fail "New Board made no folder for $id in the profile ($profile_dir)"
boards=$(dirname "$folder")

# The name, kept with ↩, and the page has the keyboard for the first note.
send type "kitchen"
send key return
expect "^browser: board $board has the keyboard\$" \
  "↩ on the new Board's name didn't give its page the keyboard"
for _ in $(seq 50); do
  [ "$(cat "$folder/name" 2>/dev/null)" = kitchen ] && break
  sleep 0.1
done
[ "$(cat "$folder/name" 2>/dev/null)" = kitchen ] ||
  fail "the Board's name wasn't kept in its folder: $(cat "$folder/name" 2>/dev/null)"
send type "shelves over the sink"
expect 'browser: board saved$' "the note typed in the new Board wasn't saved"
grep -q 'shelves over the sink' "$folder/board.json" ||
  fail "the Board's file doesn't hold the note typed: $(head -c 300 "$folder/board.json")"
# 1a: the page wrote its own Board and no other.
[ "$(ls "$boards")" = "$id" ] || fail "the profile's Boards aren't only $id: $(ls "$boards" | tr '\n' ' ')"
# Nothing of what the user wrote is logged, past the driver's own echo of the keys it typed.
written=$(grep -v '^browser: drive [0-9]*: ' "$log" | grep 'shelves\|kitchen' | head -1)
[ -z "$written" ] || fail "the log says what the Board holds: $written"
../tools/window-screenshot.sh "$pid" "$out/board.png" || fail "no window to capture"

# Library › Boards lists it.
bar_command view-boards "View Boards…" "view boards"
expect '^browser: library boards 1$' "Library › Boards didn't list the new Board"
send key escape
expect '^browser: library hidden$' "escape didn't close the Library"

# 1a on every way out: a rename while it's open, the command bar finding it by its name, and its
# tab archived and reopened. A Board's title and window title lines say "(a Board)".
bar_command rename-tab "Rename Current Tab…" "rename current tab"
expect "^browser: rename tab $board\$" "Rename Current Tab… didn't turn the Board's row into a field"
send type "pantry"
send key return
for _ in $(seq 50); do
  [ "$(cat "$folder/name" 2>/dev/null)" = pantry ] && break
  sleep 0.1
done
[ "$(cat "$folder/name" 2>/dev/null)" = pantry ] ||
  fail "the Board's new name wasn't kept in its folder: $(cat "$folder/name" 2>/dev/null)"
grep -q "^browser: title $board (a Board)\$" "$log" ||
  fail "the Board's title line doesn't say \"(a Board)\": $(grep "^browser: title $board " "$log" | tail -1)"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert 'data:text/html,<title>away</title>'
send key return
expect '^browser: loaded [0-9]* data:text/html,<title>away</title>$' "the page to leave the Board for didn't load"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type pant
expect "^browser: command bar puts tab $board first\$" \
  "the command bar didn't put the Board's tab first for the start of its name: $(grep '^browser: command bar suggestions ' "$log" | tail -1)"
send key return
expect "^browser: command bar switch to tab $board\$" "↩ on the Board's row didn't switch to its tab"
expect "^browser: selected $board\( in .*\)\{0,1\}\$" "↩ on the Board's row didn't show its tab"
expect '^browser: window title (a Board)$' "the window's title line, the Board shown, doesn't say \"(a Board)\""
send key d cmd
expect '^browser: command toggle-pin via ' "⌘D on the Board didn't reach the UI"
expect "^browser: today\( [0-9]*\)* $board\( [0-9]*\)*\$" "⌘D didn't move the Board's tab into Today: Today is \"$(today)\""
send key w cmd
expect '^browser: command close-tab via ' "⌘W on the Board didn't reach the UI"
expect "^browser: tab $board closed\$" "⌘W didn't archive the Board's tab"
send key t cmd shift
expect '^browser: command reopen-closed-tab via ' "⇧⌘T didn't reach the UI"
expect "^browser: loaded [0-9]* chrome://board/$id\$" "⇧⌘T didn't reopen the Board"
board=$(grep "^browser: loaded [0-9]* chrome://board/$id\$" "$log" | tail -1 | cut -d' ' -f3)
for _ in $(seq 50); do
  grep -q "^browser: title $board (a Board)\$" "$log" && break
  sleep 0.1
done
grep -q "^browser: title $board (a Board)\$" "$log" ||
  fail "the reopened Board's title line doesn't say \"(a Board)\": $(grep "^browser: title $board " "$log" | tail -1)"
written=$(grep -v '^browser: drive [0-9]*: ' "$log" | grep 'shelves\|kitchen\|pantry' | head -1)
[ -z "$written" ] || fail "the log says what the Board holds: $written"
# The reopened page read its file: a pan, which it saves, keeps the note. A page that couldn't
# read it would have nothing to write over it but nothing.
saves=$(grep -c 'browser: board saved$' "$log")
send scroll 640 400 40 5
for _ in $(seq 50); do
  [ "$(grep -c 'browser: board saved$' "$log")" -gt "$saves" ] && break
  sleep 0.1
done
[ "$(grep -c 'browser: board saved$' "$log")" -gt "$saves" ] ||
  fail "a pan on the reopened Board wasn't saved: $(grep 'browser: board ' "$log" | tail -1)"
grep -q 'shelves over the sink' "$folder/board.json" ||
  fail "the reopened Board's save lost its note: $(head -c 300 "$folder/board.json")"

# A reload of the Board, and a load away from it to a web page, drop what its page had on its way
# and leave the browser running (#3427); back brings the Board again for the steps after. ⌘← goes
# back from the page or the UI, wherever the bar's ↩ left the keyboard, so this claims nothing about
# who has it: which side heard it is judged after the last step (#3453), so a miss there ends no
# step before it.
send key r cmd
expect "^browser: loaded $board chrome://board/$id\$" "⌘R didn't load the Board again"
serve
send key l cmd
expect '^browser: command bar shown$' "⌘L on the Board didn't open the command bar"
send type "${site}page.html"
send key return
expect "^browser: loaded $board ${site}page.html\$" "the Board's tab didn't load the page typed in the bar"
# Where the ↩ left the keyboard, as the driver says it: the key window and the view with focus.
went=$(grep "^browser: drive $step done, " "$log" | tail -1 |
  sed 's/^[^,]*, \(key [^,]*, focus [^,]*\),.*/\1/')
send key left cmd
expect '^browser: command back via ' "⌘left on the page the bar loaded over the Board didn't run back"
back_via=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: command back via ' |
  head -1 | cut -d' ' -f5)
expect "^browser: loaded $board chrome://board/$id\$" "back didn't bring the Board again"

# 1c: a Board whose note's text is markup, and a link to javascript:, runs neither: each would
# take the page to #ran, which its page line would say, as a Board's title is never logged. Its id
# isn't in $marked, where send keeps the driver's marked text.
markup=$(printf 'b%.0s' $(seq 32))
mkdir -p "$boards/$markup"
cat > "$boards/$markup/board.json" <<'EOF'
{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": [
 {"id": "a", "kind": "note", "x": 80, "y": 80, "w": 240, "text": "<img src=x onerror=\"location.hash='ran'\">"},
 {"id": "b", "kind": "link", "x": 360, "y": 80, "w": 280, "h": 52, "url": "javascript:location.hash='ran'", "title": "ran"}
]}
EOF
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$markup"
send key return
expect "^browser: loaded [0-9]* chrome://board/$markup\$" "the Board with markup didn't load"
! grep -q "^browser: page [0-9]* .* chrome://board/$markup#ran\$" "$log" ||
  fail "a Board's note's markup or its javascript: link ran"
../tools/window-screenshot.sh "$pid" "$out/board-markup.png" || fail "no window to capture"

# A picture waiting in a Board's arrivals lands as a card when it opens: the page saves the card,
# then the core takes it off the list.
landing=$(printf 'f%.0s' $(seq 32))
mkdir -p "$boards/$landing/files"
echo '{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": []}' > "$boards/$landing/board.json"
# Puts a 64 × 40 picture, green as much as $1, in the landing Board's files and its name on its
# arrivals, ending its line with \n as the core does, which Python on Windows would write as \r\n.
# Bash 3.2 misreads a here-document inside $(), so the name is read back from the list.
put_picture() {
  python3 -I - "$boards/$landing/files" "$1" >> "$boards/$landing/arrived" <<'PYEOF'
import hashlib, os, struct, sys, zlib
w, h = 64, 40
rows = b"".join(b"\0" + bytes((40, int(sys.argv[2]), 98)) * w for _ in range(h))
chunk = lambda kind, data: struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + \
    chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")
name = hashlib.sha256(png).hexdigest() + ".png"
open(os.path.join(sys.argv[1], name), "wb").write(png)
sys.stdout.buffer.write((name + "\n").encode())
PYEOF
}
# Waits for the core to take the landing Board's arrivals off its list; $1 says what didn't happen.
taken_off() {
  for _ in $(seq 50); do
    [ -s "$boards/$landing/arrived" ] || return 0
    sleep 0.1
  done
  fail "$1: the list still holds $(cat "$boards/$landing/arrived")"
}
put_picture 120
first=$(tail -1 "$boards/$landing/arrived")
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$landing"
send key return
expect "^browser: loaded [0-9]* chrome://board/$landing\$" "the Board with a picture waiting didn't load"
landing_tab=$(grep "^browser: loaded [0-9]* chrome://board/$landing\$" "$log" | tail -1 | cut -d' ' -f3)
# The tab it opened over hides and saves its own Board too, so the list's emptying says this one
# saved its card: the page tells the core it took them only after.
taken_off "the picture that was waiting wasn't taken off the Board's arrivals when it opened"
grep -q "\"kind\":\"picture\".*\"file\":\"$first\"" "$boards/$landing/board.json" ||
  fail "the Board's file has no card of the picture that was waiting: $(head -c 300 "$boards/$landing/board.json")"
../tools/window-screenshot.sh "$pid" "$out/board-landed.png" || fail "no window to capture"

# One that comes while the Board's tab isn't showing lands when it's shown, on visibilitychange, at
# the centre of its view: a step down and right of the first, which is there.
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$board" | cut -d: -f1)
switch "$n cmd" "$board"
put_picture 160
second=$(tail -1 "$boards/$landing/arrived")
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$landing_tab" | cut -d: -f1)
switch "$n cmd" "$landing_tab"
taken_off "the picture that came while the Board's tab wasn't showing wasn't taken when it was shown"
# What's wrong, if anything, goes to a file: under set -e a failing python3 would end the check
# without fail's line.
python3 -I - "$boards/$landing/board.json" "$first" "$second" > "$out/board-landed.why" <<'PYEOF'
import json, sys
cards = {card.get("file"): card for card in json.load(open(sys.argv[1]))["cards"]}
a, b = cards.get(sys.argv[2]), cards.get(sys.argv[3])
if not b:
    print("the Board's file has no card of the picture that came while its tab wasn't showing")
elif (b["x"] - a["x"], b["y"] - a["y"]) != (24, 24):
    print("the second picture landed %d, %d from the first, not a step (24, 24) down and right; "
          "the Board's pictures: %s" % (b["x"] - a["x"], b["y"] - a["y"], ", ".join(
              "%s at %d, %d" % (card["file"][:8], card["x"], card["y"])
              for card in json.load(open(sys.argv[1]))["cards"] if card.get("kind") == "picture")))
PYEOF
[ ! -s "$out/board-landed.why" ] || fail "$(cat "$out/board-landed.why")"
../tools/window-screenshot.sh "$pid" "$out/board-landed-shown.png" || fail "no window to capture"

# A Board's file over 4 MiB, which the host doesn't read, says so with a button to show the file,
# captured, and a pan doesn't write over it.
# Other Boards save as a hover card's picture shows them, so the file is what's compared.
large=$(printf 'c%.0s' $(seq 32))
mkdir -p "$boards/$large"
python3 -I - "$boards/$large/board.json" <<'PYEOF2'
import json, sys
cards = [{"id": "a", "kind": "note", "x": 80, "y": 140, "w": 240, "text": "A note"},
         {"id": "z", "kind": "note", "x": 90000, "y": 90000, "w": 200, "text": "x" * 4300000}]
json.dump({"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": cards}, open(sys.argv[1], "w"))
PYEOF2
before=$(cksum < "$boards/$large/board.json")
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$large"
send key return
expect "^browser: loaded [0-9]* chrome://board/$large\$" "the Board over 4 MiB didn't load"
send scroll 640 400 40 5
for _ in $(seq 30); do
  # The window a save would come in: the camera's rest (0.2 s) and the page's second after it.
  sleep 0.1
done
[ "$(cksum < "$boards/$large/board.json")" = "$before" ] ||
  fail "the Board's file over 4 MiB was written over: $(wc -c < "$boards/$large/board.json") bytes"
../tools/window-screenshot.sh "$pid" "$out/board-unread.png" || fail "no window to capture"
# A file that parses but isn't an object isn't a Board, and isn't written over either.
other=$(printf 'd%.0s' $(seq 32))
mkdir -p "$boards/$other"
printf '[]' > "$boards/$other/board.json"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$other"
send key return
expect "^browser: loaded [0-9]* chrome://board/$other\$" "the Board whose file holds [] didn't load"
send scroll 640 400 40 5
for _ in $(seq 30); do
  # The window a save would come in, as above.
  sleep 0.1
done
[ "$(cat "$boards/$other/board.json")" = '[]' ] ||
  fail "the Board whose file holds [] was written over: $(head -c 200 "$boards/$other/board.json")"

# 4: a private window has no New Board, and a Board's address there finds no Board.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
# A new window opens on its command bar, where ⌘L would close it.
expect '^browser: command bar shown$' "the private window didn't open on its command bar"
send type "new board"
# Its list for "new board" is there to read; a commands line is written only when a command matches.
expect '^browser: command bar lists "new board": ' "the private window's command bar listed nothing for \"new board\""
offered=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
  grep '^browser: command bar \(commands\|lists\) "new board": ' | grep 'New Board' | head -1)
[ -z "$offered" ] || fail "a private window's command bar offers New Board: $offered"
send key escape
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the private window's command bar"
send insert "chrome://board/$id"
send key return
expect "^browser: loaded [0-9]* chrome://board/$id\$" "the Board's address didn't load in the private window"
../tools/window-screenshot.sh "$pid" "$out/board-private.png" || fail "no window to capture"
private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)

# #3224: the Board holding [] offers its file in the system's file manager, its button with the
# keyboard, and that leaves the file as it was. Last, as the Finder and Explorer take the keyboard
# from the app.
send window 2
send key w cmd shift
expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
send window 1
send key return
# The core's line has Chromium's prefix.
expect 'browser: board shows its file$' \
  "↩ on the unreadable Board, whose Show button has the keyboard, didn't show its file"
[ "$(cat "$boards/$other/board.json")" = '[]' ] ||
  fail "showing the unreadable Board's file changed it: $(head -c 200 "$boards/$other/board.json")"
# #3453: the page the bar's ↩ loaded over the Board had the keyboard, so ⌘← there went to the page
# first, as on any page. Judged here, after the file manager, from what was heard then.
[ "$back_via" = page ] ||
  fail "the page the bar's ↩ loaded over a Board didn't have the keyboard: ⌘left ran back via\
 $back_via, the ↩ having left $went"
quit
echo "ok: New Board made $id, its name kept and its note saved in its folder alone, unlogged;" \
  "Library › Boards listed it; renamed, found by the bar, archived and reopened, the log said" \
  "only \"(a Board)\", and its next save kept the note; markup and a javascript: link ran" \
  "nothing; a waiting picture landed as a card and left the arrivals, and one that came while" \
  "its tab wasn't showing landed a step from it when shown; a Board over 4 MiB and one" \
  "holding [] weren't written over, and the one holding [] showed its file; a private window had" \
  "no New Board; the page loaded over a Board from the bar had the keyboard." \
  "Windows: $out/board.png $out/board-markup.png $out/board-landed.png" \
  "$out/board-landed-shown.png $out/board-unread.png $out/board-private.png"
