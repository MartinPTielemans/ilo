# check.sh runs this at 810, only when named.
# The profile survives a change of signing identity, as from Apple Development to Developer ID,
# with no Keychain prompt (platform/mac/updates.mm, tools/requirement.sh). A copy of the prod build
# signed with identity A and its own requirement, as Apple Development's names its certificate,
# takes a login cookie. Signed with A again under the team's requirement, it makes its cookie key's
# item again, granted to that requirement. Signed with identity B, the cookie is still sent.
#
# Only in a check VM, as it uses the real Keychain, the VM's login keychain: never the owner's.
# The prod build first, ./build.sh prod, then ./check.sh keychain, alone.
#
# A and B are throwaway self-signed identities made here, in a keychain of the run's own that's
# deleted after; their OU stands in for the team. Apple's anchor has no stand-in: codesign over ssh
# can't sign with a leaf whose authority isn't trusted (errSecInternalComponent), and trusting one
# needs a user at the screen. The copies have a name of their own for each run, so the cookie key's item is the run's.
# A team's identities share a Keychain partition, teamid:<team>, which identities with no team ID
# don't, so with KEYCHAIN_PASSWORD, the password of the default keychain the item is made in, the
# VM user's login password, the run adds each copy's own to the item's as a stand-in. Without it, a prompt the partition causes fails the run, and says so.

[ "$(uname)" = Darwin ] && [ "$(sysctl -n kern.hv_vmm_present)" = 1 ] || {
  echo "skip: keychain. It uses the real Keychain, so it runs only in a check VM."
  exit 0
}
id=$(cat branding/id)
[ "$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist")" = "$id" ] ||
  fail "$app isn't a prod build, which uses the real Keychain. Build one with ./build.sh prod."
serve
dir="$PWD/$out/keychain"
mkdir -p "$dir"
check_id="$id.keychain-check"
check_name="$app_name Keychain Check $port"
first_name="$check_name"
service="$check_name Safe Storage"
procs="$dir"
log="$out/keychain-0.log"
defaults delete "$check_id" > /dev/null 2>&1 || true
# Over ssh the login keychain is locked, and Chromium then runs with no cookie key at all. The
# default keychain, as GitHub's hosted Mac makes one of its own whose password it knows.
if [ -n "${KEYCHAIN_PASSWORD:-}" ]; then
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" ||
    fail "the default keychain didn't unlock: is KEYCHAIN_PASSWORD the VM's login password?"
fi

# A and B, for code signing, in the run's own keychain, which codesign may use without asking.
openssl=/usr/bin/openssl
cat > "$dir/openssl.cnf" << 'EOF'
[req]
distinguished_name = name
[name]
[signer]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
EOF
(
  cd "$dir"
  for who in A B; do
    $openssl req -config openssl.cnf -x509 -newkey rsa:2048 -nodes -days 2 -keyout $who.key \
      -out $who.pem -subj "/CN=$check_name $who/OU=ILOCHECK" -extensions signer
    $openssl pkcs12 -export -inkey $who.key -in $who.pem -passout pass:check -out $who.p12
  done
) > "$dir/openssl.log" 2>&1 || fail "openssl couldn't make the run's identities: $dir/openssl.log"
sha1() { $openssl x509 -noout -fingerprint -sha1 -in "$dir/$1.pem" | sed 's/.*=//; s/://g'; }
signer_a=$(sha1 A)
signer_b=$(sha1 B)
identities="$dir/identities.keychain-db"
searched=$(security list-keychains -d user | tr -d '"')
keychain_cleanup() {
  # shellcheck disable=SC2086 # the keychains, one a word, as list-keychains printed them
  security list-keychains -d user -s $searched 2> /dev/null || true
  security delete-keychain "$identities" 2> /dev/null || true
  # Deleting the run's cookie key may ask, so it gets 5 s.
  for name in "$first_name" "$first_name Spare"; do
    for item in "$name Safe Storage" "$name Safe Storage (moving)"; do
      security delete-generic-password -s "$item" -a "$name" > /dev/null 2>&1 &
      deleting=$!
      (sleep 5 && kill "$deleting" 2> /dev/null) &
    done
  done
}
# cleanup takes the run's status, which keychain_cleanup would otherwise replace with its own.
trap 'status=$?; keychain_cleanup; cleanup "$status"' EXIT
security create-keychain -p check "$identities"
security set-keychain-settings "$identities"
security unlock-keychain -p check "$identities"
for who in A B; do
  security import "$dir/$who.p12" -k "$identities" -P check -T /usr/bin/codesign > /dev/null ||
    fail "the Keychain didn't take identity $who"
