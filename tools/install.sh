#!/bin/sh
# Installs the copy `app/build.sh prod` made, as /Applications/<name>.app or in the folder named,
# signed with the first code-signing identity in the Keychain, else ad hoc. macOS lets an app read
# its Keychain items while its signature stays alike, so signed with an identity it asks for the
# cookie key once, and ad hoc after every update. An identity Apple issued signs it with the
# team's requirement (requirement.sh), which a release's Developer ID meets too. Run it again to update; the profile, in
# Application Support, stays.
# Usage: tools/install.sh [folder]
set -e
root="$(cd "$(dirname "$0")/../app" && pwd)"
name=$(cat "$root/branding/name")
to=${1:-/Applications}
app=$root/build/Browser.app
if [ "$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist")" != "$(cat "$root/branding/id")" ]; then
  echo "$app isn't a prod build. Make one with app/build.sh prod." >&2
  exit 1
fi
# Swapping the bundle under a running Chromium would mix its helpers' versions.
if pgrep -f "$to/$name.app/Contents" > /dev/null; then
  echo "$name in $to is running. Quit it first." >&2
  exit 1
fi
identity=${SIGN_IDENTITY:-$(security find-identity -v -p codesigning |
  sed -n 's/^ *1) [0-9A-F]* "\(.*\)"$/\1/p')}
rm -rf "$to/$name.app"
ditto "$app" "$to/$name.app"
codesign --force --deep --sign "${identity:--}" "$to/$name.app"
# The app alone again, with the team's requirement, which --deep would give every helper too.
requirement=$("$(dirname "$0")/requirement.sh" "$(cat "$root/branding/id")" "${identity:--}")
[ -z "$requirement" ] || codesign --force --sign "$identity" -r="$requirement" "$to/$name.app"
codesign --verify --deep --strict "$to/$name.app"
echo "open $to/$name.app"
