#!/bin/bash
# Captures the same screens of the preview at 1280 by 800 and lays them side by side in sheet.png,
# so text that fits on the Mac but not in another system's faces or windows shows without anyone
# taking screenshots: on Linux on Wayland and on X11, each in a headless sway session of its own
# (tools/headless.sh), and on Windows, a CI runner's, its desktop (tools/window-capture.ps1).
# The screens: the sidebar with tabs, on the left and on the right, where the window's buttons are
# at the window's right corner (#2375), and with it on the right the Library and its Spaces, on the
# sidebar's side, the window's buttons on its column (#2375), the command bar, a tab's context menu,
# Site Controls, the icon picker, the delete-Space dialog, the sidebar of a Space whose theme has
# three colours, whose dot in the switcher draws all three (#1354), Peek over the page, its card a
# hole the shell's page shows through, and more Spaces than fit (#1401): 16 in the switcher of a
# sidebar 300 wide, and 8 in the Library's Spaces in a window 1100 wide, and Settings › Sync signed
# in, a page taller than the card, at Settings' 720 by 480 least size, scrolled, the section's name
# held at the card's top (a page that fits never scrolls, #2102), and the URL
# field's translate mark lit under the pointer, light and dark (#2112), the foot bar with two lines
# over the page's bottom, as pointing's, whose keys read as this system's menus write them (#2380),
# Settings › Agents' Model menu for Claude, Sonnet chosen, and for Codex, on its own default, which
# names its model, light and dark (#2455), and its Model rows, one assistant signed in and one not,
# then Access and Notifications under them, light and dark (#4100), and the safe-start card at the
# sidebar's foot, light and dark,
# and in the narrowest sidebar with ad blocking off (#2345), a Live Folder of made-up pull
# requests, open light and dark, closed with its count, signed out and after a failed refresh,
# light and dark, with its tooltip then, the Space's menu with Live Folders open, light and dark,
# the folder's menu, and in the narrowest sidebar, and frames of a press on its row, 4 s in, for a
# video of it (#2979), on Wayland, the footer's menu control, which only Linux and Windows draw, at
# rest and with its menu open above it, light and dark (#3148), and Library › Boards, light and dark, a Board's row's menu,
# and the protected-media note, whose card a Board's refused save follows in the page (#3073), the
# keyboard's ring on New Tab's row in an empty Space, light and dark (#3151), and on Library ›
# Boards' second row, light and dark (#3238), an outside agent's card at work, done, and done
# with a page to open, light and dark (#3652), and Settings › Agents with the ilo command's row not
# there, there, and in a folder the shell doesn't search, light and dark, and its two longest
# lines in Settings' narrowest window, and a Flatpak's, not there and there with the line to run
# on the host, light and dark and in the narrowest window (#3667), and first run's sheet at its welcome and its
# colour, light and dark (#4093). The preview's Linux host places no window, so sway
# puts Site Controls, the icon picker and the tooltip in the middle rather than where the Mac's host
# would.
# It reports: a screen that won't capture is a gap in the sheet and a line on stderr, never a
# failure.
# Usage: tools/parity.sh <preview> <output directory>
set -uo pipefail
[ $# -eq 2 ] || { echo "usage: $0 <preview> <output directory>" >&2; exit 2; }
preview=$(realpath "$1")
out=$(realpath -m "$2")
here=$(cd "$(dirname "$0")" && pwd)
face=$here/../app/assets/fonts/Inter-Regular.ttf
if command -v magick >/dev/null; then convert=(magick) montage=(magick montage); else convert=(convert) montage=(montage); fi

# Each screen: its name, what it waits for in the preview's log, and the preview's settings, as
# shell words, so a value with spaces is quoted, and for one taken under the pointer, what the
# preview logs ending in the point the pointer goes to. An overlay, a window of its own, waits for
# its line of the first frame seen: on X11 the picture can hold still before that frame reaches
# the output, so a line logged as it's shown let a capture miss it (#3805).
screens=(
  "sidebar|drawn at|"
  "sidebar-right|drawn at|PREVIEW_SIDEBAR_RIGHT=1"
  "library-right|library shown|PREVIEW_SIDEBAR_RIGHT=1 PREVIEW_LIBRARY=1"
  "library-spaces-right|library shown|PREVIEW_SIDEBAR_RIGHT=1 PREVIEW_LIBRARY=spaces"
  "command-bar|command bar drew|PREVIEW_BAR=tabs"
  "context-menu|menu drawn|PREVIEW_MENU=100,247"
  "site-controls|site controls settled|PREVIEW_SITE_CONTROLS=1"
  "icon-picker|icon picker drawn|PREVIEW_ICON_PICKER="
  "dialog|dialog settled|PREVIEW_DIALOG=1 PREVIEW_MENU=40,205,up,enter"
  "space-theme|drawn at|PREVIEW_THEME='x=0.5 y=0 colors=3'"
  "peek|link preview settled|PREVIEW_PEEK=1"
  "many-spaces|drawn at|PREVIEW_SPACES=16 PREVIEW_SIDEBAR=300"
  "many-spaces-library|library shown|PREVIEW_SPACES=8 PREVIEW_LIBRARY=spaces PREVIEW_SIZE=1100x700"
  "settings-least|settings held Sync|PREVIEW_ACCOUNT=signed-in PREVIEW_SIZE=720x480 PREVIEW_SCROLL=200"
  "translate-hover-dark|translate mark drawn|PREVIEW_TRANSLATE=es PREVIEW_APPEARANCE=dark|translate mark drawn at"
  "translate-hover-light|translate mark drawn|PREVIEW_TRANSLATE=es PREVIEW_APPEARANCE=light|translate mark drawn at"
  "foot-bar|foot bar drew|PREVIEW_FOOT_BAR=point"
  "model-menu-claude-dark|menu drawn|PREVIEW_MODEL_MENU=claude,sonnet PREVIEW_APPEARANCE=dark"
  "model-menu-claude-light|menu drawn|PREVIEW_MODEL_MENU=claude,sonnet PREVIEW_APPEARANCE=light"
  "model-menu-codex-dark|menu drawn|PREVIEW_MODEL_MENU=codex PREVIEW_APPEARANCE=dark"
  "model-menu-codex-light|menu drawn|PREVIEW_MODEL_MENU=codex PREVIEW_APPEARANCE=light"
  "safe-start-light|safe start card 0 at|PREVIEW_SAFE_START=1 PREVIEW_APPEARANCE=light"
  "safe-start-dark|safe start card 0 at|PREVIEW_SAFE_START=1 PREVIEW_APPEARANCE=dark"
  "safe-start-narrow|safe start card 0 at|PREVIEW_SAFE_START=unblocked PREVIEW_SIDEBAR=164"
  "app-menu-rest-light|drawn at|PREVIEW_APPEARANCE=light"
  "app-menu-rest-dark|drawn at|PREVIEW_APPEARANCE=dark"
  "app-menu-light|menu drawn|PREVIEW_APPEARANCE=light PREVIEW_CLICK=170,775"
  "app-menu-dark|menu drawn|PREVIEW_APPEARANCE=dark PREVIEW_CLICK=170,775"
  "live-folder-light|live folder 9000 answered|PREVIEW_LIVE_FOLDER=open PREVIEW_APPEARANCE=light"
  "live-folder-dark|live folder 9000 answered|PREVIEW_LIVE_FOLDER=open PREVIEW_APPEARANCE=dark"
  "live-folder-closed|live folder 9000 answered|PREVIEW_LIVE_FOLDER=closed"
  "live-folder-signed-out|live folder 9000 answered|PREVIEW_LIVE_FOLDER=signed-out"
  "live-folder-signed-out-dark|live folder 9000 answered|PREVIEW_LIVE_FOLDER=signed-out PREVIEW_APPEARANCE=dark"
  "live-folder-failed|live folder 9000 answered failed|PREVIEW_LIVE_FOLDER=failed"
  "live-folder-failed-dark|live folder 9000 answered failed|PREVIEW_LIVE_FOLDER=failed PREVIEW_APPEARANCE=dark"
  "live-folder-tooltip|tooltip drawn|PREVIEW_LIVE_FOLDER=failed PREVIEW_HOVER=90,386"
  "live-folder-space-menu|menu submenu Live .olders drawn|PREVIEW_LIVE_FOLDER=open PREVIEW_MENU=40,221,l,right"
  "live-folder-space-menu-dark|menu submenu Live .olders drawn|PREVIEW_LIVE_FOLDER=open PREVIEW_APPEARANCE=dark PREVIEW_MENU=40,221,l,right"
  "live-folder-menu|menu drawn|PREVIEW_LIVE_FOLDER=open PREVIEW_MENU=90,386"
  "live-folder-narrow|live folder 9000 answered|PREVIEW_LIVE_FOLDER=open PREVIEW_SIDEBAR=164"
  "live-folder-states-light|live folder 9000 answered|PREVIEW_LIVE_FOLDER=states PREVIEW_APPEARANCE=light"
  "live-folder-states-dark|live folder 9000 answered|PREVIEW_LIVE_FOLDER=states PREVIEW_APPEARANCE=dark"
  "live-folder-press|selected 9001|PREVIEW_LIVE_FOLDER=open PREVIEW_APPEARANCE=light PREVIEW_PRESS=100,553"
  "library-boards-light|library boards|PREVIEW_LIBRARY=boards PREVIEW_APPEARANCE=light"
  "library-boards-dark|library boards|PREVIEW_LIBRARY=boards PREVIEW_APPEARANCE=dark"
  "library-boards-menu|menu drawn|PREVIEW_LIBRARY=boards PREVIEW_MENU=180,111"
  "library-row-ring-light|library row ring at|PREVIEW_LIBRARY=boards PREVIEW_LIBRARY_ROW=2 PREVIEW_APPEARANCE=light"
  "library-row-ring-dark|library row ring at|PREVIEW_LIBRARY=boards PREVIEW_LIBRARY_ROW=2 PREVIEW_APPEARANCE=dark"
  "drm-note|drm note|PREVIEW_DRM=1"
  "new-tab-ring-light|sidebar ring 0 at|PREVIEW_EMPTY=new PREVIEW_FOCUS_SIDEBAR=1 PREVIEW_APPEARANCE=light"
  "new-tab-ring-dark|sidebar ring 0 at|PREVIEW_EMPTY=new PREVIEW_FOCUS_SIDEBAR=1 PREVIEW_APPEARANCE=dark"
  "session-working-light|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=working PREVIEW_APPEARANCE=light"
  "session-working-dark|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=working PREVIEW_APPEARANCE=dark"
  "session-done-light|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=done PREVIEW_APPEARANCE=light"
  "session-done-dark|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=done PREVIEW_APPEARANCE=dark"
  "session-open-light|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=open PREVIEW_APPEARANCE=light"
  "session-open-dark|drawn at|PREVIEW_TASKS=1 PREVIEW_SESSION=open PREVIEW_APPEARANCE=dark"
  "command-line-off-light|settings ilo-command|PREVIEW_COMMAND_LINE=off PREVIEW_APPEARANCE=light"
  "command-line-off-dark|settings ilo-command|PREVIEW_COMMAND_LINE=off PREVIEW_APPEARANCE=dark"
  "onboarding-welcome-light|onboarding step intro settled|PREVIEW_ONBOARDING=light PREVIEW_ONBOARDING_STEP=1 PREVIEW_APPEARANCE=light"
  "onboarding-welcome-dark|onboarding step intro settled|PREVIEW_ONBOARDING=dark PREVIEW_ONBOARDING_STEP=1 PREVIEW_APPEARANCE=dark"
  "onboarding-color-light|onboarding step color settled|PREVIEW_ONBOARDING=light PREVIEW_ONBOARDING_STEP=6 PREVIEW_ONBOARDING_PICK=4 PREVIEW_APPEARANCE=light"
  "onboarding-color-dark|onboarding step color settled|PREVIEW_ONBOARDING=dark PREVIEW_ONBOARDING_STEP=6 PREVIEW_ONBOARDING_PICK=4 PREVIEW_APPEARANCE=dark"
  "command-line-on-light|settings ilo-command|PREVIEW_COMMAND_LINE=on PREVIEW_APPEARANCE=light"
  "command-line-on-dark|settings ilo-command|PREVIEW_COMMAND_LINE=on PREVIEW_APPEARANCE=dark"
  "command-line-unsearched-light|settings ilo-command|PREVIEW_COMMAND_LINE=unsearched PREVIEW_APPEARANCE=light"
  "command-line-unsearched-dark|settings ilo-command|PREVIEW_COMMAND_LINE=unsearched PREVIEW_APPEARANCE=dark"
  "command-line-off-narrow|settings ilo-command|PREVIEW_COMMAND_LINE=off PREVIEW_APPEARANCE=light PREVIEW_SETTINGS_WIDTH=720"
  "command-line-unsearched-narrow|settings ilo-command|PREVIEW_COMMAND_LINE=unsearched PREVIEW_APPEARANCE=light PREVIEW_SETTINGS_WIDTH=720"
  "command-line-flatpak-off-light|settings ilo-command|PREVIEW_COMMAND_LINE=flatpak-off PREVIEW_APPEARANCE=light"
  "command-line-flatpak-on-light|settings ilo-command|PREVIEW_COMMAND_LINE=flatpak-on PREVIEW_APPEARANCE=light"
  "command-line-flatpak-on-dark|settings ilo-command|PREVIEW_COMMAND_LINE=flatpak-on PREVIEW_APPEARANCE=dark"
  "command-line-flatpak-on-narrow|settings ilo-command|PREVIEW_COMMAND_LINE=flatpak-on PREVIEW_APPEARANCE=light PREVIEW_SETTINGS_WIDTH=720"
  "assistants-start-light|settings assistant rows|PREVIEW_ASSISTANTS=start PREVIEW_APPEARANCE=light"
  "assistants-start-dark|settings assistant rows|PREVIEW_ASSISTANTS=start PREVIEW_APPEARANCE=dark"
  "assistants-signed-in-light|settings assistant rows|PREVIEW_ASSISTANTS=signed-in PREVIEW_APPEARANCE=light"
  "assistants-signed-in-dark|settings assistant rows|PREVIEW_ASSISTANTS=signed-in PREVIEW_APPEARANCE=dark"
  "assistants-models-light|settings assistant rows|PREVIEW_ASSISTANTS=models PREVIEW_APPEARANCE=light"
  "assistants-models-dark|settings assistant rows|PREVIEW_ASSISTANTS=models PREVIEW_APPEARANCE=dark"
)
case $(uname -s) in
  MINGW* | MSYS*) systems=(windows) drag= ;;
  *)
    systems=(wayland x11)
    cargo build -q --release --manifest-path "$here/../app/check/drag/Cargo.toml" \
      --target-dir "$here/../app/target"
    drag=$here/../app/target/release/drag
    ;;
