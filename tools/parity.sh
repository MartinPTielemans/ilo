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
# Mock only (#2340 and #2338, not for merge): the folding split row's states and its motion,
# and the return mark on the smallest tiles, in place of the screens.
screens=(
  "fold-two-left-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=dark"
  "fold-two-left-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-two-left-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=light"
  "fold-two-left-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-two-right-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=dark"
  "fold-two-right-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-two-right-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=light"
  "fold-two-right-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-three-first-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_APPEARANCE=dark"
  "fold-three-first-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-three-first-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_APPEARANCE=light"
  "fold-three-first-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-three-middle-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=dark"
  "fold-three-middle-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-three-middle-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=light"
  "fold-three-middle-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-three-last-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=dark"
  "fold-three-last-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-three-last-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=light"
  "fold-three-last-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-narrow-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_SIDEBAR=164 PREVIEW_APPEARANCE=dark"
  "fold-narrow-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_SIDEBAR=164 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-narrow-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_SIDEBAR=164 PREVIEW_APPEARANCE=light"
  "fold-narrow-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4 PREVIEW_SIDEBAR=164 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-stacked-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_STACKED=1 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=dark"
  "fold-stacked-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_STACKED=1 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-stacked-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_STACKED=1 PREVIEW_SELECT=5 PREVIEW_APPEARANCE=light"
  "fold-stacked-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_STACKED=1 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-hover-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_SPLIT_HOVER=3 PREVIEW_APPEARANCE=dark"
  "fold-hover-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_SPLIT_HOVER=3 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-hover-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_SPLIT_HOVER=3 PREVIEW_APPEARANCE=light"
  "fold-hover-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=3 PREVIEW_SPLIT_HOVER=3 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-drop-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_DROP=1 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=dark"
  "fold-drop-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_DROP=1 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-drop-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_DROP=1 PREVIEW_SELECT=4 PREVIEW_APPEARANCE=light"
  "fold-drop-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT_DROP=1 PREVIEW_SELECT=4 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "fold-unshown-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4,1 PREVIEW_APPEARANCE=dark"
  "fold-unshown-dark-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4,1 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=dark"
  "fold-unshown-light-before|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4,1 PREVIEW_APPEARANCE=light"
  "fold-unshown-light-fold|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=4,1 PREVIEW_FOLD=1 PREVIEW_APPEARANCE=light"
  "motion-0|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0 PREVIEW_APPEARANCE=dark"
  "motion-1|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.125 PREVIEW_APPEARANCE=dark"
  "motion-2|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.25 PREVIEW_APPEARANCE=dark"
  "motion-3|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.375 PREVIEW_APPEARANCE=dark"
  "motion-4|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.5 PREVIEW_APPEARANCE=dark"
  "motion-5|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.625 PREVIEW_APPEARANCE=dark"
  "motion-6|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.75 PREVIEW_APPEARANCE=dark"
  "motion-7|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=0.875 PREVIEW_APPEARANCE=dark"
  "motion-8|drawn at|PARITY_SCALE=2 PREVIEW_SPLIT=3 PREVIEW_SELECT=5 PREVIEW_FOLD=1 PREVIEW_SPLIT_FROM=3 PREVIEW_SPLIT_T=1 PREVIEW_APPEARANCE=dark"
  "tile47-dark-before|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_APPEARANCE=dark"
  "tile47-dark-badge|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=badge PREVIEW_APPEARANCE=dark"
  "tile47-dark-notch|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=notch PREVIEW_APPEARANCE=dark"
  "tile47-dark-dot|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=dot PREVIEW_APPEARANCE=dark"
  "tile47-light-before|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_APPEARANCE=light"
  "tile47-light-badge|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=badge PREVIEW_APPEARANCE=light"
  "tile47-light-notch|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=notch PREVIEW_APPEARANCE=light"
  "tile47-light-dot|drawn at|PARITY_SCALE=2 PREVIEW_AWAY=1 PREVIEW_SIDEBAR=173 PREVIEW_AWAY_MARK=dot PREVIEW_APPEARANCE=light"
)
case $(uname -s) in
  MINGW* | MSYS*) systems=(windows) drag= ;;
  *)
    systems=(x11)
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
    x11) timeout 90 "$here/headless.sh" --scale "${PARITY_SCALE:-1}" --moving 400 "${then[@]}" --x11 "$2" "$3" "$preview" ;;
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
