# A Space the driver makes for an agent, through the core as agent code will, is never the user's
# to see (#1080): the window keeps it apart, as a task's, out of the switcher, the Spaces menu and
# ⌃1 to ⌃9, and the agent's work shows on its card instead. With the Spaces menu open on the Mac,
# making it puts no menu bar up under the menu. The agent may act only on the tabs of its own
# Space: not on the user's. The core refuses an agent ad blocking, passwords, DevTools, a
# profile's settings and any action agents don't have, whatever the tab, with why in the activity
# log, and none starts a session. A task's Space, an agent's whose id starts with task-, is never
# listed either. After a quit, neither comes back as a Space of the user's.
# Settings › Agents' Connect finds Claude Code's claude where a terminal would, though the browser's
# PATH doesn't have it: on Windows where its own installer, winget or npm put it (#1420), on the Mac
# in ~/.local/bin, which only ~/.zshrc puts on PATH (#1441), run with that PATH, as npm's claude
# needs it to find node. It gives claude the browser's server whole, which runs and reaches the
# browser as Claude Code. Disconnect takes it out again. With claude gone, the row says so in words.
# Each claude is a stand-in that records its arguments, in a home folder of the run's own, never the
# user's.
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
# On the Mac the Space is made with the Spaces menu open, as AppKit can crash when the main menu is
# replaced under an open menu. Linux shows no menu bar.
mac=
made="a Space made for an agent stays out of the Spaces menu"
if [ "$(uname)" = Darwin ]; then
  mac=1
  made="a Space made for an agent with the Spaces menu open stays out of the menu bar"
  post openmenu Spaces
  expect '^browser: menu tracked New Space…' "the Spaces menu didn't open"
