# The browser's own ad blocker (--ad-block-engine=native, #2111) keeps a page a filter list names
# from opening at all, as uBlock Origin's strict blocking does: a bare hostname, as URLhaus lists
# them, blocks the host's pages, and the tab shows a warning in the page's place, which never
# reached the server. Its Open anyway, pressed from the keyboard, opens the page, and the host's
# pages for a while after. An agent's tab gets the warning with no way past it, though the user
# let the host through.

# An engine from before the page warning lets the page through.
if ! engine_has ../core/native_ad_block.cc ../core/ad_block.cc; then
  echo "skip: adblock-page. This engine is from before the native ad blocker's page warning."
  exit 0
fi
serve
printf '%s' '<title>the blocked page</title>' > "$out/site/blocked.html"
# localhost, where the check's server is too, as a list writes a host: a line of its own.
blocked="http://localhost:$port/blocked.html"
filters=localhost
profile_dir="$PWD/$out/adblock-page/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/adblock-page.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="$blocked" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" --ad-block-engine=native --ad-block-filters="$filters" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
expect "^browser: title $tab Page blocked\$" \
  "the listed page didn't show the warning: $(grep "^browser: title $tab " "$log" | tail -1)"
grep -q '/blocked\.html' "$out/site.log" && fail "the listed page reached the server"
../tools/window-screenshot.sh "$pid" "$out/adblock-page.png" || fail "no window to capture"

# Open anyway, the only button with no page to go back to: Tab from the page reaches it.
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: adblock-page, after the warning. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
send key tab
send key space
expect "^browser: title $tab the blocked page\$" \
  "Open anyway didn't open the page: $(grep "^browser: title $tab " "$log" | tail -1)"
served '/blocked\.html' "Open anyway didn't ask the server for the page"
../tools/window-screenshot.sh "$pid" "$out/adblock-page-opened.png" || fail "no window to capture"

# An agent's tab, on the host the user let through: the warning, and nothing past it.
send agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
send agentopen claude-code "$blocked?agent"
expect '^browser: agent claude-code opened [1-9][0-9]*$' "the agent didn't open a tab in its Space"
agent_tab=$(sed -n 's/^browser: agent claude-code opened \([0-9]*\)$/\1/p' "$log" | tail -1)
expect "^browser: title $agent_tab Page blocked\$" \
  "the agent's tab didn't show the warning: $(grep "^browser: title $agent_tab " "$log" | tail -1)"
grep -q '/blocked\.html?agent' "$out/site.log" && fail "the agent's page reached the server"
quit
echo "ok: a host a list names on a line of its own never reached the server: the tab showed the" \
  "warning, whose Open anyway, from the keyboard, opened the page. An agent's tab got the warning" \
  "on the host the user let through. Windows: $out/adblock-page.png, $out/adblock-page-opened.png"
