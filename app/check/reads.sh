# check.sh runs this at 385.
# Saved reads (#3822): an agent connected over MCP proposes a read of its own tab's site, the
# user's card shows what the read gave, and Save Read keeps it, which the agent then uses with no
# tab, one use of a read at a time. The pages are the check's own server's, under /reads/ on
# reads.localhost, and nothing of the owner's is read.
# The card shows the read's name, its column heads and the first rows the browser read, a cell
# with a line break, a right-to-left override, markup and 300 characters on one line that moves
# nothing after it. A read of another site, or of the same one over another scheme, is refused
# with no card and no request to that site. Don't Save declines it, and the next proposal asks
# again. Saved, it's in Settings › Agents' Saved reads, and the profile's files hold its definition
# and none of the rows it gave. A use reads only its address, its values checked before anything
# is sent; a page that changed is broken with the reason, a site that wants a sign-in, or that
# sends the read to its sign-in page or another site, needs a sign-in, and the other site is never
# asked. The user's sign-in in a tab, which the driver loads, makes it read again. A site on Always
# ask first pauses its reads and refuses new ones until it's off the list. Remove Read removes it
# with no dialog, ⌘Z brings it back, and a use after the second removal finds none. The same name
# with other columns asks again. The card keeps its
# buttons in a short window and over the page in full screen. Two uses of one read sent together
# reach the site one after the other, and a minute idle with a saved read asks the site nothing.
# Not here, but in the lib's tests: another profile's agent lists none, a private window keeps
# none, and an unanswered card is declined after the questions' wait.

log="$out/reads.log"
step=0
rm -f "$saved" "$launch_frame"
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
serve
reads="$out/site/reads"
mkdir -p "$reads"
# The table: a cell spanning two rows, filled into both, and a header with a footnote mark, which
# the read leaves out of its column's name.
reads_table() {
  printf '%s' "<!doctype html><meta charset=utf-8><title>Buildings</title><table>
<thead><tr><th>$1</th><th>City</th><th>Height (m) [1]</th></tr></thead><tbody>
<tr><td>Tower One</td><td rowspan=2>Lisbon</td><td>120</td></tr>
<tr><td>Tower Two</td><td>98</td></tr>
<tr><td>$2</td><td>Porto</td><td>75</td></tr>
<tr><td>Tower Four</td><td>Braga</td><td>60</td></tr>
<tr><td>Tower Five</td><td>Faro</td><td>44</td></tr>
</tbody></table>"
}
reads_table Name 'Tower Three' > "$reads/table.html"
# A cell a site wrote to move what follows it: a line break, a right-to-left override, markup and
# 300 characters.
long=$(printf 'x%.0s' $(seq 300))
reads_table Name "Tower<br>Three &#x202e;evil &lt;b&gt;bold&lt;/b&gt; $long" > "$reads/hostile.html"
printf '%s' "$(reads_table Name One)$(reads_table Name Two)" > "$reads/twice.html"
for id in A-1041 A-7; do
  printf '%s' '{"items":[{"name":"Bolt","size":4},{"name":"Nut","size":2},{"name":"Washer","size":1}]}' \
    > "$reads/items-$id.json"
done
printf '%s' '{"items":[{"name":"Ledger","size":3}]}' > "$reads/private.json"
printf '%s' '{"items":[{"name":"Slow","size":1}]}' > "$reads/slow.json"
printf '%s' '<!doctype html><title>Sign in</title><p>Sign in</p>' > "$reads/login"
# The sign-in the user makes in a tab, which the driver loads: the cookie private.json wants.
printf '%s' '<!doctype html><title>reads signing</title>
<script>document.cookie = "reads=1; path=/; max-age=3600"; document.title = "reads signed in"</script>' \
  > "$reads/signin.html"
printf '%s' '<!doctype html><title>reads home</title><p>The user page</p>' > "$reads/home.html"
origin="reads.localhost:$port"
host_rx="reads\.localhost:$port"

BROWSER_DRIVE=1 BROWSER_URL="http://$origin/reads/home.html" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
user_tab=$(today | cut -d' ' -f1)
await "^browser: title $user_tab reads home\$" || fail "the user's page didn't load"
await '^browser: agents server on ' || fail "the saved switch didn't start the agents' server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi
[ -x "$bridge" ] || fail "the app has no bridge beside its executable"

