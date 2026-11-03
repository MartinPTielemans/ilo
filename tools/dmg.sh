#!/bin/bash
# Packs an app into the DMG a user opens to install it: a volume named after the app, with ilo's
# icon, whose Finder window shows the app and an Applications link over tools/dmg/background.svg.
# The layout is a .DS_Store tools/dmg/layout.py writes, so nothing drives Finder and it runs on a
# headless runner. tools/release.sh calls it after notarizing the app; the DMG still needs signing.
# Usage: tools/dmg.sh <path/to/Name.app> <out.dmg>
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
app=$1
dmg=$2
name=$(basename "$app" .app)

work=$(mktemp -d)
mount=$work/mount
cleanup() {
  hdiutil detach -quiet -force "$mount" 2> /dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

src=$work/src
mkdir -p "$src/.background"
ditto "$app" "$src/$name.app"
ln -s /Applications "$src/Applications"
tiffutil -cathidpicheck "$here/dmg/background.png" "$here/dmg/background@2x.png" \
  -out "$src/.background/background.tiff" 2> /dev/null
cp "$here/../app/branding/ilo.icns" "$src/.VolumeIcon.icns"

# Writable first, with room for the .DS_Store, which needs the files' IDs on the volume.
size=$(($(du -sm "$src" | cut -f1) + 16))
hdiutil create -quiet -volname "$name" -fs HFS+ -srcfolder "$src" -size "${size}m" -format UDRW \
  -ov "$work/rw.dmg"
mkdir "$mount"
hdiutil attach -quiet -nobrowse -noautoopen -owners off -mountpoint "$mount" "$work/rw.dmg"
python3 "$here/dmg/layout.py" "$mount" "$name" "$name.app"
# The volume's own icon is .VolumeIcon.icns once its root has the custom-icon Finder flag.
xattr -wx com.apple.FinderInfo \
  0000000000000000040000000000000000000000000000000000000000000000 "$mount"
rm -rf "$mount/.fseventsd" "$mount/.Trashes"
hdiutil detach -quiet "$mount"

rm -f "$dmg"
hdiutil convert -quiet "$work/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$dmg"
