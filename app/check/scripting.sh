# On the Mac no script reaches Chromium's AppleScript dictionary, which the engine's Info.plist
# declares and Chromium's code answered outside ilo's rules: every window's tabs with their URLs and
# titles, and commands up to execute, which runs JavaScript in a page (#704). The app declares no
# scripting and has no scripting.sdef, and a script asking it for its windows or a tab's URL gets an
# error, while the quit Apple Event, which isn't scripting, still quits it. The run also writes what
# the Mac lets osascript do, for ilo's own dictionary's check: System Integrity Protection's status
# and the Automation grants TCC holds, in scripting-tcc.txt.

if [ "$(uname)" != Darwin ]; then
  echo "skip: scripting: AppleScript is the Mac's alone."
else
  log="$out/scripting.log"
  rm -f "$saved" "$launch_frame"
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
  BROWSER_URL="$site" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  # What a script gets back, and its status: what the dictionary answers, or the error macOS or the
  # app gives. Fails if it was answered. A script still waiting after 30 s, as on macOS's
  # Automation prompt, which no one answers here, is stopped and counts as refused.
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
    answer=$(tr '\n' ' ' < "$out/scripting-answer.txt")
    echo "osascript $1: status $status: $answer" >> "$out/scripting-asked.txt"
    [ "$status" != 0 ]
  }
  rm -f "$out/scripting-asked.txt"
  counted=
  ask "count windows" || counted=$answer
  read_url=
  ask "get URL of tab 1 of window 1" || read_url=$answer
  quit
  [ -z "$counted" ] || fail "a script counted the app's windows through Chromium's dictionary: $counted"
  [ -z "$read_url" ] || fail "a script read a tab's URL through Chromium's dictionary: $read_url"
  plist="$app/Contents/Info.plist"
  for key in NSAppleScriptEnabled OSAScriptingDefinition; do
    value=$(/usr/libexec/PlistBuddy -c "Print :$key" "$plist" 2>/dev/null) &&
      fail "the app's Info.plist declares $key: $value"
  done
  [ ! -e "$app/Contents/Resources/scripting.sdef" ] || fail "the app has Chromium's scripting.sdef"
  echo "ok: no script reaches Chromium's dictionary: $(tr '\n' ';' < "$out/scripting-asked.txt")" \
    "The app declares no scripting, has no scripting.sdef, and quit by its Apple Event."
fi
