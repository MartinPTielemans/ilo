# check.sh runs this at 690.
# A Board (#3073). The empty Library › Boards list's New Board makes one in the Space's profile
# and closes the Library (#3575), a pinned tab whose row's name is there to type over; ↩ keeps
# the name in the Board's folder and gives the page the keyboard, where the first note's field
# takes the keys typed next. The page saves the note a second later, in
# its own Board's folder alone, and no log line says what the note or the name is. Library › Boards
# lists it. Renamed while open, opened from the command bar by the start of its name, unpinned,
# archived and reopened, it's still only "(a Board)" in the log, never its name, and the reopened
# page reads its file: a change keeps the note. Reloaded, and left for a web page, it leaves the
# browser running. A Board whose file holds markup in a note's text and a javascript: link runs
# neither. A picture waiting in a Board's arrivals lands as a card when the Board opens, and leaves
# the list once the card is saved; one that comes while its tab isn't showing lands when it's shown,
# a step from the first, both captured. A Board's file over 4 MiB, which the host doesn't read, says
# so and is never written over, captured, as is one holding [] rather than a Board. A Board loaded
# in a second tab says it's open in the other, captured, and saves nothing; once the first closes,
# the second takes it over and saves. A private window
# has no New Board, and a Board's address there shows "This Board isn't here.", captured. New Board
# from the File menu, the sidebar's +, the command bar, and its key with a Board's page and with
# the sidebar holding the keyboard, each makes one. Duplicate Board, from the bar at once after
# typing, copies the Board with its last word, and a Board's row's menu duplicates it too, each
# copy opening in a tab of its own, named as a copy. Japanese input as a Board's first keys is its
# note; ⌘C, ⌘V and ⌘Z in the page copy, paste and undo cards, and ⌘− zooms the canvas, not the
# page, each saved. Tab reaches a Board's cards in reading order, whatever order its file has them in, the
# arrows move the one the keyboard's on, Delete takes it away and the keyboard goes on to the next,
# and ⌘Z brings it back, each saved, captured. Relaunched, the Board comes back with its note. The Board holding [] then shows
# its file in the system's file manager from its note's button, last of the steps; after it, that
# the page the bar's ↩ loaded over a Board had the keyboard is judged.
# An agent gets nothing of a Board, wherever the user opens one: one New Board made, one opened from
# Library › Boards, and one whose address the user typed into the agent's own tab, taken over. Its
# list_tabs leaves each out, read_page, snapshot, screenshot and click on it are refused as on a tab
# not its own, each refusal in the activity log, and nothing logged names the Board.

log="$out/board.log"
# The first line of the log, past the driver's own echo of the keys it typed, that names a Board's
# name or note: one of the words in $1, as a word, with no letter on either side, so the app's own
# "finished" doesn't say "shed". Japanese has no letter beside it and matches wherever it is.
names_said() {
  grep -v '^browser: drive [0-9]*: ' "$log" | grep -E "(^|[^A-Za-z])($1)([^A-Za-z]|\$)" | head -1
}
step=0
rm -f "$saved" "$launch_frame"
# An MCP client's token, saved with Settings › Threads on, which the app reads as it starts. The
# file goes once the server is up, so a run that stops early leaves the next checks' agents off.
serve
agents_file="$profile_dir/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
# Showing the file on Linux, with no file manager on the bus, falls to xdg-open, which tries
# $BROWSER before the runner's own browsers: true ends it there, so none stays on the display.
opener=${BROWSER:-}
[ "$(uname)" != Linux ] || opener=true
BROWSER=$opener BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>ready</title>' "$browser" \
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

# New Board, from the empty Library › Boards list's button, clicked where it logged itself. It runs
# New Board as its key does, and the Library goes, so the name typed next is the row's (#3572).
bar_command view-boards "View Boards…" "view boards"
expect '^browser: library shown$' "View Boards… didn't open the Library on its empty Boards list"
expect '^browser: library new board at [0-9-]* [0-9-]* [0-9]* [0-9]*$' \
  "the empty Library › Boards list drew no New Board button"
../tools/window-screenshot.sh "$pid" "$out/board-library-empty.png" || fail "no window to capture"
set -- $(grep '^browser: library new board at ' "$log" | tail -1 | cut -d' ' -f6-9)
post click $(($1 + $3 / 2)) $(($2 + $4 / 2))
expect '^browser: command new-board via library$' \
  "a click on the empty Boards list's New Board didn't run New Board"
expect '^browser: new board [0-9a-f]\{32\} tab [0-9]*$' \
  "New Board made no Board: $(grep '^browser: new board ' "$log" | tail -1)"
made=$(grep '^browser: new board [0-9a-f]\{32\} tab ' "$log" | tail -1)
id=$(echo "$made" | cut -d' ' -f4)
board=$(echo "$made" | cut -d' ' -f6)
expect "^browser: name board $board\$" "the new Board's row didn't offer its name to type over"
expect '^browser: library hidden$' \
  "New Board from the empty Boards list left the Library over the Board's name"
folder=$(find "$profile_dir" -type d -path "*/Boards/$id" | head -1)
[ -n "$folder" ] || fail "New Board made no folder for $id in the profile ($profile_dir)"
boards=$(dirname "$folder")

