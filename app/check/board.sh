# check.sh runs this at 690.
# A Board (#3073). New Board's key makes one in the Space's profile, a pinned tab whose row's name
# is there to type over; ↩ keeps the name in the Board's folder and gives the page the keyboard,
# where the first note's field takes the keys typed next. The page saves the note a second later, in
# its own Board's folder alone, and no log line says what the note or the name is. Library › Boards
# lists it. Renamed while open, opened from the command bar by the start of its name, unpinned,
# archived and reopened, it's still only "(a Board)" in the log, never its name, and the reopened
# page reads its file: a change keeps the note. A Board whose file holds markup in a note's text and
# a javascript: link runs neither. A picture waiting in a Board's arrivals lands as a card when the
# Board opens, and leaves the list once the card is saved; one that comes while its tab isn't
# showing lands when it's shown, a step from the first, both captured. A Board's file over 4 MiB,
# which the host doesn't read, says so and is never written over, captured, as is one holding []
# rather than a Board. A private window has no New Board, and a Board's address there shows "This
# Board isn't here.", captured. Japanese input as the first keys is the hosted Mac's.
# An agent gets nothing of a Board, wherever the user opens one: one New Board made, one opened from
# Library › Boards, and one whose address the user typed into the agent's own tab, taken over. Its
# list_tabs leaves each out, read_page, snapshot, screenshot and click on it are refused as on a tab
# not its own, each refusal in the activity log, and nothing logged names the Board.

log="$out/board.log"
step=0
rm -f "$saved" "$launch_frame"
# An MCP client's token, saved with Settings › Agents on, which the app reads as it starts. The file
# goes once the server is up, so a run that stops early leaves the next checks' agents off.
serve
agents_file="$profile_dir/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>ready</title>' "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved switch didn't start the agents' server"
rm -f "$agents_file"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi

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

# 1d: an agent gets nothing of a Board. The client is the bridge on two pipes, as mcp's and rules'.
rm -f "$out/board-in" "$out/board-out"
mkfifo "$out/board-in" "$out/board-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/board-in" "$out/board-out" \
    2>> "$out/board-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/board-in" > "$out/board-out" \
    2>> "$out/board-bridge.log" &
fi
bridge_pid=$!
exec 4> "$out/board-in" 5< "$out/board-out"
# The bridge's next line within 30 s, read by perl under its own alarm, as mcp's next_answer (#3286).
next_answer() {
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' 30 <&5
}
id=0
# Calls the tool $1 with the arguments $2 and prints its answer's text, after "error: " if it's an
# error, or a picture's type and size.
call() {
  id=$((id + 1))
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" >&4
  answer=$(next_answer) || fail "the browser didn't answer the agent's $1 $2"
  printf '%s\n' "$answer" | python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
content = result["content"][0]
if content["type"] == "image":
    print(content["mimeType"], len(content["data"]))
else:
    print(("error: " if result.get("isError") else "") + content["text"])' ||
    fail "the browser answered the agent's $1 $2 with: $answer"
}
# Waits for more lines than $2 matching $1.
await_more() {
  for _ in $(seq 300); do
    [ "$(grep -c "$1" "$log" || true)" -gt "$2" ] && return 0
    kill -0 "$pid" 2> /dev/null || fail "the app exited"
    sleep 0.1
  done
  return 1
}
printf '%s\n' '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' >&4
next_answer > "$out/board-initialize.json" || fail "the browser didn't answer the agent's initialize"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4
# The agent's own page, in its Space.
printf '%s' '<!doctype html><title>Agent page</title><p>A page of the check for the agent.</p>' \
  > "$out/site/board-agent.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/board-agent.html\"}")
agent_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$agent_tab" ] || fail "the agent's open_tab answered: $opened"
not_its='error: The tab isn'"'"'t in the agent'"'"'s own Space.'
# The agent gets nothing of the Board tab $1, $2 as the run says how it was opened: list_tabs
# leaves it out, and read_page, snapshot, screenshot and click on it are refused, each in the
# activity log, whose lines say only "board" of it.
gets_nothing() {
  listed=$(call list_tabs '{}')
  printf '%s\n' "$listed" | grep -q "^. $agent_tab  " ||
    [ "$1" = "$agent_tab" ] || fail "the agent's list_tabs lost its own tab: $listed"
  if printf '%s\n' "$listed" | grep -q "^. $1  \|chrome://board\|pantry\|attic\|cellar"; then
    fail "the agent's list_tabs listed the Board $2: $listed"
  fi
  for tool in read_page snapshot screenshot click; do
    case $tool in
      click) arguments="{\"tab\":$1,\"ref\":\"e1\"}" ;;
      *) arguments="{\"tab\":$1}" ;;
    esac
    entries=$(grep -c "^browser: agent logged mcp-client $tool " "$log" || true)
    got=$(call "$tool" "$arguments")
    [ "$got" = "$not_its" ] || fail "the agent's $tool on the Board $2 answered: $got"
    await_more "^browser: agent logged mcp-client $tool [a-z]* refused: The tab isn't in the agent's own Space\.\$" "$entries" ||
      fail "the agent's $tool on the Board $2 isn't refused in the activity log: $(grep "^browser: agent logged mcp-client $tool " "$log" | tail -1)"
  done
}