fi
post agentspace claude-code Claude Code
expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"
agent=$(grep '^browser: made space' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: task space $agent Claude Code\$" "the window didn't keep the agent's Space apart"
grep -q "^browser: added space $agent " "$log" && fail "the agent's Space reached the switcher"
grep '^browser: spaces ' "$log" | tail -1 | grep -qw "$agent" && fail "the window lists the agent's Space"
if [ -n "$mac" ]; then
  none_since '^browser: menu bar put up' || fail "the menu bar was put up with the menu still open"
  post dismiss
fi
send agentspace task-1 Find a flight
expect '^browser: made space [1-9][0-9]* for agent task-1$' "the driver didn't make the task's Space"
task=$(grep '^browser: made space .* for agent task-1$' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: task space $task Find a flight\$" "the window didn't keep the task's Space apart"
grep -q "^browser: added space $task " "$log" && fail "the task's Space reached the switcher"
grep '^browser: spaces ' "$log" | tail -1 | grep -qw "$task" && fail "the window lists the task's Space"
send menubar Spaces
expect '^browser: menu bar Spaces: ' "the Spaces menu didn't come"
grep '^browser: menu bar Spaces: ' "$log" | tail -1 | grep -q 'Claude Code' &&
  fail "the Spaces menu lists the agent's Space"
grep '^browser: menu bar Spaces: ' "$log" | tail -1 | grep -q 'Find a flight' &&
  fail "the Spaces menu lists the task's Space"
grep -q "^browser: agent space $first " "$log" && fail "the user's Space is an agent's"
send agentmayact claude-code "$user_tab"
expect "^browser: agent claude-code may act on $user_tab: 0\$" "the agent may act on the user's tab"
expect "^browser: agent activity claude-code $user_tab navigate refused: The tab isn't in the agent's own Space.\$" \
  "the refusal didn't reach the activity log"
# The core refuses an agent ad blocking, passwords, DevTools and a profile's settings, and any
# action agents don't have, whatever the tab, each with why in the activity log, and none starts a
# session.
refused() {
  send agentmayact claude-code "$user_tab" "$1"
  expect "^browser: agent claude-code may act on $user_tab: 0\$" "the agent may $1"
  expect "^browser: agent activity claude-code $user_tab $1 refused: $2\$" \
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
send click $page_point
driven=$active
if [ "$driven" = 1 ]; then
  # ⌃2 and ⌃3 have no second Space to show.
  send key 2 ctrl
  none_since "^browser: \(task \)\{0,1\}space $agent" || fail "⌃2 showed the agent's Space"
  send key 3 ctrl
  none_since "^browser: \(task \)\{0,1\}space $task" || fail "⌃3 showed the task's Space"
  [ "$(space)" = "$first" ] || fail "the window left the user's Space for $(space)"
fi
quit

# Launched again, the user's Space comes back alone: quit deletes the agent's Space, as it does a
# task's, or the launch does, as for one a crash left.
log="$out/agents-restored.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: spaces [0-9]*$' || fail "the relaunch didn't bring back the user's Space alone"
first=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3)
grep -q "^browser: added space [0-9]* Claude Code\$" "$log" &&
  fail "the agent's Space came back as a Space of the user's"
grep -q "^browser: agent space $first " "$log" && fail "the user's Space came back an agent's"
grep -q '^browser: added space [0-9]* Find a flight$' "$log" &&
  fail "the task's Space came back as a Space of the user's"
quit
connected=
if windows || [ -n "$mac" ]; then
  log="$out/agents-connect.log"
  step=0
  rm -f "$saved" "$launch_frame"
  # The home the app reads Claude Code's folders from, with ~/.claude, which lists its row.
  home="$PWD/$out/claude-home"
  rm -rf "$home"
  mkdir -p "$home/.claude"
  native="$home/.local/bin"
  # What each stand-in for claude runs: it writes each call's arguments, as the system hands a
  # program them, in claude-calls beside the program, a JSON array a line without spaces.
  python=$(python3 -I -c 'import sys; print(sys.executable)')
  printf '%s\n' 'import json, os, sys' \
    'with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "claude-calls"), "a", encoding="utf-8") as calls:' \
    '    calls.write(json.dumps(sys.argv[1:], separators=(",", ":")) + "\n")' > "$out/claude-calls.py"
fi
if windows; then
  winget="$home/AppData/Local/Microsoft/WinGet/Links"
  npm="$home/AppData/Roaming/npm"
  # The stand-in for Claude Code's claude.exe, which records its calls as claude-calls.py does.
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
  printf '%s\n' '@ECHO off' 'GOTO start' ':find_dp0' 'SET dp0=%~dp0' 'EXIT /b' ':start' 'SETLOCAL' \
    'CALL :find_dp0' "SET \"_prog=$python\"" \
    'endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\claude-calls.py" %*' |
    sed 's/$/\r/' > "$out/claude.cmd"
  standin="$out/claude.exe"
  bridge=$(cygpath -w "$app/mcp-bridge.exe")
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
elif [ -n "$mac" ]; then
  # Claude Code's installer has the user put ~/.local/bin on PATH in ~/.zshrc, which a login
  # shell that isn't interactive doesn't read (#1441). The stand-in is npm's kind, a script run by
  # `#!/usr/bin/env` with a node only that PATH has too, so claude has to get it.
  node="$home/.node/bin"
  mkdir -p "$node"
  printf '#!/bin/sh\nexec "%s" -I "$@"\n' "$python" > "$node/ilo-check-node"
  { echo '#!/usr/bin/env ilo-check-node'; cat "$out/claude-calls.py"; } > "$out/claude"
  chmod +x "$node/ilo-check-node" "$out/claude"
  echo 'export PATH="$HOME/.local/bin:$HOME/.node/bin:$PATH"' > "$home/.zshrc"
  standin="$out/claude"
  bridge="$(dirname "$browser")/mcp-bridge"
  agents_file="$(dirname "$saved")/Agents"
  printf '{"on":true,"agents":[]}' > "$agents_file"
  # The PATH the Dock and Finder give an app, and zsh's files from the run's home.
  HOME="$home" ZDOTDIR="$home" SHELL=/bin/zsh PATH=/usr/bin:/bin:/usr/sbin:/sbin \
    BROWSER_PROFILE_DIR="${BROWSER_PROFILE_DIR:-$profile_dir}" BROWSER_DRIVE=1 \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: agents server on /.*/Agents\.sock$' || fail "the saved switch didn't start the server"
fi
if windows || [ -n "$mac" ]; then
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
    python3 -I - "$calls" "$name" "$bridge" "$socket" << 'PY' || fail "Claude Code didn't get the server whole from $1"
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
  if windows; then installs="native winget npm"; else installs=native; fi
  for kind in $installs; do
    case $kind in
      native) install="$native/$(basename "$standin")" ;;
      winget) install="$winget/claude.exe" ;;
      npm) install="$npm/claude.cmd" ;;
    esac
    mkdir -p "$(dirname "$install")"
    cp "$out/$(basename "$install")" "$install"
    [ "${install%.cmd}" = "$install" ] || cp "$out/claude-calls.py" "$(dirname "$install")/"
    connect_with "$install" "$kind"
    rm -rf "$(dirname "$install")"
  done
  # Claude Code uninstalled while connected: Disconnect ends its token, and says it couldn't reach
  # claude to take the server out, and Connect says it isn't installed.
  mkdir -p "$native"
  cp "$standin" "$native/"
  send clickkey $(drawn "connect claude-code")
  expect_late '^browser: agent claude-code connected$' "Connect with $(basename "$standin") didn't connect"
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
  if windows; then
    connected="; on Windows, Connect finds Claude Code's claude off the PATH where its installer, winget or npm put it, gives it the server whole, which runs as Claude Code's, Disconnect takes it out, and with claude gone the row says it isn't installed"
  else
    connected="; Connect finds Claude Code's claude in ~/.local/bin, on the PATH only through ~/.zshrc, runs it with that PATH, gives it the server whole, which runs as Claude Code's, Disconnect takes it out, and with claude gone the row says it isn't installed"
  fi
fi
if [ "$driven" = 1 ]; then
  echo "ok: $made, the switcher and ⌃1 to ⌃9, as a task's does, refuses an agent the user's tab, ad blocking, passwords, DevTools, a profile's settings and any action agents lack, with why in the activity log, and comes back as no Space of the user's$connected"
else
  echo "ok: $made and the switcher, as a task's does, refuses an agent the user's tab and what agents lack, with why in the activity log, and comes back as no Space of the user's$connected. skip: ⌃2 and ⌃3. Browser wasn't the active app, so the page ignored the click."
fi
