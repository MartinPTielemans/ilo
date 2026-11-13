# What the runs share. check.sh sources it, then each run's file.

# $1, ilo's menu items as the Mac's menus write them, or a pattern of them, as the app writes them
# here (shell/menus.rs): off the Mac in sentence case, keeping each item's first word, Space,
# Spaces, Library, acronyms and names in “”. Items are split at ", " and " › ". Not for names users
# give elsewhere.
cased() {
  if [ "$(uname)" = Darwin ]; then
    printf '%s\n' "$1"
    return
  fi
  printf '%s\n' "$1" | awk '{
    first = 1
    quoted = 0
    n = split($0, words, / /)
    for (i = 1; i <= n; i++) {
      word = words[i]
      noun = word
      sub(/,$/, "", noun)
      sub(/…$/, "", noun)
      if (word ~ /^“/) quoted = 1
      keep = first || quoted || noun ~ /^[A-Z][A-Z]+s?$/ || noun == "Space" || noun == "Spaces" || noun == "Library" || noun == "Markdown"
      printf "%s%s", (i > 1 ? " " : ""), (keep ? word : tolower(word))
      if (word ~ /”/) quoted = 0
      first = word ~ /,$/ || word == "›"
    }
    printf "\n"
  }'
}

# A Space's theme in its menu: the theme picker, and once it has a colour, the way back to none.
colors=$(cased "Edit Theme Color…")
themed=$(cased "$colors, Remove Theme Color")
# Where the theme picker's swatches are, from its top left: 13 for its arrow, then a 29.5 pitch.
swatch_y=380
swatch_x() { echo "$1" | awk '{ print 13 + 60.5 + 29.5 * $1 }'; }
# Its profiles, in the one the check starts with, for a Space with tabs and an empty one.
profile=$(cased "Set Profile › Default (checked), Set Profile › New Profile (disabled)")
# A Space menu's Export “name” into Folder ›, a Space an item, wherever a run doesn't check it
# itself, as Spaces come and go (spaces checks it). It goes in a menu's line before Export as
# Markdown…, through cased.
into='\(Export “[^”]*” into Folder › [^,]*, \)*'
empty_profile=$(cased "Set Profile › Default (checked), Set Profile › New Profile")
# The product's name, which the app's menus and alerts use, and the dev build's id.
app_name=$(cat branding/name)
app_id=dev.browser.app
# The KeyboardEvent property a page reads ⌘ from, which off the Mac is pressed with Ctrl (keys.rs).
cmd_key=metaKey
{ windows || [ "$(uname)" = Linux ]; } && cmd_key=ctrlKey
# ⇧⌘C's toast's log line: with the capture toast's Share button where there's a share sheet, which
# Linux hasn't.
url_toast='^browser: toast shown Copied Current URL \[share\]$'
[ "$(uname)" != Linux ] || url_toast='^browser: toast shown Copied Current URL$'
# Points from the window's top-left corner. The sidebar is 228 wide.
page_point="600 400"
sidebar_point="100 400"

