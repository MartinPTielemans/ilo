#!/bin/sh
# Runs the app and checks what it does.
# Each run's file in check/ says what it checks. ./check.sh tabs spaces does only those runs, in
# that order, and ./check.sh all of them:
#   load keys tabs window popup history spaces agents profiles restore relaunch find pinned clear
#   split favorites cards extensions adblock adblock-lists windows private blank little capture media
#   passwords links preview access trouble dialogs crash import quit onboarding passkeys mcp rules
#   ranking appearance about sync sync-account sync-devices switch-frames devices pair filled
#   scripting notifications calls battery battery-os policies hosts idle peek-motion handback
#   webrtc count translate pictures boosts sidebar-right foot-bar later-launch
# The store run installs from Google's Web Store, as sync-extensions does on one device for the
# account's others, adblock-live blocks Google's ad servers with uBlock Origin's own lists,
# adblock-parity loads 50 public sites to record what the ad blocker blocks on them, adblock-native
# the same with the browser's own, adblock-cost them with ad blocking on and off, and widevine,
# Linux's alone, downloads Google's Widevine module, so they need the network and run only when
# named: ./check.sh store.
# Updates runs only when named too, until it passes on the Mac, and so do nightly and update-checks,
# which build a check copy of their own (build.sh updates), on the Mac and Windows. So does
# keychain, which uses the real Keychain, on the prod build and only in a check VM. Shots takes
# the Store's pictures from pages on the internet, so it runs only when named too. Tab-drag drags a
# tab's row between windows with the data device's own drag, which only Wayland's sessions can, so it
# runs only when named, and skips elsewhere, where the windows run checks it. Paste pastes another
# app's copy into the UI's fields, Linux's alone, as the clipboard there is the session's, so it
# runs only when named too.
# Sync-versions syncs one account with the release ILO_OLD names, installed as ilo's Flatpak, on
# Linux under Xvfb, so it runs only when named too.
# Perf measures the browser against #456's targets, and Chrome beside it, which takes it 16 minutes,
# so it runs only when named too.
# Every run but load and crash needs Browser to be the active app, and window, find and updates
# take it full screen for a moment. Passkeys, scripting and sidebar-right are the Mac's alone. Updates makes
# build/Updates.app on the Mac, a copy with Sparkle and a feed of its own (build.sh updates), and
# ilo's Flatpak on Linux, which a repository of the run's own updates (check/updates.sh).
# In a check VM the check turns the Mac's appearance dark for the runs, which read the window's
# dark look, and puts back the one it had after. Filled hides the VM's Dock and menu bar for its
# run, and runs only in one.
# It opens on the Mac's own screen, where there is one, so it doesn't cover what's playing on
# another; BROWSER_DISPLAY=main opens it on the main display.
# Leaves window captures and logs in build/check/. Build first with ./build.sh.
# Each run starts on an empty profile, build/check-profile, never the dev build's, which the app is
# given by BROWSER_PROFILE_DIR; a failed run's stays there for reading until the next check.
# On Windows it runs under Git Bash, with python3 on the PATH, on the app the engine's chrome.exe
# runs from build/Browser. The runs need what the Windows driver (platform/windows/drive.cc) does
# so far: load, tabs, dialogs, crash, about and onboarding.
# On Linux it runs itself in a headless sway session of its own (tools/headless.sh --run), on its
# Xwayland, and the app's windows and input then never reach the owner's session. Of the runs,
# load and history pass there so far; the others stop where they need what Linux hasn't yet, such
# as Japanese input in a page (keys), ilo's menu over the command bar (tabs) and IME (spaces),
# which the driver says it can't do. Captures are the session's whole output.
# BROWSER_XVFB=1 runs it under Xvfb and openbox instead, which CI has, and BROWSER_XVFB=kwin under
# Xvfb and KWin, which composites and blurs under the overlays that ask. BROWSER_WAYLAND=1 runs it
# on the sway session's Wayland, where the driver's input is sway's virtual pointer and keyboard
# (platform/linux/drive_wayland.rs), which it refuses outside that session.
set -e
export BROWSER_DISPLAY="${BROWSER_DISPLAY:-builtin}"
# Each run starts a new profile, which would play the onboarding first. Only its own run plays it.
export BROWSER_ONBOARDING=skip
# The runs place their clicks under the Space's name row, which a lone unnamed Space hides unless
# Show Space Header keeps it, so theirs is kept; spaces' checks it hiding (#1400).
export BROWSER_SPACE_HEADER=shown
# The browser's lines are whole, but a child process that crashes, as the crash steps make one,
# prints its stack beside them a piece at a time, so a line could start mid-way through one. So no
# process prints its stack (#2496).
export BROWSER_STACK_TRACES=off
cd "$(dirname "$0")"
if [ "$(uname)" = Linux ] && [ -z "${BROWSER_HEADLESS:-}" ]; then
  # Under Xvfb, as CI runs it (check-linux.yml): a 1280 by 800 screen, as the sway session's
  # output, captured whole. openbox manages its windows, as sway does Xwayland's: with none, the
  # keyboard follows the pointer rather than going to a window that opens, such as the find bar,
  # and no window has a state.
  if [ -n "${BROWSER_XVFB:-}" ]; then
    # shellcheck disable=SC2016 # expanded by the session's shell
    exec xvfb-run -a -s "-screen 0 1280x800x24" sh -c '
      ready=$(mktemp -u)
      if [ "$BROWSER_XVFB" = kwin ]; then
        # With OpenGL in Mesa'"'"'s software driver, which KWin composites with only when told to,
        # and its blur, which it leaves off there unless its config turns it on. Ready once the blur
        # names its property on the root window (#2134).
        config=$(mktemp -d)
        printf "[Plugins]\nblurEnabled=true\n" > "$config/kwinrc"
        XDG_CONFIG_HOME=$config KWIN_COMPOSE=O2 dbus-run-session -- kwin_x11 > "$ready.log" 2>&1 &
        tries=0
        until xprop -root _KDE_NET_WM_BLUR_BEHIND_REGION 2> /dev/null |
          grep -q "^_KDE_NET_WM_BLUR_BEHIND_REGION("; do
          tries=$((tries + 1))
          if ! kill -0 $! 2> /dev/null || [ "$tries" -gt 600 ]; then
            echo "kwin: no blur; the effects it says on the root window:" >&2
            xprop -root | grep "^_KDE" >&2
            cat "$ready.log" >&2
            exit 1
          fi
          sleep 0.1
        done
      else
        openbox --startup "touch $ready" &
        while [ ! -e "$ready" ]; do
          kill -0 $! 2> /dev/null || { echo "openbox exited" >&2; exit 1; }
          sleep 0.1
        done
        rm -f "$ready"
      fi
      # The real pointer starts mid-screen, over the app, where the X server tells GPUI it left as
      # the page views of Chromium change under it, mid-drag too. The driver sends a pointer of its
      # own, so the real one waits in the corner the window leaves.
      xdotool mousemove 1279 799
      exec env BROWSER_HEADLESS=xvfb ./check.sh "$@"' sh "$@"
  fi
  [ -z "${BROWSER_WAYLAND:-}" ] || exec ../tools/headless.sh --run ./check.sh "$@"
  # Focus steps can fail here, as wlroots' Xwayland window manager hands the X focus back to the
  # browser window (#851), so Xvfb on CI is the reference for focus.
  exec ../tools/headless.sh --x11 --run ./check.sh "$@"