# Waits for the Board $2's page in tab $1, named $3, to have loaded, failing on its last line.
board_loaded() {
  for _ in $(seq 100); do
    grep "^browser: page $1 " "$log" | tail -1 | grep -q " loading 0 chrome://board/$2\$" && return 0
    sleep 0.1
  done
  fail "$3 didn't finish loading: $(grep "^browser: page $1 " "$log" | tail -1)"
}
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
# The page has the keyboard before it has loaded, and keys typed then are lost. Once it has loaded,
# the keys typed while the Board opens are held for its first note (#3424), so they're typed
# straight away, as a user would.
board_loaded "$board" "$id" "the new Board's page"
send type "shelves over the sink"
expect 'browser: board saved$' "the note typed in the new Board wasn't saved"
grep -q 'shelves over the sink' "$folder/board.json" ||
  fail "the Board's file doesn't hold the note typed: $(head -c 300 "$folder/board.json")"
# 1a: the page wrote its own Board and no other.
[ "$(ls "$boards")" = "$id" ] || fail "the profile's Boards aren't only $id: $(ls "$boards" | tr '\n' ' ')"
# Nothing of what the user wrote is logged, past the driver's own echo of the keys it typed.
written=$(names_said 'shelves|kitchen')
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
# The page ↩ went to has the keyboard, which the UI had over the Board (#3453).
send state
in_browser RenderWidgetHostViewCocoa "after ↩ in ⌘T's bar over a Board"
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
written=$(names_said 'shelves|kitchen|pantry')
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
# and leave the browser running (#3427); back brings the Board again for the steps after. The page
# the bar's ↩ loaded over the Board has the keyboard, so ⌘← there goes to the page first, as on any
# page (#3453).
send key r cmd
expect "^browser: loaded $board chrome://board/$id\$" "⌘R didn't load the Board again"
serve
send key l cmd
expect '^browser: command bar shown$' "⌘L on the Board didn't open the command bar"
send type "${site}page.html"
send key return
expect "^browser: loaded $board ${site}page.html\$" "the Board's tab didn't load the page typed in the bar"
send state
in_browser RenderWidgetHostViewCocoa "after ↩ in ⌘L's bar over a Board"
send key left cmd
expect '^browser: command back via page$' \
  "the page the bar's ↩ loaded over a Board didn't have the keyboard: ⌘left didn't run back via page"
expect "^browser: loaded $board chrome://board/$id\$" "back didn't bring the Board again"

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
rpc=0
# Calls the tool $1 with the arguments $2 and prints its answer's text, after "error: " if it's an
# error, or a picture's type and size.
call() {
  rpc=$((rpc + 1))
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$rpc,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" >&4
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
    refused="^browser: agent logged mcp-client $tool [a-z]* refused: The tab isn't in the agent's own Space\.\$"
    entries=$(grep -c "$refused" "$log" || true)
    got=$(call "$tool" "$arguments")
    [ "$got" = "$not_its" ] || fail "the agent's $tool on the Board $2 answered: $got"
    await_more "$refused" "$entries" ||
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
# Library › Boards opens it again once its tab is archived, from Today, as ⌘W on a pinned tab keeps
# it: ↓ rings a row, ↩ opens its Board, and the one with a tab, pantry's, is only shown, so ↓ again
# rings the other.
send key d cmd
expect "^browser: today\( [0-9]*\)* $attic_tab\( [0-9]*\)*\$" "⌘D didn't move the second Board's tab into Today: Today is \"$(today)\""
send key w cmd
expect "^browser: tab $attic_tab closed\$" "⌘W didn't archive the second Board's tab"
for downs in 1 2; do
  bar_command view-boards "View Boards…" "view boards"
  expect '^browser: library shown$' "View Boards… didn't open the Library"
  # The list logs its count only when it changes, which it may have done while the Library was closed.
  listed=$(grep '^browser: library boards [0-9]*$' "$log" | tail -1)
  [ "$listed" = 'browser: library boards 2' ] || fail "Library › Boards didn't list the two Boards; its last count line: ${listed:-none}"
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
# The bar's Take Over is for the tab the agent is at work in, which it is once it reads it.
own=$(call read_page "{\"tab\":$agent_tab}")
case "$own" in "Tab $agent_tab: "*) ;; *) fail "reading the agent's own tab answered: $own" ;; esac
await "^browser: agent mcp-client at work in $agent_tab\$" || fail "the agent's read of its own tab didn't put it at work there"
send state
if [ "$active" = 1 ]; then
  await "^browser: task plate task-${session#session-} at " || fail "the agent's card wasn't drawn"
  read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
  send click $((px + pw / 2)) $((py + ph / 2))
  expect "^browser: task $session shown\$" "a click on the agent's card didn't show its conversation"
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
written=$(names_said 'shelves|kitchen|pantry|attic|cellar')
[ -z "$written" ] || fail "the log says what a Board holds or its name: $written"
exec 4>&- 5<&-
for _ in $(seq 100); do kill -0 "$bridge_pid" 2> /dev/null || break; sleep 0.1; done
rm -f "$agents_file" "$out/board-in" "$out/board-out"
# Back to the first Board, by its place in the sidebar, for the steps after these.
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$board" | cut -d: -f1)
[ -n "$n" ] || fail "the first Board's tab $board isn't in the sidebar: pinned \"$(pinned)\", Today \"$(today)\""
# From the task area, which ⌘N leaves first, the line has no Space after it.
send key "$n" cmd
expect "^browser: selected $board\( in .*\)\{0,1\}\$" "⌘$n didn't show the first Board's tab $board of \"$(today)\""