# The client, as mcp.sh's: the bridge on two pipes, a line of its answer read under perl's alarm.
# Its helpers have names of their own, as every run is sourced into one shell.
rm -f "$out/reads-in" "$out/reads-out"
mkfifo "$out/reads-in" "$out/reads-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/reads-in" "$out/reads-out" \
    2>> "$out/reads-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/reads-in" > "$out/reads-out" \
    2>> "$out/reads-bridge.log" &
fi
exec 4> "$out/reads-in" 5< "$out/reads-out"
reads_answer() {
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' "${1:-30}" <&5
}
reads_id=0
reads_send() {
  reads_id=$((reads_id + 1))
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$reads_id,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" >&4
}
# The text of the answer on stdin, after "error: " if it's an error.
reads_text() {
  raw=$(cat)
  printf '%s\n' "$raw" | python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
print(("error: " if result.get("isError") else "") + result["content"][0]["text"])' 2> /dev/null ||
    fail "the browser answered $1 with: $raw"
}
# Calls the tool $1 with the arguments $2 and prints its answer's text.
reads_call() {
  reads_send "$1" "$2"
  answer=$(reads_answer) || fail "the browser didn't answer $1 $2"
  printf '%s\n' "$answer" | reads_text "$1"
}
printf '%s\n' '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' >&4
reads_answer > "$out/reads-initialize.json" || fail "the browser didn't answer initialize"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4
printf '%s\n' '{"jsonrpc":"2.0","id":100,"method":"tools/list"}' >&4
reads_answer > "$out/reads-tools.json" || fail "the browser didn't list its tools"
for tool in propose_read use_read list_reads; do
  grep -q "\"name\":\"$tool\"" "$out/reads-tools.json" ||
    fail "tools/list didn't list $tool: $(grep -o '"name":"[^"]*"' "$out/reads-tools.json" | tr '\n' ' ')"
done

# What the check's server was asked of a host since the line count $2 of site.log, one path a line.
hits_since() {
  tail -n +"$(($2 + 1))" "$out/site.log" | sed -n "s|^GET [a-z]* $1\(/[^ ]*\) cookie .*|\1|p"
}
hits_at() { wc -l < "$out/site.log" | tr -d ' '; }

opened=$(reads_call open_tab "{\"url\":\"$origin/reads/table.html\"}")
tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$tab" ] || fail "open_tab of the table answered: $opened"
columns='[{"name":"Name","type":"text","from":"Name"},{"name":"City","type":"text","from":"City"},{"name":"Height","type":"number","from":"Height (m)"}]'
table_read() {
  printf '%s' "{\"tab\":$tab,\"name\":\"${2:-city buildings}\",\"kind\":\"table\",\"url\":\"/reads/$1\",\"columns\":${3:-$columns}}"
}
asks="^browser: agent mcp-client asks [0-9]*: save a read of $host_rx\$"
# Sends propose_read $1, waits for its card, captured as $2.png, and sets q to its question and
# qx qy qw qh to where the card is.
proposed() {
  before=$(grep -c "$asks" "$log" || true)
  reads_send propose_read "$1"
  for _ in $(seq 300); do
    [ "$(grep -c "$asks" "$log")" -gt "$before" ] && break
    kill -0 "$pid" 2> /dev/null || fail "the app exited"
    sleep 0.1
  done
  [ "$(grep -c "$asks" "$log")" -gt "$before" ] ||
    fail "propose_read $1 showed no card; the log's last lines: $(grep '^browser: agent' "$log" | tail -3 | tr '\n' ';')"
  q=$(grep "$asks" "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
  await "^browser: agent question $q at " || fail "the card of question $q wasn't drawn"
  read -r qx qy qw qh << CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
  ../tools/window-screenshot.sh "$pid" "$out/$2.png" || fail "no window to capture"
}
# Answers the card of q $1, Once for Save Read at the right of its last row or Dismiss for Don't
# Save at the left, and sets said to the proposal's answer.
answer_card() {
  case "$1" in Once) x=$((qw * 3 / 4)) ;; Dismiss) x=$((qw / 4)) ;; esac
  send click $((qx + x)) $((qy + qh - 26))
  await "^browser: agent mcp-client answered $q $1\$" || fail "a click on the card didn't answer $1"
  said=$(reads_answer) || fail "the browser didn't answer the proposal"
  said=$(printf '%s\n' "$said" | reads_text propose_read)
}

