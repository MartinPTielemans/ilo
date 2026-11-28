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
#
# Last, a profile the newest release made, with a login cookie and a saved password, opens in this
# build, which sends the cookie and fills the password in. The release is downloaded, so a download
# that fails fails the run, naming the network.

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
  for name in "$first_name" "$first_name Spare" "$first_name Upgrade"; do
    for item in "$name Safe Storage" "$name Safe Storage (moving)"; do
      security delete-generic-password -s "$item" -a "$name" > /dev/null 2>&1 &
      deleting=$!
      (sleep 5 && kill "$deleting" 2> /dev/null) &
    done
  done
  if [ -d "$dir/released/volume" ]; then
    hdiutil detach -quiet -force "$dir/released/volume" 2> /dev/null || true
  fi
  [ -z "${upgraded:-}" ] || rm -rf "${upgraded:?}"
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

# A copy of the prod build, or of the app at $4, at $dir/$1, as the check's app, without Sparkle,
# which would check the product's feed, signed with the identity $2 under the requirement $3, as
# tools/install.sh signs. An app that names its profile's folder Chromium's default
# (CrProductDirName) names the run's.
copies=
sign_copy() {
  bundle="$dir/$1/$check_name.app"
  rm -rf "${dir:?}/$1"
  mkdir -p "$dir/$1"
  ditto "${4:-$app}" "$bundle"
  rm -rf "$bundle/Contents/Frameworks/Sparkle.framework"
  plutil -replace CFBundleIdentifier -string "$check_id" "$bundle/Contents/Info.plist"
  plutil -replace CFBundleName -string "$check_name" "$bundle/Contents/Info.plist"
  if plutil -extract CrProductDirName raw "$bundle/Contents/Info.plist" > /dev/null 2>&1; then
    plutil -replace CrProductDirName -string "$check_name" "$bundle/Contents/Info.plist"
  fi
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
# Launches the copy at $dir/$1 on the profile $copy_profile with the log $2 and the page $3, and waits
# for the server to get the page with the cookies $4. With drive set, it takes the driver's
# commands; a failure says $copy_why too.
copy_profile="$dir/profile"
launch_copy() {
  log="$2"
  step=0
  # The server's lines from this launch on.
  since=$(($(wc -l < "$out/site.log") + 1))
  request="^GET chromium 127.0.0.1:$port/$3 cookie $4\$"
  sent() { tail -n "+$since" "$out/site.log" | grep -q "$request"; }
  count_off "$copy_profile"
  if [ -n "${drive:-}" ]; then
    BROWSER_DRIVE=1 BROWSER_PROFILE_DIR="$copy_profile" BROWSER_URL="$site$3" \
      "$dir/$1/$check_name.app/Contents/MacOS/Browser" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
  else
    BROWSER_PROFILE_DIR="$copy_profile" BROWSER_URL="$site$3" \
      "$dir/$1/$check_name.app/Contents/MacOS/Browser" < /dev/null > "$log" 2>&1 &
    pid=$!
  fi
  for _ in $(seq 300); do
    sent && break
    kill -0 "$pid" 2> /dev/null || fail "the $1 copy exited"
    sleep 0.1
  done
  if ! sent; then
    asked="$(pgrep -x SecurityAgent > /dev/null && echo ", and macOS is asking for a password")"
    partition=
    [ -n "${KEYCHAIN_PASSWORD:-}" ] || partition=" Without KEYCHAIN_PASSWORD, the item's partition can ask."
    fail "the $1 copy didn't send the cookies $4 for /$3$asked.$partition${copy_why:-} Sent: $(tail -n "+$since" "$out/site.log" | grep "/$3 cookie" | tail -1)"
  fi
  await_load
}
quit_copy() {
  osascript -e "tell application id \"$check_id\" to quit"
  exited 30
}
# Launches the copy as launch_copy does, and quits it.
run_copy() {
  launch_copy "$@"
  quit_copy
}

# A's own requirement, as Apple Development's names its certificate, then the team's, as
# tools/requirement.sh makes it, with ILOCHECK for the team and no anchor.
own="designated => identifier \"$check_id\" and certificate leaf = H\"$signer_a\""
team="designated => identifier \"$check_id\" and certificate leaf[subject.OU] = \"ILOCHECK\""

# A first launch makes the cookie key, and the server's page sets its cookies, h from the server,
# HttpOnly as a login's is, and n from the page. The profile keeps them, encrypted with the key.
rm -rf "${copy_profile:?}"
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
rm -rf "${copy_profile:?}"
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

# A release's profile opens in the next build: its login cookie is sent and its saved password
# filled in. From Chromium 158 only a profile in Chromium's default folder, ~/Library/Application
# Support/<CrProductDirName>, keeps the Keychain key they were saved under (#3716), and the prod
# build names its profile's folder so. The profile is in the default folder by the run's name,
# which the copy of this build names only if the build names its own, so a build that doesn't
# fails here. The old side is the newest release, as a user has it: 0.3.0, on 157, for the move to
# 158. Once 158 ships, this stays as the check that a release's profile opens in the next build.
check_name="$first_name Upgrade"
service="$check_name Safe Storage"
copies=
copy_profile="$HOME/Library/Application Support/$check_name"
upgraded=$copy_profile
rm -rf "${copy_profile:?}"
defaults delete "$check_id" > /dev/null 2>&1 || true
released="$dir/released"
mkdir -p "$released"
repository=$(cat branding/repository)
dmg="$(cat branding/name)-mac-arm64.dmg"
for file in "$dmg" SHA256SUMS; do
  curl -sSfL --retry 3 -o "$released/$file" "$repository/releases/latest/download/$file" \
    2>> "$released/download.log" ||
    fail "the newest release's $file didn't download from $repository: the network or GitHub failed. $(tail -1 "$released/download.log")"
done
(cd "$released" && grep "  $dmg\$" SHA256SUMS | shasum -a 256 -c -) >> "$released/download.log" 2>&1 ||
  fail "the newest release's $dmg isn't the one its SHA256SUMS names: $released/download.log"
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$released/volume" "$released/$dmg" ||
  fail "the newest release's $dmg didn't open"
release_app=$(find "$released/volume" -maxdepth 1 -name '*.app' | head -1)
[ -n "$release_app" ] || fail "the newest release's $dmg holds no app"
release=$(plutil -extract CFBundleShortVersionString raw "$release_app/Contents/Info.plist")
sign_copy release "$signer_a" "$team" "$release_app"
hdiutil detach -quiet "$released/volume" || fail "the newest release's $dmg didn't close"

# The release takes the cookies, then saves a login, made up, through its offer, as passwords.sh
# does: its username field across the login page's top, its password field under it.
drive=1
launch_copy release "$out/keychain-5.log" "session?set" none
go_from_bar "${site#http://}login.html"
await '^browser: title [0-9]* login$' || fail "the $release copy didn't load the login page"
tab=$(today)
send click 600 60
saved_login=
if [ "$active" = 1 ]; then
  expect "^browser: title $tab user [0-9]*\$" "the click didn't put the caret in the username field"
  send key a cmd
  send type check@example.test
  send key tab
  expect "^browser: title $tab pass [0-9]*\$" "Tab didn't move the caret to the password field"
  send key a cmd
  send type upgrade.check
  send key return
  expect "^browser: offer $tab save check@example.test " "the $release copy offered to save no login"
  expect "^browser: offer $tab takes input\$" "the $release copy's offer never took input"
  send layout
  card=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: layout child' |
    grep ' 320x180$' | head -1 | sed 's/^browser: layout child //')
  [ -n "$card" ] || fail "the $release copy's offer isn't over the browser window"
  # Its username field, then return, which saves.
  ox=${card%%,*}
  oy=${card#*,}
  oy=${oy%% *}
  send click $((ox + 56)) $((oy + 70))
  send key return
  expect "^browser: offer $tab answered Accept check@example.test\$" "return didn't save the login"
  saved_login=1
else
  echo "skip: keychain's saved password. Browser wasn't the active app, so the page ignored the click; the cookie is still checked."
fi
quit_copy

# This build, in the same folder, sends the release's cookies and fills its password in. The
# release's launch made the cookie key's item, so its partitions take both copies now.
sign_copy upgrade "$signer_a" "$team"
share_partition
dir_name=$(plutil -extract CrProductDirName raw "$app/Contents/Info.plist" 2> /dev/null ||
  echo "nothing, so its profile's folder isn't Chromium's default")
copy_why=" This build's CrProductDirName is $dir_name."
launch_copy upgrade "$out/keychain-6.log" session "h=$port; n=$port"
copy_why=
if [ -n "$saved_login" ]; then
  go_from_bar "${site#http://}login.html"
  await '^browser: title [0-9]* filled$' ||
    fail "this build didn't fill in the password the $release copy saved: $(grep '^browser: title ' "$log" | tail -1)"
fi
quit_copy

partitions="with each copy's partition added"
[ -n "${KEYCHAIN_PASSWORD:-}" ] || partitions="with no partition added"
echo "ok: keychain. A login cookie set under identity A was sent under B, after the team's requirement, with no prompt, $partitions. With the cookie key only in its spare, a launch put it back and the profile's cookies were sent. A profile $release made in the default folder opened in this build, its cookie sent${saved_login:+ and its password filled in}."