# 1b: New Board from each of its places, the File menu, the sidebar's + and the command bar, and
# its key with the page focused and with the sidebar holding the keyboard. Each makes a Board, a
# pinned tab whose name is there to type over, kept with ↩, which gives the page the keyboard.
# made <how>: the Board New Board just made, named $1, its tab in $tab and its folder in $made_folder.
made() {
  expect '^browser: new board [0-9a-f]\{32\} tab [0-9]*$' "New Board from $2 made no Board"
  line=$(grep '^browser: new board [0-9a-f]\{32\} tab ' "$log" | tail -1)
  made_id=$(echo "$line" | cut -d' ' -f4)
  tab=$(echo "$line" | cut -d' ' -f6)
  expect "^browser: name board $tab\$" "the Board New Board made from $2 didn't offer its name to type over"
  send type "$1"
  send key return
  expect "^browser: board $tab has the keyboard\$" "↩ on the name of the Board made from $2 didn't give its page the keyboard"
  made_folder="$boards/$made_id"
  for _ in $(seq 50); do
    [ "$(cat "$made_folder/name" 2> /dev/null)" = "$1" ] && break
    sleep 0.1
  done
  [ "$(cat "$made_folder/name" 2> /dev/null)" = "$1" ] ||
    fail "the name of the Board made from $2 wasn't kept in its folder: $(cat "$made_folder/name" 2> /dev/null)"
  board_loaded "$tab" "$made_id" "the page of the Board made from $2"
}
# The Board's tab $tab archived, from Today, as ⌘W keeps a pinned tab, so the sidebar's places stay
# few for the steps after.
archive() {
  send key d cmd
  expect "^browser: today\( [0-9]*\)* $tab\( [0-9]*\)*\$" "⌘D didn't move the Board's tab $tab, made from $1, into Today: Today is \"$(today)\""
  send key w cmd
  expect "^browser: tab $tab closed\$" "⌘W didn't archive the Board's tab $tab, made from $1"
}
# The cards in the Board $1's file, as "kind" begins each.
cards() {
  grep -o '"kind"' "$1/board.json" 2> /dev/null | wc -l | tr -d ' '
}
# Waits for a save of a Board after the count $1 of them, failing with $2.
saved_after() {
  for _ in $(seq 50); do
    [ "$(grep -c 'browser: board saved$' "$log")" -gt "$1" ] && return 0
    sleep 0.1
  done
  fail "$2: $(grep 'browser: board ' "$log" | tail -1)"
}

send menu "New Board"
expect '^browser: command new-board via ' "the File menu's New Board didn't run New Board"
made loft "the File menu"
loft=$made_folder
# Japanese input as the Board's first keys: it composes in the first note's field and the note
# keeps what was committed. The driver's input method is the Mac's and sway's; on Windows it's
# Microsoft's Japanese IME where the runner has it, as keys.sh finds, and on X11 the Linux driver
# types plain keys into a page. Where there's none, the first note is plain text.
ime="ime on"
if windows; then
  ime=
  powershell.exe -NoProfile -NonInteractive -Command "(Get-WinUserLanguageList).InputMethodTips" \
    2> /dev/null | grep -qi 03B5835F-F03C-411B-9CE2-AA23E1171E36 && ime="ime ja"
fi
[ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ] && ime=
saves=$(grep -c 'browser: board saved$' "$log")
if [ -n "$ime" ]; then
  send $ime
  send type nihongo
  send key space
  send key return
  send ime off
  note=日本語
else
  echo "skip: Japanese typed into a Board's note: the runner has no input method that reaches a page here; check/ime.sh covers one."
  send type "a loft note"
  note="a loft note"
fi
saved_after "$saves" "the Board's first note, $note, wasn't saved"
for _ in $(seq 20); do
  grep -q "$note" "$loft/board.json" && break
  sleep 0.1
done
grep -q "$note" "$loft/board.json" ||
  fail "the Board's note didn't keep $note: $(head -c 300 "$loft/board.json")"
# The page's own keys while it has the keyboard: escape leaves the note, ⌘A takes every card, ⌘C
# and ⌘V copy it to a second one, and ⌘Z takes the paste back, each saved. ⌘− and ⌘0 zoom the
# canvas, which the Board keeps, not the browser's page.
send key escape
send key a cmd
send key c cmd
saves=$(grep -c 'browser: board saved$' "$log")
send key v cmd
saved_after "$saves" "⌘V in the Board saved nothing"
for _ in $(seq 20); do
  [ "$(cards "$loft")" = 2 ] && break
  sleep 0.1