esac
# Captures the screen, with the preview's settings in the environment, and with a point, again
# with the pointer there, at the window's own point, as the window fills the output. A headless
# sway seat has no pointer, so check/drag lends it one, which lives until the capture's taken; it
# moves twice, as the window may take the seat's pointer only once the first move has given it one.
capture() { # <system> <output.png> <ready> [<point>]
  local then=() point=()
  [ -z "${4:-}" ] || point=(-Point "$4")
  [ -z "${4:-}" ] || then=(--then "set -e; at=\$(grep -m1 '$4' \"\$LOG\" | awk '{ print \$(NF - 1), \$NF }')
    set -- \$at
    { sleep 0.5; echo \"move \$1 \$2\"; sleep 0.5; echo \"move \$((\$1 + 1)) \$2\"; sleep 3; } |
      '$drag' 1280x800 > /dev/null &
    sleep 2; grim -c '$2'; wait")
  case $1 in
    windows) timeout 90 powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$here/window-capture.ps1" \
      "$2" "$3" "$preview" -Moving 400 "${point[@]}" ;;
    x11) timeout 90 "$here/headless.sh" --moving 400 "${then[@]}" --x11 "$2" "$3" "$preview" ;;
    *) timeout 90 "$here/headless.sh" --moving 400 "${then[@]}" "$2" "$3" "$preview" ;;
  esac
}
missing=()
tiles=()
for screen in "${screens[@]}"; do
  IFS='|' read -r name ready settings point <<<"$screen"
  for system in "${systems[@]}"; do
    mkdir -p "$out/$system"
    png=$out/$system/$name.png
    if ! (eval "export PREVIEW_SIZE=1280x800 $settings" && capture "$system" "$png" "$ready" "$point" >/dev/null); then
      missing+=("$system/$name")
      rm -f "$png"
      "${convert[@]}" -size 1280x800 xc:'#301010' -font "$face" -pointsize 28 -fill white \
        -gravity center -annotate +0+0 "no capture: see $system/$name.log" "$out/$system/$name.missing.png"
      png=$out/$system/$name.missing.png
    fi
    tiles+=(-label "$name, $system" "$png")
  done
