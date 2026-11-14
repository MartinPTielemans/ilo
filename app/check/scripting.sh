# check.sh runs this at 500.
# AppleScript (#704), the Mac's alone. The app declares ilo's dictionary (platform/mac/ilo.sdef),
# never Chromium's: no execute, save or view source, and nothing that reads a page. Off at first,
# Settings › Agents' "Let scripts control the browser" refuses every script, reads too, with
# errAEEventNotPermitted, and logs it under its sender. Let in, a script reads the windows, the
# user's Spaces and their tabs, never an agent's Space or a private window; focuses a Space and
# selects a tab as a click does; makes a window as ⌘N does, and a tab loading a page in a window or
# a Space, never a page that isn't http or https. Each Apple Event is one activity-log entry.
# In a check VM with System Integrity Protection off the run lets osascript control the app
# first, as no one answers macOS's Automation prompt there, so a script is answered or refused by
# the app, not held at the prompt; scripting-tcc.txt says what TCC held. Never on a Mac with it on.

if [ "$(uname)" != Darwin ]; then
  echo "skip: scripting: AppleScript is the Mac's alone."
else
  log="$out/scripting-off.log"
  step=0
  rm -f "$saved" "$launch_frame"
  agents_file="$(dirname "$saved")/Agents"
  rm -f "$agents_file"
  serve
  {
    echo "csrutil: $(csrutil status 2>&1)"
    for db in "$HOME/Library/Application Support/com.apple.TCC/TCC.db" \
      "/Library/Application Support/com.apple.TCC/TCC.db"; do
      echo "$db:"
      sqlite3 "$db" "select client, indirect_object_identifier, auth_value from access
        where service = 'kTCCServiceAppleEvents'" 2>&1 || true
    done
  } > "$out/scripting-tcc.txt"
  granted=
  if [ "$(sysctl -n kern.hv_vmm_present)" = 1 ] && csrutil status | grep -q disabled; then
    cat > "$out/scripting-grant.py" << 'EOF'
import sqlite3, sys
db, target = sys.argv[1:]
tcc = sqlite3.connect(db)
names = [column[1] for column in tcc.execute("pragma table_info(access)")]
rows = tcc.execute("select * from access where service = 'kTCCServiceAppleEvents'"
                   " and indirect_object_identifier = 'com.apple.systemevents'").fetchall()
for row in rows:
    row = dict(zip(names, row))
    row["indirect_object_identifier"] = target
    row["indirect_object_code_identity"] = None
    tcc.execute(f"insert or replace into access ({', '.join(names)})"
                f" values ({', '.join('?' * len(names))})", [row[name] for name in names])
