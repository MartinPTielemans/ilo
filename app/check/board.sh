# check.sh runs this at 690.
# A Board (#3073). New Board's key makes one in the Space's profile, a pinned tab whose row's name
# is there to type over; ↩ keeps the name in the Board's folder and gives the page the keyboard,
# where the first note's field takes the keys typed next. The page saves the note a second later,
# in its own Board's folder alone, and no log line says what the note or the name is. Library ›
# Boards lists it. Renamed while open, opened from the command bar by the start of its name,
# unpinned, archived and reopened, it's still only "(a Board)" in the log, never its name. A Board whose file holds markup in a note's text and a javascript: link runs
# neither. A Board's file over 4 MB opens, and a change to it isn't saved: the page says so in a
# note in its top corner, captured. A private window has no New Board, and a Board's address there
# shows "This Board isn't here.", captured. Japanese input as the first keys is the hosted Mac's.

log="$out/board.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>ready</title>' "$browser" \
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

# A Board's file over 4 MB opens, and a change isn't saved: the page says so in its corner.
large=$(printf 'c%.0s' $(seq 32))
mkdir -p "$boards/$large"
python3 -I - "$boards/$large/board.json" <<'PYEOF'
import json, sys
cards = [{"id": "a", "kind": "note", "x": 80, "y": 140, "w": 240, "text": "A note"},
         {"id": "z", "kind": "note", "x": 90000, "y": 90000, "w": 200, "text": "x" * 4300000}]
json.dump({"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": cards}, open(sys.argv[1], "w"))
PYEOF
before=$(wc -c < "$boards/$large/board.json")
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$large"
send key return
expect "^browser: loaded [0-9]* chrome://board/$large\$" "the Board over 4 MB didn't load"
# A pan is a change, which the page saves a second after the camera rests.
send scroll 640 400 40 5
saves=$(grep -c 'browser: board saved$' "$log")
for _ in $(seq 30); do
  # The window a save would come in: the camera's rest (0.2 s) and the page's second after it.
  sleep 0.1
done
[ "$(grep -c 'browser: board saved$' "$log")" = "$saves" ] || fail "the Board over 4 MB was saved"
[ "$(wc -c < "$boards/$large/board.json")" = "$before" ] || fail "the Board's file over 4 MB changed"
../tools/window-screenshot.sh "$pid" "$out/board-refused.png" || fail "no window to capture"

# 4: a private window has no New Board, and a Board's address there finds no Board.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the private window's command bar"
send type "new board"
expect '^browser: command bar commands "new board": ' "the private window's command bar found nothing for \"new board\""
! grep '^browser: command bar commands "new board": ' "$log" | tail -1 | grep -q ': New Board' ||
  fail "a private window's command bar offers New Board"
send key escape
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the private window's command bar"
send insert "chrome://board/$id"
send key return
expect "^browser: loaded [0-9]* chrome://board/$id\$" "the Board's address didn't load in the private window"
../tools/window-screenshot.sh "$pid" "$out/board-private.png" || fail "no window to capture"
quit
echo "ok: New Board made $id, its name kept and its note saved in its folder alone, unlogged;" \
  "Library › Boards listed it; renamed, found by the bar, archived and reopened, the log said" \
  "only \"(a Board)\"; markup and a javascript: link ran nothing; a Board over 4 MB" \
  "wasn't saved; a private window had no New Board. Windows: $out/board.png" \
  "$out/board-markup.png $out/board-refused.png $out/board-private.png"