fi
runs="load keys tabs window popup history spaces agents profiles restore relaunch find pinned clear"
runs="$runs split favorites cards extensions adblock adblock-lists windows private blank little capture media"
runs="$runs passwords links preview access trouble dialogs crash import quit onboarding passkeys mcp rules ranking"
runs="$runs appearance about sync sync-account sync-devices switch-frames devices pair filled scripting notifications calls battery battery-os policies hosts idle peek-motion"
runs="$runs everyday"
runs="$runs handback webrtc count translate pictures boosts sidebar-right foot-bar later-launch"
named="store sync-extensions sync-versions adblock-live passwords-live adblock-parity adblock-native adblock-cost widevine paste updates nightly update-checks keychain shots tab-drag perf"
for run in "$@"; do
  case " $runs $named " in
    *" $run "*) ;;
    *) echo "FAIL: there's no run $run. The runs: $runs $named" >&2; exit 1 ;;
  esac
done
# A file sourced with a syntax error ends the check where it stands, with the status of the EXIT
# trap's last command, 0 under the Mac's sh, and the runs after it never start (#1267). So each
# file the check sources is parsed before any run starts.
for file in lib site ilo-server webstore ${*:-$runs}; do
  why=$(sh -n "check/$file.sh" 2>&1) || {
    echo "FAIL: check/$file.sh doesn't parse: $why" >&2
    exit 1
  }
done
out=build/check
windows() {
  case "$(uname)" in MINGW* | MSYS*) return 0 ;; esac
  return 1
}
# The absolute path $1, as chrome.exe takes it in a switch's value. Git Bash converts the path a
# value starts with, not the others in a list such as --load-extension's, and Chrome on Windows
# resolves a relative path against its own folder, not the directory it started in (#997).
app_path() {
  if windows; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}
# The absolute path $1 as Windows names it, with backslashes, for a download folder: Chrome moves
# each download into it with the Shell's IFileOperation, which takes no D:/a/..., so every download
# failed (#872).
native_path() {
  if windows; then cygpath -w "$1"; else printf '%s\n' "$1"; fi
}
# The app, what runs it, and the pattern that finds its processes, Chromium's helpers too.
if windows; then
  . check/git-bash.sh
  app="$PWD/build/Browser"
  browser=windows_browser
  procs="$(cygpath -m "$app/chrome.exe")"
