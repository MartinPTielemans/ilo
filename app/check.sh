#!/bin/sh
# Runs the app and checks what it does.
# Each run's file in check/ says what it checks. ./check.sh tabs spaces does only those runs, in
# that order, and ./check.sh all of them:
#   load keys tabs window popup history spaces agents profiles restore relaunch find pinned clear
#   split favorites cards extensions adblock adblock-lists windows private blank little capture media
#   passwords links preview access trouble dialogs crash import quit onboarding passkeys mcp ranking
#   appearance about sync sync-account sync-devices switch-frames devices pair filled scripting
#   notifications calls battery battery-os policies hosts idle handback webrtc count translate
# The store run installs from Google's Web Store, as sync-extensions does on one device for the
# account's others, adblock-live blocks Google's ad servers with uBlock Origin's own lists and
# widevine, Linux's alone, downloads Google's Widevine module, so they need the network and run
# only when named: ./check.sh store.
# Updates runs only when named too, until it passes on the Mac, and so do nightly and update-checks,
# which build a check copy of their own (build.sh updates), on the Mac and Windows. So does
# keychain, which uses the real Keychain, on the prod build and only in a check VM. Shots takes
# the Store's pictures from pages on the internet, so it runs only when named too. Tab-drag drags a
# tab's row between windows with the data device's own drag, which only Wayland's sessions can, so it
# runs only when named, and skips elsewhere, where the windows run checks it.
# Every run but load and crash needs Browser to be the active app, and window, find and updates
# take it full screen for a moment. Passkeys and scripting are the Mac's alone. Updates makes
# build/Updates.app on the Mac, a copy with Sparkle and a feed of its own (build.sh updates), and
# ilo's Flatpak on Linux, which a repository of the run's own updates (check/updates.sh).
# In a check VM the check turns the Mac's appearance dark for the runs, which read the window's
# dark look, and puts back the one it had after. Filled hides the VM's Dock and menu bar for its
# run, and runs only in one.
# It opens on the Mac's own screen, where there is one, so it doesn't cover what's playing on
# another; BROWSER_DISPLAY=main opens it on the main display.
# Leaves window captures and logs in build/check/. Build first with ./build.sh.
# BROWSER_PROFILE_DIR gives the app and the runs another profile directory, such as a new one for
# each run, since one an earlier run left can lack what a run expects, such as its search engines.
# On Windows it runs under Git Bash, with python3 on the PATH, on the app the engine's chrome.exe
# runs from build/Browser. The runs need what the Windows driver (platform/windows/drive.cc) does
# so far: load, tabs, dialogs, crash, about and onboarding.
# On Linux it runs itself in a headless sway session of its own (tools/headless.sh --run), on its
# Xwayland, and the app's windows and input then never reach the owner's session. Of the runs,
# load and history pass there so far; the others stop where they need what Linux hasn't yet, such
# as Japanese input in a page (keys), ilo's menu over the command bar (tabs) and IME (spaces),
# which the driver says it can't do. Captures are the session's whole output.
# BROWSER_XVFB=1 runs it under Xvfb and openbox instead, which CI has. BROWSER_WAYLAND=1 runs it
# on the sway session's Wayland, where the driver's input is sway's virtual pointer and keyboard
# (platform/linux/drive_wayland.rs), which it refuses outside that session.
set -e
export BROWSER_DISPLAY="${BROWSER_DISPLAY:-builtin}"
# Each run starts a new profile, which would play the onboarding first. Only its own run plays it.
export BROWSER_ONBOARDING=skip
# The runs place their clicks under the Space's name row, which a lone unnamed Space hides unless
# Show Space Header keeps it, so theirs is kept; spaces' checks it hiding (#1400).
export BROWSER_SPACE_HEADER=shown
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
      openbox --startup "touch $ready" &
      while [ ! -e "$ready" ]; do
        kill -0 $! 2> /dev/null || { echo "openbox exited" >&2; exit 1; }
        sleep 0.1
      done
      rm -f "$ready"
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
runs="$runs passwords links preview access trouble dialogs crash import quit onboarding passkeys mcp ranking"
runs="$runs appearance about sync sync-account sync-devices switch-frames devices pair filled scripting notifications calls battery battery-os policies hosts idle"
runs="$runs everyday"
runs="$runs handback webrtc count translate"
named="store sync-extensions adblock-live widevine updates nightly update-checks keychain shots tab-drag"
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
# The app, what runs it, the pattern that finds its processes, Chromium's helpers too, and its dev
# profile.
if windows; then
  . check/git-bash.sh
  app="$PWD/build/Browser"
  browser=windows_browser
  procs="$(cygpath -m "$app/chrome.exe")"
  profile_dir="$(cygpath -u "$LOCALAPPDATA")/browser-app"
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
    exec flatpak run --cwd="$PWD" --filesystem="$PWD" --filesystem="$dir:create" \
      --env=BROWSER_PROFILE_DIR="$dir" "$@"
  }
  browser=flatpak_browser
  procs="$app/chrome"
  profile_dir="$HOME/.cache/browser-app"
