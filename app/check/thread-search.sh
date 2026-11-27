# check.sh runs this at 85.
# A thread searches with the search engine of its Space's profile, whichever Space the window
# shows: one asked from a Space in a second profile, which picked Microsoft Bing, searches Bing
# while the window shows the first profile's Space, and one asked from that Space searches
# DuckDuckGo while the window shows the second's. Each thread runs on a stand-in for claude, found
# where Claude Code's installer puts it, in a home of the run's own, never a real account: it
# searches with the browser's own tool once the run says go. A core from before
# bc_profile_search_url searches by the window's Space, which the app says as it starts, and the
# run skips. The Mac's, as the stand-in thread runs there.

if [ "$(uname)" != Darwin ]; then
  echo "skip: a thread's search by its Space's profile. The stand-in thread runs on the Mac."
  exit 0
fi
log="$out/thread-search.log"
step=0
rm -f "$saved" "$launch_frame"
home="$PWD/$out/thread-search-home"
rm -rf "$home"
mkdir -p "$home/.local/bin"
python=$(python3 -I -c 'import sys; print(sys.executable)')
# The stand-in: signed in, nothing for the models ask, and on a task the words its first message
# asks for after "Search for ", searched once go-<words> is beside it. The page load proves the
# search, as a runner may never get an engine's answer (#1875), so the turn ends as the run quits.
cat > "$home/.local/bin/claude.py" << 'EOF'
import json, os, subprocess, sys, time
here = os.path.dirname(os.path.abspath(__file__))
args = sys.argv[1:]
def say(message):
    print(json.dumps(message), flush=True)
if args == ["auth", "status", "--json"]:
    say({"loggedIn": True, "authMethod": "check"})
    sys.exit(0)
if "--mcp-config" not in args:
    sys.exit(0)
with open(args[args.index("--mcp-config") + 1], encoding="utf-8") as file:
    (server,) = json.load(file)["mcpServers"].values()
first = None
for line in sys.stdin:
    message = json.loads(line)
    if message.get("type") == "user":
        first = message["message"]["content"]
        break
words = first.split("\n")[0].removeprefix("Search for ")
slug = words.replace(" ", "-")
bridge = subprocess.Popen([server["command"], *server.get("args", [])], env={**os.environ, **server.get("env", {})},
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
def ask(id, method, params):
    bridge.stdin.write(json.dumps({"jsonrpc": "2.0", "id": id, "method": method, "params": params}) + "\n")
    bridge.stdin.flush()
    for line in bridge.stdout:
        answer = json.loads(line)
        if answer.get("id") == id:
            return answer
ask(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "check"}})
bridge.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
bridge.stdin.flush()
go = os.path.join(here, f"go-{slug}")
for _ in range(600):
    if os.path.exists(go):
        break
    time.sleep(0.1)
ask(2, "tools/call", {"name": "search", "arguments": {"words": words}})
say({"type": "assistant", "message": {"content": [{"type": "text", "text": "Done, from the stand-in."}]}})
say({"type": "result", "subtype": "success", "is_error": False, "result": "Done"})
for line in sys.stdin:
    pass
EOF
printf '#!/bin/sh\nexec "%s" -I "$(dirname "$0")/claude.py" "$@"\n' "$python" > "$home/.local/bin/claude"
chmod +x "$home/.local/bin/claude"
echo 'export PATH="$HOME/.local/bin:$PATH"' > "$home/.zshrc"
agents_file="$(dirname "$saved")/Agents"
printf '{"on":true,"agents":[]}' > "$agents_file"
# The PATH the Dock and Finder give an app, and zsh's files from the run's home.
HOME="$home" ZDOTDIR="$home" SHELL=/bin/zsh PATH=/usr/bin:/bin:/usr/sbin:/sbin BROWSER_DRIVE=1 \
  BROWSER_URL='data:text/html,<title>thread search</title>' "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: core searches by ' || fail "the app didn't say how the core searches"
if grep -q '^browser: core searches by window$' "$log"; then
  exec 3>&-
  quit
  rm -rf "$home" "$agents_file"
  echo "skip: a thread's search by its Space's profile. The linked core has no bc_profile_search_url, which build 14's has, so threads search by the window's Space."
  exit 0