# After a failure, nothing the check started keeps running.
cleanup() {
  status=${1:-$?}
  # A command that failed under set -e ends the check without fail's line, so a result would read
  # as an exit with no failure, as sync-extensions' mkfifo on the pipe a run before left did (#2486).
  if [ "$status" != 0 ] && [ -z "${failed:-}" ]; then
    echo "FAIL: ${run:-the check} stopped at a command that failed, with status $status, before saying why; the lines above are its last" >&2
  fi
  # The servers may be gone already, and under set -e a failed kill would end the trap here.
  [ -z "${server:-}" ] || kill "$server" 2>/dev/null || true
  [ -z "${collector:-}" ] || kill "$collector" 2>/dev/null || true
  [ -z "${stuck:-}" ] || kill "$stuck" 2>/dev/null || true
  [ -z "${helper:-}" ] || kill "$helper" 2>/dev/null || true
  [ -z "${ilo_server:-}" ] || kill "$ilo_server" 2>/dev/null || true
  [ -z "${tls:-}" ] || kill "$tls" 2>/dev/null || true
  # The other copies of the app a run started, as sync-devices' devices: on Windows by taskkill, as
  # the app is below, since Git Bash's kill leaves a crash report from each (#1017).
  if [ -n "${others:-}" ] && windows; then
    for other in $others; do
      taskkill //F //T //PID "$(cat "/proc/$other/winpid" 2>/dev/null)" > /dev/null 2>&1 || true
    done
  elif [ -n "${others:-}" ]; then
    kill $others 2>/dev/null || true
  fi
  # The access run's session and AT-SPI buses.
  [ -z "${buses:-}" ] || kill $buses 2>/dev/null || true
  # A run's own way back, named in run_undo, such as policies' taking its policies off the system.
  [ -z "${run_undo:-}" ] || $run_undo || true
  # A run's temp dir, such as the updates run's with its key.
  [ -z "${scratch:-}" ] || rm -rf "$scratch"
  # A check VM that a run turned light or dark goes back to the appearance it had.
  [ -z "${dark_mode:-}" ] || appearance "$dark_mode" || true
  # So does one whose Dock and menu bar a run hid.
  [ -z "${bars:-}" ] || show_bars || true
  # A crash's minidump goes with the run's captures, once Crashpad's handler has written it.
  for _ in $(seq 60); do
    ls "$profile_dir"/Crashpad/new/*.dmp > /dev/null 2>&1 || break
    sleep 1
  done
  cp "$profile_dir"/Crashpad/*/*.dmp "$out/" 2>/dev/null || true
  kill -0 "$pid" 2>/dev/null || return 0
  if windows; then
    # Git Bash's kill ends a Windows program by starting ExitProcess in it and in each process it
    # started, and chrome.dll faults as that thread unloads it, so each helper would leave a crash
    # report and the run would read as a crash after its failure (#1017). taskkill /F ends them.
    taskkill //F //T //PID "$(cat "/proc/$pid/winpid")" > /dev/null 2>&1 || true
    return 0
  fi
  kill "$pid" 2>/dev/null || return 0
  for _ in $(seq 10); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  # Most often the app ended at the kill above, and this one fails, which would end the trap before
  # the helpers go, and before what a run's own trap does after cleanup, such as mcp's forget_login.
  kill -9 "$pid" 2>/dev/null || true
  pkill -9 -f "$procs" || true
}
fail() {
  failed=1
  echo "FAIL: $1" >&2
  # After a wait that ran out, what it waited for and what came instead (#2566).
  if [ -n "${waited_for:-}" ]; then
    wait_report >&2
  else
    grep -v '^$' "$log" | tail -5 >&2
  fi
  # What the window showed, for a failure the log doesn't explain, as on CI.
  if [ -n "${pid:-}" ] && [ -n "${out:-}" ] && kill -0 "$pid" 2>/dev/null; then
    ../tools/window-screenshot.sh "$pid" "$out/failed.png" > /dev/null 2>&1 || true
  fi
  exit 1
}
# The app ended by itself.
gone() {
  wait "$pid" 2>/dev/null && ended=0 || ended=$?
  fail "the app exited with status $ended"
}
# A prod build or a released package sends its daily count to ilo's hosted server 30 s in, which
# counted each check's launch on a fresh profile as a new user (#2303). Each such launch starts on
# the profile $1 with the count off, as Settings › Privacy leaves it, which every release since
# 0.1.10 honours, so its log says "daily count not sent: off" and nothing leaves. count.sh alone
# sends counts, to a server of its own.
count_off() {
  mkdir -p "$1"
  : > "$1/no-daily-count"
}
# The logs of launches that named a count server of the run's own, as count.sh's do.
own_count_logs=
# Fails the check if a launch's log says a count was sent and it named no server of the run's own:
# that count reached the hosted server. check.sh asks after each run.
counted_nowhere() {
  # The run is over, and so is what its last wait was about.
  waited_for=
  for counted in $(grep -rlx --include='*.log' 'browser: daily count sent' "$out" || true); do
    case " $own_count_logs " in
      *" $counted "*) ;;
      *)
        log=$counted
        fail "$counted says a daily count was sent with no server of the run's own: the launch needs count_off"
        ;;
    esac
  done
}
# Whether the engine linked was built with the core's files $@ as this checkout has them, for a
# step that needs a core change: an engine built before it skips the step, saying so. Only the
# files of core/ and engine/patches/ are compared (tools/engine-changes.sh).
engine_has() {
  local changes file
  if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    # The linked engine, prebuilt or a local build, against this checkout as it is now.
    changes=$(../tools/engine-changes.sh 2> /dev/null) || return 1
  else
    # A copy of the tree with no repository, as a check VM's (#2554): build.sh compared it.
    [ -f build/engine-changes ] ||
      fail "this tree has no repository and build/engine-changes isn't there, so whether the engine has ${*} is unknown: build with ./build.sh where git is, and copy build/ with the app"
    changes=$(cat build/engine-changes)
  fi
  for file in "$@"; do
    case $file in
      ../*) file=${file#../} ;;
      *) file=app/$file ;;
    esac
    ! printf '%s\n' "$changes" | grep -qxF "$file" || return 1
  done
}
# Waits up to 30 s for a line matching $1 in the log of the current run.
await() {
  wait_begins
  for _ in $(seq 300); do
    grep -q "$1" "$log" && return 0
    kill -0 "$pid" 2>/dev/null || {
      ran_out "$1" "" exited
      gone
    }
    sleep 0.1
  done
  ran_out "$1" ""
  return 1
}
# A wait for a line starts: the one before it, met or not, is no longer what a failure is about.
# Seconds by the shell's clock where it has one, as Git Bash forks slowly, or else by date's.
wait_begins() {
  waited_for=
  wait_began=${SECONDS:-$(date +%s)}
}
# Notes, for fail, a wait for a line matching $1 that ran out, or that the app's exit ended with
# $3 "exited", which read the app's lines from command $2 on, or all of them.
ran_out() {
  waited_for=$1
  waited_from=$2
  waited_end=${3:-}
  waited_secs=$((${SECONDS:-$(date +%s)} - ${wait_began:-0}))
}
# What fail says after a wait ran out, for someone who has never seen the run (#2566): what it
# waited for and how long, the app's lines from the wait's command on, and the log's lines nearest
# to what it waited for, those starting as the pattern's plain start does. Most often the line came,
# but in another form, from another layer, or before the command.
wait_report() {
  local plain= near= at=
  if [ "$waited_end" = exited ]; then
    echo "The app exited during a wait for a line matching: $waited_for"
  else
    echo "The wait ran out after ${waited_secs} s, for a line matching: $waited_for"
  fi
  if [ -n "$waited_from" ]; then
    echo "What the app logged from command $waited_from ($(sed -n "s/^browser: drive $waited_from: //p" "$log" | head -1)) on, the last 40 lines:"
    since "$waited_from" | grep -v '^$' | fold_alike | tail -40 | sed 's/^/  /'
    at=$(grep -n "^browser: drive $waited_from: " "$log" | head -1 | cut -d: -f1)
  else
    echo "What the app logged last, 40 lines:"
    grep -v '^$' "$log" | fold_alike | tail -40 | sed 's/^/  /'
  fi
  # The pattern's start up to its first special character, then a word shorter each time while two
  # are left, until some line starts with it: "browser:" alone would match every line. No "[." in
  # the bracket, which starts a collating symbol there.
  plain=$(printf '%s\n' "$waited_for" | sed 's/^\^//; s/[]$*.\\[].*//; s/ *$//')
  while [ "${plain% *}" != "$plain" ]; do
    near=$(awk -v plain="$plain" 'index($0, plain) == 1 { print NR ": " $0 }' "$log" | tail -5)
    if [ -n "$near" ]; then
      echo "The log's last lines starting \"$plain\", by line number${at:+ (command $waited_from is line $at)}:"
      printf '%s\n' "$near" | sed 's/^/  /'
      break
    fi
    plain=${plain% *}
  done
  [ -n "$near" ] || echo "No line in the log starts as the pattern does."
  waited_for=
}
# Folds each run of lines that are alike but for their numbers into its first and last line,
# saying how many there were.
fold_alike() {
  awk '
    function flush() {
      if (n == 1) print first
      else if (same) print first "  (" n " times)"
      else {
        print first
        if (n > 2) print "... " n - 2 " more like it ..."
        print last
      }
    }
    { key = $0; gsub(/[0-9]+/, "#", key) }
    n && key == run { n++; last = $0; if ($0 != first) same = 0; next }
    { if (n) flush(); run = key; first = last = $0; n = 1; same = 1 }
    END { if (n) flush() }'
}
# Puts View › Appearance back to Automatic, the profile's first, by taking away the choice the core
# keeps with the Spaces, and the file an older app kept it in, which a run that picks Light or Dark
# and fails before Automatic leaves for the runs after (#1791). The core reads it only as it
# launches, so this goes before a launch. On every OS, and it fails if it stays.
forget_appearance() {
  rm -f "$profile_dir/Appearance"
  [ -e "$saved" ] || return 0
  sed 's/"appearance":[0-9]*/"appearance":0/' "$saved" > "$saved.forget" && mv "$saved.forget" "$saved"
  ! grep -q '"appearance":[12]' "$saved" ||
    fail "the profile kept its saved appearance, $(grep -o '"appearance":[0-9]*' "$saved")"
}
# The app logs "browser: loaded" when the main frame has loaded.
await_load() {
  await '^browser: load' || fail "the page didn't finish loading"
  grep -q '^browser: loaded' "$log" || fail "the page didn't load"
}
# The last quit stage the core logged (main_delegate.cc), or that it logged none.
stage() {
  grep '^browser: quit: ' "$log" | tail -1 | cut -c16- | grep . || echo "none"
}
# Waits up to $1 seconds (10) for the app to exit and checks that no Chromium process outlives it.
# On the Mac, sample records the app's last 2 s before the limit in hang.txt, with what it said in
# hang-sample.txt: started after the limit, it found no app to attach to when the app went just
# then (#960).
exited() {
  # The driver's pipe goes with the app, so the next launch's is a new one: under Git Bash a native
  # app opening it while the last writer is still open reads that writer's pipe, which ends as
  # `exec 3>` replaces it, and the relaunched app never gets a command.
  exec 3>&-
  ticks=$((${1:-10} * 2))
  sampler=
  stopping=
  for tick in $(seq "$ticks"); do
    kill -0 "$pid" 2>/dev/null || break
    # The tick Chromium said it was stopping, so a slow end says how long its rest took (#959).
    [ -n "$stopping" ] || ! grep -q '^browser: stopping$' "$log" || stopping=$tick
    if [ "$tick" = $((ticks - 3)) ] && [ "$(uname)" = Darwin ]; then
      sample "$pid" 2 -file "$out/hang.txt" > "$out/hang-sample.txt" 2>&1 &
      sampler=$!
    fi
    sleep 0.5
  done
  if ! kill -0 "$pid" 2>/dev/null; then
    # Quit in time, maybe while sampled, which then shows no hang.
    [ -z "$sampler" ] || { wait "$sampler" || true; rm -f "$out/hang.txt" "$out/hang-sample.txt"; }
  else
    [ -z "$sampler" ] || wait "$sampler" || true
    # Whether it's stuck or only slow, which the sample alone doesn't say.
    late=
    for tick in $(seq 20); do
      kill -0 "$pid" 2>/dev/null || { late=$tick; break; }
      sleep 0.5
    done
    if [ -n "$late" ]; then
      why="the app quit late, within $(((late + 1) / 2)) s after the ${1:-10} s it had"
    else
      why="the app didn't quit in ${1:-10} s, nor in the 10 s after"
      # Windows has no sample, but the windows the app has left say what's keeping it.
      ! windows || python3 -I check/app-windows.py list "$(cat "/proc/$pid/winpid")" \
        > "$out/hang.txt" 2>&1 || true
    fi
    # The stage it stopped at, as the sample shows only where it was once the check gave up (#959).
    if [ -n "$stopping" ]; then
      why="$why; after stopping, its last stage was $(stage)"
    fi
    if [ -s "$out/hang.txt" ]; then
      fail "$why. Where it was: $out/hang.txt"
    elif [ -n "$sampler" ]; then
      fail "$why, and sample wrote nothing: $(tail -1 "$out/hang-sample.txt" 2>/dev/null)"
    fi
    fail "$why"
  fi
  wait "$pid" || fail "the app exited with status $?"
  # A quit in time but slow says so, with its last stage: #959's bar is 2 s after stopping, to the
  # half second the ticks see. It doesn't fail the run, as the bar is new.
  if [ -n "$stopping" ] && [ $((tick - stopping)) -gt 4 ]; then
    echo "slow quit: the app ended $(((tick - stopping) * 500)) ms after stopping, its last stage $(stage)"
  fi
  for _ in $(seq 10); do
    pgrep -f "$procs" > /dev/null || break
    sleep 0.5
  done
  # It exits once no process is left to send it a crash, which can take it a moment after the
  # last one; a handler still there after the wait is stuck, as #761's fork child was.
  for _ in $(seq 10); do
    pgrep -f "$app/.*chrome_crashpad_handler" > /dev/null || break
    sleep 0.5
  done
  pgrep -f "$app/.*chrome_crashpad_handler" > /dev/null &&
    fail "Crashpad's handler outlived the app"
  pgrep -f "$procs" > /dev/null && fail "Chromium helper processes were left running"
  # What the app left for Safari's engine as it quit, which the next forget takes away.
  [ "$(uname)" != Darwin ] || echo "engine sites after the quit: $(engine_sites)"
  return 0
}
# What the runs that trace record: tabs, and perf's traced switches. Unset, it's latency, Chrome's
# tab switch spans, which tabs reads, and perf traces nothing. CHECK_TRACE_CATEGORIES names others for
# a breakdown, as #2377's, latency among them, and a run on CI takes them from its check workflow's
# trace input. More than latency fills tabs' trace before its run ends, so then it keeps the run's
# last part, and says how many of its switches have a span; perf traces only its switches, through
# DevTools, so its buffer holds them all.
trace_categories=${CHECK_TRACE_CATEGORIES:-latency}
trace_mode=
[ "$trace_categories" = latency ] || trace_mode=--trace-startup-record-mode=record-continuously
# A quit Apple Event goes through -[NSApplication terminate:], the way macOS quits apps when the
# user logs out. On Windows each browser window is asked to close, as its close button does, and
# Chromium quits with its last one: taskkill without /F closed one of two (#1017). On Linux SIGINT
# is that way: Chromium quits as from its menu, where SIGTERM ends the session at once, without the
# startup trace tabs.sh reads.
quit_app() {
  if windows; then
    python3 -I check/app-windows.py close "$(cat "/proc/$pid/winpid")"
  elif [ "$(uname)" = Linux ]; then
    kill -INT "$(app_pid)"
  else
    osascript -e "tell application id \"$app_id\" to quit"
  fi
}
# Ends the session as the system does at a logout or restart. The Mac and Linux send SIGTERM, which
# Chromium takes for the session ending. Windows asks the app's windows and then tells them it ends
# (app-windows.py end-session, #1990): Git Bash's kill would end the app's children first, and then
# the app from a thread of its own, which no sign-out does.
end_session() {
  if windows; then
    python3 -I check/app-windows.py end-session "$(cat "/proc/$pid/winpid")"
  else
    kill -TERM "$pid"
  fi
}
# The app's own process: the one started, or in the Flatpak, where that's flatpak's, the browser
# process in its sandbox, as the host numbers it, the chrome process that isn't a helper.
app_pid() {
  if [ -z "${BROWSER_FLATPAK:-}" ]; then
    echo "$pid"
    return
  fi
  for p in $(pgrep -f '^/app/ilo/chrome( |$)'); do
    tr '\0' '\n' < "/proc/$p/cmdline" 2>/dev/null | grep -q '^--type=' || { echo "$p"; return; }
  done
  echo "$pid"
}
# The number the app's windows carry as theirs (_NET_WM_PID): in a sandbox its own there.
window_pid() {
  awk '/^NSpid:/ { print $NF }' "/proc/$(app_pid)/status"
}
# Turns the Mac's appearance dark (true) or light (false), as System Settings › Appearance does.
appearance() {
  osascript -e "tell application \"System Events\" to tell appearance preferences to set dark mode to $1"
}
# Hides the Dock and, for the apps launched after, the menu bar, as System Settings › Desktop &
# Dock and Control Center do, keeping what they were in $bars for show_bars.
hide_bars() {
  bars="$(defaults read com.apple.dock autohide 2>/dev/null || echo none)"
  bars="$bars $(defaults read -g _HIHideMenuBar 2>/dev/null || echo none)"
  defaults write com.apple.dock autohide -bool true
  defaults write -g _HIHideMenuBar -bool true
  killall Dock 2>/dev/null || true
}
# Puts the Dock and the menu bar back as hide_bars found them.
show_bars() {
  set -- $bars
  bars=
  case $1 in
    none) defaults delete com.apple.dock autohide ;;
    1) defaults write com.apple.dock autohide -bool true ;;
    *) defaults write com.apple.dock autohide -bool false ;;
  esac
  case $2 in
    none) defaults delete -g _HIHideMenuBar ;;
    1) defaults write -g _HIHideMenuBar -bool true ;;
    *) defaults write -g _HIHideMenuBar -bool false ;;
  esac
  killall Dock 2>/dev/null || true
}
quit() {
  quit_app
  exited "$@"
}
# uBlock Origin's selfie of its filters (#2381, #2502), which goes with uBlock Origin (#2111). A
# launch takes its filters in from it, or with none compiles them, and 53 s after its lists last
# changed writes it, about a minute into a new profile's first launch, in the browser's pool, main
# and IO threads and the extension's process: 0.85 s of CPU in 4 s on the Mac. A user's launches
# after are on it, so a run's that measure them are too.
# ublock_stores <command>: <command> on each of its two stores, failing as it fails on one.
ublock_stores() {
  local ublock=oghkpboejbohceocapipmnjifoklonij
  "$@" "$profile_dir/Default/IndexedDB/chrome-extension_${ublock}_0.indexeddb.leveldb" &&
    "$@" "$profile_dir/Default/Local Extension Settings/$ublock"
}
ublock_store_selfie() {
  local keys key
  keys=$(python3 -I check/leveldb_keys.py "$1") || return 1
  for key in staticMain staticExtFilteringEngine staticNetFilteringEngine; do
    printf '%s\n' "$keys" | LC_ALL=C grep -aq "selfie/$key\$" || return 1
  done
}
# ublock_selfie: whether uBlock Origin wrote its selfie's three parts, which it takes in only
# together, to its IndexedDB and to its extension storage, by the keys their files hold. A key it
# takes out stays in them until they compact, so this says only that one was written.
ublock_selfie() {
  ublock_stores ublock_store_selfie
}
# What <store> holds of the selfie, and its files, for a run that ends with none.
ublock_store_note() {
  echo "$1"
  ls -lR "$1" "${1%.leveldb}.blob" 2> /dev/null || true
  python3 -I check/leveldb_keys.py "$1" | LC_ALL=C grep -ao 'selfie/.*' | sort | uniq -c
}
# selfie_launch <url> <log>: a launch on page <url>, logged to <log>, that stays until uBlock Origin
# has written its selfie, checking each second, and quits, so its work has ended before the next
# launch. $selfie_at is the seconds it took.
selfie_launch() {
  local run_log=$log stayed
  log=$2
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  stayed=$(date +%s)
  BROWSER_URL="$1" BROWSER_PROFILE_DIR="$profile_dir" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  selfie_at=
  for _ in $(seq 180); do
    ublock_selfie && selfie_at=$(($(date +%s) - stayed)) && break
    kill -0 "$pid" 2> /dev/null || gone
    sleep 1
  done
  quit
  if [ -z "$selfie_at" ]; then
    # Whether it wrote none or some, for a run that ends here.
    ublock_stores ublock_store_note > "$out/selfie-stores.txt" 2>&1 || true
    fail "uBlock Origin wrote no selfie in a launch that stayed 180 s; its stores: $out/selfie-stores.txt"
  fi
  rm -f "$profile_dir/Spaces.json" "$profile_dir/launch-frame"
  log=$run_log
}
# Whether the run is on X11 through sway's Xwayland, where a child window such as the password
# offer or the find bar can't keep the X focus, which wlroots' window manager sends back to the
# browser window (#1646). Wayland and Xvfb keep it.
sway_xwayland() {
  [ "$(uname)" = Linux ] && [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ] && [ -z "${WAYLAND_DISPLAY:-}" ]
}
# On sway's Xwayland, the steps from here on, $1, need a child window to keep the X focus. There
# they're skipped and the run ends.
skip_on_sway_xwayland() {
  sway_xwayland || return 0
  exec 3>&-
  quit
  echo "skip: $1: sway's Xwayland sends child windows' X focus back (#1646)"
  exit 0
}
# Under Xvfb and KWin (check.sh's BROWSER_XVFB=kwin), whether a window that shows asks KWin to blur
# under it within 2 s, as an overlay does once its motion ends at rest (#2134). Elsewhere, false.
kwin_blurs() {
  [ "${BROWSER_XVFB:-}" = kwin ] || return 1
  for _ in $(seq 20); do
    for window in $(xwininfo -root -children | awk '$1 ~ /^0x/ { print $1 }'); do
      xwininfo -id "$window" | grep -q 'Map State: IsViewable' || continue
      xprop -id "$window" _KDE_NET_WM_BLUR_BEHIND_REGION | grep -q ' = [0-9]' && return 0
    done
    sleep 0.1
  done
  return 1
}