elif [ "$(uname)" = Linux ]; then
  app="$PWD/build/Browser"
  browser="$app/chrome"
  procs="$app/chrome"
  profile_dir="$HOME/.cache/browser-app"
else
  app="$PWD/build/Browser.app"
  browser="$app/Contents/MacOS/Browser"
  procs="$app/Contents"
  profile_dir="$HOME/Library/Caches/browser-app"
fi
if [ -n "${BROWSER_PROFILE_DIR:-}" ]; then
  profile_dir="$BROWSER_PROFILE_DIR"
  ! windows || profile_dir="$(cygpath -u "$profile_dir")"
fi
# Another Browser holds the profile directory, so this one couldn't start.
if pgrep -f "$procs" > /dev/null; then
  echo "FAIL: Browser is already running. Quit it first." >&2
  exit 1
fi
rm -rf "$out"
mkdir -p "$out"
# The Spaces the app saves for its next launch. Each run starts without them, but the relaunches.
saved="$profile_dir/Spaces.json"
# The frame the launch window opens at, which the app keeps next to them. A run that starts without
# the Spaces starts without it too, so its launch window isn't at the last run's frame.
launch_frame="$(dirname "$saved")/launch-frame"
# The mcp run turns agents on with a token of its own, which a failed one left. The keys run gives
# Go to Space 1 ⌃⌘1 and leaves the favorites out of ⌘1–⌘8, which a failed one left for the runs after.
# The translate run always translates Spanish, which a failed one left.
rm -f "$(dirname "$saved")/Agents" "$(dirname "$saved")/Shortcuts" \
  "$(dirname "$saved")/Always Translated"
# DevTools open where the profile last had them, so an undocked try would leave them in a window.
# The find run docks them, so they start on the right. The restore run's download goes in the
# check's own folder, never the user's Downloads. A failed extensions run may have left one of its
# extensions pinned. The profile keeps the clear run's answers to its sites' questions, and how
# often it dismissed them, for sites whose ports a later run may get, and a failed clear run may
# have left localhost out of Developer Mode. A failed media run may have left the Mini Player off,
# which the core keeps as Block for every site's Automatic picture-in-picture and doesn't undo at
# launch, so no later run's call floated (#1165). A failed favorites run may have left the first
# profile's archive at 24 hours, which the profile keeps as its own and no later run's menu then
# showed at 12 (#1982); dropped, the profile takes Settings' default again.
# Each file is Chrome's or the core's, in UTF-8, which Windows' Python doesn't read by default, as a
# Space's emoji in Spaces.json needs (#1673).
prefs="$profile_dir/Default/Preferences"
mkdir -p "$(dirname "$prefs")" "$out/downloads"
python3 -I -c '
import json, os, sys
prefs = {}
if os.path.exists(sys.argv[1]):
    with open(sys.argv[1], encoding="utf-8") as f:
        prefs = json.load(f)
prefs.setdefault("devtools", {}).setdefault("preferences", {})["currentDockState"] = "\"right\""
prefs.setdefault("download", {})["default_directory"] = sys.argv[2]
prefs.setdefault("extensions", {})["pinned_extensions"] = []
exceptions = prefs.get("profile", {}).get("content_settings", {}).get("exceptions", {})
for kind in ("window_placement", "permission_autoblocking_data", "auto_picture_in_picture"):
    exceptions.pop(kind, None)
