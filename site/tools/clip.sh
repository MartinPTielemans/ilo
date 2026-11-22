#!/bin/sh
# Makes a changelog clip from a recording of ilo: public/media/changelog/NAME.mp4, 1280 wide,
# silent and ready to stream, NAME.webp, its first frame, which shows until it plays, and
# NAME-still.webp, the frame at STILL seconds (default: the middle), which is all a visitor who
# prefers reduced motion sees. A changelog entry then names it with `clip: "NAME"` (src/changelog.ts).
# Usage: tools/clip.sh recording.mp4 NAME [STILL]
set -eu
src=${1:?usage: tools/clip.sh recording.mp4 NAME [STILL]}
name=${2:?usage: tools/clip.sh recording.mp4 NAME [STILL]}
out="$(cd "$(dirname "$0")/.." && pwd)/public/media/changelog"
mkdir -p "$out"
length=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$src")
still=${3:-$(echo "$length / 2" | bc -l)}

# 60 fps keeps the motion as ilo draws it; the interface's flat colour costs little at CRF 30.
ffmpeg -v error -y -i "$src" -an -vf "scale=1280:-2:flags=lanczos,fps=60" \
  -c:v libx264 -profile:v high -preset veryslow -crf 30 -pix_fmt yuv420p -movflags +faststart \
  "$out/$name.mp4"
# Homebrew's ffmpeg has no WebP encoder, so each frame goes through cwebp.
frame() {
  png=$(mktemp -d)/frame.png
  ffmpeg -v error -y -ss "$1" -i "$out/$name.mp4" -frames:v 1 "$png"
  cwebp -quiet -q 80 "$png" -o "$2"
  rm -r "$(dirname "$png")"
}
frame 0 "$out/$name.webp"
frame "$still" "$out/$name-still.webp"
ls -l "$out/$name.mp4" "$out/$name.webp" "$out/$name-still.webp"
