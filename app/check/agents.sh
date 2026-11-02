# A Space the driver makes for an agent, through the core as agent code will, with the Spaces menu
# open, reaches the menu once it closes. It shows in the switcher as the agent's, and comes back the
# agent's, with its owner, after a quit. The agent may act only on the tabs of its own Space: not on
# the user's, nor another agent's on its. ⌃2 shows it as any second Space, a tab opened there is the
# agent's, and the user's tab's Move to lists it after the user's own Space, checked: choosing it
# gives the agent the tab, which moving it back takes away. The agent's mark on its tab goes as its
# session there ends: it releases the tab, its client disconnects, or the user takes the tab back.
# On Windows, Settings › Agents' Connect finds Claude Code's claude where its own installer, winget
# or npm put it, though none of those is on the PATH (#1420), and gives it the browser's server
# whole, which runs and reaches the browser as Claude Code. Disconnect takes it out again. With
# claude gone, the row says so in words. Each claude is a stand-in that records its arguments, in a
# home folder of the run's own, never the user's.

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

connected=
if windows; then
  log="$out/agents-connect.log"
  step=0
  rm -f "$saved" "$launch_frame"
  # The home the app reads Claude Code's folders from, with ~/.claude, which lists its row.
  home="$PWD/$out/claude-home"
  rm -rf "$home"
  mkdir -p "$home/.claude"
  native="$home/.local/bin"
  winget="$home/AppData/Local/Microsoft/WinGet/Links"
  npm="$home/AppData/Roaming/npm"
  # The stand-in for Claude Code's claude.exe: it writes each call's arguments, as Windows hands a
  # program them, in claude-calls beside it, a JSON array a line without spaces.
  cat > "$out/claude-stand-in.rs" << 'EOF'
