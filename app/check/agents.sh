# check.sh runs this at 80.
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
# user's. On Windows the task agents find claude and codex as each installer puts them, and read
# them signed in, with no console window shown (#1676).

log="$out/agents.log"
step=0
rm -f "$saved" "$launch_frame"
# Library › Agents keeps the profile's past tasks on this computer, 30 days from their end (#1080):
# the run's, a task that ended an hour ago and one that ended 30 days ago, in the first profile's
# own folder, as the browser keeps them. The hour-old one's page lists its steps, one refused, and
# its site, where Claude Code may always act, as the agents' settings keep it, which Revoke takes.
history_file="$profile_dir/Default/Agent sessions"
mkdir -p "$profile_dir/Default"
python3 -I - "$history_file" << 'PY'
import json, sys, time
now = int(time.time())
def past(id, title, ago, orders):
    def step(at, action, site, refused=""):
        return {"at": now - ago - at, "action": action, "site": site, "refused": refused}
    return {"id": id, "title": title, "agent": "Claude Code", "started": now - ago - 300,
            "ended": now - ago,
            "pages": [{"title": "Booking", "url": "https://check-allowed.example/booking"}],
            "orders": orders, "messages": 0, "answers": "claude-code",
            "steps": [step(240, "open_tab", "check-allowed.example"),
                      step(180, "click", "check-allowed.example"),
                      step(120, "passwords", "",
                           "Agents never read passwords, cookies or autofill data.")]}
with open(sys.argv[1], "w", encoding="utf-8") as file:
    for entry in (past("check-new", "Book a table for four", 3600, 1),
                  past("check-old", "Compare the museum passes", 30 * 24 * 3600, 0)):
        file.write(json.dumps(entry) + "\n")
PY
agents_file="$(dirname "$saved")/Agents"
printf '{"on":false,"agents":[],"grants":{"claude-code":{"check-allowed.example":"always"}}}' > "$agents_file"
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
  expect '^browser: command select-space via page$' "⌃2 didn't reach the UI"
  none_since "^browser: \(task \)\{0,1\}space $agent" || fail "⌃2 showed the agent's Space"
  send key 3 ctrl
  expect '^browser: command select-space via page$' "⌃3 didn't reach the UI"
  none_since "^browser: \(task \)\{0,1\}space $task" || fail "⌃3 showed the task's Space"
  [ "$(space)" = "$first" ] || fail "the window left the user's Space for $(space)"
fi
# Window › View Agents… lists them, and the one 30 days old has gone, from the file too.
send menu 'View Agents…'
expect '^browser: command view-agents via ui$' "Window › View Agents… didn't run"
expect '^browser: library section agents$' "View Agents… didn't open the Library on Agents"
await '^browser: library shown$' || fail "the Library didn't settle open on Agents"
../tools/window-screenshot.sh "$pid" "$out/library-agents.png" || fail "no window to capture"
grep -q 'check-new' "$history_file" || fail "the hour-old past task went: $(cat "$history_file")"
grep -q 'check-old' "$history_file" && fail "the 30-day-old past task stayed: $(cat "$history_file")"
# A click on its row opens its page: its site, where Claude Code may always act, with Revoke, and its
# steps. Revoke there takes the answer, as Settings › Agents' does, and Back goes to the list.
session_page=
if [ "$driven" = 1 ]; then
  at=$(drawn 'library past check-new') || fail "the past task's row wasn't drawn"
  send click $at
  expect '^browser: agents history shows check-new: 3 steps, 1 sites, answers check-allowed.example allowed$' \
    "the row didn't open its page: $(grep '^browser: agents history' "$log" | tail -1)"
  at=$(drawn 'library revoke check-allowed.example') || fail "the page drew no Revoke"
  ../tools/window-screenshot.sh "$pid" "$out/library-agents-session.png" || fail "no window to capture"
  send click $at
  expect '^browser: agent claude-code revoked check-allowed.example$' "Revoke didn't take the answer"
  expect '^browser: agents history shows check-new: 3 steps, 1 sites, answers none$' \
    "the page still has the answer Revoke took"
  grep -q 'check-allowed' "$agents_file" && fail "the agents' settings kept the answer: $(cat "$agents_file")"
  at=$(drawn 'library back') || fail "the page drew no Back"
  send click $at
  expect '^browser: agents history lists$' "Back didn't go back to the list"
  session_page=", and a past task's page lists its steps and its site, whose answer for good its Revoke takes"