fi
first=$(space)
default=$(sed -n "s/^browser: profiled space $first \([0-9]*\)\$/\1/p" "$log" | tail -1)
# A second Space, empty, which the command bar's New Profile puts in a new profile.
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
send type work
send key return
expect "^browser: create space $second work\$" "return didn't name the Space"
expect '^browser: space form gone$' "the form didn't leave after return"
bar_command change-profile "New Profile" "new profile"
expect '^browser: dialog shown New Profile$' "the command bar's New Profile didn't ask for its name"
send key return
expect '^browser: create profile [0-9]* Profile 2$' "New Profile didn't make a profile"
work=$(sed -n 's/^browser: create profile \([0-9]*\) .*/\1/p' "$log" | tail -1)
expect "^browser: profiled space $second $work\$" "the second Space didn't go into the new profile"
# The second profile searches with Microsoft Bing, the first keeping DuckDuckGo.
send menu Settings…
settings_on_general "Settings… didn't show Settings"
post clickkey $(drawn "scope")
expect '^browser: menu shown Default (checked), .*Profile 2' "General's chip didn't list the profiles"
send choose "Profile 2"
expect "^browser: settings scope profile $work\$" "picking Profile 2 from the chip didn't show its settings"
post clickkey $(drawn "search engine $work")
expect '^browser: menu shown .*Microsoft Bing' "the second profile's Search engine didn't show the engines"
send choose Microsoft Bing
expect "^browser: profile $work searches with Microsoft Bing\$" "choosing Bing didn't set the second profile's"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
expect_late "^browser: task agent claude is SignedIn at $home/.local/bin/claude\$" \
  "the stand-in claude wasn't found signed in: $(grep '^browser: task agent claude ' "$log" | tail -1)"
# search <words> <Space it's asked from> <Space then shown> <⌃ key for it> <engine's search URL>:
# a thread asked from the one searches for words with the engine of its profile while the window
# shows the other.
search() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "Search for $1"
  send key tab
  expect '^browser: command bar asks an agent$' "⇥ didn't make what was typed a task"
  send key return
  expect '^browser: command bar runs a task on claude$' "↩ didn't run the task on claude"
  expect_late "^browser: task task-[0-9]* on claude asked from space $2\$" "the task from Space $2 didn't start"
  task=$(sed -n "s/^browser: task \(task-[0-9]*\) on claude asked from space $2\$/\1/p" "$log" | tail -1)
  send key "$4" ctrl
  expect "^browser: space $3 in " "⌃$4 didn't show Space $3"
  slug=$(echo "$1" | tr ' ' '-')
  words=$(echo "$1" | tr ' ' '+')
  touch "$home/.local/bin/go-$slug"
  expect_late "^browser: page [0-9]* back [0-9]* forward [0-9]* loading [01] $5$words" \
    "the thread from Space $2 didn't search with its profile's engine: $(grep "^browser: page .*$words" "$log" | tail -1)"
  grep "^browser: page .*q=$words" "$log" | grep -v "loading [01] $5$words" &&
    fail "the thread from Space $2 searched with another engine"
  expect_late "^browser: agent logged $task search *\$" "the thread's search wasn't in the activity log"
}
search "night trains" "$second" "$first" 1 'https://www\.bing\.com/search?q='
search "sleeper trains" "$first" "$second" 2 'https://duckduckgo\.com/?q='
../tools/window-screenshot.sh "$pid" "$out/thread-search.png" || fail "no window to capture"
# Closing the last window quits, and each task ends without the browser crashing on its Space.
quit
[ "$(grep -c '^browser: task task-[0-9]* dropped$' "$log")" = 2 ] ||
  fail "quitting didn't end both tasks: $(grep '^browser: task ' "$log" | tail -2)"
rm -rf "$home" "$agents_file"
echo "ok: a thread asked from a Space searches with its profile's engine, Microsoft Bing in the second profile's while the window shows the first's, and DuckDuckGo in the first's while it shows the second's, each in the activity log."
