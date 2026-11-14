# check.sh runs this at 672.
# Point (#2352): an agent's point tool asks the user to click on a page what they mean. Its card
# offers Point, which turns the mode on in the tab in front: a frame round the page, the foot bar
# with the question, and a mark on what's under the pointer, all drawn by the window over the page.
# A click opens the note panel at the element, ↩ Add numbers it, and ⌘↩ sends the notes as the
# tool's answer. The page is a sign-in section holding an email and a password the page's script
# filled, as autofill would, at an address with a query and a fragment: the answer has the section's
# text and none of the fields' values, says a field's value never goes, and gives the address
# without its query or fragment, and the panel's Show what's sent lists the rows that go. The
# answer is in the activity log. It's done light, then dark. A third ask, with about:blank in front,
# has Point dimmed on the card, saying why under the pointer, and the card's Not Now there tells
# the agent the user pointed at nothing. Through all of it the page's own MutationObserver hears
# nothing, so Point added nothing to the page's DOM, not even an attribute. Each state is captured,
# light and dark (-dark), as point-ask, point-hover, point-note, point-note-open, point-noted and
# point-cant.

serve
log="$out/point.log"
step=0
rm -f "$saved" "$launch_frame"
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
# The section is a box in the page's middle, so the page's middle is the section's own and its
# mark shows whole round it. Its title says it's ready once the fields are filled; the observer starts after that title,
# so the only title it sets is "changed", once, on anything added, removed or changed in the page.
printf '%s' '<!doctype html><meta charset=utf-8><title>-</title>
<style>body { margin: 0 } section { width: 50%; min-height: 40vh; margin: 25vh auto 0; padding: 20px; box-sizing: border-box; border: 1px solid #ccc }</style>
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

# Waits up to 30 s for more lines matching $1 than the $2 there were.
more() {
  for _ in $(seq 300); do
    [ "$(grep -c "$1" "$log" || true)" -gt "$2" ] && return 0
    sleep 0.1
  done
  return 1
}
# The agent asks to point, as JSON-RPC call $1 asking $2, and $id is the ask's.
ask() {
  asked=$(grep -c '^browser: agent mcp-client asks to point ' "$log" || true)
  printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"point","arguments":{"question":"%s"}}}\n' "$1" "$2" >&4
  more '^browser: agent mcp-client asks to point ' "$asked" || fail "the agent's ask to point $1 didn't come"
  id=$(sed -n 's/^browser: agent mcp-client asks to point \([0-9]*\)$/\1/p' "$log" | tail -1)
}
# An outside agent's ask waits in the card under the URL field (a ⌘T task's has chips on its own
# card). This waits for ask $id's card, failing with $1, and sets qx qy qw qh to where it was last
# drawn. Its buttons sit at its foot, Not Now at the left and Point at the right, as mcp.sh presses
# a card's. The ask comes from the client, not a driver command, so this reads the whole log, where
# the ask's id is its own.
card() {
  wait_begins
  more "^browser: agent question $id at " 0 || {
    ran_out "^browser: agent question $id at " ""
    fail "$1"
  }
  read -r qx qy qw qh << CARD
$(grep "^browser: agent question $id at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
}
not_now_button() { echo $((qx + 30)) $((qy + qh - 26)); }
point_button() { echo $((qx + qw - 30)) $((qy + qh - 26)); }
# One ask, pointed at and sent, as JSON-RPC call $1, its captures named with $2: "" for light and
# "-dark" for dark. The ask waits for the user, so its answer is read once they've sent.
round() {
  ask "$1" "Which part of the page looks wrong?"
  card "the agent's ask drew no card under the URL field"
  ../tools/window-screenshot.sh "$pid" "$out/point-ask$2.png" || fail "no window to capture"
  send click $(point_button)
  if [ "$active" != 1 ]; then
    exec 3>&- 4>&-
    quit
    echo "skip: point. Browser wasn't the active app, so its page ignored the clicks."
    exit 0
  fi
  expect "^browser: point $id started in $tab\$" "Point on the card didn't start pointing in tab $tab"

  # Over the section, its mark; a click opens the note panel at it, saying a field's value in it
  # is withheld. Show what's sent opens the rows, which say the password's value never goes.
  send move $page_point
  # An element's kind is its own role, else its tag, as Arrange's is (core/point.cc's KindOf), so
  # the section with its name and no role is a section.
  expect "browser: tab $tab point over section\$" "the section under the pointer wasn't marked"
  ../tools/window-screenshot.sh "$pid" "$out/point-hover$2.png" || fail "no window to capture"
  send click $page_point
  expect '^browser: point picked section section withheld$' \
    "a click on the section didn't pick it, with its fields' values withheld"
  expect '^browser: note panel 1 withheld$' "the note panel didn't open at the section"
  expect '^browser: note panel fold at ' "the note panel drew no Show what's sent"
  expect '^browser: note panel drawn [0-9]* tall$' "the note panel's window didn't take its content's height"
  ../tools/window-screenshot.sh "$pid" "$out/point-note$2.png" || fail "no window to capture"
  read -r fx fy fw fh << FOLD
$(grep '^browser: note panel fold at ' "$log" | tail -1 | sed 's/.* at //')
FOLD
  send click $((fx + fw / 2)) $((fy + fh / 2))
  expect '^browser: note panel fold at ' "Show what's sent didn't open the panel's rows"
  expect '^browser: note panel drawn [0-9]* tall$' "the note panel's window didn't grow to its rows"
  ../tools/window-screenshot.sh "$pid" "$out/point-note-open$2.png" || fail "no window to capture"
  # Esc drops the note, and the section picked again opens the panel with the keyboard.
  send key escape
  expect '^browser: note panel dismissed$' "esc in the note panel didn't drop the note"
  # The pointer left the page for the panel, so it comes back and marks the section before the
  # click, as a hand's does: a press before its hover is answered picks nothing.
  send move $page_point
  expect "browser: tab $tab point over section\$" "the section wasn't marked again after the note panel"
  send click $page_point
  expect '^browser: note panel 1 withheld$' "the section picked again opened no note panel"
  send type the title is cut off
  send key return
  expect '^browser: point note 1 added$' "↩ in the note panel didn't add the note"
  ../tools/window-screenshot.sh "$pid" "$out/point-noted$2.png" || fail "no window to capture"
  logged_before=$(grep -c '^browser: agent logged mcp-client pointed ' "$log" || true)
  off_before=$(grep -c "browser: tab $tab point off\$" "$log" || true)
  send key return cmd
  expect "^browser: point $id sent 1\$" "⌘↩ didn't send the note"

  answer=$(next_answer | python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
print(("error: " if result.get("isError") else "") + result["content"][0]["text"])') ||
    fail "the point tool didn't answer"
  printf '%s\n' "$answer" > "$out/point-answer$2.txt"
  for part in 'Their note: the title is cut off' 'Sign in' 'Forgot your password?' '/point.html' \
    "A form field's value never goes"; do
    case "$answer" in *"$part"*) ;; *) fail "the answer didn't have \"$part\": $answer" ;; esac
  done
  for part in 'ada@example.com' 'lovelace-check' 's3cr3t' 'session=' '#top'; do
    case "$answer" in *"$part"*) fail "the answer gave away \"$part\": $answer" ;; esac
  done
  more '^browser: agent logged mcp-client pointed ' "$logged_before" ||
    fail "the answer didn't reach the activity log"
  more "browser: tab $tab point off\$" "$off_before" || fail "Point didn't turn off in the page once sent"
  # An if, as this is the round's last command, and a grep finding nothing mustn't be its status.
  if grep -q "^browser: title $tab changed\$" "$log"; then
    fail "the page's DOM changed while the user pointed"
  fi
}
round 1 ""
send menu "Appearance › Dark"
expect '^browser: appearance dark$' "View › Appearance › Dark didn't make the app dark"
round 2 -dark