done
# The frames: grim as fast as it goes once the folder's rows are still, to 2 s after the press,
# which the preview logs, named by the ms since the first, with the press's ms in press. A press
# before the first frame, or none in 10 s, is no capture, never frames that missed it.
if [ "${systems[0]}" = wayland ]; then
  frames=$out/wayland/live-folder-press-frames
  mkdir -p "$frames"
  # shellcheck disable=SC2016 # headless.sh runs it, with LOG its program's log.
  grab='set -e
    ms() { date +%s%3N; }
    ! grep -q "^browser: preview press$" "$LOG" || { echo "pressed before the first frame" >&2; exit 1; }
    start=$(ms) pressed= n=0
    while [ -z "$pressed" ] || [ $(($(ms) - pressed)) -lt 2000 ]; do
      [ $(($(ms) - start)) -lt 10000 ] || { echo "no press in 10 s" >&2; exit 1; }
      grim "$FRAMES/frame-$n.png"
      echo "$n $(ms)" >> "$FRAMES/times"
      n=$((n + 1))
      [ -n "$pressed" ] || ! grep -q "^browser: preview press$" "$LOG" || pressed=$(ms)
    done
    [ "$n" -gt 1 ] && [ "$(head -1 "$FRAMES/times" | cut -d" " -f2)" -lt "$pressed" ] ||
      { echo "no frame before the press" >&2; exit 1; }
    first=$(head -1 "$FRAMES/times" | cut -d" " -f2)
    while read -r i at; do
      mv "$FRAMES/frame-$i.png" "$FRAMES/$(printf %05d $((at - first))).png"
    done < "$FRAMES/times"
    echo $((pressed - first)) > "$FRAMES/press"
    rm "$FRAMES/times"'
  if ! (export PREVIEW_SIZE=1280x800 PREVIEW_LIVE_FOLDER=open PREVIEW_APPEARANCE=light \
    PREVIEW_PRESS=100,553,4000 FRAMES="$frames" &&
    timeout 90 "$here/headless.sh" --moving 400 --then "$grab" "$frames/start.png" \
      'live folder 9000 answered' "$preview" > "$out/wayland/live-folder-press-frames.log" 2>&1); then
    missing+=("wayland/live-folder-press-frames")
  fi
fi
"${montage[@]}" -font "$face" -pointsize 20 -fill white -background '#101010' -tile 2x \
  -geometry +12+12 "${tiles[@]}" "$out/sheet.png"
echo "$out/sheet.png"
for gap in "${missing[@]}"; do
  echo "No capture of $gap: its log is $out/$gap.log." >&2
  [ -z "${GITHUB_ACTIONS:-}" ] || echo "::warning title=Parity captures::No capture of $gap"
done
exit 0