done
[ "$(cards "$loft")" = 2 ] || fail "⌘C and ⌘V in the Board didn't make a second card: $(cards "$loft") in its file"
saves=$(grep -c 'browser: board saved$' "$log")
send key z cmd
saved_after "$saves" "⌘Z in the Board saved nothing"
for _ in $(seq 20); do
  [ "$(cards "$loft")" = 1 ] && break
  sleep 0.1
done
[ "$(cards "$loft")" = 1 ] || fail "⌘Z in the Board didn't take the paste back: $(cards "$loft") cards in its file"
saves=$(grep -c 'browser: board saved$' "$log")
send key - cmd
saved_after "$saves" "⌘− in the Board saved nothing"
for _ in $(seq 20); do
  grep -q '"zoom": *0\.8[,}]' "$loft/board.json" && break
  sleep 0.1
done
grep -q '"zoom": *0\.8[,}]' "$loft/board.json" ||
  fail "⌘− in the Board didn't zoom its canvas out: $(head -c 200 "$loft/board.json")"
send key 0 cmd
grep -q "^browser: zoom $tab " "$log" && fail "⌘− or ⌘0 in the Board zoomed the browser's page: $(grep "^browser: zoom $tab " "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/board-first-note.png" || fail "no window to capture"
archive "the File menu"

# The sidebar's + offers New Board, and New Board's key from the Board's page makes another.
send layout
height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
# On Linux, a menu shown in the dispatch that hides the last one keeps the keyboard it took
# (#3572): the + pressed while a row's menu shows opens its own in that menu's place, then ↓ rings
# one of its rows. On the Mac and Windows a press outside a menu goes no further than closing it.
replaced=
if [ "$(uname)" = Linux ]; then
  replaced=", pressed while a row's menu showed,"
  post rightclick $(row 0)
  expect '^browser: menu shown ' "a right-click on the sidebar's first row didn't show its menu"
fi
post click 204 $((height - 25))
expect "^browser: menu shown .*$(cased 'New Board')" "the sidebar's +$replaced didn't offer New Board"
if [ -n "$replaced" ]; then
  expect '^browser: menu at .* closed by ' "the row's menu didn't close as the sidebar's + opened its own"
  expect '^browser: menu window at ' "the + menu's window didn't take the keyboard"
  send key down
  expect '^browser: menu highlights .* at ' "↓ in the + menu, opened in a row's menu's place, rang no row: it hasn't the keyboard"
fi
send choose "$(cased 'New Board')"
expect '^browser: command new-board via ' "the + menu's New Board didn't run New Board"
made porch "the sidebar's +"
porch=$tab
# Escape leaves the note's field for the page itself: in a text field on the Mac, ⌃⇧E is the text
# system's select to the end of the line, which the field keeps.
send key escape
send key e ctrl shift
expect '^browser: command new-board via ' "⌃⇧E with a Board's page focused didn't run New Board"
made shed "⌃⇧E with a Board's page focused"
archive "⌃⇧E with a Board's page focused"
# The shed opened over the porch's tab, which archiving it shows again (#3813). Its ⌘ digit then
# shows it from the top Today tab.
expect "^browser: selected $porch\( in .*\)\{0,1\}\$" \
  "archiving the Board made over the porch's didn't show the porch's tab $porch again"
tab=$porch
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$porch" | cut -d: -f1)
[ -n "$n" ] || fail "the Board's tab $porch isn't in the sidebar: pinned \"$(pinned)\", Today \"$(today)\""
top=$(today | cut -d' ' -f1)
[ -n "$top" ] && [ "$top" != "$porch" ] || fail "no Today tab but the porch's: Today is \"$(today)\""
m=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$top" | cut -d: -f1)
switch "$m cmd" "$top"
switch "$n cmd" "$porch"
archive "the sidebar's +"

# The command bar finds New Board, and New Board's key with the sidebar holding the keyboard runs
# it too.
bar_command new-board "New Board" "new board"
made barn "the command bar"
archive "the command bar"
send key f6
expect '^browser: sidebar keyed ' "F6 with the page focused didn't give the sidebar the keyboard"
send key e ctrl shift
expect '^browser: command new-board via ' "⌃⇧E with the sidebar holding the keyboard didn't run New Board"
made deck "⌃⇧E with the sidebar holding the keyboard"
archive "⌃⇧E with the sidebar holding the keyboard"

# Duplicate a Board (#3406). The page saves as it loses the keyboard, so the bar opened at once
# after typing finds the last word saved, and the copy has it. Over a Board, "dup" puts Duplicate
# Board first, as Duplicate Current Tab isn't offered there. The copy opens in a tab of its own,
# with its own id, and a Board's row's menu in the Library duplicates it too, closing the Library.
send menu "New Board"
expect '^browser: command new-board via ' "the File menu's New Board for the Board to duplicate didn't run it"
made garden "the File menu, for the Board to duplicate"
garden=$made_id
garden_tab=$tab
send type "plum cherry"
send key l cmd
expect '^browser: command bar shown$' "⌘L right after typing in the Board didn't open the command bar"
send type dup
expect '^browser: command bar commands "dup": Duplicate Board' "the command bar over a Board didn't put Duplicate Board first for dup"
send key return
expect '^browser: command duplicate-board via command bar$' "Duplicate Board didn't run from the command bar"
expect "^browser: board $garden duplicated as [0-9a-f]\{32\}\$" "Duplicate Board made no copy of the Board shown"
copy=$(grep "^browser: board $garden duplicated as " "$log" | tail -1 | cut -d' ' -f6)
grep -q cherry "$boards/$copy/board.json" ||
  fail "the copy hasn't the word typed just before the bar opened: $(head -c 300 "$boards/$copy/board.json")"
