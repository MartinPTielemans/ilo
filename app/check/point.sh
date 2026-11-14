# check.sh runs this at 672.
# Point (#2352): an agent's point tool asks the user to click on a page what they mean. Its card
# offers Point, which turns the mode on in the tab in front: a frame round the page, the foot bar
# with the question, and a mark on what's under the pointer, all drawn by the window over the page.
# A click opens the note panel at the element, ↩ Add numbers it, and ⌘↩ sends the notes as the
# tool's answer. The page is a sign-in section holding an email and a password the page's script
# filled, as autofill would, at an address with a query and a fragment: the answer has the section's
# text and none of the fields' values, says a field's value never goes, and gives the address
# without its query or fragment. The answer is in the activity log. A second ask, answered with
# its card's Not Now, tells the agent the user pointed at nothing. Through all of it the page's
# own MutationObserver hears nothing, so Point added nothing to the page's DOM, not even an
# attribute.

serve
log="$out/point.log"
step=0
rm -f "$saved" "$launch_frame"
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
# The section fills the page under its heading and fields, so the page's middle is the section's
# own. Its title says it's ready once the fields are filled; the observer starts after that title,
# so the only title it sets is "changed", once, on anything added, removed or changed in the page.
printf '%s' '<!doctype html><meta charset=utf-8><title>-</title>
<style>body { margin: 0 } section { height: 100vh; padding: 20px; box-sizing: border-box }</style>
<section aria-label="Sign in"><h1>Sign in</h1>
<input id=email type=email autocomplete=email> <input id=password type=password>
<p>Forgot your password?</p></section>
<script>
email.value = "ada@example.com";
password.value = "lovelace-check";
requestAnimationFrame(() => requestAnimationFrame(() => {
  document.title = "point ready";
  const watch = new MutationObserver(() => {
    watch.disconnect();
    document.title = "changed";
  });
  watch.observe(document.documentElement,
    { subtree: true, childList: true, attributes: true, characterData: true });
}));
</script>' > "$out/site/point.html"
BROWSER_DRIVE=1 BROWSER_URL="${site}point.html?session=s3cr3t#top" "$browser" < "$out/drive" \
  > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab point ready\$" || fail "the page didn't fill its fields"
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved switch didn't start the agents' server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then
  . check/git-bash.sh
  bridge="$app/mcp-bridge.exe"
else
  bridge="$(dirname "$browser")/mcp-bridge"
fi
rm -f "$out/point-in" "$out/point-out"
mkfifo "$out/point-in" "$out/point-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/point-in" "$out/point-out" \
    2> "$out/point-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/point-in" > "$out/point-out" \
    2> "$out/point-bridge.log" &
fi
exec 4> "$out/point-in" 5< "$out/point-out"
# The bridge's next line within $1 seconds, as mcp.sh reads it.
next_answer() {
  perl -e "alarm ${1:-30}; exec @ARGV" sh -c 'read -r line && printf "%s\n" "$line"' <&5
}
printf '%s\n' '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' >&4
next_answer > "$out/point-initialize.json" || fail "the browser didn't answer the client's hello"
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4

# The ask waits for the user, so its answer is read once they've sent.
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"point","arguments":{"question":"Which part of the page looks wrong?"}}}' >&4
await '^browser: agent mcp-client asks to point [0-9]*$' || fail "the agent's ask to point didn't come"
id=$(sed -n 's/^browser: agent mcp-client asks to point \([0-9]*\)$/\1/p' "$log" | tail -1)
await '^browser: task chip task-[0-9]* Point at ' || fail "the agent's card drew no Point"
read -r cx cy cw ch << CHIP
$(grep '^browser: task chip task-[0-9]* Point at ' "$log" | tail -1 | sed 's/.* Point at //')
CHIP
send click $((cx + cw / 2)) $((cy + ch / 2))
if [ "$active" != 1 ]; then
  exec 3>&- 4>&-
  quit
  echo "skip: point. Browser wasn't the active app, so its page ignored the clicks."
  exit 0