use std::io::Write;
fn main() {
    let quoted: Vec<String> = std::env::args().skip(1).map(|arg| {
        let mut quoted = String::from("\"");
        for c in arg.chars() {
            match c {
                '"' | '\\' => { quoted.push('\\'); quoted.push(c); }
                c if (c as u32) < 0x20 => quoted += &format!("\\u{:04x}", c as u32),
                c => quoted.push(c),
            }
        }
        quoted + "\""
    }).collect();
    let calls = std::env::current_exe().unwrap().with_file_name("claude-calls");
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(calls).unwrap();
    writeln!(file, "[{}]", quoted.join(",")).unwrap();
}
EOF
  rustc -O -o "$out/claude.exe" "$out/claude-stand-in.rs" ||
    fail "rustc couldn't build the stand-in for claude.exe"
  # npm's claude.cmd, as npm writes it for a global install, which runs Python with a script that
  # records the arguments where it runs node with Claude Code's cli.js: both read them alike.
  python=$(python3 -I -c 'import sys; print(sys.executable)')
  printf '%s\n' '@ECHO off' 'GOTO start' ':find_dp0' 'SET dp0=%~dp0' 'EXIT /b' ':start' 'SETLOCAL' \
    'CALL :find_dp0' "SET \"_prog=$python\"" \
    'endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\claude-calls.py" %*' |
    sed 's/$/\r/' > "$out/claude.cmd"
  printf '%s\n' 'import json, os, sys' \
    'with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "claude-calls"), "a", encoding="utf-8") as calls:' \
    '    calls.write(json.dumps(sys.argv[1:], separators=(",", ":")) + "\n")' > "$out/claude-calls.py"
  agents_file="$(dirname "$saved")/Agents"
  printf '{"on":true,"agents":[]}' > "$agents_file"
  # The PATH stays the run's, which has none of the three folders.
  USERPROFILE=$(cygpath -w "$home") LOCALAPPDATA=$(cygpath -w "$home/AppData/Local") \
    APPDATA=$(cygpath -w "$home/AppData/Roaming") \
    BROWSER_PROFILE_DIR=$(cygpath -w "${BROWSER_PROFILE_DIR:-$profile_dir}") BROWSER_DRIVE=1 \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: agents server on \\\\\.\\pipe\\' || fail "the saved switch didn't start the server"
  socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
  name="$(echo "$app_name" | tr '[:upper:]' '[:lower:]')-dev"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  # Shows Settings' tab $1, drawn, which a click needs: a row that changed draws at the next frame,
  # and a click before it would land on what the row showed before.
  settings_tab() {
    send clickkey $(drawn "toolbar $1")
    expect "^browser: settings tab $1\$" "a click on $1 didn't show its tab"
    settings_drew "$1"
  }
  settings_tab Agents
  # Waits up to 30 s for the calls file $1 to have $2 lines.
  calls_reach() {
    for _ in $(seq 300); do
      [ "$(cat "$1" 2>/dev/null | wc -l)" -ge "$2" ] && return 0
      sleep 0.1
    done
    return 1
  }
  # Connect with claude at $1, the program it records its calls beside. Claude Code gets a remove,
  # then the server whole, with the bridge beside the browser's executable, its socket and a token
  # the browser takes as Claude Code's. Then Disconnect removes it.
  connect_with() {
    calls="$(dirname "$1")/claude-calls"
    rm -f "$calls"
    send clickkey $(drawn "connect claude-code")
    expect '^browser: connect agent claude-code$' "a click on Connect didn't connect Claude Code"
    expect_late '^browser: agent claude-code \(connected\|couldn.t connect: .*\)$' \
      "Connect with $(basename "$1") didn't finish"
    logged '^browser: agent claude-code connected$' ||
      fail "Connect with $1 failed: $(sed -n 's/^browser: agent claude-code couldn.t connect: //p' "$log" | tail -1)"
    hellos=$(grep -c '^browser: agent claude-code hello from check$' "$log" || true)
    python3 -I - "$calls" "$name" "$(cygpath -w "$app/mcp-bridge.exe")" "$socket" << 'PY' || fail "Claude Code didn't get the server whole from $1"
import json, os, subprocess, sys, threading
path, name, bridge, socket = sys.argv[1:]
with open(path, encoding="utf-8") as file:
    calls = [json.loads(line) for line in file]
added = ["mcp", "add-json", "--scope", "user", name]
if len(calls) != 2 or calls[0] != ["mcp", "remove", "--scope", "user", name] or calls[1][:5] != added or len(calls[1]) != 6:
    sys.exit(f"claude was called with {calls}")
server = json.loads(calls[1][5])
token = server.get("env", {}).get("BROWSER_AGENT_TOKEN", "")
if (sorted(server) != ["args", "command", "env", "type"] or server["type"] != "stdio"
        or os.path.normcase(server["command"]) != os.path.normcase(bridge)
        or server["args"] != [socket] or len(token) != 64):
    sys.exit(f"claude add-json got {calls[1][5]}")
# Runs the server as Claude Code does, and asks it to start a session.
bridge = subprocess.Popen([server["command"], *server["args"]], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          env=dict(os.environ, **server["env"]))
threading.Timer(30, bridge.kill).start()
bridge.stdin.write(b'{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}\n')
bridge.stdin.flush()
answer = bridge.stdout.readline()
bridge.stdin.close()
bridge.wait()
if b'"protocolVersion":"2025-06-18"' not in answer:
    sys.exit(f"the server Claude Code got answered {answer!r}: {bridge.stderr.read()!r}")
PY
    [ "$(grep -c '^browser: agent claude-code hello from check$' "$log")" -gt "$hellos" ] ||
      fail "the server Claude Code got didn't take its token as Claude Code's"
    settings_tab General
    settings_tab Agents
    ../tools/window-screenshot.sh "$pid" "$out/connected-$2.png" || fail "no window to capture"
    send clickkey $(drawn "disconnect claude-code")
    expect '^browser: agent claude-code disconnected$' "a click on Disconnect didn't disconnect Claude Code"
    calls_reach "$calls" 3 || fail "Disconnect didn't run claude"
    [ "$(tail -1 "$calls")" = "[\"mcp\",\"remove\",\"--scope\",\"user\",\"$name\"]" ] ||
      fail "Disconnect ran claude with $(tail -1 "$calls")"
    settings_tab General
    settings_tab Agents
  }
  for install in "$native/claude.exe" "$winget/claude.exe" "$npm/claude.cmd"; do
    mkdir -p "$(dirname "$install")"
    cp "$out/$(basename "$install")" "$install"
    [ "${install%.cmd}" = "$install" ] || cp "$out/claude-calls.py" "$(dirname "$install")/"
    case $install in
      "$native"/*) connect_with "$install" native ;;
      "$winget"/*) connect_with "$install" winget ;;
      *) connect_with "$install" npm ;;
    esac
    rm -rf "$(dirname "$install")"
  done
  # Claude Code uninstalled while connected: Disconnect ends its token, and says it couldn't reach
  # claude to take the server out, and Connect says it isn't installed.
  mkdir -p "$native"
  cp "$out/claude.exe" "$native/"
  send clickkey $(drawn "connect claude-code")
  expect_late '^browser: agent claude-code connected$' "Connect with claude.exe didn't connect"
  rm -rf "$native"
  settings_tab General
  settings_tab Agents
  send clickkey $(drawn "disconnect claude-code")
  expect '^browser: agent claude-code disconnected$' "a click on Disconnect didn't disconnect Claude Code"
  expect_late "^browser: agent claude-code config kept: Claude Code's claude command isn't installed\.\$" \
    "Disconnect with claude gone didn't say it isn't installed"
  settings_tab General
  settings_tab Agents
  send clickkey $(drawn "connect claude-code")
  expect_late "^browser: agent claude-code couldn't connect: Claude Code's claude command isn't installed\.\$" \
    "Connect with no claude didn't say it isn't installed"
  settings_tab General
  settings_tab Agents
  ../tools/window-screenshot.sh "$pid" "$out/not-installed.png" || fail "no window to capture"
  quit
  rm -f "$agents_file"
  rm -rf "$home"
  connected="; on Windows, Connect finds Claude Code's claude off the PATH where its installer, winget or npm put it, gives it the server whole, which runs as Claude Code's, Disconnect takes it out, and with claude gone the row says it isn't installed"
fi
if [ "$driven" = 1 ]; then
  echo "ok: $made; an agent's Space shows and switches as the user's, keeps the user's tabs out, refuses an agent ad blocking, passwords, DevTools, a profile's settings and any action agents lack, with why in the activity log, gets a tab the user gives it until they move it back, clears its mark as each session ends, and comes back the agent's$connected"
else
  echo "ok: $made; an agent's Space comes back the agent's$connected. skip: switching to it and its tab. Browser wasn't the active app, so the page ignored the click."
fi