# New Board makes its Board in the Space the window shows, the user's: a window never shows an
# agent's Space as its own (#1080).
send key e ctrl shift
expect '^browser: new board [0-9a-f]\{32\} tab [0-9]*$' "⌃⇧E made no second Board"
attic=$(grep '^browser: new board [0-9a-f]\{32\} tab ' "$log" | tail -1)
attic_id=$(echo "$attic" | cut -d' ' -f4)
attic_tab=$(echo "$attic" | cut -d' ' -f6)
expect "^browser: name board $attic_tab\$" "the second Board's row didn't offer its name to type over"
send type "attic"
send key return
expect "^browser: board $attic_tab has the keyboard\$" "↩ on the second Board's name didn't give its page the keyboard"
gets_nothing "$attic_tab" "New Board made"
# Library › Boards opens it again once its tab is archived: ↓ rings a row, ↩ opens its Board, and
# the one with a tab, pantry's, is only shown, so ↓ again rings the other.
send key w cmd
expect "^browser: tab $attic_tab closed\$" "⌘W didn't archive the second Board's tab"
for downs in 1 2; do
  bar_command view-boards "View Boards…" "view boards"
  expect '^browser: library boards 2$' "Library › Boards didn't list the two Boards"
  for _ in $(seq "$downs"); do
    send key down
    expect '^browser: library row keyed ' "↓ in Library › Boards rang no row"
  done
  send key return
  expect '^browser: library board [0-9a-f]\{32\} \(shown\|opened\)$' "↩ on a Board's row in Library › Boards did nothing"
  grep -q "^browser: library board $attic_id opened\$" "$log" && break
done
expect "^browser: loaded [0-9]* chrome://board/$attic_id\$" "Library › Boards didn't open the archived Board"
attic_tab=$(grep "^browser: loaded [0-9]* chrome://board/$attic_id\$" "$log" | tail -1 | cut -d' ' -f3)
gets_nothing "$attic_tab" "opened from Library › Boards"

# The user takes the agent's tab over from its page, watched in the conversation, and types a
# Board's address into it: the tab, in the agent's Space and handed back, still gives the agent
# nothing. The Board is one with no tab, open or archived, so the bar loads it rather than going to
# a tab of it.
cellar=$(printf 'c%.0s' $(seq 32))
mkdir -p "$boards/$cellar"
printf '%s' cellar > "$boards/$cellar/name"
printf '%s' '{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": []}' > "$boards/$cellar/board.json"
session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
[ -n "$session" ] || fail "the agent's open_tab started no card: $(grep '^browser: session ' "$log" | tail -1)"
send state
if [ "$active" = 1 ]; then
  await "^browser: task plate task-${session#session-} at " || fail "the agent's card wasn't drawn"
  read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
  send click $((px + pw / 2)) $((py + ph / 2))
  expect "^browser: task $session shown\$" "a click on the agent's card didn't show its conversation"
  await '^browser: task pages at ' || fail "the conversation's header drew no Pages"
  read -r gx gy gw gh <<CHIP
$(grep '^browser: task pages at ' "$log" | tail -1 | cut -d' ' -f5-)
CHIP
  send click $((gx + gw / 2)) $((gy + gh / 2))
  expect "^browser: task $session shows pages\$" "a click on Pages didn't show the grid"
  await "^browser: task picture $agent_tab at " || fail "the conversation didn't picture the agent's tab $agent_tab"
  read -r wx wy ww wh <<PICTURE
$(grep "^browser: task picture $agent_tab at " "$log" | tail -1 | cut -d' ' -f6-)
PICTURE
  send click $((wx + ww / 2)) $((wy + wh / 2))
  expect "^browser: task page $agent_tab in place\$" "a click on the agent's tab's picture didn't watch it"
  send key return cmd
  expect "^browser: agent mcp-client taken over in $agent_tab\$" "⌘↩ on the agent's page, watched, didn't take it over"
  go_from_bar "chrome://board/$cellar"
  expect "^browser: loaded $agent_tab chrome://board/$cellar\$" \
    "a Board's address typed in the agent's tab didn't load there: $(grep '^browser: loaded ' "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/board-agent-tab.png" || fail "no window to capture"
  send key return cmd
  expect "^browser: agent mcp-client handed back $agent_tab\$" "⌘↩ didn't hand the agent's tab back"
  gets_nothing "$agent_tab" "typed into the agent's own tab"
else
  echo "skip: board: a Board typed into the agent's own tab. Watching it takes clicks, and Browser wasn't the active app."
fi
written=$(grep -v '^browser: drive [0-9]*: ' "$log" | grep 'shelves\|kitchen\|pantry\|attic\|cellar' | head -1)
[ -z "$written" ] || fail "the log says what a Board holds or its name: $written"
exec 4>&- 5<&-
for _ in $(seq 100); do kill -0 "$bridge_pid" 2> /dev/null || break; sleep 0.1; done
rm -f "$agents_file" "$out/board-in" "$out/board-out"
# Back to the first Board, by its place in the sidebar, for the steps after these.
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$board" | cut -d: -f1)
[ -n "$n" ] || fail "the first Board's tab $board isn't in the sidebar: pinned \"$(pinned)\", Today \"$(today)\""
switch "$n cmd" "$board"

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

# A Board's file over 4 MiB, which the host doesn't read, says so, and a pan doesn't write over it.
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
quit
echo "ok: New Board made $id, its name kept and its note saved in its folder alone, unlogged;" \
  "Library › Boards listed it; renamed, found by the bar, archived and reopened, the log said" \
  "only \"(a Board)\", and its next save kept the note; an agent got nothing of a Board made," \
  "opened from the Library or typed into its own tab, each refusal logged without a name;" \
  "markup and a javascript: link ran" \
  "nothing; a waiting picture landed as a card and left the arrivals, and one that came while" \
  "its tab wasn't showing landed a step from it when shown; a Board over 4 MiB and one" \
  "holding [] weren't written over; a private window had no New Board." \
  "Windows: $out/board.png $out/board-markup.png $out/board-landed.png" \
  "$out/board-landed-shown.png $out/board-unread.png $out/board-private.png" \
  "$([ -f "$out/board-agent-tab.png" ] && echo "$out/board-agent-tab.png")"