fi
expect "^browser: point $id started in $tab\$" "Point on the card didn't start pointing in tab $tab"

# Over the section, its mark; a click opens the note panel at it, saying a field's value in it is
# withheld.
send move $page_point
expect "browser: tab $tab point over region\$" "the section under the pointer wasn't marked"
../tools/window-screenshot.sh "$pid" "$out/point-hover.png" || fail "no window to capture"
send click $page_point
expect '^browser: point picked region section withheld$' \
  "a click on the section didn't pick it, with its fields' values withheld"
await '^browser: note panel 1 withheld$' || fail "the note panel didn't open at the section"
../tools/window-screenshot.sh "$pid" "$out/point-note.png" || fail "no window to capture"
send type the title is cut off
send key return
expect '^browser: point note 1 added$' "↩ in the note panel didn't add the note"
../tools/window-screenshot.sh "$pid" "$out/point-noted.png" || fail "no window to capture"
send key return cmd
expect "^browser: point $id sent 1\$" "⌘↩ didn't send the note"

answer=$(next_answer | python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
print(("error: " if result.get("isError") else "") + result["content"][0]["text"])') ||
  fail "the point tool didn't answer"
printf '%s\n' "$answer" > "$out/point-answer.txt"
for part in 'Their note: the title is cut off' 'Sign in' 'Forgot your password?' '/point.html' \
  "A form field's value never goes"; do
  case "$answer" in *"$part"*) ;; *) fail "the answer didn't have \"$part\": $answer" ;; esac
done
for part in 'ada@example.com' 'lovelace-check' 's3cr3t' 'session=' '#top'; do
  case "$answer" in *"$part"*) fail "the answer gave away \"$part\": $answer" ;; esac
done
await '^browser: agent logged mcp-client pointed ' || fail "the answer didn't reach the activity log"
await "browser: tab $tab point off\$" || fail "Point didn't turn off in the page once sent"
grep -q "^browser: title $tab changed\$" "$log" && fail "the page's DOM changed while the user pointed"

# Asked again, the user's Not Now on the card answers that they pointed at nothing.
printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"point","arguments":{"question":"And the footer?"}}}' >&4
asked=$(grep -c '^browser: agent mcp-client asks to point ' "$log")
for _ in $(seq 300); do
  [ "$(grep -c '^browser: agent mcp-client asks to point ' "$log")" -gt "$asked" ] && break
  sleep 0.1
done
again=$(sed -n 's/^browser: agent mcp-client asks to point \([0-9]*\)$/\1/p' "$log" | tail -1)
[ "$again" != "$id" ] || fail "the agent's second ask to point didn't come"
expect '^browser: task chip task-[0-9]* Not Now at ' "the agent's card drew no Not Now for its second ask"
read -r nx ny nw nh << CHIP
$(grep '^browser: task chip task-[0-9]* Not Now at ' "$log" | tail -1 | sed 's/.* Not Now at //')
CHIP
send click $((nx + nw / 2)) $((ny + nh / 2))
expect "^browser: point $again not now\$" "Not Now on the card didn't answer the ask"
expect "^browser: point $again unanswered\$" "the agent didn't hear its ask went unanswered"
nothing=$(next_answer | python3 -I -c '
import json, sys
print(json.load(sys.stdin)["result"]["content"][0]["text"])') || fail "the second ask didn't answer"
[ "$nothing" = "The user didn't point at anything." ] || fail "Not Now answered: $nothing"
grep -q "^browser: title $tab changed\$" "$log" && fail "the page's DOM changed after Not Now"
exec 4>&-
quit
echo "ok: the agent's ask to point started from its card, marked the section, took a note in the" \
  "panel and sent it, with the section's text, no field's value and the address without its" \
  "query, logged, and nothing added to the page. Not Now answered a second ask with nothing." \
  "Windows: $out/point-hover.png $out/point-note.png $out/point-noted.png"