[ "$(cat "$boards/$copy/name")" = "garden copy" ] ||
  fail "the copy is named \"$(cat "$boards/$copy/name")\", not \"garden copy\""
expect "^browser: loaded [0-9]* chrome://board/$copy\$" "the Board's copy didn't open in a tab"
copy_tab=$(grep "^browser: loaded [0-9]* chrome://board/$copy\$" "$log" | tail -1 | cut -d' ' -f3)
# garden hid as its copy opened, which is when a Board takes its picture for the Library. Its one
# note has to be on it: a note's card keeps no height, as its words size it, and a picture drawn at
# that height has nothing on it (#3568).
inked() {
  python3 -I - "$1" << 'PY'
import struct, sys, zlib
data = open(sys.argv[1], "rb").read()
at, idat, width = 8, b"", 0
while at < len(data):
    size, kind = struct.unpack_from(">I4s", data, at)
    if kind == b"IHDR":
        width, _, depth, colour = struct.unpack_from(">IIBB", data, at + 8)
        if (depth, colour) != (8, 6):
            sys.exit("not the 8-bit RGBA a canvas writes: depth %d, colour type %d" % (depth, colour))
    elif kind == b"IDAT":
        idat += data[at + 8:at + 8 + size]
    at += size + 12
# A row filtered from nothing but clear pixels is all zeros past its filter byte, whichever filter.
rows, stride = zlib.decompress(idat), 1 + width * 4
sys.exit(0 if any(rows[i + 1:i + stride].strip(b"\0") for i in range(0, len(rows), stride)) else 1)
PY
}
picture="$boards/$garden/thumb.png"
for _ in $(seq 50); do
  [ -f "$picture" ] && inked "$picture" && break
  sleep 0.1
done
[ -f "$picture" ] && inked "$picture" ||
  fail "the Library's picture of garden, taken as its copy opened over it, has nothing drawn though garden has a note: $([ -f "$picture" ] && echo "$(wc -c < "$picture") bytes, all clear" || echo "no picture")"
bar_command view-boards "View Boards…" "view boards"
expect '^browser: library shown$' "View Boards… didn't open the Library to duplicate a Board from"
send key down
expect '^browser: library row keyed ' "↓ in Library › Boards rang no row to duplicate"
expect '^browser: library row ring at [0-9-]* [0-9-]* [0-9]* [0-9]*$' "the Library's ring on a Board's row wasn't drawn"
set -- $(grep '^browser: library row ring at ' "$log" | tail -1 | cut -d' ' -f6-9)
post rightclick $(($1 + $3 / 2)) $(($2 + $4 / 2))
expect "^browser: menu shown $(cased 'Open, Duplicate, Delete')\$" "a right-click on a Board's row didn't offer Open, Duplicate, then Delete"
send choose Duplicate
expect '^browser: board [0-9a-f]\{32\} duplicated as [0-9a-f]\{32\}$' "Duplicate in a Board's row's menu made no copy"
set -- $(grep '^browser: board [0-9a-f]\{32\} duplicated as ' "$log" | tail -1 | cut -d' ' -f3,6)
[ "$(cat "$boards/$1/board.json")" = "$(cat "$boards/$2/board.json")" ] ||
  fail "the row's copy $2 hasn't the file of the Board $1 it copied"
# Whichever row was first, the Board or its copy, the next copy is garden's second.
[ "$(cat "$boards/$2/name")" = "garden copy 2" ] ||
  fail "the row's copy is named \"$(cat "$boards/$2/name")\", not \"garden copy 2\""
expect '^browser: library hidden$' "Duplicate in a Board's row's menu didn't close the Library"
expect "^browser: loaded [0-9]* chrome://board/$2\$" "the row's copy didn't open in a tab"
tab=$(grep "^browser: loaded [0-9]* chrome://board/$2\$" "$log" | tail -1 | cut -d' ' -f3)
# The bar's "dup" over a Board and a Board's row's menu, in each tone, for the change's pictures.
tone() {
  case "$(grep '^browser: window light ' "$log" | tail -1)" in
    *true) echo light ;;
    *) echo dark ;;
  esac
}
was=$(tone)
for each in light dark; do
  if [ "$(tone)" != "$each" ]; then
    send menu "$(cased "Appearance › $([ $each = light ] && echo Light || echo Dark)")"
    expect "^browser: window light $([ $each = light ] && echo true || echo false)\$" \
      "View › Appearance didn't make the window $each for the Duplicate pictures"
  fi
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar for its $each picture"
  send type dup
  expect '^browser: command bar commands "dup": Duplicate Board' "the command bar didn't put Duplicate Board first for dup, $each"
  ../tools/window-screenshot.sh "$pid" "$out/board-duplicate-bar-$each.png" all || fail "no window to capture"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the command bar of the $each picture"
  bar_command view-boards "View Boards…" "view boards"
  expect '^browser: library shown$' "View Boards… didn't open the Library for its $each picture"
  send key down
  expect '^browser: library row ring at [0-9-]* [0-9-]* [0-9]* [0-9]*$' "the Library's ring on a Board's row wasn't drawn, $each"
  set -- $(grep '^browser: library row ring at ' "$log" | tail -1 | cut -d' ' -f6-9)
  post rightclick $(($1 + $3 / 2)) $(($2 + $4 / 2))
  expect "^browser: menu shown $(cased 'Open, Duplicate, Delete')\$" "a right-click on a Board's row didn't show its menu, $each"
  ../tools/window-screenshot.sh "$pid" "$out/board-duplicate-menu-$each.png" all || fail "no window to capture"
  expect '^browser: menu window at ' "the menu's window didn't take the keyboard"
  send key escape
  expect '^browser: menu closed' "escape didn't close a Board's row's menu, $each"
  send key escape
  expect '^browser: library hidden$' "escape didn't close the Library of the $each picture"