done
security set-key-partition-list -S apple-tool:,apple: -s -k check "$identities" > /dev/null ||
  fail "codesign wasn't allowed the run's identities"
# shellcheck disable=SC2086
security list-keychains -d user -s "$identities" $searched

# A copy of the prod build at $dir/$1, as the check's app, without Sparkle, which would check the
# product's feed, signed with the identity $2 under the requirement $3, as tools/install.sh signs.
copies=
sign_copy() {
  bundle="$dir/$1/$check_name.app"
  rm -rf "${dir:?}/$1"
  mkdir -p "$dir/$1"
  ditto "$app" "$bundle"
  rm -rf "$bundle/Contents/Frameworks/Sparkle.framework"
  plutil -replace CFBundleIdentifier -string "$check_id" "$bundle/Contents/Info.plist"
  plutil -replace CFBundleName -string "$check_name" "$bundle/Contents/Info.plist"
  if ! { codesign --force --deep --keychain "$identities" --sign "$2" "$bundle" &&
    codesign --force --keychain "$identities" --sign "$2" -r="$3" "$bundle"; } > "$dir/$1.sign.log" 2>&1; then
    fail "codesign couldn't sign the $1 copy: $dir/$1.sign.log"
  fi
  copies="$copies,cdhash:$(codesign -dvvv "$bundle" 2>&1 | sed -n 's/^CDHash=//p')"
}
# The stand-in for the team's partition, once the item is there.
share_partition() {
  [ -n "${KEYCHAIN_PASSWORD:-}" ] || return 0
  security set-generic-password-partition-list -s "$service" -a "$check_name" \
    -S "apple-tool:,apple:$copies" -k "$KEYCHAIN_PASSWORD" > /dev/null 2>&1 ||
    fail "the cookie key's partitions couldn't be set: is KEYCHAIN_PASSWORD the VM's login password?"
}
# Launches the copy at $dir/$1 with the log $2 and the page $3, waits for the server to get the
# page with the cookies $4, and quits it.
run_copy() {
  log="$2"
  # The server's lines from this launch on.
  since=$(($(wc -l < "$out/site.log") + 1))
  request="^GET chromium 127.0.0.1:$port/$3 cookie $4\$"
  sent() { tail -n "+$since" "$out/site.log" | grep -q "$request"; }
  count_off "$dir/profile"
  BROWSER_PROFILE_DIR="$dir/profile" BROWSER_URL="$site$3" \
    "$dir/$1/$check_name.app/Contents/MacOS/Browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  for _ in $(seq 300); do
    sent && break
    kill -0 "$pid" 2> /dev/null || fail "the $1 copy exited"
    sleep 0.1
  done
  if ! sent; then
    asked="$(pgrep -x SecurityAgent > /dev/null && echo ", and macOS is asking for a password")"
    partition=
    [ -n "${KEYCHAIN_PASSWORD:-}" ] || partition=" Without KEYCHAIN_PASSWORD, the item's partition can ask."
    fail "the $1 copy didn't send the cookies $4 for /$3$asked.$partition Sent: $(tail -n "+$since" "$out/site.log" | grep "/$3 cookie" | tail -1)"
  fi
  await_load
  osascript -e "tell application id \"$check_id\" to quit"
  exited 30
}