# 1 and a: the card of the plain table and of the hostile cell's are the same height: the cell is
# one line, cut where its column ends, and moves nothing after it.
proposed "$(table_read table.html)" reads-card
plain_h=$qh
plain_bottom=$((qy + qh))
answer_card Dismiss
hostile_tab=$(reads_call open_tab "{\"url\":\"$origin/reads/hostile.html\"}" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$hostile_tab" ] || fail "open_tab of the hostile table opened nothing"
was_tab=$tab
tab=$hostile_tab
proposed "$(table_read hostile.html hostile)" reads-card-hostile
[ "$qh" = "$plain_h" ] ||
  fail "a cell with a line break, markup and 300 characters made the card $qh high, not the plain table's $plain_h: the cell isn't one clipped line (reads-card-hostile.png)"
answer_card Dismiss
reads_call close_tab "{\"tab\":$hostile_tab}" > /dev/null
tab=$was_tab

# 2: a read of another site, and of this one over https, is refused with no card, and the other
# site is never asked.
away_from=$(hits_at)
for url in "http://away.localhost:$port/reads/items-A-7.json" "https://$origin/reads/table.html"; do
  before=$(grep -c "$asks" "$log" || true)
  refused=$(reads_call propose_read "$(table_read table.html | sed "s|\"/reads/table.html\"|\"$url\"|")")
  case "$refused" in
    "error: url must be a path on the tab's site ($origin), "*) ;;
    *) fail "propose_read of $url answered: $refused" ;;
  esac
  [ "$(grep -c "$asks" "$log")" = "$before" ] || fail "propose_read of $url showed a card"
done
[ -z "$(hits_since "away\.localhost:$port" "$away_from")" ] ||
  fail "a refused read asked the other site: $(hits_since "away\.localhost:$port" "$away_from" | tr '\n' ' ')"

# 3: Don't Save declines, with its log row, and a proposal straight after asks again.
proposed "$(table_read table.html)" reads-card-again
answer_card Dismiss
case "$said" in
  "error: declined: the user didn't save \"city buildings\". "*) ;;
  *) fail "Don't Save answered: $said" ;;
esac
await "^browser: agent logged mcp-client decline_read $host_rx\$" || fail "Don't Save wasn't logged"
listed=$(reads_call list_reads '{}')
case "$listed" in *'"reads":[]'*) ;; *) fail "after Don't Save, list_reads answered: $listed" ;; esac

# 4: Save Read keeps it: its log row, and the profile's files hold its definition and none of its
# rows. "Tower Five" is a cell of its page alone.
proposed "$(table_read table.html)" reads-card-save
answer_card Once
case "$said" in
  "saved: the user saved \"city buildings\". Call use_read with its name"*) ;;
  *) fail "Save Read answered: $said" ;;
esac
await "^browser: agent logged mcp-client save_read $host_rx\$" || fail "Save Read wasn't logged"
kept=$(find "$profile_dir" -name 'Saved reads' | head -1)
[ -n "$kept" ] || fail "Save Read wrote no Saved reads file in the profile"
grep -q 'city buildings' "$kept" || fail "the Saved reads file doesn't hold the read: $(head -c 300 "$kept")"
grep -q 'Tower Five' "$kept" && fail "the Saved reads file holds a row the read gave"

# 5, b: with the agent's tabs closed, a use reads the table's address and nothing else, logged as a
# use, on the agent's card too.
reads_call close_tab "{\"tab\":$tab}" > /dev/null
from=$(hits_at)
used=$(reads_call use_read '{"name":"city buildings"}')
case "$used" in
  *'"Tower Five"'*) ;;
  *) fail "use_read with no tab answered: $used" ;;
esac
case "$used" in *'"Lisbon"'*'"Lisbon"'*) ;; *) fail "the cell spanning two rows wasn't in both: $used" ;; esac
[ "$(hits_since "$host_rx" "$from" | tr '\n' ' ')" = "/reads/table.html " ] ||
  fail "use_read asked the site for more than its address: $(hits_since "$host_rx" "$from" | tr '\n' ' ')"