fi
send menu 'View Agents…'
expect '^browser: library hidden$' "View Agents… didn't close the Library"
rm -f "$history_file" "$agents_file"
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
    click_in_sight "connect claude-code"
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
    click_in_sight "disconnect claude-code"
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
  click_in_sight "connect claude-code"
  expect_late '^browser: agent claude-code connected$' "Connect with $(basename "$standin") didn't connect"
  rm -rf "$native"
  settings_tab General
  settings_tab Agents
  click_in_sight "disconnect claude-code"
  expect '^browser: agent claude-code disconnected$' "a click on Disconnect didn't disconnect Claude Code"
  expect_late "^browser: agent claude-code config kept: Claude Code's claude command isn't installed\.\$" \
    "Disconnect with claude gone didn't say it isn't installed"
  settings_tab General
  settings_tab Agents
  click_in_sight "connect claude-code"
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
# On Windows the task agents find claude and codex however each was installed, off the browser's
# PATH, and read that each is signed in (#1676): Claude Code's claude.exe and npm's codex.cmd, then
# npm's claude.cmd and winget's codex.exe, each a stand-in that answers the status calls and lists
# a model of its own, never a real account. No console window shows meanwhile, which a watcher of
# every window Windows shows sees, once it has seen the one it opens itself.
tasks_found=
if windows; then
  home="$PWD/$out/task-home"
  rm -rf "$home"
  python=$(python3 -I -c 'import sys; print(sys.executable)')
  # What a stand-in answers, as the CLI named first, which is the kind of program it is second:
  # claude's and codex's status, codex's app-server's model list, with a model named for the kind,
  # and a task on either, whose instructions and first message it writes beside itself, as
  # task-<cli>.json, before it answers and ends its turn.
  cat > "$out/task-cli.py" << 'EOF'
import json, os, sys
cli, kind, args = sys.argv[1], sys.argv[2], sys.argv[3:]
here = os.path.dirname(os.path.abspath(__file__))
def record(instructions, first):
    with open(os.path.join(here, f"task-{cli}.json"), "w", encoding="utf-8") as file:
        json.dump({"kind": kind, "args": args, "instructions": instructions, "first": first}, file)
def say(message):
    print(json.dumps(message), flush=True)
if cli == "claude" and args == ["auth", "status", "--json"]:
    say({"loggedIn": True, "authMethod": "check"})
elif cli == "codex" and args == ["login", "status"]:
    print("Logged in using ChatGPT")
elif cli == "claude" and "-p" in args:
    at = args.index("--append-system-prompt-file") + 1
    with open(args[at], encoding="utf-8") as file:
        instructions = file.read()
    first = json.loads(sys.stdin.readline())["message"]["content"]
    record(instructions, first)
    say({"type": "assistant", "message": {"content": [{"type": "text", "text": f"Done, from the {kind} stand-in."}]}})
    say({"type": "result", "subtype": "success", "is_error": False, "result": "Done"})
    for line in sys.stdin:
        pass