# A's own requirement, as Apple Development's names its certificate, then the team's, as
# tools/requirement.sh makes it, with ILOCHECK for the team and no anchor.
own="designated => identifier \"$check_id\" and certificate leaf = H\"$signer_a\""
team="designated => identifier \"$check_id\" and certificate leaf[subject.OU] = \"ILOCHECK\""

# A first launch makes the cookie key, and the server's page sets its cookies, h from the server,
# HttpOnly as a login's is, and n from the page. The profile keeps them, encrypted with the key.
rm -rf "$dir/profile"
sign_copy own-a "$signer_a" "$own"
run_copy own-a "$out/keychain-0.log" "session?set" none

# Under the team's requirement, a new signature: the copy reads the key, as A meets the item's
# requirement, and makes the item again, granted to the team's.
sign_copy team-a "$signer_a" "$team"
share_partition
run_copy team-a "$out/keychain-1.log" session "h=$port; n=$port"
grep -q "^browser: the cookie key regranted to .*: 0\$" "$out/keychain-1.log" ||
  fail "the copy under the team's requirement didn't make the cookie key's item again: $(grep 'cookie key' "$out/keychain-1.log")"
# The spare holds the key only while the item is made again.
! security find-generic-password -s "$service (moving)" -a "$check_name" > /dev/null 2>&1 ||
  fail "the cookie key's spare, \"$service (moving)\", is still in the Keychain"

# B, a new identity under the same requirement, reads the key with no prompt, and sends the cookies.
sign_copy team-b "$signer_b" "$team"
share_partition
run_copy team-b "$out/keychain-2.log" session "h=$port; n=$port"
grep -q "cookie key" "$out/keychain-2.log" &&
  fail "B's copy went through the cookie key again: $(grep 'cookie key' "$out/keychain-2.log")"
# A launch that stopped while the item was made again left the key only in the spare. The next
# launch puts it back, and the profile's cookies, encrypted with it, are still sent. The run can't
# delete an item the app made without macOS asking, so this copy has a name, and items, of its own,
# made here with a key the run chose, open to any app as the app's own would be to it.
check_name="$first_name Spare"
service="$check_name Safe Storage"
key=$($openssl rand -base64 16)
security add-generic-password -s "$service" -a "$check_name" -w "$key" -A ||
  fail "the Keychain didn't take the spare copy's cookie key"
rm -rf "$dir/profile"
sign_copy team-spare "$signer_a" "$team"
share_partition
# The team's signature is the one the last launch read the key under, so this one goes straight on.
run_copy team-spare "$out/keychain-3.log" "session?set" none
grep -q "cookie key" "$out/keychain-3.log" &&
  fail "the spare copy's first launch went through the cookie key: $(grep 'cookie key' "$out/keychain-3.log")"
security delete-generic-password -s "$service" -a "$check_name" > /dev/null ||
  fail "the spare copy's cookie key couldn't be deleted"
security add-generic-password -s "$service (moving)" -a "$check_name" -w "$key" -A ||
  fail "the Keychain didn't take the spare"
# The spare, too, as the app reads it.
item="$service"
service="$item (moving)"
share_partition
service="$item"
defaults delete "$check_id" KeychainKeySignature
run_copy team-spare "$out/keychain-4.log" session "h=$port; n=$port"
grep -q "^browser: the cookie key came back from its spare: 0\$" "$out/keychain-4.log" ||
  fail "the cookie key didn't come back from its spare: $(grep 'cookie key' "$out/keychain-4.log")"
! security find-generic-password -s "$service (moving)" -a "$check_name" > /dev/null 2>&1 ||
  fail "the cookie key's spare, \"$service (moving)\", is still in the Keychain after it came back"

partitions="with each copy's partition added"
[ -n "${KEYCHAIN_PASSWORD:-}" ] || partitions="with no partition added"
echo "ok: keychain. A login cookie set under identity A was sent under B, after the team's requirement, with no prompt, $partitions. With the cookie key only in its spare, a launch put it back and the profile's cookies were sent."