done
# The pictures end dark, so Automatic changes the look only where the system's is light.
send menu "$(cased 'Appearance › Automatic')"
expect '^browser: appearance automatic$' "View › Appearance › Automatic didn't run after the Duplicate pictures"
[ "$was" = dark ] ||
  expect '^browser: window light true$' \
    "View › Appearance › Automatic didn't give the window its light look back after the Duplicate pictures"
send key w cmd
expect "^browser: tab $tab closed\$" "⌘W didn't close the row's copy's tab"
for tab in "$copy_tab" "$garden_tab"; do
  n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$tab" | cut -d: -f1)
  [ -n "$n" ] || fail "the Board's tab $tab isn't in the sidebar: pinned \"$(pinned)\", Today \"$(today)\""
  # Closing a tab shows the one it opened over or the one under it, which may be this one already.
  [ "$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)" = "$tab" ] || switch "$n cmd" "$tab"
  if [ "$tab" = "$garden_tab" ]; then
    archive "the File menu, for the Board to duplicate"
  else
    send key w cmd
    expect "^browser: tab $tab closed\$" "⌘W didn't close the Board's copy's tab"
  fi
done
written=$(names_said '日本語|loft|porch|shed|barn|deck|garden|cherry')
[ -z "$written" ] || fail "the log says what a Board holds or its name: $written"
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
unread=$(grep "^browser: loaded [0-9]* chrome://board/$other\$" "$log" | tail -1 | cut -d' ' -f3)
send scroll 640 400 40 5
for _ in $(seq 30); do
  # The window a save would come in, as above.
  sleep 0.1
done
[ "$(cat "$boards/$other/board.json")" = '[]' ] ||
  fail "the Board whose file holds [] was written over: $(head -c 200 "$boards/$other/board.json")"
# Its tab closes, or the relaunch brings it back, and the bar after it opens the Board in a second
# tab, which says it's open in another and offers no file.
send key w cmd
expect "^browser: tab $unread closed\$" "⌘W didn't close the Board whose file holds []"

# A Board open in one tab and loaded in a second: the second says it's open in the other and
# writes nothing of it, and once the first closes it takes the Board over, as its page waits to.
twice=$(printf 'e%.0s' $(seq 32))
mkdir -p "$boards/$twice"
printf '%s' '{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": []}' > "$boards/$twice/board.json"
# Where the Board's file has its view, which a pan moves and a save keeps.
camera() {
  python3 -I -c 'import json, sys; c = json.load(open(sys.argv[1]))["camera"]; print(c["x"], c["y"])' \
    "$boards/$twice/board.json"
}
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "chrome://board/$twice"
send key return
expect "^browser: loaded [0-9]* chrome://board/$twice\$" "the Board to open in two tabs didn't load"
first=$(grep "^browser: loaded [0-9]* chrome://board/$twice\$" "$log" | tail -1 | cut -d' ' -f3)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar over the Board's tab"
# Over the Board's own tab the bar doesn't offer that tab, and ↩ on the address opens another.
send insert "chrome://board/$twice"
send key return
expect "^browser: loaded [0-9]* chrome://board/$twice\$" "the Board's address didn't load in a second tab"
second=$(grep "^browser: loaded [0-9]* chrome://board/$twice\$" "$log" | tail -1 | cut -d' ' -f3)
[ "$second" != "$first" ] || fail "the bar's address row went to the Board's tab $first instead of opening a second"
../tools/window-screenshot.sh "$pid" "$out/board-elsewhere.png" || fail "no window to capture"
send scroll 640 400 40 5
for _ in $(seq 30); do
  # The window a save would come in, as above.
  sleep 0.1
done
[ "$(camera)" = '0 0' ] || fail "a pan in the Board's second tab was saved to its file: its view is at $(camera)"
# A quick ⌃Tab goes back to the first tab, which ⌘W closes: the second's page loads the Board again,
# now its own, even unseen, and once shown a pan there is saved.
send key tab ctrl
tapped=$step
send flags
# The tab is selected as ⌃ goes down, before the step letting go of it.
expect "^browser: selected $first in " "a quick ⌃Tab didn't go back to the Board's first tab, $first" "$tapped"
send key w cmd
expect "^browser: tab $first closed\$" "⌘W didn't close the Board's first tab"
expect "^browser: loaded $second chrome://board/$twice\$" \
  "the Board's second tab didn't load it again once the first closed"
