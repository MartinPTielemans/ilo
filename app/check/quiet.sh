# check.sh runs this at 595.
# A site's request to show notifications takes the form of the user's intent (#2323), on a page of
# the check's own server, a secure context as notifications need:
# - asked from a key in the page, it gets the card under the URL field, whose × dismisses it;
# - asked on load, with no gesture behind it, it gets no card but a bell in the URL field, which
#   holds the request: a press opens the Site Controls at Notifications, and Allow there answers
#   the page's own request. They then list Notifications as allowed, and Reset permissions, from
#   the keyboard, has the site ask again;
# - left alone, the request stays held: the command bar's Site Controls, from the page and from
#   the UI, open them at Notifications too, and the bell stays. A click on the URL field opens the
#   command bar over the field, the bell under it with the field's other buttons;
# - Block from the bell refuses the page's request, and the Site Controls list it as blocked until
#   Reset permissions.
# Needs an engine with the core's bc_tab_permission_quiet.

serve
mkdir -p "$out/site/quiet"
# Its title says the answer to each request: one on load with ?load, and one from the key n.
cat > "$out/site/quiet/index.html" << 'HTML'
<title>quiet</title>
<script>
const ask = () => Notification.requestPermission().then(answer => document.title = 'notifications ' + answer);
if (location.search == '?load') ask();
document.addEventListener('keydown', event => {
  if (event.key == 'n') ask();
});
</script>
HTML

log="$out/quiet.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}quiet/index.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)

# From a key: the card, as today, and no bell. × dismisses it, and the site may ask again.
send click $page_point
send type n
expect "^browser: page $tab asks ${site%/} to .*notifications\$" \
  "a request from a key didn't ask with the card: $(grep "^browser: page $tab asks" "$log" | tail -1)"