defaults = prefs.get("profile", {}).get("default_content_setting_values", {})
defaults.pop("auto_picture_in_picture", None)
for key in ("developer_mode", "archive_after"):
    prefs.get("browser_core", {}).pop(key, None)
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(prefs, f)
' "$prefs" "$(native_path "$PWD/$out/downloads")"
python3 -I -c '
import json, os, sys
if not os.path.exists(sys.argv[1]):
    sys.exit()
with open(sys.argv[1], encoding="utf-8") as f:
    saved = json.load(f)
if saved.get("settings", {}).get("auto_picture_in_picture") is False:
    saved["settings"]["auto_picture_in_picture"] = True
    with open(sys.argv[1], "w", encoding="utf-8") as f:
        json.dump(saved, f)
' "$saved"
# Each run starts in the one profile, Default: Chrome deletes the others, which a run before made,
# as import's Work, or a failed run left, at its next launch.
only_default() {
  python3 -I -c '
import json, os, sys
if not os.path.exists(sys.argv[1]):
    sys.exit()
with open(sys.argv[1], encoding="utf-8") as f:
    local = json.load(f)
profile = local.setdefault("profile", {})
left = [name for name in profile.get("info_cache", {}) if name != "Default"]
for name in left:
    del profile["info_cache"][name]
profile["last_used"] = "Default"
profile["last_active_profiles"] = profile["profiles_order"] = ["Default"]
deleted = local.setdefault("profiles", {}).setdefault("profile_basenames_deleted", [])
deleted.extend(name for name in left if name not in deleted)
with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(local, f)
' "$(dirname "$saved")/Local State"
}
. check/lib.sh
trap cleanup EXIT
# The window follows the Mac's appearance and the runs read its dark look, so a check VM runs dark,
# and cleanup puts back the appearance it had. The owner's Mac keeps its own.
if [ "$(uname)" = Darwin ] && [ "$(sysctl -n kern.hv_vmm_present)" = 1 ]; then
  dark_mode=$(osascript -e 'tell application "System Events" to tell appearance preferences to get dark mode') || {
    echo "FAIL: System Events didn't say the Mac's appearance: allow the check to control it" >&2
    exit 1
  }
  appearance true
fi
# The runs send the app's driver its commands through it.
mkfifo "$out/drive"
# A run with a profile of its own, such as adblock's, sets profile_dir to it, and the runs after
# start with the check's again, or what they reset would be that profile's (#731).
check_profile_dir=$profile_dir
for run in ${*:-$runs}; do
  profile_dir=$check_profile_dir
  only_default
  # Rules a run made, as import's from Arc, would send a later run's links to another Space and
  # put their rows under its clicks, so each run starts without them, as it does without Spaces.
  rm -f "$profile_dir/Link rules"
  # And without Hide Space Header's choice, which spaces' makes and takes back.
  rm -f "$profile_dir/Space Header"
  # And without the command bar's picks, which put a command picked first among equal matches
  # (#1395), so a run finds the bar's own order and not what an earlier run picked, as Wayland's
  # pinned did after X11's clear had picked Clear Cookies and Refresh (#1953).
  rm -f "$profile_dir/Command bar picks"
  # The windows and clear runs keep 127.0.0.1 for Safari's engine, whose pages would then open there
  # in a later run if one failed, and clear the note it closed and the listed host it turned off.
  forget_engine_sites
  forget_appearance
  # Every run is sourced into this one shell, so a helper a run defines under a lib.sh name would
  # replace lib.sh's for the runs after it.
  . "check/$run.sh"
  # On Windows GPUI lost a window's move, size or modifiers that came during an update (#1358).
  if windows && grep -q '^browser: gpui dropped ' "$log"; then
    fail "GPUI dropped updates that came during another: $(grep '^browser: gpui dropped ' "$log" |
      sort | uniq -c | tr -s ' ' | tr '\n' ',')"
  fi
done
