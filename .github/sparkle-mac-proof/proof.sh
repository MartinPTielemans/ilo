#!/bin/bash
# The proof sparkle-mac-proof.yml runs on a hosted macOS runner, from the checkout's root: a
# released Mac copy updates itself to the next release through Sparkle, as a user's copy would.
#   1. FROM's DMG, from its GitHub release, is mounted and its ilo.app copied to /Applications.
#   2. TO's appcast and DMG, signed with the real update key, are served on 127.0.0.1:443 for
#      github.com, under the paths of the feed (app/branding/update-feed) and of the entry.
#   3. FROM launches with its last check long past, so Sparkle checks at launch; it downloads TO,
#      checks its signatures and says it's ready to install on quit.
#   4. FROM quits, Sparkle's installer puts TO in /Applications, and TO launches on FROM's
#      profile, where its own updater finds nothing newer than itself on the same appcast.
# Bash 3.2, macOS's. Usage: proof.sh <out directory> <FROM tag> release:<TO tag>|run:<release.yml run id>
set -uo pipefail
out=$1 from=$2 to=$3
mkdir -p "$out/shots"
out=$(cd "$out" && pwd)
repo=$GITHUB_REPOSITORY
installed=/Applications/ilo.app
pid=
server=
mounted=

ok() { echo "ok: $*"; }
note() { echo "-- $*"; }
# Sparkle's own log, which says more than the app's lines do.
sparkle_log() {
  /usr/bin/log show --last 30m --style compact \
    --predicate 'subsystem == "org.sparkle-project.Sparkle" OR process == "Autoupdate" OR process == "Installer Progress"' \
    > "$out/sparkle.txt" 2>&1 || true
}
fail() {
  echo "FAIL: $*"
  [ -z "$pid" ] || shot fail
  ps -axo pid,ppid,command | grep -i -E 'ilo\.app|Autoupdate|Sparkle' | grep -v grep > "$out/fail-processes.txt"
  exit 1
}
cleanup() {
  [ -z "$pid" ] || kill "$pid" 2>/dev/null
  [ -z "$server" ] || sudo kill "$server" 2>/dev/null
  [ -z "$mounted" ] || hdiutil detach -force "$mounted" > /dev/null 2>&1
  if [ -f "$out/hosts.orig" ]; then
    sudo cp "$out/hosts.orig" /etc/hosts
    sudo dscacheutil -flushcache
    sudo killall -HUP mDNSResponder 2>/dev/null
  fi
  sparkle_log
  [ -z "${id:-}" ] || defaults read "$id" > "$out/defaults.txt" 2>&1
}
trap cleanup EXIT
# ilo's window only, never the screen.
shot() {
  tools/window-screenshot.sh "$pid" "$out/shots/$1.png" all > "$out/shots/$1.txt" 2>&1 ||
    note "no capture of ilo's window for $1: $(tail -1 "$out/shots/$1.txt")"
}
plist() { plutil -extract "$1" raw "$installed/Contents/Info.plist" 2>/dev/null; }
# macOS's grep takes a $ before \| as a character, so an alternation has no end anchors.
await() {
  for _ in $(seq $((${3:-30} * 10))); do
    grep -q "$1" "$2" && return 0
    sleep 0.1
  done
  return 1
}
updates() { grep 'browser: update' "$1" | tail -${2:-5} | tr '\n' ';'; }
# The app's own process, from the install, not its helpers in its frameworks.
ilo_pid() { pgrep -n -f "^$installed/Contents/MacOS/" || true; }
launch() { # <log> <what>
  # Last checked long ago, so Sparkle checks once the updater starts, as a copy left a day would.
  defaults write "$id" SULastCheckTime -date "2020-01-01 00:00:00 +0000"
  open -n -a "$installed" --stdout "$1" --stderr "$1" \
    --env 'BROWSER_URL=data:text/html,<title>Proof tab</title><h1>Proof tab</h1>' ||
    fail "$2 didn't open"
  pid=
  for _ in $(seq 300); do
    pid=$(ilo_pid)
    [ -n "$pid" ] && break
    sleep 0.1
  done
  [ -n "$pid" ] || fail "$2 didn't start"
  await '^browser: updater started' "$1" 120 || fail "$2's updater didn't start: $(tail -5 "$1")"
}
quit_ilo() { # <what>
  kill "$pid" 2>/dev/null
  for _ in $(seq 600); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -0 "$pid" 2>/dev/null && fail "$1 didn't quit"
  pid=
}

note "downloads: $from's DMG, $to's appcast and DMG"
gh release download "$from" -R "$repo" -p ilo-mac-arm64.dmg -D "$out/from" || fail "no Mac DMG in $from"
case $to in
  release:*)
    gh release download "${to#release:}" -R "$repo" -p appcast.xml -p ilo-mac-arm64.dmg \
      -D "$out/to" || fail "no appcast or Mac DMG in ${to#release:}"
    ;;
  run:*)
    gh run download "${to#run:}" -R "$repo" -n mac -D "$out/to" &&
      { [ -f "$out/to/appcast.xml" ] ||
        gh run download "${to#run:}" -R "$repo" -n appcast -D "$out/to"; } ||
      fail "run ${to#run:} has no mac or appcast artifact"
    ;;
  *) fail "TO is release:<tag> or run:<id>, not $to" ;;
