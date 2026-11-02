#!/bin/bash
# Captures the same screens of the preview at 1280 by 800 and lays them side by side in sheet.png,
# so text that fits on the Mac but not in another system's faces or windows shows without anyone
# taking screenshots: on Linux on Wayland and on X11, each in a headless sway session of its own
# (tools/headless.sh), and on Windows, a CI runner's, its desktop (tools/window-capture.ps1).
# The screens: the sidebar with tabs, the command bar, a tab's context menu, Site Controls, the
# icon picker, the delete-Space dialog, the sidebar of a Space whose theme has three colours,
# whose dot in the switcher draws all three (#1354), and Peek over the page, its card a hole the
# shell's page shows through. The preview's Linux host places no window, so
# sway puts Site Controls and the icon picker in the middle rather than where the Mac's host would.
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
# shell words, so a value with spaces is quoted.
screens=(
  "sidebar|drawn at|"
  "command-bar|command bar drew|PREVIEW_BAR=tabs"
  "context-menu|menu shown|PREVIEW_MENU=100,247"
  "site-controls|site controls settled|PREVIEW_SITE_CONTROLS=1"
  "icon-picker|icon picker for|PREVIEW_ICON_PICKER="
  "dialog|dialog settled|PREVIEW_DIALOG=1 PREVIEW_MENU=40,205,up,enter"
  "space-theme|drawn at|PREVIEW_THEME='x=0.5 y=0 colors=3'"
  "peek|link preview settled|PREVIEW_PEEK=1"
)
case $(uname -s) in
  MINGW* | MSYS*) systems=(windows) ;;
  *) systems=(wayland x11) ;;
esac
# Captures the screen, with the preview's settings in the environment.
capture() { # <system> <output.png> <ready>
  case $1 in
    windows) timeout 90 powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$here/window-capture.ps1" \
      "$2" "$3" "$preview" -Moving 400 ;;
    x11) timeout 90 "$here/headless.sh" --moving 400 --x11 "$2" "$3" "$preview" ;;
    *) timeout 90 "$here/headless.sh" --moving 400 "$2" "$3" "$preview" ;;
  esac
}
missing=()
tiles=()
for screen in "${screens[@]}"; do
  IFS='|' read -r name ready settings <<<"$screen"
  for system in "${systems[@]}"; do
    mkdir -p "$out/$system"
    png=$out/$system/$name.png
    if ! (eval "export PREVIEW_SIZE=1280x800 $settings" && capture "$system" "$png" "$ready" >/dev/null); then
      missing+=("$system/$name")
      rm -f "$png"
      "${convert[@]}" -size 1280x800 xc:'#301010' -font "$face" -pointsize 28 -fill white \
        -gravity center -annotate +0+0 "no capture: see $system/$name.log" "$out/$system/$name.missing.png"
      png=$out/$system/$name.missing.png
    fi
    tiles+=(-label "$name, $system" "$png")
  done
done
"${montage[@]}" -font "$face" -pointsize 20 -fill white -background '#101010' -tile 2x \
  -geometry +12+12 "${tiles[@]}" "$out/sheet.png"
echo "$out/sheet.png"
for gap in "${missing[@]}"; do
  echo "No capture of $gap: its log is $out/$gap.log." >&2
  [ -z "${GITHUB_ACTIONS:-}" ] || echo "::warning title=Parity captures::No capture of $gap"
done
exit 0
