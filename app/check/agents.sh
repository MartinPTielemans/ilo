# A Space the driver makes for an agent, through the core as agent code will, with the Spaces menu
# open, reaches the menu once it closes. It shows in the switcher as the agent's, and comes back the
# agent's, with its owner, after a quit. The agent may act only on the tabs of its own Space: not on
# the user's, nor another agent's on its. ⌃2 shows it as any second Space, a tab opened there is the
# agent's, and the user's tab's Move to lists it after the user's own Space, checked: choosing it
# gives the agent the tab, which moving it back takes away. The agent's mark on its tab goes as its
# session there ends: it releases the tab, its client disconnects, or the user takes the tab back.
# A task's Space, an agent's whose id starts with task-, is never listed as a Space: not in the
# switcher, the Spaces menu or ⌃1 to ⌃9, nor after the relaunch.

log="$out/agents.log"
step=0
rm -f "$saved" "$launch_frame"
# The user's tab, as the first run opens none.
BROWSER_DRIVE=1 BROWSER_URL='https://en.wikipedia.org/wiki/Arc_(web_browser)' \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
first=$(space)
user_tab=$(today | cut -d' ' -f1)
# On the Mac the Space is made with the Spaces menu open, which holds the menu bar back until the
# menu closes, as AppKit can crash when the main menu is replaced under an open menu. Linux shows
# no menu bar.
mac=
made="a Space made for an agent reaches the Spaces menu"
if [ "$(uname)" = Darwin ]; then
  mac=1
  made="a Space made with the Spaces menu open reaches the menu bar once it closes"
  post openmenu Spaces
  expect '^browser: menu tracked New Space…' "the Spaces menu didn't open"