await "^browser: agent logged mcp-client use_read $host_rx\$" || fail "the use wasn't logged"
await '^browser: session ' || fail "the use started no card of the agent's"

# 6: a read with a parameter reads with a value that fits it, and one that doesn't, or a parameter
# it hasn't, is refused before anything is sent.
item='{"name":"Name","type":"text","from":"name"}'
tab=$(reads_call open_tab "{\"url\":\"$origin/reads/home.html\"}" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$tab" ] || fail "open_tab of the home page opened nothing"
proposed "{\"tab\":$tab,\"name\":\"items\",\"kind\":\"json\",\"url\":\"/reads/items-{id}.json\",\"rows\":\"items\",\"params\":{\"id\":{\"type\":\"text\",\"pattern\":\"A-[0-9]{1,6}\"}},\"sample\":{\"id\":\"A-7\"},\"columns\":[$item]}" reads-card-items
answer_card Once
case "$said" in "saved: "*) ;; *) fail "saving the items read answered: $said" ;; esac
from=$(hits_at)
used=$(reads_call use_read '{"name":"items","params":{"id":"A-1041"}}')
case "$used" in *'"Washer"'*) ;; *) fail "use_read items A-1041 answered: $used" ;; esac
for params in '{"id":"../../away"}' '{"id":"..%2F..%2Faway"}' '{"id":"A-1","other":"1"}'; do
  refused=$(reads_call use_read "{\"name\":\"items\",\"params\":$params}")
  case "$refused" in "error: "*) ;; *) fail "use_read items $params answered: $refused" ;; esac
done
[ "$(hits_since "$host_rx" "$from" | tr '\n' ' ')" = "/reads/items-A-1041.json " ] ||
  fail "a refused value reached the site: $(hits_since "$host_rx" "$from" | tr '\n' ' ')"

# 7: a page that changed is broken, with the reason, and gives no rows; so is one with two tables
# of the columns.
reads_table Building 'Tower Three' > "$reads/table.html"
broken=$(reads_call use_read '{"name":"city buildings"}')
case "$broken" in
  "error: broken: $origin didn't match the saved read. "*) ;;
  *) fail "a use after the page changed answered: $broken" ;;
esac
case "$broken" in *Tower*) fail "the broken read gave rows: $broken" ;; esac
await "^browser: agent logged mcp-client read_broken $host_rx\$" || fail "the broken read wasn't logged"
cp "$reads/twice.html" "$reads/table.html"
broken=$(reads_call use_read '{"name":"city buildings"}')
case "$broken" in "error: broken: $origin didn't match the saved read. "*) ;; *) fail "a page with two tables answered: $broken" ;; esac
reads_table Name 'Tower Three' > "$reads/table.html"

# 8, d: a site that wants a sign-in needs one, until the user signs in in a tab, which the driver
# loads; a read the site sends to its sign-in page needs one, and so does one it sends to another
# site, which is never asked and never named to the agent.
data_read() {
  printf '%s' "{\"tab\":$tab,\"name\":\"$1\",\"kind\":\"json\",\"url\":\"/reads/$2\",\"rows\":\"items\",\"sign_in\":\"/reads/login\",\"columns\":[$item]}"
}
signed_in() {
  case "$1" in "error: needs_sign_in: "*) ;; *) fail "$2 answered: $1" ;; esac
  case "$1" in *away.localhost*) fail "$2 named the site it was sent to: $1" ;; esac
}
# A read at the ask that needs a sign-in shows no card: the agent hears so at once.
from=$(hits_at)
for path in private.json gone.json away.json; do
  before=$(grep -c "$asks" "$log" || true)
  said=$(reads_call propose_read "$(data_read "${path%.json}" "$path")")
  signed_in "$said" "proposing a read of $path"
  [ "$(grep -c "$asks" "$log")" = "$before" ] || fail "proposing a read of $path showed a card"
done
[ -z "$(hits_since "away\.localhost:$port" "$from")" ] ||
  fail "a read the site sent to another asked it: $(hits_since "away\.localhost:$port" "$from" | tr '\n' ' ')"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type "$origin/reads/signin.html"
