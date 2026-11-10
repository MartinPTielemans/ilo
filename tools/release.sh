#!/bin/bash
# Makes the copy of ilo that goes to other Macs from the one `app/build.sh prod` made: signed with
# the Developer ID SIGN_IDENTITY names, with the hardened runtime, notarized and stapled, in
# app/build/release/<name>.dmg. tools/install.sh signs the owner's own copy instead.
# The notarytool profile is NOTARY_PROFILE, else "notary" (xcrun notarytool store-credentials
# notary). Without it the app is signed and packed but not notarized, and the run fails.
# The app carries the Developer ID provisioning profile PROVISIONING_PROFILE names, else
# app/branding/developer-id.provisionprofile, and app/entitlements/release.plist's keys.
# SIGN_IDENTITY=- signs ad hoc, to try the signing without the key; it stops before the DMG, and
# like the dev builds it has neither the profile nor release.plist's keys.
# Usage: SIGN_IDENTITY="Developer ID Application: … (TEAMID)" tools/release.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../app" && pwd)"
name=$(cat "$root/branding/name")
built=$root/build/Browser.app
out=$root/build/release
app=$out/$name.app
dmg=$out/$name.dmg
profile=${NOTARY_PROFILE:-notary}
identity=${SIGN_IDENTITY:?"name the Developer ID: SIGN_IDENTITY=\"Developer ID Application: … (TEAMID)\" (security find-identity -v -p codesigning)"}

if [ "$(plutil -extract CFBundleIdentifier raw "$built/Contents/Info.plist")" != "$(cat "$root/branding/id")" ]; then
  echo "$built isn't a prod build. Make one with app/build.sh prod." >&2
  exit 1
fi
# A copy without Sparkle's key could never be updated, so only a try at the signing may lack it.
if [ "$identity" != - ] && ! plutil -extract SUPublicEDKey raw "$built/Contents/Info.plist" > /dev/null 2>&1; then
  echo "$built has no updater. Build it with the key in app/branding/update-key (tools/appcast.sh)." >&2
  exit 1
fi

# macOS stops an app at launch that claims a key its embedded profile doesn't grant, so a profile
# that wouldn't do is refused here rather than shipped.
entitlements=$root/entitlements/app.plist
if [ "$identity" != - ]; then
  entitlements=$root/entitlements/release.plist
  provision=${PROVISIONING_PROFILE:-$root/branding/developer-id.provisionprofile}
  granted=$(mktemp)
  trap 'rm -f "$granted"' EXIT
  if ! security cms -D -i "$provision" > "$granted" 2> /dev/null; then
    echo "$provision isn't a provisioning profile. Make the Developer ID one (docs/releasing.md)." >&2
    exit 1
  fi
  signer=$(security find-certificate -c "$identity" -Z | awk '/^SHA-1/ { print $3; exit }')
  listed=
  i=0
  while certificate=$(plutil -extract "DeveloperCertificates.$i" raw -o - "$granted" 2> /dev/null); do
    [ "$(echo "$certificate" | base64 -D | shasum -a 1 | cut -d ' ' -f 1 | tr a-f A-F)" != "$signer" ] || listed=1
    i=$((i + 1))
  done
  if [ -z "$listed" ]; then
    echo "$provision doesn't list the certificate of $identity. Make one that does (docs/releasing.md)." >&2
    exit 1
  fi
  # Each key the app claims, but the hardened runtime's, which need no profile, has each of its
  # values among the profile's, some of which are patterns such as <team>.*.
  values() {
    local printed
    printed=$(/usr/libexec/PlistBuddy -c "Print :$2" "$1" 2> /dev/null) || return 0
    echo "$printed" | sed -e '/^Array {$/d' -e '/^}$/d' -e 's/^ *//'
  }
  set -f
  for key in $(plutil -convert xml1 -o - "$entitlements" | sed -n 's/^	<key>\(.*\)<\/key>$/\1/p'); do
    case $key in com.apple.security.*) continue ;; esac
    allowed=$(values "$granted" "Entitlements:$key")
    for value in $(values "$entitlements" "$key"); do
      ok=
      for pattern in $allowed; do
        case $value in $pattern) ok=1 ;; esac
      done
      if [ -z "$ok" ]; then
        echo "$provision doesn't grant $key = $value, which $entitlements claims." >&2
        exit 1
      fi
    done
  done
  set +f
