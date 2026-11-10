# A site's request to show notifications takes the form of the user's intent (#2323), on a page of
# the check's own server, a secure context as notifications need:
# - asked from a key in the page, it gets the card under the URL field, whose × dismisses it;
# - asked on load, with no gesture behind it, it gets no card but a bell in the URL field, which
#   holds the request: a press opens the Site Controls at Notifications, and Allow there answers
#   the page's own request.
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
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
exec 3>&-
quit
echo "ok: a request to show notifications from a key in the page got the card, and × dismissed it;" \
  "one on load got no card but the URL field's bell, which opened the Site Controls at" \
  "Notifications, where Allow granted the page's request. Windows: $out/quiet-card.png," \
  "$out/quiet-bell.png, $out/quiet-site-controls.png"