fi
post agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
agent=$(grep '^browser: made space' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: added space $agent Claude Code\$" "the agent's Space didn't reach the switcher"
expect "^browser: agent space $agent Claude Code\$" "the switcher didn't hear whose the Space is"
if [ -n "$mac" ]; then
  expect '^browser: menu bar waits for the menu open$' "the menu bar was replaced under the open menu"
  none_since '^browser: menu bar put up' || fail "the menu bar was put up with the menu still open"
  post dismiss
  expect '^browser: menu bar put up once the menus closed$' \
    "the menu bar didn't come once the menu closed"
fi
send menubar Spaces
expect '^browser: menu bar Spaces: .*Claude Code' "the Spaces menu didn't list the Space made while it was open"
send agentspace task-1 Find a flight
expect '^browser: made space [1-9][0-9]* for agent task-1$' "the driver didn't make the task's Space"
task=$(grep '^browser: made space .* for agent task-1$' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: task space $task Find a flight\$" "the window didn't keep the task's Space apart"
grep -q "^browser: added space $task " "$log" && fail "the task's Space reached the switcher"
grep '^browser: spaces ' "$log" | tail -1 | grep -qw "$task" && fail "the window lists the task's Space"
send menubar Spaces
expect '^browser: menu bar Spaces: .*Claude Code' "the Spaces menu didn't come again"
grep '^browser: menu bar Spaces: ' "$log" | tail -1 | grep -q 'Find a flight' &&
  fail "the Spaces menu lists the task's Space"
grep -q "^browser: agent space $first " "$log" && fail "the user's Space is an agent's"
send agentmayact claude-code "$user_tab"
expect "^browser: agent claude-code may act on $user_tab: 0\$" "the agent may act on the user's tab"
expect "^browser: agent activity claude-code $user_tab navigate refused: The tab isn't in the agent's own Space.\$" \
  "the refusal didn't reach the activity log"
send click $page_point
driven=$active
if [ "$driven" = 1 ]; then
  send key 2 ctrl
  expect "^browser: space $agent\( in .*\)\{0,1\}\$" "⌃2 didn't show the agent's Space"
  send key 3 ctrl
  none_since "^browser: \(task \)\{0,1\}space $task" || fail "⌃3 showed the task's Space"
  open_from_bar example.com
  agent_tab=$tab
  # Even in its own Space, the core refuses an agent ad blocking, passwords, DevTools and a
  # profile's settings, and any action agents don't have, each with why in the activity log, and
  # none starts a session.
  refused() {
    send agentmayact claude-code "$agent_tab" "$1"
    expect "^browser: agent claude-code may act on $agent_tab: 0\$" "the agent may $1 in its own Space"
    expect "^browser: agent activity claude-code $agent_tab $1 refused: $2\$" \
      "the $1 refusal didn't reach the activity log"
  }
  before=$step
  refused ad_blocking "Agents can't change ad blocking. It's the user's, in Site Controls."
  refused passwords "Agents never read passwords, cookies or autofill data."
  refused devtools "Agents get no DevTools or debugging port."
  refused profile_settings "Agents can't change a profile's settings. They're the user's, in Settings › Profiles."
  refused run_script 'Agents have no action "run_script".'
  sed -n "/^browser: drive $before: /,\$p" "$log" | grep -q "^browser: agent claude-code at work" &&
    fail "a refused action started the agent's session"
  send agentmayact claude-code "$agent_tab"
  expect "^browser: agent claude-code may act on $agent_tab: 1\$" \
    "the agent may not act on a tab of its own Space"
  expect "^browser: agent claude-code at work in $agent_tab\$" "the agent's tab didn't show it at work"
  # Each way an agent's session with the tab ends takes its mark off, with an activity-log entry.
  send agentrelease claude-code "$agent_tab"
  expect "^browser: agent claude-code released $agent_tab: 1\$" "the core had no session to release"
  expect "^browser: agent session ended claude-code $agent_tab released\$" \
    "the released session didn't reach the activity log"
  expect "^browser: agent claude-code done in $agent_tab\$" "the released tab still shows the agent at work"
  send agentrelease claude-code "$agent_tab"
  expect "^browser: agent claude-code released $agent_tab: 0\$" "a session ended twice"
  send agentmayact claude-code "$agent_tab"
  expect "^browser: agent claude-code at work in $agent_tab\$" "acting again didn't show the agent at work"
  send agentdisconnect claude-code
  expect "^browser: agent claude-code disconnected, 1 sessions ended\$" \
    "the disconnect didn't end the agent's session"
  expect "^browser: agent session ended claude-code $agent_tab disconnected\$" \
    "the disconnect didn't reach the activity log"
  expect "^browser: agent claude-code done in $agent_tab\$" "the tab still shows a disconnected agent at work"
  send agentmayact claude-code "$agent_tab"
  expect "^browser: agent claude-code at work in $agent_tab\$" "acting again didn't show the agent at work"
  send takeback "$agent_tab"
  expect "^browser: took back $agent_tab: 1\$" "the user couldn't take the tab back"
  expect "^browser: agent session ended claude-code $agent_tab taken back\$" \
    "taking the tab back didn't reach the activity log"
  expect "^browser: agent claude-code done in $agent_tab\$" "the tab taken back still shows the agent at work"
  send agentmayact codex "$agent_tab"
  expect "^browser: agent codex may act on $agent_tab: 0\$" "another agent may act on its tab"
  send key 1 ctrl
  expect "^browser: space $first\( in .*\)\{0,1\}\$" "⌃1 didn't show the user's Space again"
  expect "^browser: settled space $first\$" "the user's Space's page didn't settle"
  # The user's tab's Move to lists the agent's Space after theirs, and choosing it gives the agent
  # the tab, with an entry in its log. Moving it back to the user's Space takes it away again.
  post rightclick $(row 0)
  expect "^browser: menu shown .*, Move to › [^,]* (checked), Move to › Claude Code, Move to › $(cased 'New Space…'), " \
    "the user's tab's menu didn't offer the agent's Space to give it the tab"
  user_space=$(grep '^browser: menu shown' "$log" | tail -1 | sed 's/.*Move to › \([^,]*\) (checked).*/\1/')
  send choose Duplicate
  expect '^browser: today [1-9][0-9]* [1-9][0-9]*$' "Duplicate didn't copy the user's tab"
  at() { today | tr ' ' '\n' | grep -nx "$1" | cut -d: -f1; }
  copy=$(today | tr ' ' '\n' | grep -vx "$user_tab")
  send click $(row $(($(at "$user_tab") - 1)))
  expect "^browser: selected $user_tab\( in .*\)\{0,1\}\$" "the user's tab didn't show again"
  post rightclick $(row $(($(at "$copy") - 1)))
  expect '^browser: menu shown .*, Move to › Claude Code, ' "the copy's menu didn't offer the agent's Space"
  send choose 'Claude Code'
  expect "^browser: gave $copy to agent claude-code\$" "Move to › Claude Code didn't give the agent the tab"
  expect "^browser: today $user_tab\$" "the tab given stayed in the user's Space"
  send agentmayact claude-code "$copy"
  expect "^browser: agent claude-code may act on $copy: 1\$" "the agent may not act on the tab the user gave it"
  send key 2 ctrl
  expect "^browser: space $agent\( in .*\)\{0,1\}\$" "⌃2 didn't show the agent's Space with the tab given"
  expect "^browser: today $copy $agent_tab\$" "the tab given isn't at the top of the agent's Space"
  post rightclick $(row 0)
  expect "^browser: menu shown .*, Move to › $user_space, " "the tab given can't go back to the user's Space"
  send choose "$user_space"
  expect "^browser: today $agent_tab\$" "Move to didn't take the tab given back"
  send agentmayact claude-code "$copy"
  expect "^browser: agent claude-code may act on $copy: 0\$" "the agent may act on the tab taken back to the user's Space"
  send key 1 ctrl
  expect "^browser: space $first\( in .*\)\{0,1\}\$" "⌃1 didn't show the user's Space again"
fi
quit

# Launched again, the agent's Space comes back the agent's, with its tab, and the user's the user's.
log="$out/agents-restored.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: spaces [0-9]* [0-9]*$' || fail "the relaunch didn't bring back both Spaces"
first=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3)
agent=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
grep -q "^browser: added space $agent Claude Code\$" "$log" ||
  fail "the agent's Space came back without its name"
grep -q "^browser: agent space $agent Claude Code\$" "$log" ||
  fail "the agent's Space came back the user's"
grep -q "^browser: agent space $first " "$log" && fail "the user's Space came back an agent's"
grep -q '^browser: task space [1-9][0-9]* Find a flight$' "$log" ||
  fail "the task's Space didn't come back apart from the Spaces"
if [ "$driven" = 1 ]; then
  await_load
  send click $page_point
  send key 2 ctrl
  expect "^browser: space $agent\( in .*\)\{0,1\}\$" "⌃2 didn't show the agent's Space after the relaunch"
  expect '^browser: today [1-9][0-9]*$' "the agent's Space didn't come back with its tab"
  agent_tab=$(today)
  send agentmayact claude-code "$agent_tab"
  expect "^browser: agent claude-code may act on $agent_tab: 1\$" \
    "the agent may not act on its tab after the relaunch"
fi
quit
if [ "$driven" = 1 ]; then
  echo "ok: $made; an agent's Space shows and switches as the user's, keeps the user's tabs out, refuses an agent ad blocking, passwords, DevTools, a profile's settings and any action agents lack, with why in the activity log, gets a tab the user gives it until they move it back, clears its mark as each session ends, and comes back the agent's; a task's Space is in no list of Spaces"
else
  echo "ok: $made; an agent's Space comes back the agent's; a task's Space is in no list of Spaces. skip: switching to it and its tab. Browser wasn't the active app, so the page ignored the click."
fi