# On a page that isn't http(s), Point is dimmed on the card and says why under the pointer. The
# user's Not Now there answers that they pointed at nothing.
go_from_bar about:blank
expect "^browser: page $tab back [0-9]* forward 0 loading 0 about:blank\$" "the tab didn't go to about:blank"
ask 3 "And the footer?"
card "the agent's ask on about:blank drew no card under the URL field"
for look in dark light; do
  if [ $look = light ]; then
    send menu "Appearance › Light"
    expect '^browser: appearance light$' "View › Appearance › Light didn't make the app light"
    away
  fi
  # The card stays where it was in either look, and logs its place only when it moves.
  send move $(point_button)
  expect '^browser: tooltip shown ' "resting on the dimmed Point showed no reason"
  name=point-cant-$look
  [ $look = dark ] || name=point-cant
  ../tools/window-screenshot.sh "$pid" "$out/$name.png" || fail "no window to capture"
done
send click $(not_now_button)
expect "^browser: point $id not now\$" "Not Now on the card didn't answer the ask"
expect "^browser: point $id unanswered\$" "the agent didn't hear its ask went unanswered"
nothing=$(next_answer | python3 -I -c '
import json, sys
print(json.load(sys.stdin)["result"]["content"][0]["text"])') || fail "the ask on about:blank didn't answer"
[ "$nothing" = "The user didn't point at anything." ] || fail "Not Now answered: $nothing"
exec 4>&-
quit
echo "ok: the agent's ask to point started from its card, marked the section, took a note in the" \
  "panel, whose rows said what goes, and sent it, with the section's text, no field's value and" \
  "the address without its query, logged, and nothing added to the page, light and dark. On" \
  "about:blank Point was dimmed with its reason, and Not Now answered with nothing. Windows:" \
  "$out/point-*.png"
