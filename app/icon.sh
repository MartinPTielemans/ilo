#!/bin/sh
# Makes the app's icon: the knot's drawing (assets/brand/knot.svg) becomes the Icon Composer
# document's one layer (icon-outline.swift says why it is one filled shape), and the document is
# compiled into the Assets.car and .icns build.sh copies into the app. All three are committed, so
# the app builds without Xcode. Run it after changing the drawing or the document; it needs
# Xcode's swiftc and actool. macOS 26 draws the four icon styles from Assets.car, and older ones
# show the .icns. The ground and the knot trade their two colours in the dark
# fill-specializations, or the knot disappears on the Dark style's ground.
set -e
cd "$(dirname "$0")/branding"
name=$(cat name)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
xcrun swiftc -O ../icon-outline.swift -o "$tmp/icon-outline"
"$tmp/icon-outline" ../assets/brand/knot.svg "$name.icon/Assets/knot.svg"
# The same shape with its curves as lines, and the two smaller drawings' shapes, for what fills
# only polygons: the Windows icons' script draws each size from the one that is its own. They
# are committed as the layer is, since that script runs where there is no Xcode.
for drawing in knot knot-32 knot-16; do
  "$tmp/icon-outline" "../assets/brand/$drawing.svg" "outline/$drawing.svg" lines
done
xcrun actool "$name.icon" --compile "$tmp" --platform macosx --minimum-deployment-target 26.0 \
  --app-icon "$name" --output-partial-info-plist "$tmp/partial.plist" > /dev/null
cp "$tmp/Assets.car" "$tmp/$name.icns" .