elif [ -n "${BROWSER_FLATPAK:-}" ]; then
  # The Flatpak flatpak.yml installed, app.ilo.browser: the prod build in its sandbox, given the
  # run's BROWSER_ settings, this folder and the profile's. Only sync-account runs in it so far.
  app=/app/ilo
  flatpak_browser() {
    set -- app.ilo.browser "$@"
    for name in $(env | sed -n 's/^\(BROWSER_[A-Z0-9_]*\)=.*/\1/p'); do
      eval "set -- \"--env=$name=\${$name}\" \"\$@\""
    done
    # A run's own profile, as sync-devices gives each device, or the check's.
    dir=${BROWSER_PROFILE_DIR:-$profile_dir}
    count_off "$dir"
    exec flatpak run --cwd="$PWD" --filesystem="$PWD" --filesystem="$dir:create" \
      --env=BROWSER_PROFILE_DIR="$dir" "$@"
  }
  browser=flatpak_browser
  procs="$app/chrome"
elif [ "$(uname)" = Linux ]; then
  app="$PWD/build/Browser"
  browser="$app/chrome"
  procs="$app/chrome"
else
  app="$PWD/build/Browser.app"
  browser="$app/Contents/MacOS/Browser"
  procs="$app/Contents"
fi
# Another Browser holds the profile directory, so this one couldn't start.
if pgrep -f "$procs" > /dev/null; then
  echo "FAIL: Browser is already running. Quit it first." >&2
  exit 1
fi
rm -rf "$out"
mkdir -p "$out/downloads"
# Each run starts on a profile no earlier run wrote to, as each hosted job's first run does on a
# machine of its own: what one run left filled the rows a later one expects, as keys' history of
# docs.new's sign-in pages did tabs' (#2486). It's the check's own folder, never the dev build's
# profile the app uses by itself, and it's emptied before each run. A failed run's stays for
# reading, until the next check; after the last run passes it goes. A run with a profile of its
# own, such as adblock's, sets profile_dir to it, and the next run's is the check's again (#731).
check_profile_dir="$PWD/build/check-profile"
# The Spaces the app saves for its next launch, which a run's relaunches come back to, and the frame
# its launch window opens at, which the app keeps next to them.
saved="$check_profile_dir/Spaces.json"
launch_frame="$check_profile_dir/launch-frame"
. check/lib.sh
trap cleanup EXIT
# The window follows the Mac's appearance and the runs read its dark look, so a check VM runs dark,
# and cleanup puts back the appearance it had. The owner's Mac keeps its own.
if [ "$(uname)" = Darwin ] && [ "$(sysctl -n kern.hv_vmm_present)" = 1 ]; then
  dark_mode=$(osascript -e 'tell application "System Events" to tell appearance preferences to get dark mode') || {
    failed=1
    echo "FAIL: System Events didn't say the Mac's appearance: allow the check to control it" >&2
    exit 1
  }
  appearance true
fi
# The runs send the app's driver its commands through it.
mkfifo "$out/drive"
for run in ${*:-$runs}; do
  # The last run's app may still be writing its profile as it ends, and Windows doesn't delete a
  # file a process holds.
  for _ in $(seq 200); do
    pgrep -f "$procs" > /dev/null || break
    sleep 0.1
  done
  ! pgrep -f "$procs" > /dev/null || fail "the app from the run before $run is still running"
  # And without the pipes a run before made for its servers and devices, such as sync-devices'
  # drive-a, which mkfifo won't make again. The drivers' own pipe stays.
  find "$out" -maxdepth 1 -type p ! -name drive -exec rm -f {} +
  profile_dir=$check_profile_dir
  rm -rf "$profile_dir"
  mkdir -p "$profile_dir/Default"
  BROWSER_PROFILE_DIR=$(native_path "$profile_dir")
  export BROWSER_PROFILE_DIR
  saved="$profile_dir/Spaces.json"
  launch_frame="$profile_dir/launch-frame"
  prefs="$profile_dir/Default/Preferences"
  # DevTools open docked on the right, as the find run expects, and downloads go in the check's own
  # folder, never the user's Downloads. In UTF-8, which Windows' Python doesn't write by default.
  python3 -I -c '
import json, sys
prefs = {"devtools": {"preferences": {"currentDockState": "\"right\""}},
         "download": {"default_directory": sys.argv[2]}}
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(prefs, f)
' "$prefs" "$(native_path "$PWD/$out/downloads")"
  # Every run is sourced into this one shell, so a helper a run defines under a lib.sh name would
  # replace lib.sh's for the runs after it.
  . "check/$run.sh"
  counted_nowhere
  # On Windows GPUI lost a window's move, size or modifiers that came during an update (#1358).
  if windows && grep -q '^browser: gpui dropped ' "$log"; then
    fail "GPUI dropped updates that came during another: $(grep '^browser: gpui dropped ' "$log" |
      sort | uniq -c | tr -s ' ' | tr '\n' ',')"
  fi
done
rm -rf "$check_profile_dir"