send key return
await "^browser: title [0-9]* reads signed in\$" || fail "the sign-in page didn't load in the user's tab"
proposed "$(data_read ledger private.json)" reads-card-signed-in
answer_card Once
case "$said" in "saved: "*) ;; *) fail "after the sign-in, saving the read answered: $said" ;; esac
used=$(reads_call use_read '{"name":"ledger"}')
case "$used" in *'"Ledger"'*) ;; *) fail "after the sign-in, use_read answered: $used" ;; esac

# Settings › Agents, as mcp.sh opens it.
agents_settings() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  send clickkey $(drawn "toolbar Agents")
  expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  settings_drew Agents
}
agents_settings
in_sight "saved read 0" || fail "Settings › Agents listed no saved read"
../tools/window-screenshot.sh "$pid" "$out/reads-settings.png" || fail "no window to capture"

# 9: Always ask first pauses the site's reads and refuses a new one, until the site is off it.
click_in_sight "fence open"
expect '^browser: settings fence field open$' "Add Site… didn't open Always ask first's field"
send type reads.localhost
send key return
await '^browser: agents fenced reads\.localhost$' || fail "Return in Always ask first's field didn't add the site"
../tools/window-screenshot.sh "$pid" "$out/reads-settings-paused.png" || fail "no window to capture"
paused=$(reads_call use_read '{"name":"city buildings"}')
case "$paused" in
  "error: paused: $origin is on Always ask first, so saved reads for it are stopped.") ;;
  *) fail "a use of a read of a site on Always ask first answered: $paused" ;;
esac
await "^browser: agent logged mcp-client read_paused $host_rx\$" || fail "the paused use wasn't logged"
refused=$(reads_call propose_read "$(table_read table.html other)")
case "$refused" in "error: not_allowed: $origin is on Always ask first"*) ;; *) fail "a proposal on a listed site answered: $refused" ;; esac
click_in_sight "unfence reads.localhost"
expect '^browser: agents unfenced reads\.localhost$' "Remove didn't take the site off Always ask first"
used=$(reads_call use_read '{"name":"city buildings"}')
case "$used" in *'"Tower Five"'*) ;; *) fail "off the list, use_read answered: $used" ;; esac

# 10: Remove Read removes with no dialog, ⌘Z brings it back, and after a second removal a use
# finds none.
# The names list_reads gives, sorted, one a line.
reads_names() {
  reads_call list_reads '{}' |
    python3 -I -c 'import json, sys; print("\n".join(sorted(r["name"] for r in json.load(sys.stdin)["reads"])))'
}
remove_read() {
  click_in_sight "saved read 0"
  expect '^browser: settings saved read 0 opened true$' "a click on the first saved read didn't open it"
  click_in_sight "saved read 0 remove"
  expect '^browser: saved read removed in profile [0-9]*$' "Remove Read didn't remove the read"
}
all=$(reads_names)
remove_read
first=$(printf '%s\n' "$all" | grep -vxF "$(reads_names)" | head -1)
[ -n "$first" ] || fail "after Remove Read, list_reads still lists every read: $(reads_names | tr '\n' ' ')"
grep -q '^browser: page [0-9]* dialog [^n]' "$log" && fail "Remove Read asked first"
send key z cmd
expect '^browser: settings undo brought a saved read back$' "⌘Z didn't bring the saved read back"
await '^browser: saved read restored in profile [0-9]*$' || fail "⌘Z didn't restore the read"
[ "$(reads_names)" = "$all" ] || fail "after ⌘Z, list_reads gave $(reads_names | tr '\n' ' '), not $(printf '%s\n' "$all" | tr '\n' ' ')"
remove_read
await "^browser: agent logged mcp-client remove_read $host_rx\$" || fail "Remove Read wasn't logged"
gone=$(reads_call use_read "{\"name\":\"$first\"}")
case "$gone" in "error: not_found: no saved read named \"$first\"."*) ;; *) fail "a use of the removed read answered: $gone" ;; esac
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"

# 11: the same name with another column asks again, and Save logs the change.
proposed "{\"tab\":$tab,\"name\":\"items\",\"kind\":\"json\",\"url\":\"/reads/items-{id}.json\",\"rows\":\"items\",\"params\":{\"id\":{\"type\":\"text\",\"pattern\":\"A-[0-9]{1,6}\"}},\"sample\":{\"id\":\"A-7\"},\"columns\":[$item,{\"name\":\"Size\",\"type\":\"number\",\"from\":\"size\"}]}" reads-card-change
answer_card Once
await "^browser: agent logged mcp-client change_read $host_rx\$" || fail "the changed read's Save wasn't logged as a change"