fi

rm -rf "$out"
mkdir -p "$out"
ditto --norsrc --noextattr "$built" "$app"

sign() {
  # A timestamp makes the signature outlive the certificate; ad hoc signatures can't have one.
  if [ "$identity" = - ]; then
    codesign --force --sign - --options runtime "$@"
  else
    codesign --force --sign "$identity" --options runtime --timestamp "$@"
  fi
}

# Inside out and without --deep, which would sign every nested part with the outer part's
# entitlements: the loose libraries and executables first, then each helper app, then the
# framework that holds them, then the agents' bridge beside the app's executable, then the app.
framework=$app/Contents/Frameworks/Chromium\ Framework.framework
version=$(cd "$framework/Versions/Current" && pwd -P)
while IFS= read -r -d '' f; do
  case $(file -bh "$f") in Mach-O*) sign "$f" ;; esac
done < <(find "$version/Libraries" "$version/Helpers" -name '*.app' -prune -o -type f -print0)
for helper in "$version"/Helpers/*.app; do
  case $helper in
    # V8 and the GPU process's shader compilers need JIT, as in Chromium's own signing.
    *"Renderer).app" | *"GPU).app") sign --entitlements "$root/entitlements/helper-jit.plist" "$helper" ;;
    *) sign "$helper" ;;
  esac
done
sign "$framework"
sign "$app/Contents/MacOS/mcp-bridge"
# Sparkle's installer and its window, then Sparkle, as Sparkle's documentation signs them.
sparkle=$app/Contents/Frameworks/Sparkle.framework
if [ -d "$sparkle" ]; then
  sign "$sparkle/Versions/B/Autoupdate"
  sign "$sparkle/Versions/B/Updater.app"
  sign "$sparkle"
fi
# The team's requirement, as tools/install.sh signs the owner's copy, so either reads the other's
# Keychain items.
requirement=$("$(dirname "$0")/requirement.sh" "$(cat "$root/branding/id")" "$identity")
[ "$identity" = - ] || cp "$provision" "$app/Contents/embedded.provisionprofile"
sign --entitlements "$entitlements" ${requirement:+"-r=$requirement"} "$app"
codesign --verify --deep --strict --verbose=2 "$app"
[ "$identity" = - ] && exit 0
# What the app carries, read back from it.
codesign -d --entitlements - --xml "$app" 2> /dev/null | plutil -convert xml1 -o "$out/signed.plist" -
plutil -convert xml1 -o "$out/claimed.plist" "$entitlements"
if ! cmp -s "$out/signed.plist" "$out/claimed.plist" || ! cmp -s "$provision" "$app/Contents/embedded.provisionprofile"; then
  echo "$app doesn't carry $entitlements and $provision as signed." >&2
  exit 1
fi
rm "$out/signed.plist" "$out/claimed.plist"
plutil -p "$entitlements"

notarize() {
  local result id
  result=$(xcrun notarytool submit "$1" --keychain-profile "$profile" --wait)
  echo "$result"
  if ! echo "$result" | grep -q '^ *status: Accepted'; then
    id=$(echo "$result" | sed -n 's/^ *id: //p' | head -1)
    echo "Notarization didn't accept $1. Why: xcrun notarytool log $id --keychain-profile $profile" >&2
    exit 1
  fi
}

# The app goes to Apple first, so the copy in the DMG carries its own ticket and opens offline
# once it has left the DMG.
notarized=
if xcrun notarytool history --keychain-profile "$profile" > /dev/null 2>&1; then
  ditto -c -k --keepParent "$app" "$out/$name.zip"
  notarize "$out/$name.zip"
  rm "$out/$name.zip"
  xcrun stapler staple "$app"
  notarized=1
fi

"$(dirname "$0")/dmg.sh" "$app" "$dmg"
codesign --force --sign "$identity" --timestamp "$dmg"

if [ -z "$notarized" ]; then
  echo "Signed $dmg, but not notarized: there's no notarytool profile \"$profile\" in the Keychain." >&2
  echo "Make one with: xcrun notarytool store-credentials $profile, then run this again." >&2
  exit 1
fi
notarize "$dmg"
xcrun stapler staple "$dmg"
spctl -a -t exec -vv "$app"
spctl -a -t open --context context:primary-signature -vv "$dmg"

echo "$dmg"
