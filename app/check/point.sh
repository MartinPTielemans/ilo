# check.sh runs this at 672.
# Point (#2352): an agent's point tool asks the user to click on a page what they mean. Its card
# offers Point, which turns the mode on in the tab in front: a frame round the page, the foot bar
# with the question, and a mark on what's under the pointer, all drawn by the window over the page.
# A click opens the note panel at the element, ↩ Add numbers it, and ⌘↩ sends the notes as the
# tool's answer. The page is a sign-in section holding an email, a password, a select and a
# textarea the page's script filled, as autofill would, at an address with a query and a fragment:
# the answer has the section's text and none of the fields' values, says a field's value never
# goes, and gives the address without its query or fragment, and its picture of the section has
# every field's box hatched, read by its pixels, where a line of the page's own text shows. The
# panel's Show what's sent lists the rows that go, scrolling with a fade at its cut edge past the
# panel's height. The answer is in the activity log. It's done light, then dark. Between, esc
# mid-point answers that the user pointed at nothing, and the frame and mark go. A last ask, with
# about:blank in front, has Point dimmed on the card, saying why under the pointer, and the card's
# Not Now there answers the same. Once the client leaves, its session's page in Library › Agents
# lists the steps. Through all of it the page's own MutationObserver hears nothing, so Point added
# nothing to the page's DOM, not even an attribute. Each state is captured, light and dark (-dark),
# as point-ask, point-hover, point-note, point-note-open, point-note-end, point-noted,
# point-picture and point-cant, and once as point-way-out and point-sent.

serve
log="$out/point.log"
step=0
rm -f "$saved" "$launch_frame"
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
# The section is a box in the page's middle, so the page's middle is the section's own, clear of
# its fields, and its mark shows whole round it. Its fields sit where FIELDS below says, each
# holding what its value is drawn in red: an email and a password as autofill fills them, a select
# and a textarea as the user leaves them. The red line under them is no field's, so the picture
# shows it. Its title says it's ready once the fields are filled; the observer starts after that
# title, so the only title it sets is "changed", once, on anything added, removed or changed.
printf '%s' '<!doctype html><meta charset=utf-8><title>-</title>
<style>body { margin: 0 }
section { position: relative; width: 520px; height: 320px; margin: 25vh auto 0;
  box-sizing: border-box; border: 1px solid #ccc; font: 16px sans-serif }
section > * { position: absolute; margin: 0; box-sizing: border-box }
h1 { left: 20px; top: 16px; font-size: 24px }
input, select, textarea { width: 220px; height: 30px; padding: 2px 6px; color: red;
  font: 16px monospace; background: white }
#email { left: 20px; top: 64px } #password { left: 260px; top: 64px }
#country { left: 20px; top: 108px } #words { left: 260px; top: 108px; height: 54px; resize: none }
p { left: 20px; top: 220px } #shown { left: 20px; top: 260px; color: red; font: 24px monospace }
</style>
<section aria-label="Sign in"><h1>Sign in</h1>
<input id=email type=email autocomplete=email> <input id=password type=password>
<select id=country><option>-</option><option>Netherlands</option></select>
<textarea id=words></textarea>
<p>Forgot your password?</p><div id=shown>██████████</div></section>
<script>
email.value = "ada@example.com";
password.value = "lovelace-check";
country.value = "Netherlands";
words.value = "Meet at noon";
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
# Each field's box in the section's padding box, in CSS pixels, as the page's style puts it, and
# whether its red shows in the picture: no field's does, as the panel says no field's value is sent.
FIELDS="email 20 64 220 30 hatched
password 260 64 220 30 hatched
select 20 108 220 30 hatched
textarea 260 108 220 54 hatched
line 20 260 240 28 shown"
# Checks the sent picture $1 against FIELDS, writing each one's red to $3. The picture is the
# section's box, which the answer $2 gives, and 8 around it (core/point.cc's kAround), so a field is
# at the section's border (1) and its own place inside that, at the picture's scale. A box is read
# 3 in from its edge, clear of the field's border.
pictured() {
  pixel "$1" 0 0 > /dev/null
  python3 -I - "$out/pixel.bmp" "$2" "$FIELDS" > "$3" << 'PY' ||
import re, struct, sys
data = open(sys.argv[1], "rb").read()
offset, = struct.unpack_from("<I", data, 10)
width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
stride = (bits * width + 31) // 32 * 4
box = re.search(r"^Box: (\d+) × (\d+) at", sys.argv[2], re.M)
scale = width / (int(box.group(1)) + 16)
failed = False
for line in sys.argv[3].splitlines():
    name, left, top, w, h, want = line.split()
    x0, y0 = (8 + 1 + int(left) + 3) * scale, (8 + 1 + int(top) + 3) * scale
    x1, y1 = x0 + (int(w) - 6) * scale, y0 + (int(h) - 6) * scale
    red = total = 0
    for y in range(int(y0), min(int(y1), abs(height))):
        row = y if height < 0 else abs(height) - 1 - y
        for x in range(int(x0), min(int(x1), width)):
            b, g, r = data[offset + row * stride + x * bits // 8:][:3]
            red += r > 160 and g < 100 and b < 100
            total += 1
    print(f"{name}: {red} of {total} pixels red, {want}")
    failed |= total == 0 or (red > 0 if want == "hatched" else red * 10 < total)
sys.exit(failed)
PY
    fail "the sent picture showed a field's value, or not the section's red line: $(cat "$3")"
}
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
  # The rows are taller than the panel's most, so they scroll, and their cut edge fades as
  # Settings' card's does until the wheel brings their end in sight.
  expect '^browser: note panel rows go on below$' "the panel's rows, past its height, drew no fade at their cut edge"
  ../tools/window-screenshot.sh "$pid" "$out/point-note-open$2.png" || fail "no window to capture"
  send scrollkey $((fx + fw / 2)) $((fy + fh + 80)) 400 2
  expect '^browser: note panel rows end in sight$' "the wheel over the panel's rows didn't bring their end in sight"
  ../tools/window-screenshot.sh "$pid" "$out/point-note-end$2.png" || fail "no window to capture"
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

  # Its text, and its one picture saved as point-picture.png (-dark).
  answer=$(next_answer | python3 -I -c '
import base64, json, sys
result = json.load(sys.stdin)["result"]
for each in result["content"][1:]:
    if each.get("type") == "image":
        open(sys.argv[1], "wb").write(base64.b64decode(each["data"]))
print(("error: " if result.get("isError") else "") + result["content"][0]["text"])' \
    "$out/point-picture$2.png") || fail "the point tool didn't answer"
  printf '%s\n' "$answer" > "$out/point-answer$2.txt"
  for part in 'Their note: the title is cut off' 'Sign in' 'Forgot your password?' '/point.html' \
    "A form field's value never goes"; do
    case "$answer" in *"$part"*) ;; *) fail "the answer didn't have \"$part\": $answer" ;; esac
  done
  for part in 'ada@example.com' 'lovelace-check' 'Netherlands' 'Meet at noon' 's3cr3t' 'session=' \
    '#top'; do
    case "$answer" in *"$part"*) fail "the answer gave away \"$part\": $answer" ;; esac
  done
  [ -s "$out/point-picture$2.png" ] || fail "the answer had no picture of the section: $answer"
  pictured "$out/point-picture$2.png" "$answer" "$out/point-picture$2.txt"
  more '^browser: agent logged mcp-client pointed ' "$logged_before" ||
    fail "the answer didn't reach the activity log"
  more "browser: tab $tab point off\$" "$off_before" || fail "Point didn't turn off in the page once sent"
  # An if, as this is the round's last command, and a grep finding nothing mustn't be its status.
  if grep -q "^browser: title $tab changed\$" "$log"; then
    fail "the page's DOM changed while the user pointed"
  fi
}
round 1 ""