elif cli == "codex" and args == ["app-server"]:
    instructions = None
    for line in sys.stdin:
        message = json.loads(line)
        method, id = message.get("method"), message.get("id")
        if method == "initialize":
            say({"id": id, "result": {}})
        elif method == "model/list":
            say({"id": id, "result": {"data": [{"id": f"check-{kind}", "displayName": f"Check {kind}"}]}})
        elif method == "thread/start":
            instructions = message["params"].get("developerInstructions")
            say({"id": id, "result": {"thread": {"id": "check-thread"}}})
        elif method == "turn/start":
            record(instructions, message["params"]["input"][0]["text"])
            say({"id": id, "result": {"turn": {"id": "check-turn"}}})
            say({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": f"Done, from the {kind} stand-in."}}})
            say({"method": "turn/completed", "params": {"turn": {"id": "check-turn", "status": "completed"}}})
else:
    sys.exit(2)
EOF
  # The .exe kind: a program of its own, as CreateProcess starts it, that runs the stand-in with
  # the Python beside it in task-cli.python, its input and output its own.
  cat > "$out/task-cli.rs" << 'EOF'
fn main() {
    let exe = std::env::current_exe().unwrap();
    let cli = exe.file_stem().unwrap().to_string_lossy().to_lowercase();
    let python = std::fs::read_to_string(exe.with_file_name("task-cli.python")).unwrap();
    let status = std::process::Command::new(python.trim())
        .arg(exe.with_file_name("task-cli.py"))
        .args([cli.as_str(), "exe"])
        .args(std::env::args_os().skip(1))
        .status()
        .unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
EOF
  rustc -O -o "$out/task-cli.exe" "$out/task-cli.rs" || fail "rustc couldn't build the stand-in CLI"
  printf '%s' "$python" > "$out/task-cli.python"
  # An npm shim for the CLI $1, as npm writes one, that runs the Python stand-in.
  npm_shim() {
    printf '%s\n' '@ECHO off' 'GOTO start' ':find_dp0' 'SET dp0=%~dp0' 'EXIT /b' ':start' 'SETLOCAL' \
      'CALL :find_dp0' "SET \"_prog=$python\"" \
      "endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\task-cli.py\" $1 cmd %*" |
      sed 's/$/\r/'
  }
  # Every console window Windows shows, from a WinEvent hook, written to $2 once $1 is there. It
  # first opens one of its own and writes "ready" to $3 once it has seen it.
  cat > "$out/consoles.py" << 'EOF'
import ctypes, ctypes.wintypes as w, os, subprocess, sys, time
stop, report, ready = sys.argv[1:]
user32, kernel32 = ctypes.windll.user32, ctypes.windll.kernel32
EVENT_OBJECT_SHOW, OBJID_WINDOW, CREATE_NEW_CONSOLE = 0x8002, 0, 0x10
user32.SetWinEventHook.restype = w.HANDLE
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
shown = []
def name_of(pid):
    process = kernel32.OpenProcess(0x1000, False, pid)
    if not process:
        return "?"
    name, size = ctypes.create_unicode_buffer(1024), w.DWORD(1024)
    found = kernel32.QueryFullProcessImageNameW(process, 0, name, ctypes.byref(size))
    kernel32.CloseHandle(process)
    return os.path.basename(name.value) if found else "?"
Hook = ctypes.WINFUNCTYPE(None, w.HANDLE, w.DWORD, w.HWND, w.LONG, w.LONG, w.DWORD, w.DWORD)
def shows(hook, event, hwnd, object, child, thread, when):
    if object != OBJID_WINDOW or not hwnd:
        return
    kind = ctypes.create_unicode_buffer(64)
    user32.GetClassNameW(hwnd, kind, 64)
    if kind.value == "ConsoleWindowClass":
        pid = w.DWORD()
        user32.GetWindowThreadProcessId(w.HWND(hwnd), ctypes.byref(pid))
        shown.append(f"{hwnd} {pid.value} {name_of(pid.value)}")
hook_function = Hook(shows)
if not user32.SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, 0, hook_function, 0, 0, 0):
    sys.exit("no WinEvent hook")
message = w.MSG()
def pump(done, seconds):
    end = time.time() + seconds
    while not done() and time.time() < end:
        while user32.PeekMessageW(ctypes.byref(message), 0, 0, 0, 1):
            user32.TranslateMessage(ctypes.byref(message))
            user32.DispatchMessageW(ctypes.byref(message))
        time.sleep(0.01)
own = subprocess.Popen(["cmd", "/c", "ping -n 3 127.0.0.1 > nul"], creationflags=CREATE_NEW_CONSOLE)
pump(lambda: shown, 20)
own.wait()
seen_own = len(shown)
shown.clear()
with open(ready, "w", encoding="utf-8") as file:
    file.write(f"ready {seen_own}\n")
pump(lambda: os.path.exists(stop), 600)
pump(lambda: False, 1)
with open(report, "w", encoding="utf-8") as file:
    file.write("".join(line + "\n" for line in shown))
EOF
  rm -f "$out/consoles-stop" "$out/consoles-ready" "$out/consoles"
  "$python" -I "$out/consoles.py" "$(cygpath -w "$out/consoles-stop")" "$(cygpath -w "$out/consoles")" \
    "$(cygpath -w "$out/consoles-ready")" &
  watcher=$!
  for _ in $(seq 300); do
    [ -s "$out/consoles-ready" ] && break
    kill -0 "$watcher" 2>/dev/null || break
    sleep 0.1
  done
  grep -q '^ready [1-9]' "$out/consoles-ready" 2>/dev/null ||
    fail "the console watcher didn't see the console window it opened: $(cat "$out/consoles-ready" 2>/dev/null)"
  agents_file="$(dirname "$saved")/Agents"
  native="$home/.local/bin"
  winget="$home/AppData/Local/Microsoft/WinGet/Links"
  npm="$home/AppData/Roaming/npm"
  # Launches the browser with the CLIs $1 and $2, at the paths given, and expects each signed in
  # there, and Codex's model $3 listed.
  task_clis() {
    log="$out/agents-tasks-$3.log"
    step=0
    rm -rf "$home" "$agents_file" "$saved" "$launch_frame"
    mkdir -p "$home/.claude" "$(dirname "$1")" "$(dirname "$2")"
    for cli in "$1" "$2"; do
      case $cli in
        *.exe) cp "$out/task-cli.exe" "$cli" ;;
        *.cmd) npm_shim "$(basename "$cli" .cmd)" > "$cli" ;;
      esac
      cp "$out/task-cli.py" "$out/task-cli.python" "$(dirname "$cli")/"
    done
    printf '{"on":true,"agents":[]}' > "$agents_file"
    # The PATH stays the run's, which has none of these folders.
    USERPROFILE=$(cygpath -w "$home") LOCALAPPDATA=$(cygpath -w "$home/AppData/Local") \
      APPDATA=$(cygpath -w "$home/AppData/Roaming") \
      BROWSER_PROFILE_DIR=$(cygpath -w "${BROWSER_PROFILE_DIR:-$profile_dir}") BROWSER_DRIVE=1 \
      "$browser" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await '^browser: agents server on ' || fail "the saved switch didn't start the server"
    send menu Settings…
    expect '^browser: settings shown ' "Settings… didn't show Settings"
    settings_drew
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its tab"
    for cli in "$1" "$2"; do
      task_cli=$(basename "$cli"); task_cli=${task_cli%.*}
      line="browser: task agent $task_cli is SignedIn at $(cygpath -w "$cli")"
      for _ in $(seq 300); do
        grep -qxF "$line" "$log" && break
        sleep 0.1
      done
      grep -qxF "$line" "$log" ||
        fail "the task agent $task_cli wasn't found signed in at $cli: $(grep "^browser: task agent $task_cli " "$log" | tail -1)"
    done
    for _ in $(seq 300); do
      grep -q "\"$3\"" "$agents_file" 2>/dev/null && break
      sleep 0.1
    done
    grep -q "\"$3\"" "$agents_file" || fail "Codex at $2 didn't list its model $3: $(cat "$agents_file")"
    ../tools/window-screenshot.sh "$pid" "$out/task-agents-$3.png" || fail "no window to capture"
    send key w cmd
    expect '^browser: settings hidden$' "⌘W didn't hide Settings"
    # A task from ⌘T on each, Claude first, as the agent listed first, then Codex, below it.
    for cli in "$1" "$2"; do
      task_cli=$(basename "$cli"); task_cli=${task_cli%.*}
      asked="Find a flight on $task_cli"
      send key t cmd
      expect '^browser: command bar shown$' "⌘T didn't open the command bar"
      send type "$asked"
      send key tab
      expect '^browser: command bar asks an agent$' "⇥ didn't make what was typed a task"
      expect '^browser: command bar agents: Claude, Codex$' "⇥ didn't list Claude and Codex, signed in"
      [ "$task_cli" = codex ] && send key down
      send key return
      expect "^browser: command bar runs a task on $task_cli\$" "↩ didn't run the task on $task_cli"
      expect_late "^browser: task task-[0-9]* on $task_cli asked from space " "the task on $task_cli didn't start"
      task=$(sed -n "s/^browser: task \(task-[0-9]*\) on $task_cli asked from space .*/\1/p" "$log" | tail -1)
      recorded="$(dirname "$cli")/task-$task_cli.json"
      for _ in $(seq 300); do
        [ -s "$recorded" ] && break
        grep -q "^browser: task $task \(couldn.t start\|failed\)" "$log" && break
        sleep 0.1
      done
      grep "^browser: task $task \(couldn.t start\|failed\|ended\)" "$log" &&
        fail "the task on $task_cli at $cli didn't run: $(grep "^browser: task $task " "$log" | tail -1)"
      python3 -I - "$recorded" "$asked" << 'PY' || fail "the task's CLI at $cli didn't get what it's told whole"
import json, sys
path, asked = sys.argv[1:]
with open(path, encoding="utf-8") as file:
    task = json.load(file)
instructions = task.get("instructions") or ""
if not instructions.startswith("You do a task the user asked in") or "\n\n" not in instructions:
    sys.exit(f"its instructions were {instructions[:200]!r}")
if not task.get("first", "").startswith(asked):
    sys.exit(f"its first message was {task.get('first')!r}")
if any("\n" in arg or "\r" in arg for arg in task.get("args", [])):
    sys.exit(f"an argument had a line break: {task['args']!r}")
PY
      # It answered, which ends its turn.
      sleep 1
      grep "^browser: task $task \(couldn.t start\|failed\|ended\|had no answer\)" "$log" &&
        fail "the task on $task_cli at $cli ended badly: $(grep "^browser: task $task " "$log" | tail -1)"
      ../tools/window-screenshot.sh "$pid" "$out/task-$3-$task_cli.png" || fail "no window to capture"
    done
    # Closing the last window quits, and each task ends without the browser crashing on its Space.
    quit
    [ "$(grep -c '^browser: task task-[0-9]* dropped$' "$log")" = 2 ] ||
      fail "quitting didn't end both tasks: $(grep '^browser: task ' "$log" | tail -2)"
  }
  task_clis "$native/claude.exe" "$npm/codex.cmd" check-cmd
  task_clis "$npm/claude.cmd" "$winget/codex.exe" check-exe
  touch "$out/consoles-stop"
  wait "$watcher" || fail "the console watcher failed"
  [ -s "$out/consoles" ] && fail "console windows showed as the task agents were looked for: $(tr '\n' ';' < "$out/consoles")"
  rm -rf "$home" "$agents_file"
  tasks_found="; on Windows the task agents find claude.exe and codex.cmd, then claude.cmd and codex.exe, off the PATH, each signed in, Codex's models listed, a task from ⌘T starts on each and its CLI gets its instructions whole and the first message, and no console window shows"
fi
if [ "$driven" = 1 ]; then
  echo "ok: $made, the switcher and ⌃1 to ⌃9, as a task's does, refuses an agent the user's tab, ad blocking, passwords, DevTools, a profile's settings and any action agents lack, with why in the activity log, and comes back as no Space of the user's$session_page$connected$tasks_found"
else
  echo "ok: $made and the switcher, as a task's does, refuses an agent the user's tab and what agents lack, with why in the activity log, and comes back as no Space of the user's$connected$tasks_found. skip: ⌃2 and ⌃3, and a past task's page. Browser wasn't the active app, so the page ignored the click."
fi