# Closing the first shows a tab it opened, the second, or the one under it (#3813); the second is
# shown by its key if it isn't already.
expect "^browser: selected [0-9][0-9]*\( in .*\)\{0,1\}\$" "closing the Board's first tab showed no tab"
n=$(echo $(pinned) $(today) | tr ' ' '\n' | grep -nx "$second" | cut -d: -f1)
[ -n "$n" ] || fail "the Board's second tab $second isn't in the sidebar: pinned \"$(pinned)\", Today \"$(today)\""
[ "$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)" = "$second" ] || switch "$n cmd" "$second"
saves=$(grep -c 'browser: board saved$' "$log" || true)
send scroll 640 400 40 5
saved_after "$saves" "a pan in the Board's second tab, once the first had closed, wasn't saved"
[ "$(camera)" != '0 0' ] || fail "the save from the Board's second tab, once the first had closed, kept the view where it was"
send key w cmd
expect "^browser: tab $second closed\$" "⌘W didn't close the Board's second tab"

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

# The private window closes before the relaunch, which brings back only the regular one.
send window 2
send key w cmd shift
expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
send window 1

# #3407: the keyboard reaches a Board's cards. Tab goes into them at the first in reading order, top
# to bottom then left to right, whatever order the file has them in, and on from card to card; the
# arrows move the card the keyboard's on, a point and with ⇧ ten; Delete takes it away and the
# keyboard goes on to the next card; ⌘Z brings it back. NVDA's words for them are screen-reader.sh's.
# An id of its own: the c×32 Board of the steps before is still open in a tab, where this would only say so.
walk=$(printf '7%.0s' $(seq 32))
mkdir -p "$boards/$walk"
printf '%s' '{"version": 1, "camera": {"x": 0, "y": 0, "zoom": 1}, "cards": [
  {"id": "d", "kind": "stroke", "x": 500, "y": 400, "w": 60, "h": 20, "z": 4, "points": [[0, 0], [60, 20]], "color": "ink", "width": 4},
  {"id": "b", "kind": "note", "x": 500, "y": 100, "w": 240, "h": 0, "z": 2, "text": "top right", "fill": "plain", "size": "m"},
  {"id": "c", "kind": "link", "x": 100, "y": 400, "w": 280, "h": 56, "z": 3, "url": "https://example.com/", "title": "lower link"},
  {"id": "a", "kind": "note", "x": 100, "y": 100, "w": 240, "h": 0, "z": 1, "text": "top left\nand more", "fill": "plain", "size": "m"}]}' \
  > "$boards/$walk/board.json"
# Each card's id and place in the file, in reading order: "a 100 100 b 500 100 …".
places() {
  python3 -I -c 'import json, sys
cards = json.load(open(sys.argv[1]))["cards"]
print(" ".join("%s %g %g" % (c["id"], c["x"], c["y"]) for c in sorted(cards, key=lambda c: (c["y"], c["x"]))))' \
    "$boards/$walk/board.json"
}
moved_to() {
  saved_after "$saves" "$2 wasn't saved"
  for _ in $(seq 50); do
    [ "$(places)" = "$1" ] && return 0
    sleep 0.1
  done
  fail "$2 left the cards at $(places), not $1"
}
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar for the Board to walk"
send insert "chrome://board/$walk"
send key return
expect "^browser: loaded [0-9]* chrome://board/$walk\$" "the Board to walk with the keyboard didn't load"
walked=$(grep "^browser: loaded [0-9]* chrome://board/$walk\$" "$log" | tail -1 | cut -d' ' -f3)
for _ in $(seq 100); do
  grep "^browser: page $walked " "$log" | tail -1 | grep -q " loading 0 chrome://board/$walk\$" && break
  sleep 0.1
done
send key tab
send key tab
saves=$(grep -c 'browser: board saved$' "$log" || true)
send key right
moved_to "a 100 100 b 501 100 c 100 400 d 500 400" "→ on the second card in reading order, after Tab twice,"
send key tab
send key tab
saves=$(grep -c 'browser: board saved$' "$log" || true)
send key down shift
moved_to "a 100 100 b 501 100 c 100 400 d 500 410" "⇧↓ on the last card, after Tab twice more,"
send key tab shift
saves=$(grep -c 'browser: board saved$' "$log" || true)
send key backspace
moved_to "a 100 100 b 501 100 d 500 410" "Delete on the link card, after ⇧Tab,"
saves=$(grep -c 'browser: board saved$' "$log" || true)
# The keyboard went on to the card after the one deleted, the ink.
send key left
moved_to "a 100 100 b 501 100 d 499 410" "← after Delete, on the card after the one deleted,"
saves=$(grep -c 'browser: board saved$' "$log" || true)
send key z cmd
moved_to "a 100 100 b 501 100 d 500 410" "⌘Z of the ←"
saves=$(grep -c 'browser: board saved$' "$log" || true)
send key z cmd
moved_to "a 100 100 b 501 100 c 100 400 d 500 410" "⌘Z of the Delete"
../tools/window-screenshot.sh "$pid" "$out/board-keyboard.png" || fail "no window to capture"
send key w cmd
expect "^browser: tab $walked closed\$" "⌘W didn't close the Board walked with the keyboard"