tcc.commit()
print(len(rows))
EOF
    granted=$(python3 -I "$out/scripting-grant.py" \
      "$HOME/Library/Application Support/com.apple.TCC/TCC.db" "$app_id") ||
      fail "the VM's TCC database couldn't let osascript control the app"
    echo "granted: $granted clients" >> "$out/scripting-tcc.txt"
    # TCC's daemon reads the database again as it starts, on the next request.
    killall tccd 2> /dev/null || true
  fi

  # What a script gets back, in $answer, and its status, in $status: 0 answered, else the error,
  # or timeout for one still waiting after 30 s, as on macOS's Automation prompt.
  rm -f "$out/scripting-asked.txt"
  ask() {
    osascript -e "tell application id \"$app_id\" to $1" > "$out/scripting-answer.txt" 2>&1 &
    asking=$!
    for _ in $(seq 300); do
      kill -0 "$asking" 2>/dev/null || break
      sleep 0.1
    done
    status=0
    if kill -0 "$asking" 2>/dev/null; then
      kill "$asking" 2>/dev/null || true
      wait "$asking" 2>/dev/null || true
      status=timeout
    else
      wait "$asking" || status=$?
    fi
    answer=$(tr '\n' ' ' < "$out/scripting-answer.txt" | sed 's/ *$//')
    echo "osascript $1: status $status: $answer" >> "$out/scripting-asked.txt"
  }
  # Asks $1 and fails with $2 unless the app answers.
  answered() {
    ask "$1"
    [ "$status" = 0 ] || fail "$2: $answer"
  }

  plist="$app/Contents/Info.plist"
  [ "$(/usr/libexec/PlistBuddy -c 'Print :OSAScriptingDefinition' "$plist" 2>/dev/null)" = ilo.sdef ] ||
    fail "the app's Info.plist doesn't declare ilo's dictionary"
  [ ! -e "$app/Contents/Resources/scripting.sdef" ] || fail "the app has Chromium's scripting.sdef"
  # The bundle's own dictionary, as macOS reads it: ilo.sdef includes nothing, so sdef, which needs
  # the developer tools a check VM hasn't (#2554), would print the same file. The scripts below
  # compile its terms, which shows macOS loads it.
  dictionary="$app/Contents/Resources/ilo.sdef"
  xmllint --noout --nonet "$dictionary" 2> "$out/scripting-sdef.txt" ||
    fail "the app's dictionary, $dictionary, isn't XML that parses: $(head -3 "$out/scripting-sdef.txt")"
  # An include, such as the standard suite's, would bring in terms this file doesn't show, and the
  # check below would pass on a dictionary that has them.
  ! grep -qE 'xi:include|XInclude' "$dictionary" ||
    fail "the app's dictionary, $dictionary, includes another file, so the check for the terms it mustn't have needs the dictionary with its includes resolved, as sdef gives it"
  cp "$dictionary" "$out/scripting.sdef"
  for term in execute save print 'view source' 'copy selection' 'bookmark'; do
    grep -q "name=\"$term\"" "$out/scripting.sdef" && fail "the app's dictionary has $term"
  done

  # Off, as at first.
  BROWSER_URL="$site" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  ask "count windows"
  [ "$status" = timeout ] &&
    fail "osascript was held at macOS's Automation prompt: TCC doesn't let it control the app"
  [ "$status" != 0 ] || fail "a script counted the windows with scripts off: $answer"
  echo "$answer" | grep -q -- "-1743" || fail "with scripts off, count windows wasn't refused as not permitted: $answer"
  echo "$answer" | grep -q "Scripts are off" || fail "the refusal didn't say scripts are off: $answer"
  grep -q '^browser: script read .*by osascript.* refused: scripts are off$' "$log" ||
    fail "the refused read wasn't logged under its sender"
  quit

  # Let in, as Settings › Agents' switch saves it.
  log="$out/scripting.log"
  step=0
  rm -f "$saved" "$launch_frame"
  printf '{"scripts":true}' > "$agents_file"
  BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  first=$(space)
  page=$(today | cut -d' ' -f1)
  send menu New Space…
  expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
  second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
  send type Home
  send key return
  expect "^browser: create space $second Home\$" "the second Space wasn't named Home"
  # An agent's Space, which no script sees.
  send agentspace claude-code Claude Code
  expect '^browser: made space [1-9][0-9]* for agent claude-code$' "the driver didn't make the agent's Space"

  answered "count windows" "a script let in couldn't count the windows"
  [ "$answer" = 1 ] || fail "a script counted $answer windows, not 1"
  answered "get name of every space of window 1" "a script couldn't read the Spaces"
  case "$answer" in
  *Home*) ;;
  *) fail "the Spaces a script read had no Home: $answer" ;;
  esac
  case "$answer" in
  *Claude*) fail "a script read the agent's Space: $answer" ;;
  esac
  answered "count spaces of window 1" "a script couldn't count the Spaces"
  [ "$answer" = 2 ] || fail "a script counted $answer Spaces, not the user's 2"
  answered "get id of active space of window 1" "a script couldn't read the Space shown"
  [ "$answer" = "$second" ] || fail "the Space shown was $answer to a script, not $second"

  answered "focus space 1 of window 1" "focus didn't show the first Space"
  expect "^browser: space $first\\( .*\\)\\{0,1\}\$" "focus didn't show the first Space"
  expect '^browser: script show_space .* by osascript' "focus wasn't logged"
  answered "get URL of tab 1 of window 1" "a script couldn't read a tab's URL"
  case "$answer" in
  "$site"*) ;;
  *) fail "tab 1's URL was $answer to a script, not $site" ;;
  esac
  answered "get title of active tab of window 1" "a script couldn't read the shown tab's title"

  answered "make new tab at end of tabs of window 1 with properties {URL:\"${site}?made\"}" \
    "make didn't open a tab"
  expect "^browser: loaded [0-9]* ${site}?made\$" "the made tab didn't load its page"
  expect "^browser: script new_tab .* by osascript" "make new tab wasn't logged"
  answered "select (tab id $page of window 1)" "select didn't show the first tab"
  expect "^browser: selected $page\\( .*\\)\\{0,1\}\$" "select didn't show the first tab"
  answered "make new tab at end of tabs of space 2 of window 1 with properties {URL:\"${site}?home\"}" \
    "make didn't open a tab in the second Space"
  expect "^browser: space $second\\( .*\\)\\{0,1\}\$" "a tab made in Home didn't show Home"
  expect "^browser: loaded [0-9]* ${site}?home\$" "the tab made in Home didn't load its page"

  ask "make new tab at end of tabs of window 1 with properties {URL:\"javascript:alert(1)\"}"
  [ "$status" != 0 ] || fail "a script opened a javascript: URL"
  grep -q '^browser: script new_tab  by osascript.* refused: only an http or https page$' "$log" ||
    fail "the javascript: URL's refusal wasn't logged"
  ask "execute (tab 1 of window 1) javascript \"1\""
  [ "$status" != 0 ] || fail "a script ran JavaScript in a page"

  send key n cmd shift
  expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
  answered "make new window" "make didn't open a window"
  expect '^browser: window opened [0-9]*$' "make new window didn't open a window"
  answered "count windows" "a script couldn't count the windows"
  [ "$answer" = 2 ] || fail "a script counted $answer windows, not the 2 regular ones"

  # Settings › Agents shows the switch on and what the scripts did.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  fi
  settings_drew Agents
  drawn "switch scripts" > /dev/null || fail "Settings › Agents drew no switch for scripts"
  ../tools/window-screenshot.sh "$pid" "$out/scripting-settings.png" || fail "no window to capture"
  quit
  rm -f "$agents_file"
  entries=$(grep -c '^browser: script ' "$log")
  echo "ok: the app answers ilo's dictionary alone; off, a script is refused and logged; let in," \
    "it read the windows and the user's Spaces and tabs, none of the agent's Space or the private" \
    "window, focused a Space, selected a tab, made tabs and a window as the UI does, and was" \
    "refused a javascript: page and execute; $entries log entries. TCC: ${granted:-no} clients" \
    "granted. Settings: $out/scripting-settings.png"
fi
