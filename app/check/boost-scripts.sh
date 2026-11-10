# A Boost's script (#2297): the app gives the core the script in the site's file, which runs on
# none of the site's pages until the user turns it on, from the Boost card's Turn On Script… and
# Turn On in the dialog, which takes no answer for its first moment on screen. Then it runs as each of
# the site's pages loads, though not in a private window. A change to the file outside ilo turns
# it off from the next load, and turning it back on asks about what changed; Cancel leaves it off.

serve
# The page says in its title, once it has loaded, what the script set on it: the script runs as
# the document finishes parsing, before the load event.
printf '%s' '<!doctype html><meta charset="utf-8"><title>loading</title>
<body style="margin:0">
<div style="height:100vh">a page with a Boost script</div>
<script>
addEventListener("load", () => {
  document.title = "script " + (document.documentElement.dataset.script || "none");
});
</script>' > "$out/site/boost-script.html"
page="${site}boost-script.html"

host=${site#http://}
host=${host%%:*}
boosts="$profile_dir/Default/Boosts"
rm -rf "$boosts" "$profile_dir/Default/Boost scripts turned on"
mkdir -p "$boosts"
printf '%s\n' 'body { color: rgb(0, 0, 128) !important; }' > "$boosts/$host.css"
printf '%s\n' "document.documentElement.dataset.script = 'one';" > "$boosts/$host.js"

log="$out/boost-scripts.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: boost script given $host ([0-9]*): New\$" ||
  fail "the app didn't give the core the site's script as new"
await '^browser: title [0-9]* script none$' ||
  fail "a script never turned on ran: $(grep '^browser: title' "$log" | tail -1)"
tab=$(grep '^browser: title [0-9]* script none$' "$log" | tail -1 | cut -d' ' -f3)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: boost-scripts' Turn On. Browser wasn't the active app, so the page ignored the" \
    "click. The app gave the core the site's script as new, and it didn't run."
  exit 0
fi

# The Boost card's Turn On Script…, from Edit Boost in the command bar. It's the line under the
# script's state, its middle 40 points over Zap's row, which is 62 over the card's bottom, and it
# starts at the card's left.
turn_on_from_card() {
  bar_command edit-boost "Edit Boost" "edit boost"
  expect "^browser: boost card $tab opened\$" "Edit Boost opened no card"
  expect "^browser: boost card $tab at " "the Boost's card wasn't drawn"
  send layout
  read -r bx by bw bh << EOF
$(grep "^browser: boost card $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
  ../tools/window-screenshot.sh "$pid" "$out/boost-script-$1-card.png" ||
    fail "no window to capture"
  send click $((bx + bw / 4)) $((by + bh - 102))
  expect "^browser: boost script asked $host $1\$" \
    "the card's Turn On Script… didn't ask about the $1 script"
  expect '^browser: dialog takes input$' "the dialog didn't settle"
  ../tools/window-screenshot.sh "$pid" "$out/boost-script-$1-dialog.png" ||
    fail "no window to capture"
}

turn_on_from_card New
send choose 'Turn On'
expect "^browser: boost script turned on $host\$" "Turn On didn't turn the script on"
send key r cmd
expect "^browser: title $tab script one\$" "the script turned on didn't run as the page loaded"

# A private window runs no script.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
send insert "$page"
send key return
expect "^browser: loaded [0-9]* $page\$" "the private window didn't load the page"
private=$(sed -n "s|^browser: loaded \([0-9]*\) $page\$|\1|p" "$log" | tail -1)
expect "^browser: title $private script " "the private window's page didn't load"
grep -q "^browser: title $private script none\$" "$log" ||
  fail "the script ran in a private window: $(grep "^browser: title $private " "$log" | tail -1)"
send key w cmd shift
expect '^browser: window closed [0-9]*$' "⇧⌘W didn't close the private window"

# A save outside ilo turns it off from the next load.
printf '%s\n' "document.documentElement.dataset.script = 'two';" > "$boosts/$host.js"
await "^browser: boost script given $host ([0-9]*): Changed\$" ||
  fail "the app didn't give the core the changed script"
send key r cmd
expect "^browser: title $tab script none\$" "the changed script ran"

turn_on_from_card Changed
send choose Cancel
expect "^browser: boost script cancelled $host\$" "Cancel didn't leave the script off"
send key r cmd
expect "^browser: title $tab script none\$" "the script cancelled ran"
exec 3>&-
quit
echo "ok: a Boost's script didn't run until the card's Turn On Script… and Turn On in the" \
  "dialog, then ran as the page loaded, though not in a private window; a change to its file" \
  "turned it off, and Cancel on what changed left it off." \
  "Windows: $out/boost-script-New-card.png, $out/boost-script-New-dialog.png," \
  "$out/boost-script-Changed-dialog.png"