# 13: the card keeps its buttons in a short window and stays over the page in full screen. The
# window is 10 points shorter than the plain card's foot, too short for all its rows and well
# taller than the card with one.
short=$((plain_bottom - 10))
send resize 900 $short
expect "^browser: drawn at 900x$short\$" "the window didn't draw at 900 by $short"
proposed "$(table_read table.html keys)" reads-card-short
[ $((qy + qh)) -le $short ] || fail "in a $short-point window the card ends at $((qy + qh)), past its foot: its buttons aren't on screen"
[ "$qh" -lt "$plain_h" ] || fail "in a short window the card is $qh high, as tall as the plain one, $plain_h: it kept every row"
answer_card Once
case "$said" in "saved: "*) ;; *) fail "Save Read in a short window answered: $said" ;; esac
send resize 1200 760
expect '^browser: drawn at 1200x760$' "the window didn't draw back at its size"
send fullscreen
expect '^browser: window entered full screen$' "the window didn't go full screen"
proposed "$(table_read table.html full)" reads-card-full
answer_card Dismiss
send fullscreen
expect '^browser: window left full screen$' "the window didn't leave full screen"

# c: two uses of one read sent together reach the site one after the other. One connection's
# calls with no tab wait for each other in the server, so the second comes on a second connection,
# as a second agent's would.
proposed "$(data_read slow slow.json)" reads-card-slow
answer_card Once
case "$said" in "saved: "*) ;; *) fail "saving the slow read answered: $said" ;; esac
rm -f "$out/reads2-in" "$out/reads2-out"
mkfifo "$out/reads2-in" "$out/reads2-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/reads2-in" "$out/reads2-out" \
    2>> "$out/reads-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/reads2-in" > "$out/reads2-out" \
    2>> "$out/reads-bridge.log" &
fi
exec 6> "$out/reads2-in" 7< "$out/reads2-out"
reads_second() {
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' "${1:-30}" <&7
}
printf '%s\n' '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' >&6
reads_second > /dev/null || fail "the browser didn't answer the second connection's initialize"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&6
from=$(wc -l < "$log")
reads_send use_read '{"name":"slow"}'
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"use_read","arguments":{"name":"slow"}}}' >&6
reads_answer 60 > /dev/null || fail "the first of two uses sent together got no answer"
reads_second 60 > /dev/null || fail "the second of two uses sent together, on a second connection, got no answer"
exec 6>&- 7<&-
# Both uses reached the browser before the site answered either, or the step shows nothing.
order=$(tail -n "+$((from + 1))" "$log" | grep -E 'calls use_read$|saved read [0-9]+ answered' | head -2 | tr '\n' '|')
case "$order" in *"calls use_read|"*"calls use_read|") ;; *)
  fail "the second use reached the browser after the first was answered, so the two weren't together: $order" ;;
esac
most=$(sed -n 's/^READS most //p' "$out/site.log" | tail -1)
[ "$most" = 1 ] || fail "two uses of one read sent together reached the site $most at once"

# 14: a minute with saved reads, the command bar opened and closed, asks the site nothing.
from=$(hits_at)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send key escape
expect '^browser: command bar hidden$' "Escape didn't close the command bar"
sleep 60 # the idle minute measured, in which nothing may ask the site
[ -z "$(hits_since "$host_rx" "$from")" ] ||
  fail "idle, the browser asked the site: $(hits_since "$host_rx" "$from" | tr '\n' ' ')"

exec 4>&-
quit
# The 30-day log names the reads and holds none of their rows.
sessions=$(find "$profile_dir" -name 'Agent sessions' | head -1)
[ -n "$sessions" ] || fail "the agents wrote no Agent sessions file"
grep -q 'city buildings' "$sessions" || fail "the Agent sessions file doesn't name the read used"
grep -q 'Tower Five' "$sessions" && fail "the Agent sessions file holds a row a read gave"
grep -rq 'away\.localhost' "$sessions" && fail "the Agent sessions file holds the address a read was sent to"
true