# Sends a command to the app's driver (platform/mac/drive.mm, platform/windows/drive.cc,
# platform/linux/drive.rs) and waits
# until the app has handled it.
send() {
  step=$((step + 1))
  echo "$*" >&3
  await "^browser: drive $step done" || fail "the driver didn't finish \"$*\""
  grep -q "^browser: drive $step: can't do that" "$log" && fail "the driver can't do \"$*\""
  done=$(grep "^browser: drive $step done" "$log")
  key=$(echo "$done" | sed 's/.*key \([^,]*\),.*/\1/')
  focus=$(echo "$done" | sed 's/.*focus \([^,]*\),.*/\1/')
  active=$(echo "$done" | sed 's/.*active \([01]\).*/\1/')
  marked=$(echo "$done" | sed 's/.*marked //')
}
# Sends a command without waiting for it: a menu or an alert it opens keeps the driver's events
# until it closes.
post() {
  step=$((step + 1))
  echo "$*" >&3
}
# click_placed <send|post> <part>: a click on the Site Controls' <part>, such as "shield",
# "switch Developer Mode" or "line Location", where they last logged it. They log each part's place
# whenever it changes while open, so a check needn't know the panel's geometry.
click_placed() {
  at=$(grep "^browser: site controls $2 at " "$log" | tail -1 | sed "s/^browser: site controls $2 at //")
  [ -n "$at" ] || fail "the Site Controls didn't draw $2"
  "$1" click $at
}
# A Today tab's Share as its menu logs it, with the comma before it: this Mac's share services, or
# with none its one Share… for the share sheet, dimmed for a page with no link, or nothing on Linux,
# which has neither. A check reads past them and never chooses one.
share='\(, Share…\( (disabled)\)\?\|, Share › [^,]*\(, Share › [^,]*\)*\)\?'
# Waits for a line matching $1 logged after the last command was sent, or after command $3, and
# fails with $2.
expect() {
  wait_begins
  for _ in $(seq 100); do
    logged "$1" "${3:-}" && return 0
    exited_without "$1" "$2" "${3:-}"
    sleep 0.1
  done
  ran_out "$1" "${3:-$step}"
  fail "$2"
}
# expect, for up to 30 s, for what waits on a process that may still be starting.
expect_late() {
  wait_begins
  for _ in $(seq 300); do
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "$1" && return 0
    sleep 0.1
  done
  ran_out "$1" "$step"
  fail "$2"
}
# expect, looking every 10 ms, for a press that has to land within the first frames of a motion.
expect_soon() {
  wait_begins
  for _ in $(seq 1000); do
    logged "$1" && return 0
    exited_without "$1" "$2"
    sleep 0.01
  done
  ran_out "$1" "$step"
  fail "$2"
}
# A line matching $1 was logged after the last command was sent, or after command $2.
logged() {
  sed -n "/^browser: drive ${2:-$step}: /,\$p" "$log" | grep -q "$1"
}
# The app that was to log $1 is gone without it, which says why the step $2 failed. It's looked
# for again, as the app may have logged it as it went.
exited_without() {
  [ -z "${pid:-}" ] || kill -0 "$pid" 2>/dev/null || logged "$1" "${3:-}" || {
    ran_out "$1" "${3:-$step}" exited
    fail "the app exited: $2"
  }
}
# Settings has drawn its tab $1, or whichever it shows, since the last command, on showing or on a
# change of tab: until it has, a click in it finds nothing to land on (#1101, #1133).
settings_drew() {
  expect "^browser: settings drew ${1:-.*}\$" "Settings didn't draw ${1:-its tab}"
}
# Where Settings last drew its control $1, "x y" for clickkey, once it has: it logs each place it
# draws one ("browser: settings $1 at x y"). That can be a frame from before the state a run just
# waited for, which drawn_after leaves out.
drawn() {
  for _ in $(seq 100); do
    sed -n "s/^browser: settings $1 at //p" "$log" | tail -1 | grep . && return 0
    sleep 0.1
  done
  return 1
}
# As drawn, from a frame drawn after the last line that matches $2, the state the run waited for:
# a point drawn before it is the old layout's, where a press lands on whatever was there (#1885).
drawn_after() {
  for _ in $(seq 100); do
    awk -v state="$2" -v name="browser: settings $1 at " '
      $0 ~ state { at = "" }
      index($0, name) == 1 { at = substr($0, length(name) + 1) }
      END { if (at == "") exit 1; print at }' "$log" && return 0
    sleep 0.1
  done
  return 1
}
# Sets at to where Settings drew its control $1, as drawn does, in the card's sight: between its
# held title and its fade, where the card shows a row whole ("browser: settings card shows <top> to
# <bottom>"). One drawn past them, as a section with more rows than the window's height puts at its
# foot, is under the card's edge, where a press lands on nothing, so the card scrolls it into
# sight first, from its middle, and at is where the frame after that draws it (#1950). Returns 1
# if it never drew it.
in_sight() {
  at=$(drawn "$1") || return 1
  sight=$(grep '^browser: settings card shows ' "$log" | tail -1 | cut -d' ' -f5-)
  [ -n "$sight" ] || return 0
  top=${sight%% to *}
  bottom=${sight##* to }
  y=${at#* }
  [ "$y" -ge "$top" ] && [ "$y" -le "$bottom" ] && return 0
  # Past it by its row's half, 24, and a little more.
  if [ "$y" -gt "$bottom" ]; then by=$((y - bottom + 32)); else by=$((y - top - 32)); fi
  send scrollkey "${at% *}" $(((top + bottom) / 2)) "$by" 1
  at=$(drawn_after "$1" "^browser: drive $step: ") || return 1
  # GPUI on Windows takes the driver's wheel messages for lines, about two thirds of the points
  # asked, which can leave the row under the edge: scroll the rest. Where the card can't scroll
  # further, nothing draws again and at stays where it was.
  y=${at#* }
  [ "$y" -ge "$top" ] && [ "$y" -le "$bottom" ] && return 0
  if [ "$y" -gt "$bottom" ]; then by=$((y - bottom + 32)); else by=$((y - top - 32)); fi
  send scrollkey "${at% *}" $(((top + bottom) / 2)) "$by" 1
  rest=$(drawn_after "$1" "^browser: drive $step: ") && at=$rest
  return 0
}
# Clicks Settings' control $1 in the card, where the card shows it whole, as in_sight scrolls it:
# one at $(drawn …) under a short window's edge takes the click on nothing (#2503). $2 is the
# driver's command, clickkey unless named. in_sight's variables are its own here, so a caller's at,
# a mark it awaits from, keeps.
click_in_sight() {
  local at sight top bottom y by rest
  in_sight "$1" || fail "Settings didn't draw $1"
  send "${2:-clickkey}" $at
}
# ⌘N's and ⇧⌘N's window opens with ⌘T's command bar once it's on screen, as Arc's (window.sh
# checks it). Escape closes it, leaving the window with no page that the steps after expect. With
# a path, the windows are captured there while it's open.
new_window_bar() {
  expect '^browser: command new-tab via new window$' "the new window didn't open the command bar"
  expect '^browser: command bar shown$' "the new window's command bar didn't show"
  [ -z "${1:-}" ] || ../tools/window-screenshot.sh "$pid" "$1" all || fail "no window to capture"
  send key escape
  expect '^browser: command bar hidden$' "escape didn't close the new window's command bar"
}
# What the app logged since the last command was sent, or since command $1.
since() {
  sed -n "/^browser: drive ${1:-$step}: /,\$p" "$log"
}
# Nothing logged since the last command was sent, or since command $2, matches $1. Straight after
# a send of input it can pass before the app has got to it: the app handles input in the order it
# comes, so a later step's expect is what says it has.
none_since() {
  ! since "${2:-}" | grep -q "$1"
}
# Nothing logged while the driver handled the last command matches $1: what it did at once, not
# what a timer of the app's did after, which on Windows send's own greps can outlast.
none_while() {
  ! sed -n "/^browser: drive $step: /,/^browser: drive $step done/p" "$log" | grep -q "$1"
}
# Kills the app's pages' renderers, as macOS kills one under memory pressure. Not an extension's,
# which is a renderer too and whose death shows Chromium's balloon saying the extension crashed.
kill_pages() {
  # On Windows, Git Bash's ps lists its own processes alone: the build's renderers, from Windows',
  # ended as TerminateProcess ends one.
  if windows; then
    renderers=$(powershell.exe -NoProfile -NonInteractive -Command "Get-CimInstance Win32_Process \
      -Filter \"Name = 'chrome.exe'\" | Where-Object { \$_.CommandLine -match '--type=renderer' -and \
      \$_.CommandLine -notmatch '--extension-process' -and \$_.ExecutablePath -like '*\\build\\Browser\\*' } |
      ForEach-Object { \$_.ProcessId }" | tr -d '\r')
    [ -n "$renderers" ] || return 1
    for renderer in $renderers; do
      taskkill //F //PID "$renderer" > /dev/null 2>&1
    done
    return 0
  fi
  renderers=$(ps -Ao pid=,args= | grep -e "$procs.*--type=rendere[r]" |
    grep -v -e '--extension-process' | awk '{ print $1 }')
  [ -n "$renderers" ] && kill -KILL $renderers
}
# Where Settings' close button is, in $close: AppKit's on the Mac, the middle of the 14 pt button
# Settings puts at 12, 16, else where Settings drew its own, which on Windows is in the top right.
settings_close() {
  close="19 23"
  [ "$(uname)" = Darwin ] && return
  await '^browser: settings close at ' || fail "Settings didn't draw its close button"
  close=$(sed -n 's/^browser: settings close at //p' "$log" | tail -1)
}
# How many detents the app has asked the trackpad for: one for each place a dragged row passes.
detents() {
  grep -c '^browser: detent$' "$log" || true
}
# ⌘Q, and return for Quit in the dialog that asks first, once it has the keyboard. Quit is the
# driver's last command: the app doesn't report it done.
quit_by_key() {
  post key q cmd
  expect "^browser: dialog shown Quit $app_name?\$" "⌘Q didn't ask before quitting"
  expect '^browser: dialog settled$' "the dialog asking to quit didn't settle"
  post key return
  exec 3>&-
}
# Chrome's browser window has the keyboard, with focus on $1.
in_browser() {
  [ "$key" = BrowserCoreNSWindow ] && [ "$focus" = "$1" ] ||
    fail "$2: the keyboard is in $key, on $focus"
}
# ⌘$1 with the page focused reaches the UI as command $2.
page_shortcut() {
  send key "$1" cmd
  expect "^browser: command $2 via page$" "⌘$(echo "$1" | tr a-z A-Z) with the page focused didn't reach the UI"
}
# Command $1 ran once since the last command was sent, which $2 names: under Wayland GPUI reads a
# page's key after Chromium, and once the app's answer has given the UI the keyboard, as closing
# the page that had it does, GPUI would run the key again (#2442). Read once the command's effect
# is logged, which comes after a second run would.
ran_once() {
  ran=$(since | grep -c "^browser: command $1 via ")
  [ "$ran" = 1 ] || fail "$2 ran $1 $ran times"
}

# What the sidebar last logged of $1, "sidebar hovered" or "sidebar tools": 1 or shown, 0 or hidden,
# or nothing before its first change.
last() { grep "^browser: $1 " "$log" | tail -1 | sed 's/.* //'; }
# Records the window to $out/$1.mp4 for $2 seconds, for the parity rows whose target moves: it
# returns once frames are being written, so the motion driven next is in it, and recorded waits
# for the file. A still taken while it records ends the recording (ReplayKit's -5814), so a run
# takes its stills after recorded. A recording that doesn't start is tried once more, then the
# run goes on without it, saying why: the video is the motion's proof, never a step's.
record() {
  # tools/window-record.sh records with the Mac's ScreenCaptureKit alone.
  if [ "$(uname)" != Darwin ]; then
    recording=""
    echo "skip: recording $1, as tools/window-record.sh records only the Mac's windows so far."
    return 0
  fi
  recording="$out/$1.mp4"
  for try in 1 2; do
    ../tools/window-record.sh "$pid" "$2" "$recording" > "$out/$1-record.log" 2>&1 &
    recorder=$!
    for _ in $(seq 300); do
      grep -q '^recording$' "$out/$1-record.log" && return 0
      kill -0 "$recorder" 2> /dev/null || break
      sleep 0.1
    done
    if kill -0 "$recorder" 2> /dev/null; then
      kill "$recorder" 2> /dev/null
      wait "$recorder" 2> /dev/null
      echo "the recording didn't start in 30 s" >> "$out/$1-record.log"
    fi
    unrecorded && return 0
    [ "$try" = 2 ] || echo "retry: recording $1, as it didn't start: $(tail -1 "$out/$1-record.log")"
  done
  echo "skip: recording $1, as it didn't start, twice: $(tail -1 "$out/$1-record.log")"
  recording=""
}
recorded() {
  [ -n "$recording" ] || return 0
  wait "$recorder" && return 0
  unrecorded && return 0
  # A stream that ended by itself says why when it does, before the failure it leads to (#2280).
  said=$(grep '^the stream ended by itself' "${recording%.mp4}-record.log" | tail -1)
  [ -n "$said" ] || said=$(tail -1 "${recording%.mp4}-record.log")
  case "$said" in
  *Code=-5814*) fail "the window's recording ended early, as a still taken while it records ends it: $said" ;;
  esac
  fail "the window's recording didn't finish: $said"
}
# GitHub's hosted Mac starts a recording and never gets its first frame, ReplayKit's -5822 (#1108),
# and a check VM's guest stops a stream partway through, ScreenCaptureKit's -3821 (#2396). The
# motion can't be taken again once it's driven, so the run goes on without the video, saying so.
# Any other recording that started and then failed fails the run.
unrecorded() {
  said=$(tail -1 "${recording%.mp4}-record.log")
  stopped=$(grep '^the stream ended by itself.*Code=-3821 ' "${recording%.mp4}-record.log" | tail -1)
  case "${GITHUB_ACTIONS:-}: $said" in
  true:*Code=-5822*)
    echo "skip: recording $(basename "$recording" .mp4), as the hosted Mac's recording got no first frame: $said" ;;
  *)
    [ -n "$stopped" ] || return 1
    echo "skip: recording $(basename "$recording" .mp4), as the system stopped it: $stopped" ;;
  esac
  recording=""
}
# The driver's clicks come with no move, and one in the sidebar tells it the pointer is there, so
# where the sidebar last saw the pointer depends on the steps before. This moves it to the page and
# waits until the sidebar has seen it go and its tools have faded out, whether or not this move was
# what changed them, so the next move over the sidebar shows them anew.
away() {
  send move $page_point
  for _ in $(seq 100); do
    [ "$(last 'sidebar hovered')" != 1 ] && [ "$(last 'sidebar tools')" != shown ] && return 0
    sleep 0.1
  done
  fail "the sidebar's tools stayed with the pointer over the page"
}
# The Today tabs the app last showed, top to bottom.
today() {
  grep '^browser: today' "$log" | tail -1 | sed 's/^browser: today *//'
}
# Waits for the app to show Today as $1, with $2 shown. $3 says what went wrong if it doesn't.
expect_tabs() {
  expect "^browser: today $1\$" "$3: Today is \"$(today)\", not \"$1\""
  expect "^browser: selected $2\( in .*\)\{0,1\}\$" "$3: tab $2 isn't shown"
}
# Opens a tab from the command bar, to https://$1. The scheme is typed: a bare host goes to http
# and Chrome's HTTPS-Upgrades takes it to https, falling back to http when the runner's https is
# slow or fails (#1261), and the runs that follow expect https. history checks the upgrade itself.
open_from_bar() {
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  expect '^browser: new tab plate 1$' "⌘T didn't give New Tab the selected plate"
  send type "https://$1"
  send key return
  expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the tab for $1 didn't open at the top of Today"
  tab=$(today | cut -d' ' -f1)
  expect "^browser: selected $tab\$" "the tab for $1 wasn't shown"
  expect '^browser: new tab plate 0$' "the tab for $1 didn't take the selected plate"
  expect "^browser: loaded $tab https://$1/\$" "$1 didn't load in its tab"
}
# The driver's key $1 shows tab $2, as a switch the app times.
switch() {
  send key $1
  expect "^browser: selected $2 in " "key $1 didn't show tab $2 of \"$(today)\""
}
# The center of row $1 of Today, counting from 0: rows are 41 apart under "+ New Tab", which is
# under the divider, 15 tall, and the Space's name, 44 tall and 90 below the window's top.
row() {
  echo "100 $((90 + 44 + 15 + 41 * ($1 + 1) + 20))"
}
# The CPU time the browser's processes have taken, in seconds, and how many there are: on Linux
# from /proc, in clock ticks, on Windows from WMI, in 100 ns, on the Mac from ps, in hundredths.
# cpu <not> leaves out the processes whose command line holds <not>.
cpu() {
  if windows; then
    local not=
    [ -z "${1:-}" ] || not=" -and \$_.CommandLine -notlike '*$1*'"
    powershell.exe -NoProfile -NonInteractive -Command "Get-CimInstance Win32_Process \
      -Filter \"Name = 'chrome.exe'\" | Where-Object { \$_.ExecutablePath -like '*\\build\\Browser\\*'$not } |
      ForEach-Object -Begin { \$n = 0; \$t = 0 } -Process { \$n++; \$t += \$_.UserModeTime + \$_.KernelModeTime } \
      -End { '{0:F2} {1}' -f (\$t / 1e7), \$n }" | tr -d '\r'
  elif [ -d /proc ]; then
    for p in $(pgrep -f -- "$procs"); do
      # Chromium writes its processes' command lines over with spaces, others keep their NULs.
      [ -z "${1:-}" ] || ! tr '\0' ' ' < "/proc/$p/cmdline" 2> /dev/null | grep -qF -e "$1" || continue
      sed 's/^.*) //' "/proc/$p/stat" 2> /dev/null
    done | awk -v hz="$(getconf CLK_TCK)" '{ n++; t += $12 + $13 } END { printf "%.2f %d\n", t / hz, n }'
  else
    ps -Ao time=,args= | grep -F -e "$procs" | grep -v -e 'grep -F' |
      awk -v not="${1:-}" 'not == "" || !index($0, not)' | awk '{
      n++; s = 0; k = split($1, part, ":"); for (i = 1; i <= k; i++) s = s * 60 + part[i]; t += s }
      END { printf "%.2f %d\n", t, n }'
  fi
}
# Switch times in milliseconds, one a line, as "median <m> ms, max <x> ms, <n> switches".
summary() {
  sort -n | awk '{ v[NR] = $1 } END { if (NR) printf "median %.1f ms, max %.1f ms, %d switches", v[int((NR + 1) / 2)], v[NR], NR }'
}

