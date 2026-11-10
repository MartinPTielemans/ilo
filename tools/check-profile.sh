#!/bin/bash
# Whether the Developer ID provisioning profile can sign a release with the identity $1. macOS
# won't start an app whose profile doesn't list its certificate, isn't for its App ID or doesn't
# grant a key it claims, so a release with such a profile would ship an app nobody can open.
# It reads only the profile and the Keychain, never Apple's API. release.yml runs it before the
# build, and tools/release.sh before signing.
# The profile is PROVISIONING_PROFILE, else app/entitlements/developer-id.provisionprofile. It
# must be for app/entitlements/release.plist's App ID, else PROFILE_APP_ID, and last
# PROFILE_DAYS more days, else 30.
# Usage: tools/check-profile.sh <identity>
set -euo pipefail
root="$(cd "$(dirname "$0")/../app" && pwd)"
identity=$1
profile=${PROVISIONING_PROFILE:-$root/entitlements/developer-id.provisionprofile}
entitlements=$root/entitlements/release.plist
days=${PROFILE_DAYS:-30}
renew="Make a new one in the developer portal and commit it (docs/releasing.md)."
fail() {
  echo "$profile $*" >&2
  exit 1
}

granted=$(mktemp)
trap 'rm -f "$granted"' EXIT
security cms -D -i "$profile" > "$granted" 2> /dev/null || fail "isn't a provisioning profile. $renew"

app_id=${PROFILE_APP_ID:-$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "$entitlements")}
for_id=$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.application-identifier' "$granted" 2> /dev/null || true)
[ "$for_id" = "$app_id" ] || fail "is for the App ID ${for_id:-(none)}, not $app_id. $renew"

certificate=$(security find-certificate -c "$identity" -p 2> /dev/null) || fail "can't be checked: no certificate \"$identity\" in the Keychain."
signer=$(echo "$certificate" | openssl x509 -noout -fingerprint -sha1 | sed 's/.*=//; s/://g')
listed=
i=0
while listing=$(plutil -extract "DeveloperCertificates.$i" raw -o - "$granted" 2> /dev/null); do
  [ "$(echo "$listing" | base64 -D | shasum -a 1 | cut -d ' ' -f 1 | tr a-f A-F)" != "$signer" ] || listed=1
  i=$((i + 1))
done
[ -n "$listed" ] || fail "doesn't list the signing certificate, \"$identity\" (SHA-1 $signer). $renew"

# The profile's own date, and its certificate's, which comes far sooner: a profile lasts 18 years.
expires=$(plutil -extract ExpirationDate raw "$granted")
[ "$(date -j -u -f '%Y-%m-%dT%H:%M:%SZ' "$expires" +%s)" -gt $(($(date +%s) + days * 86400)) ] ||
  fail "expires on $expires, within $days days. $renew"
ends=$(echo "$certificate" | openssl x509 -noout -enddate | sed 's/^notAfter=//')
echo "$certificate" | openssl x509 -noout -checkend $((days * 86400)) > /dev/null ||
  fail "lists a certificate that expires on $ends, within $days days. Renew the certificate in MAC_CERTIFICATE_P12, then the profile with it. $renew"

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
    [ -n "$ok" ] || fail "doesn't grant $key = $value, which $entitlements claims. $renew"
  done
done
set +f

echo "$profile can sign $app_id with \"$identity\": the certificate lasts until $ends, the profile until $expires."