# 1b: a Board kept across a relaunch. Its tab comes back named, the bar finds it by the start of its
# name, and its page reads its file: a pan saves it with its note. The first note's is kept too.
exec 3>&-
quit
log="$out/board-restored.log"
step=0
# As at the first launch, $BROWSER ends xdg-open for the file shown last.
BROWSER=$opener BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar after the relaunch"
send type pant
expect '^browser: command bar puts tab [0-9]* first$' \
  "the command bar didn't put the Board's tab first after the relaunch: $(grep '^browser: command bar suggestions ' "$log" | tail -1)"
board=$(grep '^browser: command bar puts tab [0-9]* first$' "$log" | tail -1 | cut -d' ' -f6)
send key return
expect "^browser: command bar switch to tab $board\$" "↩ on the Board's row didn't switch to its tab after the relaunch"
expect "^browser: loaded $board chrome://board/$id\$" "the Board's tab didn't load its Board after the relaunch"
# None yet in the relaunch's log, where grep -c's status 1 would end the check under set -e.
saves=$(grep -c 'browser: board saved$' "$log" || true)
send scroll 640 400 40 5
saved_after "$saves" "a pan on the Board after the relaunch wasn't saved"
grep -q 'shelves over the sink' "$folder/board.json" ||
  fail "the Board's save after the relaunch lost its note: $(head -c 300 "$folder/board.json")"
grep -q "$note" "$loft/board.json" ||
  fail "the first note's Board lost it across the relaunch, \"$note\": $(head -c 300 "$loft/board.json")"
written=$(names_said 'shelves|kitchen|pantry|日本語|loft')
[ -z "$written" ] || fail "the relaunch's log says what a Board holds or its name: $written"
../tools/window-screenshot.sh "$pid" "$out/board-relaunched.png" || fail "no window to capture"
# #3224: the Board holding [] offers its file in the system's file manager, its button with the
# keyboard, and that leaves the file as it was. Last, after the relaunch, as the Finder and
# Explorer take the keyboard from the app. The bar opens the Board in a tab of its own.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar after the relaunch"
send insert "chrome://board/$other"
send key return
expect "^browser: loaded [0-9]* chrome://board/$other\$" "the Board whose file holds [] didn't load after the relaunch"
# The page offers the button, with the keyboard, once it has read the file, after the load the
# app logs and with no line of its own: a ↩ before then lands on the page and does nothing, so it
# goes again, three times at most, as profiles.sh's Tab and Space do. The core's line has
# Chromium's prefix.
shown=
for _ in 1 2 3; do
  send key return
  for _ in $(seq 30); do
    logged 'browser: board shows its file$' && shown=1 && break
    sleep 0.1
  done
  [ -z "$shown" ] || break
done
[ -n "$shown" ] ||
  fail "↩ on the unreadable Board, whose Show button has the keyboard, didn't show its file"
[ "$(cat "$boards/$other/board.json")" = '[]' ] ||
  fail "showing the unreadable Board's file changed it: $(head -c 200 "$boards/$other/board.json")"
quit
echo "ok: New Board made $id, its name kept and its note saved in its folder alone, unlogged;" \
  "Library › Boards listed it; renamed, found by the bar, archived and reopened, the log said" \
  "only \"(a Board)\", and its next save kept the note; New Board from the File menu, the" \
  "sidebar's +, the command bar and its key with the page and the bar focused; duplicated from" \
  "the bar with its last word and from its row's menu, each copy its own Board; Japanese kept as" \
  "the first note; ⌘C ⌘V and ⌘Z in the page saved; ⌘− the canvas's, not the page's;" \
  "Tab walked a Board's cards in reading order, the arrows moved them, Delete and ⌘Z saved," \
  "an agent got nothing of a Board made," \
  "opened from the Library or typed into its own tab, each refusal logged without a name;" \
  "markup and a javascript: link ran" \
  "nothing; a waiting picture landed as a card and left the arrivals, and one that came while" \
  "its tab wasn't showing landed a step from it when shown; a Board over 4 MiB and one" \
  "holding [] weren't written over; a Board's second tab saved nothing until its first closed," \
  "then took it over; a private window had no New Board; after a relaunch the" \
  "Board came back, found by its name, and its next save kept its note; the one holding [] showed" \
  "its file; the page loaded over a Board from the bar had the keyboard." \
  "Windows: $out/board.png $out/board-markup.png $out/board-landed.png" \
  "$out/board-landed-shown.png $out/board-unread.png $out/board-elsewhere.png $out/board-private.png" \
  "$out/board-keyboard.png" \
  "$([ -f "$out/board-agent-tab.png" ] && echo "$out/board-agent-tab.png")" \
  "$out/board-duplicate-bar-light.png $out/board-duplicate-bar-dark.png" \
  "$out/board-duplicate-menu-light.png $out/board-duplicate-menu-dark.png"
