#!/bin/bash
# Captures the same screens of the preview at 1280 by 800 and lays them side by side in sheet.png,
# so text that fits on the Mac but not in another system's faces or windows shows without anyone
# taking screenshots: on Linux on Wayland and on X11, each in a headless sway session of its own
# (tools/headless.sh), and on Windows, a CI runner's, its desktop (tools/window-capture.ps1).
# The screens: the sidebar with tabs, the command bar, a tab's context menu, Site Controls, the
# icon picker, the delete-Space dialog, the sidebar of a Space whose theme has three colours,
# whose dot in the switcher draws all three (#1354), Peek over the page, its card a hole the
# shell's page shows through, and more Spaces than fit (#1401): 16 in the switcher of a sidebar
# 300 wide, and 8 in the Library's Spaces in a window 1100 wide, and Settings › Sync at Settings'
# 720 by 480 least size, scrolled, the section's name held at the card's top, and the URL field's
# translate mark lit under the pointer, light and dark (#2112). The preview's Linux host places no window, so
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
# shell words, so a value with spaces is quoted, and for one taken under the pointer, what the
# preview logs ending in the point the pointer goes to.
screens=(
  "activity-closed|activity drawn|PREVIEW_ACTIVITY=closed"
  "activity-write|activity drawn|PREVIEW_ACTIVITY=write"
  "activity-refused|activity drawn|PREVIEW_ACTIVITY=refused"
  "activity-read|activity drawn|PREVIEW_ACTIVITY=read"
  "activity-write-light|activity drawn|PREVIEW_ACTIVITY=write PREVIEW_APPEARANCE=light"
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
"${montage[@]}" -font "$face" -pointsize 20 -fill white -background '#101010' -tile 2x \
  -geometry +12+12 "${tiles[@]}" "$out/sheet.png"
echo "$out/sheet.png"
for gap in "${missing[@]}"; do
  echo "No capture of $gap: its log is $out/$gap.log." >&2
  [ -z "${GITHUB_ACTIONS:-}" ] || echo "::warning title=Parity captures::No capture of $gap"
done
exit 0