# The way out mid-point: esc with the section marked answers the ask as Not Now does, that the user
# pointed at nothing, and the frame, its tab and the mark go with Point.
ask 3 "Is the header the right height?"
card "the agent's third ask drew no card under the URL field"
send click $(point_button)
expect "^browser: point $id started in $tab\$" "Point on the card didn't start pointing in tab $tab"
send move $page_point
expect "browser: tab $tab point over section\$" "the section under the pointer wasn't marked"
hidden=$(grep -c '^browser: drop targets hidden$' "$log" || true)
send key escape
expect "^browser: point $id not now\$" "esc while pointing didn't answer the ask"
expect "browser: tab $tab point off\$" "esc didn't turn Point off in the page"
more '^browser: drop targets hidden$' "$hidden" || fail "the frame, its tab and the mark stayed after esc"
nothing=$(next_answer | python3 -I -c '
import json, sys
print(json.load(sys.stdin)["result"]["content"][0]["text"])') || fail "the ask left with esc didn't answer"
[ "$nothing" = "The user didn't point at anything." ] || fail "esc answered: $nothing"
../tools/window-screenshot.sh "$pid" "$out/point-way-out.png" || fail "no window to capture"

send menu "Appearance › Dark"
expect '^browser: appearance dark$' "View › Appearance › Dark didn't make the app dark"
round 2 -dark

# On a page that isn't http(s), Point is dimmed on the card and says why under the pointer. The
# user's Not Now there answers that they pointed at nothing.
go_from_bar about:blank
expect "^browser: page $tab back [0-9]* forward 0 loading 0 about:blank\$" "the tab didn't go to about:blank"
ask 4 "And the footer?"
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
# What went, as the user sees it once the client leaves: its session's page in Library › Agents,
# whose steps are the asks to point and the user's sends.
session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log")
[ -n "$session" ] || fail "the agent's asks started no session"
exec 4>&-
await "^browser: session $session done\$" || fail "the client's session didn't end when it left"
send menu 'View Agents…'
expect '^browser: library section agents$' "View Agents… didn't open the Library on Agents"
await '^browser: library shown$' || fail "the Library didn't settle open on Agents"
at=$(drawn "library past $session-[0-9]*") || fail "Library › Agents drew no row for the client's session"
past=$(sed -n "s/^browser: settings library past \($session-[0-9]*\) at .*/\1/p" "$log" | tail -1)
send click $at
expect "^browser: agents history shows $past: " "the session's row didn't open its page"
../tools/window-screenshot.sh "$pid" "$out/point-sent.png" || fail "no window to capture"
quit
echo "ok: the agent's ask to point started from its card, marked the section, took a note in the" \
  "panel, whose rows said what goes, and sent it, with the section's text, no field's value and" \
  "the address without its query, logged, and nothing added to the page, light and dark. On" \
  "about:blank Point was dimmed with its reason, and Not Now, as esc mid-point, answered with" \
  "nothing. Library › Agents listed the session's steps. Windows:" \
  "$out/point-*.png"