expect "^browser: question $tab at " "the key's request didn't draw the card"
expect "^browser: question $tab takes input\$" "the card never took input"
send layout
read -r qx qy qw qh << EOF
$(grep "^browser: question $tab at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
none_since "^browser: notification bell $tab at " 1 || fail "a request from a key drew the bell"
../tools/window-screenshot.sh "$pid" "$out/quiet-card.png" || fail "no window to capture"
send click $((qx + qw - 18)) $((qy + 18))
expect "^browser: page $tab asks nothing\$" "× didn't take the card down"
expect "^browser: title $tab notifications default\$" "× didn't dismiss the page's request"

# On load: held quietly, with the bell and no card.
go_from_bar "${site#http://}quiet/index.html?load"
from=$step
expect "^browser: loaded $tab ${site}quiet/index.html.load\$" "the page didn't load with ?load"
expect "^browser: page $tab asks ${site%/} to .*notifications, quietly\$" \
  "a request on load wasn't held quietly: $(grep "^browser: page $tab asks" "$log" | tail -1)"
expect "^browser: notification bell $tab at " "the held request's bell wasn't drawn"
send layout
none_since "^browser: question $tab \(at \|takes input\)" "$from" || fail "a request on load brought the card"
[ "$(sed -n "s/^browser: title $tab //p" "$log" | tail -1)" = notifications\ default ] ||
  fail "the held request was answered by itself: $(grep "^browser: title $tab " "$log" | tail -1)"
read -r bx by bw bh << EOF
$(grep "^browser: notification bell $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
send move $((bx + bw / 2)) $((by + bh / 2))
../tools/window-screenshot.sh "$pid" "$out/quiet-bell.png" || fail "no window to capture"

# The bell opens the Site Controls at Notifications, whose Allow answers the page's request.
send click $((bx + bw / 2)) $((by + bh / 2))
expect '^browser: command site-controls via bell$' "a click on the bell didn't open the Site Controls"
expect '^browser: site controls keyed Permission(' "the Site Controls didn't open at Notifications"
expect '^browser: site controls settled$' "the Site Controls didn't settle"
../tools/window-screenshot.sh "$pid" "$out/quiet-site-controls.png" || fail "no window to capture"
send key space
expect '^browser: menu shown .*Allow' "space on Notifications didn't offer Allow"
send choose Allow
expect '^browser: site controls set Notifications Allow$' "Allow in the menu didn't run"
expect "^browser: title $tab notifications granted\$" \
  "Allow in the Site Controls didn't grant the page's held request"
expect "^browser: page $tab asks nothing\$" "Allow didn't take the bell's request down"
controls="^browser: site controls ${site%/}: [^;]*; "
expect "${controls}.*Notifications Allow" "the Site Controls didn't list Notifications as allowed"
# Down to Reset permissions, whose space has the site ask again.
reset_from_keys() {
  for _ in $(seq 16); do
    send key down
    grep '^browser: site controls keyed ' "$log" | tail -1 | grep -q 'keyed Reset$' && break
  done
  grep '^browser: site controls keyed ' "$log" | tail -1 | grep -q 'keyed Reset$' ||
    fail "the arrows didn't reach Reset permissions: $(grep '^browser: site controls keyed ' "$log" | tail -1)"
  send key space
  expect '^browser: site controls reset$' "space on Reset permissions didn't run it"
  expect "${controls}.*Notifications Ask" "Reset permissions didn't set Notifications back to asking"
}
reset_from_keys
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"

# Asked on load again, left alone: the bell stays, and the command bar's Site Controls open them at
# Notifications from the page and from the UI, without answering.
go_from_bar "${site#http://}quiet/index.html?load"
expect "^browser: page $tab asks ${site%/} to .*notifications, quietly\$" \
  "a request on load after Reset permissions wasn't held quietly"
expect "^browser: notification bell $tab at " "the held request's bell wasn't drawn again"
read -r bx by bw bh << EOF
$(grep "^browser: notification bell $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
for focus in page ui; do
  case $focus in
    page) send click $page_point ;;
    # The sidebar's empty space under its rows.
    ui) send click 120 650 ;;
  esac
  from=$step
  send key l cmd
  expect '^browser: command bar shown$' "⌘L with the $focus focused didn't open the command bar"
  send type notifications
  expect '^browser: command bar commands "notifications": Site Controls' \
    "the command bar didn't find Site Controls for \"notifications\" with the $focus focused"
  send key return
  expect '^browser: command site-controls via command bar$' "Site Controls didn't run from the bar"
  expect '^browser: site controls keyed Permission(' \
    "the command bar's Site Controls, with the $focus focused, didn't open at Notifications"
  expect '^browser: site controls settled$' "the Site Controls didn't settle"
  if [ "$focus" = page ]; then
    ../tools/window-screenshot.sh "$pid" "$out/quiet-keys.png" || fail "no window to capture"
  fi
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  none_since "^browser: page $tab asks nothing\$" "$from" ||
    fail "leaving the Site Controls with the $focus focused answered the held request"
done
[ "$(sed -n "s/^browser: title $tab //p" "$log" | tail -1)" = notifications\ default ] ||
  fail "the request left alone was answered: $(grep "^browser: title $tab " "$log" | tail -1)"

# A click on the URL field opens the command bar over the field (tabs.sh), the bell under it with
# the field's other buttons while the field is edited.
send click 100 64
expect '^browser: command bar opens over the URL field$' \
  "a click on the URL field didn't open the command bar over the field"
expect '^browser: command bar shown$' "a click on the URL field didn't open the command bar"
read -r ax ay aw ah << EOF
$(grep '^browser: command bar at' "$log" | tail -1 | cut -d' ' -f5-)
EOF
[ "$bx" -ge "$ax" ] && [ $((bx + bw)) -le $((ax + aw)) ] && [ "$by" -ge "$ay" ] &&
  [ $((by + bh)) -le $((ay + ah)) ] ||
  fail "the command bar at $ax $ay $aw $ah doesn't cover the bell at $bx $by $bw $bh"
../tools/window-screenshot.sh "$pid" "$out/quiet-field-edited.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

# Block from the bell refuses the page's request, and the Site Controls list it as blocked.
send move $((bx + bw / 2)) $((by + bh / 2))
send click $((bx + bw / 2)) $((by + bh / 2))
expect '^browser: command site-controls via bell$' "a click on the bell didn't open the Site Controls"
expect '^browser: site controls keyed Permission(' "the Site Controls didn't open at Notifications"
expect '^browser: site controls settled$' "the Site Controls didn't settle"
send key space
expect '^browser: menu shown .*Block' "space on Notifications didn't offer Block"
send choose Block
expect '^browser: site controls set Notifications Block$' "Block in the menu didn't run"
expect "^browser: title $tab notifications denied\$" \
  "Block in the Site Controls didn't refuse the page's held request"
expect "^browser: page $tab asks nothing\$" "Block didn't take the bell's request down"
expect "${controls}.*Notifications Block" "the Site Controls didn't list Notifications as blocked"
../tools/window-screenshot.sh "$pid" "$out/quiet-blocked.png" || fail "no window to capture"
reset_from_keys
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
exec 3>&-
quit
echo "ok: a request to show notifications from a key in the page got the card, and × dismissed it;" \
  "one on load got no card but the URL field's bell, which opened the Site Controls at" \
  "Notifications, where Allow granted the page's request, they listed it allowed, and Reset" \
  "permissions from the keyboard had the site ask again. Left alone, the bell stayed, and the" \
  "command bar's Site Controls, found by \"notifications\" from the page and the UI, opened them at" \
  "Notifications; the command bar from the URL field covered the bell. Block from the bell refused" \
  "the page's request and was listed until Reset permissions. Windows: $out/quiet-card.png," \
  "$out/quiet-bell.png, $out/quiet-site-controls.png, $out/quiet-keys.png," \
  "$out/quiet-field-edited.png, $out/quiet-blocked.png"