# The page view keeps the sidebar's 228 points on its left and 10 on its other sides, over GPUI's
# view, which fills the window. A full-screen page ("fills") covers the whole window, and with the
# sidebar hidden ("inset") it's 10 in from every side.
check_layout() {
  send layout
  layout=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout')
  content=$(echo "$layout" | sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
  view=$(echo "$layout" | sed -n 's/.*WebContentsViewCocoa \([0-9,]* [0-9x]*\).*/\1/p')
  radius=$(echo "$layout" | sed -n 's/.*WebContentsViewCocoa [0-9,]* [0-9x]* radius \([0-9]*\).*/\1/p')
  ui=$(echo "$layout" | sed -n 's/.*GPUIView \([0-9,]* [0-9x]*\).*/\1/p')
  width=${content%x*}
  height=${content#*x}
  # Past the sidebar, 228 wide unless $2 gives its width.
  left=228
  case "${2:-}" in [0-9]*) left=$2 ;; esac
  expected="$left,10 $((width - left - 10))x$((height - 20))"
  # Rounded as the card's corners, square when it fills the window.
  expected_radius=6
  [ "${2:-}" = fills ] && expected="0,0 $content" && expected_radius=
  [ "${2:-}" = inset ] && expected="10,10 $((width - 20))x$((height - 20))"
  [ "$view" = "$expected" ] || fail "$1: the page view is at $view in a $content window"
  [ "$radius" = "$expected_radius" ] ||
    fail "$1: the page's corners have a radius of ${radius:-0}, not ${expected_radius:-0}"
  [ "$ui" = "0,0 $content" ] || fail "$1: GPUI's view is at $ui in a $content window"
}

# Waits for the tab's page to be at https://$1/, done loading, with back and forward as $2 and $3
# (1 or 0). $4 says when.
expect_page() {
  expect "^browser: page $tab back $2 forward $3 loading 0 https://$1/\$" "$4: the tab isn't at $1 with back $2 and forward $3: $(grep "^browser: page $tab" "$log" | tail -1)"
}
# Loads https://$1 in the tab from the command bar.
go_from_bar() {
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type "$1"
  send key return
  expect '^browser: command bar hidden$' "going to $1 didn't close the command bar"
}
# Runs command $1, named $2, from the command bar, found by typing $3.
bar_command() {
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type "$3"
  expect "^browser: command bar commands \"$3\": $2" "the command bar didn't find $2 first"
  send key return
  expect "^browser: command $1 via command bar\$" "$2 didn't run from the command bar"
}
# The driver's key $1, with ⌘, runs command $2, which the UI hears from side $3. Chromium and GPUI
# name keys through the Mac's keyboard layout, where the keys for ⌘[ and ⌘] on a US one can be
# others (å and a dead key on a Danish one), so this drives ⌘← and ⌘→, the same on every layout.
history_key() {
  send key "$1" cmd
  expect "^browser: command $2 via $3\$" "⌘$1 with the $3 focused didn't run $2"
}

# The Space the app last showed.
space() {
  grep '^browser: space [0-9]' "$log" | tail -1 | cut -d' ' -f3
}

# The pinned tabs the app last showed, top to bottom.
pinned() {
  # Only the list's lines, not "pinned extensions" or the chevron's "pinned caret".
  grep -E '^browser: pinned( [0-9]+)* ?$' "$log" | tail -1 | sed 's/^browser: pinned *//'
}
# Waits for the app to show the pinned tabs $1 over Today $2. $3 says what went wrong if it doesn't.
expect_lists() {
  expect "^browser: pinned $1\$" "$3: the pinned tabs are \"$(pinned)\", not \"$1\""
  expect "^browser: today $2\$" "$3: Today is \"$(today)\", not \"$2\""
}
# What the footer's + menu ends with to add from the desktop: one item where the system's open
# panel picks files and folders together, the Mac's, else one for each.
plus_adds() {
  if [ "$(uname)" = Darwin ]; then
    echo 'Add File or Folder…'
  else
    echo 'Add Files…, Add Folder…'
  fi
}
# The centre of pinned row $1, counting from 0, under the Space's name, and of the ✕ it shows
# under the pointer.
pinned_row() {
  echo "100 $((90 + 44 + 41 * $1 + 20))"
}
pinned_close() {
  echo "202 $((90 + 44 + 41 * $1 + 20))"
}
# Its favicon's.
pinned_favicon() {
  echo "26 $((90 + 44 + 41 * $1 + 20))"
}

# What the profile keeps for Safari's engine, as JSON: its sites, the notes it closed and the listed
# hosts turned off, or {} for none. The Mac's alone, as Safari's engine is.
engine_sites() {
  prefs="$profile_dir/Default/Preferences"
  [ -e "$prefs" ] || { echo "{}"; return 0; }
  python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    engine = json.load(f).get("browser_core", {}).get("system_engine", {})
print(json.dumps({kind: engine[kind] for kind in ("sites", "closed_notes", "listed_off") if kind in engine}))
' "$prefs"
}
# Forgets the sites the profile keeps for Safari's engine, the notes it closed and the listed hosts
# it turned off, which a run that turns them on leaves for the runs after (#1782). The app writes
# its preferences as it quits, so a forget while any process of it is left loses to that write: it
# waits up to 20 s for the last to go, and fails if one stays. It says what it found, which a run
# before it left, and the profile then has none of it before the launch that follows. On the Mac
# only, as only its Safari's engine keeps them.
forget_engine_sites() {
  [ "$(uname)" = Darwin ] || return 0
  local _ left
  for _ in $(seq 200); do
    pgrep -f "$procs" > /dev/null || break
    sleep 0.1
  done
  left=$(pgrep -f "$procs" | tr '\n' ' ')
  [ -z "$left" ] ||
    fail "the app is still running ($left), and its last write of its preferences would undo the forget of Safari's engine's sites"
  prefs="$profile_dir/Default/Preferences"
  echo "engine sites before the forget: $(engine_sites)"
  [ -e "$prefs" ] || return 0
  python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    prefs = json.load(f)
engine = prefs.get("browser_core", {}).get("system_engine", {})
for kind in ("sites", "closed_notes", "listed_off"):
    engine.pop(kind, None)
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(prefs, f)
' "$prefs" || fail "the profile's sites for Safari's engine couldn't be reset"
  [ "$(engine_sites)" = "{}" ] ||
    fail "the profile kept Safari's engine's sites through the forget: $(engine_sites)"
}

# Starts the app on the page $1 with the profile $2 and the arguments after $3, and check/cdp.py
# attached to the page whose address ends in $3, or with $3 empty to none until its attach command,
# logging to $log and $cdp_log, with the driver on fd 3 and cdp.py's commands on fd 5, through fifos
# in $scratch. Sets $pid and $helper.
start_with_cdp() {
  cdp_url=$1 cdp_profile=$2 cdp_page=$3
  shift 3
  mkfifo "$scratch/cdp"
  if windows; then
    # Chrome on Windows takes its DevTools pipe as handles, which Git Bash can't give it, so
    # DevTools listens on a loopback port instead, which the browser says on its standard error. A
    # native program can't read Git Bash's fifo, so cat passes cdp.py its commands, and the page is
    # named without its slash, which Git Bash would make a path of Git's own.
    BROWSER_DRIVE=1 BROWSER_URL="$cdp_url" BROWSER_PROFILE_DIR="$cdp_profile" \
      "$browser" --remote-debugging-port=0 "$@" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    await '^DevTools listening on ws://' || fail "DevTools didn't say where it listens"
    socket=$(grep -m1 '^DevTools listening on ws://' "$log" | cut -d' ' -f4 | tr -d '\r')
    cat "$scratch/cdp" | python3 -I -u check/cdp.py --socket "$socket" "${cdp_page#/}" \
      > "$cdp_log" 2>&1 &
  else
    mkfifo "$scratch/to-browser" "$scratch/from-browser"
    # Each end of a pipe waits for the other to open, in this order: the driver's, the browser's
    # DevTools pipe, then cdp.py's input.
    BROWSER_DRIVE=1 BROWSER_URL="$cdp_url" BROWSER_PROFILE_DIR="$cdp_profile" \
      "$browser" --remote-debugging-pipe "$@" < "$out/drive" 3< "$scratch/to-browser" \
      4> "$scratch/from-browser" > "$log" 2>&1 &
    pid=$!
    python3 -I -u check/cdp.py "$scratch/to-browser" "$scratch/from-browser" "$cdp_page" \
      < "$scratch/cdp" > "$cdp_log" 2>&1 &
    exec 3> "$out/drive"
  fi
  helper=$!
  exec 5> "$scratch/cdp"
}
# The display the app opens on, "X Y WIDTH HEIGHT" in points from the main display's top left, as
# Chrome's --window-position takes a place: with BROWSER_DISPLAY=builtin the Mac's own screen, as the
# app's host finds it (platform/mac/host.mm), so a Chrome a run starts doesn't cover what's playing
# on another, else the main display.
display_bounds() {
  [ "$(uname)" = Darwin ] || fail "display_bounds is the Mac's"
  osascript -l JavaScript -e '
    ObjC.import("AppKit");
    ObjC.import("CoreGraphics");
    const screens = $.NSScreen.screens.js;
    const main = screens[0].frame;
    const builtin = ObjC.unwrap(
      $.NSProcessInfo.processInfo.environment.objectForKey("BROWSER_DISPLAY")) == "builtin";
    const own = screens.find((screen) => builtin &&
      $.CGDisplayIsBuiltin(screen.deviceDescription.objectForKey("NSScreenNumber").unsignedIntValue));
    const frame = (own || screens[0]).frame;
    [frame.origin.x, main.size.height - frame.origin.y - frame.size.height, frame.size.width,
     frame.size.height].map(Math.round).join(" ");' || fail "the displays didn't say where the Mac's own is"
}
# Starts the check's web server, in site.sh, unless a run before has. Its pages are at $site. It
# serves every run after, so only cleanup stops it: a run that did would leave the next its address
# and no server (#1369).
serve() {
  [ -n "${site:-}" ] || . check/site.sh
}
# Waits for the tab $3, or any, to start loading the default engine's search for $1, the query as
# its URL has it, failing with $2, and sets $searched to the tab. The app logs where a page goes as
# its load starts, so this proves the search without DuckDuckGo's page, which a runner may never
# get an answer for (#1875).
searching() {
  local line="^browser: page ${3:-[0-9]*} back [0-9]* forward [0-9]* loading [01] https://duckduckgo\.com/?q=$1\$"
  expect "$line" "$2"
  searched=$(grep "$line" "$log" | tail -1 | cut -d' ' -f3)
}
# Of the addresses on stdin, one a line, those of Google's servers, with ungoogled-chromium's
# substitutes for its domains, but the security components' updates that engine/README.md › What
# reaches Google lets through and every user's browser makes, the first a minute after launch: the
# update check, and CRLSet's, PKI Metadata's and the SSL error assistant's downloads, over https,
# from the hosts it names. A profile that has a component already gets its update as a diff, at
# diffgen-puffin/<id>/<hash>.
google_asked() {
  hosts='\(dl\.google\.com\|[a-z0-9.-]*\.gvt1\.com\|www\.google\.com/dl\)'
  ids='\(hfnkpimlhhgieaddgfemjhofmfblmnib\|efniojlnjndmcbiieegkicadnoecjjef\)'
  grep -i '^[a-z]*://[^/]*\(google\|gstatic\|gvt1\|9oo91e\|qjz9zk\)' |
    grep -v -e '^https://update\.googleapis\.com/service/update2/json\(?[^/]*\)\{0,1\}$' \
      -e "^https://$hosts/.*/${ids}_[0-9.]*_[a-z0-9_]*\.crx3\$" \
      -e "^https://$hosts/.*/diffgen-puffin/$ids/[0-9a-f]*\$" \
      -e "^https://$hosts/.*/[0-9.]*_all_sslErrorAssistant\.crx3\$" || true
}
# Waits up to 3 s for the clipboard to hold the text $1, off the Mac, where a run's clipboard is its
# headless session's, which wl-paste reads, Xwayland's too, as sway hands it over. The Mac's
# pasteboard is the run's own, which nothing else reads, so it passes there, as under Xvfb.
clipboard_holds() {
  case "${BROWSER_HEADLESS:-xvfb}" in xvfb) return 0 ;; esac
  command -v wl-paste > /dev/null || fail "reading the clipboard needs wl-clipboard's wl-paste"
  for _ in $(seq 30); do
    [ "$(WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -n 2>/dev/null)" = "$1" ] && return 0
    sleep 0.1
  done
  return 1
}
# Waits for the check's server to log a request matching $1, as site.sh writes them.
served() {
  for _ in $(seq 100); do
    grep -q "$1" "$out/site.log" && return 0
    sleep 0.1
  done
  # With $3, the failure quotes the first requests the site logged that match it.
  sent=
  [ -z "${3:-}" ] || sent=": $(grep -- "$3" "$out/site.log" | head -2 | tr '\n' ' ')"
  fail "$2$sent"
}
# The colour at ($2, $3), as fractions of the width and height, in the capture $1: "r g b".
pixel() {
  if windows; then
    to_bmp "$1" "$out/pixel.bmp" || fail "PowerShell couldn't read $1"
  elif [ "$(uname)" = Linux ]; then
    magick "$1" BMP3:"$out/pixel.bmp" || fail "ImageMagick couldn't read $1"
  else
    sips -s format bmp "$1" --out "$out/pixel.bmp" > /dev/null || fail "sips couldn't read $1"
  fi
  python3 -I - "$out/pixel.bmp" "$2" "$3" << 'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
offset, = struct.unpack_from("<I", data, 10)
width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
x = int(float(sys.argv[2]) * width)
y = int(float(sys.argv[3]) * abs(height))
row = y if height < 0 else abs(height) - 1 - y
stride = (bits * width + 31) // 32 * 4
b, g, r = data[offset + row * stride + x * bits // 8:][:3]
print(r, g, b)
PY
}
# Whether the colour "r g b" is as dark as the card is over any Space's colour.
dark() {
  set -- $1
  [ "$1" -lt 64 ] && [ "$2" -lt 64 ] && [ "$3" -lt 64 ]
}
# Whether the colour "r g b" is lighter than "r2 g2 b2" by 8 or more in each channel.
brighter() {
  set -- $1 $2
  [ "$1" -ge $(($4 + 8)) ] && [ "$2" -ge $(($5 + 8)) ] && [ "$3" -ge $(($6 + 8)) ]
}
# Under Wayland: in $other_app_token, an activation token for a click in another app's window,
# check/drag's, as an app hands on the one it got for the user's click in it with the link they
# clicked, or a desktop with a click on a notification. sway brings an open window forward for it,
# where before sway 1.10 one from its own exec, as a launch gets, only marks the window urgent. The
# window, the click's, holds the keyboard until other_app_closes.
other_app_click() {
  if [ -z "${other_app:-}" ]; then
    cargo build -q --release --manifest-path check/drag/Cargo.toml --target-dir target ||
      fail "check/drag didn't build"
    rm -f "$out/other-app-in"
    : > "$out/other-app.log"
    mkfifo "$out/other-app-in"
    target/release/drag 1280x800 window < "$out/other-app-in" > "$out/other-app.log" 2>&1 &
    other_app=$!
    exec 4> "$out/other-app-in"
    other_app_says 0 '^drag: ready$' "check/drag's window didn't show"
    swaymsg '[app_id=ilo-drag] move absolute position 1220 740' > /dev/null
  fi
  # In front again, as a window that came forward since is over it.
  swaymsg '[app_id=ilo-drag] focus' > /dev/null
  set -- "$(wc -l < "$out/other-app.log")"
  printf '%s\n' 'move 1244 764' press release >&4
  other_app_says "$1" '^drag: pressed$' "the click in check/drag's window didn't reach it"
  set -- "$(wc -l < "$out/other-app.log")"
  echo token >&4
  other_app_says "$1" '^drag: token ' "check/drag got no activation token for its click"
  other_app_token=$(tail -n "+$(($1 + 1))" "$out/other-app.log" | sed -n 's/^drag: token //p')
}
# Waits for check/drag's line $2 after its first $1, failing with $3.
other_app_says() {
  for _ in $(seq 100); do
    tail -n "+$(($1 + 1))" "$out/other-app.log" | grep -q "$2" && return 0
    sleep 0.05
  done
  fail "$3: $(tr '\n' ' ' < "$out/other-app.log")"
}
# Closes check/drag's window other_app_click opened.
other_app_closes() {
  [ -n "${other_app:-}" ] || return 0
  exec 4>&-
  wait "$other_app" || true
  other_app=
  other_app_token=
}
