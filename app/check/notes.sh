# check.sh runs this at 725, only when named.
# Site notes (#4122), which a person reads as skills, checked against the sites they're about. Each
# note's file names landmarks: a public address on its site and words the page there must show.
# An MCP client on the bridge, as mcp.sh's, opens each landmark in a tab of its own, in a new
# profile signed in to nothing: the first answer on a site starts with the site's note, framed as
# only the app frames one, as a part of its own ahead of the page's, and a second answer on that
# site has none. The page's outline or its text has each of the landmark's words. A note
# with no landmark has facts only an account shows, so the run lists it as unproven, which a
# thread run with and without it, by the owner, settles. It needs the network and the sites, so
# it runs only when named: ./check.sh notes.

log="$out/notes.log"
step=0
rm -f "$saved"
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s","name":"%s"}]}' "$token" "$product" > "$agents_file"
chmod 600 "$agents_file"

# The landmarks, a line each: the note's name, the address and its words, split by tabs; and the
# notes with none.
python3 -I - src/agents/notes > "$out/notes-landmarks.tsv" 2> "$out/notes-unproven.txt" << 'PY' ||
import glob, os, re, sys
names = {}
source = open(os.path.join(os.path.dirname(sys.argv[1]), "notes.rs"), encoding="utf-8").read()
for name, file in re.findall(r'note!\("([^"]+)", "([^"]+)"', source):
    names[file] = name
for path in sorted(glob.glob(os.path.join(sys.argv[1], "*.txt"))):
    file = os.path.basename(path)[:-4]
    record = open(path, encoding="utf-8").read().split("\n---\n", 1)[0]
    landmarks = [line[len("landmark: "):] for line in record.splitlines() if line.startswith("landmark: ")]
    for landmark in landmarks:
        url, words = landmark.split(" | ", 1)
        print(f"{names[file]}\t{url}\t{words}")
    if not landmarks:
        print(names[file], file=sys.stderr)
PY
  fail "the notes' landmarks couldn't be read: $(cat "$out/notes-unproven.txt")"
[ -s "$out/notes-landmarks.tsv" ] || fail "no note names a landmark"

BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved Agents switch didn't start the agents server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi
rm -f "$out/notes-in" "$out/notes-out"
mkfifo "$out/notes-in" "$out/notes-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/notes-in" "$out/notes-out" \
    2> "$out/notes-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/notes-in" > "$out/notes-out" \
    2> "$out/notes-bridge.log" &
fi
exec 4> "$out/notes-in" 5< "$out/notes-out"
# Sends the JSON-RPC request $1 and prints the bridge's answer, a line read under perl's alarm, as
# mcp.sh's next_answer does, within 60 s, as a public site may be slow.
answer() {
  printf '%s\n' "$1" >&4
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' 60 <&5 ||
    fail "the agents server didn't answer $1; the bridge said: $(cat "$out/notes-bridge.log")"
}
answer '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' > /dev/null
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4

# Prints what's wrong with the answer on stdin to a call on a landmark of note $1 whose words are
# $2, its first on the site if $3 is 1, or nothing. A word may be in the outline or, where the
# outline is cut, in the text a read_page gives, $4.
judged() {
  python3 -I -c '
import json, sys
name, words, first, text = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4]
frame = sys.argv[5] + "\x27s notes for " + name + ":\n"
result = json.load(sys.stdin)["result"]
parts = [part.get("text", "") for part in result["content"]]
wrong = []
if result.get("isError"):
    wrong.append("the call failed: " + parts[0][:200])
noted = [part for part in parts if part.startswith(frame)]
if first and (not noted or parts[0] != noted[0] or len(parts) < 2):
    wrong.append("its first answer on the site had no note ahead of the page")
if not first and noted:
    wrong.append("a second answer on the site had the note again")
said = "\n".join(part for part in parts if not part.startswith(frame)) + "\n" + text
for word in (word.strip() for word in words.split(";")):
    if word not in said:
        wrong.append("the page didn\x27t show " + repr(word))
print("; ".join(wrong))' "$1" "$2" "$3" "$4" "$product"
}

id=10
seen=" "
failed=""
proven=""
while IFS="$(printf '\t')" read -r name url words; do
  id=$((id + 2))
  case "$seen" in *" $name "*) first=0 ;; *) first=1 ;; esac
  seen="$seen$name "
  url_json=$(python3 -I -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$url")
  opened=$(answer "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"tools/call\",\"params\":{\"name\":\"open_tab\",\"arguments\":{\"url\":$url_json}}}")
  printf '%s\n' "$opened" > "$out/notes-$id.json"
  tab=$(printf '%s\n' "$opened" | sed -n 's/.*Opened tab \([0-9]*\)\..*/\1/p' | head -1)
  text=""
  if [ -n "$tab" ]; then
    text=$(answer "{\"jsonrpc\":\"2.0\",\"id\":$((id + 1)),\"method\":\"tools/call\",\"params\":{\"name\":\"read_page\",\"arguments\":{\"tab\":$tab}}}" |
      python3 -I -c 'import json, sys; print("\n".join(p.get("text", "") for p in json.load(sys.stdin)["result"]["content"]))')
  fi
  wrong=$(printf '%s\n' "$opened" | judged "$name" "$words" "$first" "$text") ||
    fail "the answer on $url couldn't be read: $(head -c 300 "$out/notes-$id.json")"
  if [ -n "$wrong" ]; then
    failed="$failed
  $name, $url: $wrong"
  else
    proven="$proven $name,"
  fi
done < "$out/notes-landmarks.tsv"

exec 4>&- 5<&-
exec 3>&-
quit
[ -z "$failed" ] || fail "a note's landmarks are gone from its site, or its note didn't come:$failed"
echo "notes: landmarks found, each site's note first and once, for${proven%,}; unproven, needing an account: $(tr '\n' ',' < "$out/notes-unproven.txt" | sed 's/,$//; s/,/, /g')"
