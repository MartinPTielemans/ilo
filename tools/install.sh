#!/bin/sh
# Installs the copy `app/build.sh prod` made, as /Applications/<name>.app or in the folder named,
# signed with the first code-signing identity in the Keychain, else ad hoc. macOS lets an app read
# its Keychain items while its signature stays alike, so signed with an identity it asks for the
# cookie key once, and ad hoc after every update. An identity Apple issued signs it with the
# team's requirement (requirement.sh), which a release's Developer ID meets too. Run it again to update; the profile, in
# Application Support, stays.
# --rework installs the copy `app/build.sh rework` made instead, the owner's daily copy of the
# rework, as its own app beside the daily one: branding/rework's name and id, whose profile folder
# and Keychain items are its own. It refuses a rework branding, a build or an app already in the
# folder that is the daily app's, so it never replaces the daily app.
# --dry-run says what it would install where, and changes nothing.
# Usage: tools/install.sh [--rework] [--dry-run] [folder]
set -e
root="$(cd "$(dirname "$0")/../app" && pwd)"
rework= dry=
while :; do
  case ${1:-} in
  --rework) rework=1 ;;
  --dry-run) dry=1 ;;
  *) break ;;
  esac
  shift
done
daily_name=$(cat "$root/branding/name") daily_id=$(cat "$root/branding/id")
name=$daily_name id=$daily_id build="app/build.sh prod"
if [ -n "$rework" ]; then
  name=$(cat "$root/branding/rework/name") id=$(cat "$root/branding/rework/id")
  build="app/build.sh rework"
  if [ "$name" = "$daily_name" ] || [ "$id" = "$daily_id" ]; then
    echo "branding/rework names the daily app ($daily_name, $daily_id); the rework's copy never installs as it." >&2
    exit 1
  fi
fi
to=${1:-/Applications}
app=$root/build/Browser.app
target=$to/$name.app
bundle_id() {
  python3 -I -c 'import plistlib, sys; print(plistlib.load(open(sys.argv[1], "rb")).get("CFBundleIdentifier", ""))' \
    "$1/Contents/Info.plist" 2>/dev/null || true
}
if [ "$(bundle_id "$app")" != "$id" ]; then
  echo "$app isn't a build of $id. Make one with $build." >&2
  exit 1
fi
# An app of another id where the copy goes is another's, the daily app's above all.
if [ -e "$target" ] && [ "$(bundle_id "$target")" != "$id" ]; then
  echo "$target is $(bundle_id "$target"), not $id. It's left as it is." >&2
  exit 1
fi
# Swapping the bundle under a running Chromium would mix its helpers' versions.
if pgrep -f "$target/Contents" > /dev/null; then
  echo "$name in $to is running. Quit it first." >&2
  exit 1
fi
if [ -n "$dry" ]; then
  echo "would install $app ($id) as $target"
  exit 0
fi
identity=${SIGN_IDENTITY:-$(security find-identity -v -p codesigning |
  sed -n 's/^ *1) [0-9A-F]* "\(.*\)"$/\1/p')}
rm -rf "$target"
ditto "$app" "$target"
codesign --force --deep --sign "${identity:--}" "$target"
# The app alone again, with the team's requirement, which --deep would give every helper too.
requirement=$("$(dirname "$0")/requirement.sh" "$id" "${identity:--}")
[ -z "$requirement" ] || codesign --force --sign "$identity" -r="$requirement" "$target"
codesign --verify --deep --strict "$target"
echo "open $target"