esac
ls -l "$out/from" "$out/to"
feed=$(cat app/branding/update-feed)
read -r url length version build <<EOF
$(python3 -I - "$out/to/appcast.xml" <<'PY'
import sys, xml.etree.ElementTree as ET
S = "{http://www.andymatuschak.org/xml-namespaces/sparkle}"
entries = []
for item in ET.parse(sys.argv[1]).getroot().iter("item"):
    e = item.find("enclosure")
    if e is not None and e.get(S + "os", "macos") == "macos":
        entries.append((int(item.findtext(S + "version")), e.get("url"), e.get("length"),
                        item.findtext(S + "shortVersionString")))
build, url, length, version = max(entries)
print(url, length, version, build)
PY
)
EOF
echo "feed $feed; newest Mac entry: $version, build $build, $url, $length bytes"
cp "$out/to/appcast.xml" "$out/appcast.txt"
case $feed$url in https://github.com/*https://github.com/*) ;; *) fail "the feed or the entry isn't on github.com" ;; esac
dmg=$out/to/$(basename "$url")
[ -f "$dmg" ] || fail "TO has no $(basename "$url")"
[ "$(stat -f %z "$dmg")" = "$length" ] || fail "the entry's length isn't the DMG's"
mkdir -p "$out/site$(dirname "${feed#https://github.com}")" "$out/site$(dirname "${url#https://github.com}")"
cp "$out/to/appcast.xml" "$out/site${feed#https://github.com}"
mv "$dmg" "$out/site${url#https://github.com}"

note "1. $from installs"
[ ! -e "$installed" ] || fail "$installed is already there"
[ -w /Applications ] || fail "/Applications isn't writable without an administrator's password"
mounted=$out/from-volume
hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mounted" "$out/from/ilo-mac-arm64.dmg" \
  > "$out/hdiutil.log" 2>&1 || fail "$from's DMG didn't mount: $(tail -3 "$out/hdiutil.log")"
ls -la "$mounted"
[ -d "$mounted/ilo.app" ] || fail "$from's DMG has no ilo.app"
ditto "$mounted/ilo.app" "$installed" || fail "ilo.app didn't copy to /Applications"
hdiutil detach "$mounted" > /dev/null 2>&1 || hdiutil detach -force "$mounted" > /dev/null 2>&1
mounted=
was=$(plist CFBundleShortVersionString)
[ "$was" = "${from#v}" ] || fail "$from installed $was"
id=$(plist CFBundleIdentifier)
echo "bundle id $id, build $(plist CFBundleVersion), feed $(plist SUFeedURL)"
[ "$(plist SUFeedURL)" = "$feed" ] || fail "$from's feed is $(plist SUFeedURL), not $feed"
codesign --verify --deep --strict "$installed" 2> "$out/codesign-from.txt" ||
  fail "$from's ilo.app isn't signed whole: $(tail -3 "$out/codesign-from.txt")"
codesign -dvv "$installed" 2>&1 | grep -E '^(Authority|TeamIdentifier)=' | head -2
ok "$from ($was) installed in /Applications from its DMG, signed, with Sparkle on $feed"

note "2. github.com on 127.0.0.1"
cat > "$out/tls.cnf" <<'EOF'
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = github.com
[ext]
subjectAltName = DNS:github.com
basicConstraints = critical,CA:TRUE
keyUsage = critical,digitalSignature,keyEncipherment,keyCertSign
extendedKeyUsage = serverAuth
EOF
openssl req -x509 -newkey rsa:2048 -nodes -days 2 -config "$out/tls.cnf" \
  -keyout "$out/tls-key.pem" -out "$out/tls.pem" > "$out/openssl.log" 2>&1 ||
  fail "no certificate: $(tail -3 "$out/openssl.log")"
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain "$out/tls.pem" ||
  fail "the certificate couldn't be trusted"
cp /etc/hosts "$out/hosts.orig"
printf '\n127.0.0.1 github.com\n' | sudo tee -a /etc/hosts > /dev/null
sudo dscacheutil -flushcache
sudo killall -HUP mDNSResponder
cat > "$out/server.py" <<'PY'
import functools, http.server, ssl, sys
root, cert, key = sys.argv[1:4]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=root)
server = http.server.ThreadingHTTPServer(("127.0.0.1", 443), handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(cert, key)
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
PY
sudo python3 -I -u "$out/server.py" "$out/site" "$out/tls.pem" "$out/tls-key.pem" \
  > "$out/server.log" 2>&1 &
server=$!
for _ in $(seq 50); do
  curl -sf --cacert "$out/tls.pem" "$feed" > /dev/null && break
  sleep 0.2
done
curl -sf --cacert "$out/tls.pem" "$feed" | grep -q "$version" || fail "the feed isn't served"
security verify-cert -c "$out/tls.pem" -p ssl -s github.com > "$out/verify-cert.txt" 2>&1 ||
  fail "macOS doesn't trust the run's certificate for github.com: $(tail -2 "$out/verify-cert.txt")"
ok "github.com is this run's server, trusted by macOS, with $to's appcast and DMG"

note "3. $from launches, and Sparkle gets $version ready"
launch "$out/from.log" "$from"
# A check that's due runs as Sparkle starts, without saying when it's scheduled.
await "GET ${feed#https://github.com}" "$out/server.log" 180 ||
  fail "$from didn't check at launch: $(updates "$out/from.log")"
await "^browser: update $build found" "$out/from.log" 60 ||
  fail "$from didn't find build $build: $(updates "$out/from.log")"
ok "$from's Sparkle checked the feed at launch and found $version (build $build)"
await "^browser: update stage ready $version\|^browser: update stage failed\|^browser: update failed" \
  "$out/from.log" 900
grep -q "^browser: update stage ready $version\$" "$out/from.log" ||
  fail "$from didn't get $version ready: $(updates "$out/from.log" 8)"
grep -q "GET ${url#https://github.com}" "$out/server.log" || fail "$from didn't download $version"
grep -q "^browser: update stage downloading $version\$" "$out/from.log" ||
  note "the stage never said downloading"
# macOS's own check of the staged copy, which runs while the user goes on browsing.
await "^browser: update $build prewarmed" "$out/from.log" 120 &&
  grep "^browser: update $build \(signature checked\|prewarmed\)" "$out/from.log"
profile=$(find "$HOME/Library/Application Support" -maxdepth 3 -name "Local State" -path "*ilo*" 2>/dev/null | head -1)
[ -n "$profile" ] || fail "$from made no profile"
profile=$(dirname "$profile")
echo "profile: $profile"
ok "$from's Sparkle downloaded $version in the background, checked its signatures and has it ready to install on quit"
shot 3-ready

note "4. $from quits, and Sparkle installs $version"
# What $from made, which $version must read rather than make anew: databases, changed in place.
kept=
for file in Default/History Default/Cookies Default/Favicons Default/Shortcuts; do
  [ -f "$profile/$file" ] && kept="$kept $file:$(stat -f '%i-%B' "$profile/$file")"
done
echo "kept:$kept"
[ -n "$kept" ] || fail "$from's profile has no History, Cookies, Favicons or Shortcuts"
quit_ilo "$from"
for _ in $(seq 1200); do
  [ "$(plist CFBundleShortVersionString)" = "$version" ] && ! pgrep -f Autoupdate > /dev/null && break
  sleep 0.5
done
now=$(plist CFBundleShortVersionString)
[ "$now" = "$version" ] || fail "$installed is ${now:-gone}, not $version"
[ "$(plist CFBundleVersion)" = "$build" ] || fail "$installed is build $(plist CFBundleVersion), not $build"
codesign --verify --deep --strict "$installed" 2> "$out/codesign-to.txt" ||
  fail "the installed $version isn't signed whole: $(tail -3 "$out/codesign-to.txt")"
[ -z "$(ilo_pid)" ] || fail "Sparkle relaunched ilo after a plain quit"
ok "Sparkle installed $version over $from as ilo quit: $installed is $now (build $build), signed whole, not relaunched"

note "5. $version launches on $from's profile"
started=$(date +%s)
launch "$out/to.log" "$version"
await "^browser: update none: .*running $build\|^browser: update failed\|^browser: update [0-9]* found" \
  "$out/to.log" 180
grep -q "^browser: update none: .*running $build\$" "$out/to.log" ||
  fail "$version didn't find itself current: $(updates "$out/to.log")"
await '^browser: UI shown' "$out/to.log" 30 || note "no UI shown line"
sleep 3
shot 5-updated
quit_ilo "$version"
grep -i 'cookie key' "$out/to.log"
grep -q "cookie key couldn't be read" "$out/to.log" && fail "$version couldn't read the cookie key"
for entry in $kept; do
  file=${entry%%:*}
  [ "$file:$(stat -f '%i-%B' "$profile/$file" 2>/dev/null)" = "$entry" ] ||
    fail "$version made $file anew rather than reading $from's"
done
[ "$(stat -f %m "$profile/Local State")" -ge "$started" ] ||
  fail "$version didn't write $from's Local State, so it didn't run on that profile"
others=$(find "$HOME/Library/Application Support" -maxdepth 3 -name "Local State" -path "*ilo*" | grep -v -F "$profile/Local State")
[ -z "$others" ] || fail "$version made another profile: $others"
ok "$version launched on $from's profile ($profile, its $(echo "$kept" | sed 's/:[^ ]*//g; s/^ //') kept), and its own updater finds nothing newer than build $build"
