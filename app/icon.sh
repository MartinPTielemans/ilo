#!/bin/sh
# Compiles branding/'s Icon Composer icon into the Assets.car build.sh copies into the app, which is
# committed so the app builds without Xcode. Run it after changing the icon; it needs Xcode's
# actool. macOS 26 draws the four icon styles from Assets.car. The .icns, which the DMG's volume
# and anything reading the bundle's icon file show, is branding/render.sh's, from the masters drawn
# to whole pixels, not actool's, which macOS would draw small from the large art. The background
# and the dot keep their dark fill-specializations, the field is hidden in dark, and the dot, the
# stem, the ground line and the field stay layers of their own, so the Tinted and Clear styles
# draw each apart.
set -e
cd "$(dirname "$0")/branding"
name=$(cat name)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
xcrun actool "$name.icon" --compile "$tmp" --platform macosx --minimum-deployment-target 26.0 \
  --app-icon "$name" --output-partial-info-plist "$tmp/partial.plist" > /dev/null
cp "$tmp/Assets.car" .
