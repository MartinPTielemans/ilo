#!/bin/bash
# The proof winsparkle-proof.yml runs, in Git Bash on a hosted Windows runner, from the checkout's
# root: a released copy still on WinSparkle updates to the next release as a user's copy would.
#   1. FROM's Windows installer, from its GitHub release, installs per user as a release does.
#   2. TO's appcast and installer, signed with the real update key, are served on 127.0.0.1:443 for
#      github.com, under the paths the feed (app/branding/update-feed) and the entry name.
#   3. FROM launches, WinSparkle checks at launch and offers TO; its Install update downloads it,
#      checks its signature and runs the installer, whose wizard is clicked through.
#   4. TO is installed, without WinSparkle's DLL or settings, with the profile kept, and its own
#      updater finds nothing newer than itself on the same appcast.
# With own, TO is a release with ilo's own updater, so step 4 checks WinSparkle is gone too.
# With from-own, FROM has ilo's own updater as well (0.1.2 on): it checks at launch, downloads and
# verifies TO, says it's ready, and installs it silently as ilo quits, with no window to click.
# Usage: proof.sh <out directory> <FROM tag> release:<TO tag>|run:<release.yml run id> [own|from-own]
set -uo pipefail
out=$1 from=$2 to=$3 own=${4:-}
mkdir -p "$out/site" "$out/shots"
out=$(cd "$out" && pwd)
here=$(cd "$(dirname "$0")" && pwd)
repo=$GITHUB_REPOSITORY
installed="$(cygpath -u "$LOCALAPPDATA")/Programs/ilo"
uninstall='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\app.ilo.browser_is1'
hosts=/c/Windows/System32/drivers/etc/hosts
pid=
server=

fail() {
  echo "FAIL: $*"
  ui dump "${winpid:-^$}" > "$out/fail-ilo-windows.txt" 2>&1
  ui dump '.' > "$out/fail-all-windows.txt" 2>&1
  screen fail
  exit 1
}
ok() { echo "ok: $*"; }
note() { echo "-- $*"; }
cleanup() {
  [ -z "$server" ] || kill "$server" 2>/dev/null
  [ ! -f "$out/hosts.orig" ] || cp "$out/hosts.orig" "$hosts"
}
trap cleanup EXIT
reg_() { MSYS2_ARG_CONV_EXCL='*' reg "$@"; }
installed_version() {
  reg_ query "$uninstall" /v DisplayVersion 2>/dev/null | sed -n 's/.*REG_SZ *//p' | tr -d '\r'
}
ui() { powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$(cygpath -w "$here/ui.ps1")" "$@" | tr -d '\r'; }
# The whole screen, which on the runner holds nothing but what this run starts.
screen() {
  OUT="$(cygpath -w "$out/shots/$1.png")" powershell.exe -NoProfile -NonInteractive -Command '
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $i = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($i)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $i.Save($env:OUT, [System.Drawing.Imaging.ImageFormat]::Png)'
}
# The Windows ids of ilo's browser processes running from the install, not its helpers.
ilo_ids() {
  DIR="$(cygpath -w "$installed")\\chrome.exe" powershell.exe -NoProfile -NonInteractive -Command '
    Get-CimInstance Win32_Process | Where-Object {
      $_.ExecutablePath -eq $env:DIR -and $_.CommandLine -notmatch "--type="
    } | ForEach-Object { $_.ProcessId }' | tr -d '\r'
}
close_ilo() {
  for id in $(ilo_ids); do python -I app/check/app-windows.py close "$id"; done
  for _ in $(seq 60); do
    [ -z "$(ilo_ids)" ] && return 0
    sleep 0.5
  done
  fail "ilo didn't quit"
}
await() {
  for _ in $(seq $((${3:-30} * 10))); do
    grep -q "$1" "$2" && return 0
    sleep 0.1
  done
  return 1
}

note "downloads: $from's installer, $to's appcast and installer"
gh release download "$from" -R "$repo" -p ilo-windows-x64-setup.exe -D "$out/from" ||
  fail "no installer in $from"
case $to in
  release:*)
    gh release download "${to#release:}" -R "$repo" -p appcast.xml -p ilo-windows-x64-setup.exe \
      -D "$out/to" || fail "no appcast or installer in ${to#release:}"
    ;;
  run:*)
    gh run download "${to#run:}" -R "$repo" -n windows -D "$out/to-windows" &&
      gh run download "${to#run:}" -R "$repo" -n mac -D "$out/to-mac" ||
      fail "run ${to#run:} has no windows or mac artifact"
    mkdir -p "$out/to"
    cp "$out/to-mac/appcast.xml" "$out/to-windows/ilo-windows-x64-setup.exe" "$out/to/" ||
      fail "run ${to#run:}'s artifacts have no appcast.xml or ilo-windows-x64-setup.exe"
    ;;
  *) fail "TO is release:<tag> or run:<id>, not $to" ;;
esac
feed=$(cat app/branding/update-feed)
read -r url length version build <<EOF
$(python -I - "$out/to/appcast.xml" <<'PY'
import sys, xml.etree.ElementTree as ET
S = "{http://www.andymatuschak.org/xml-namespaces/sparkle}"
entries = []
for item in ET.parse(sys.argv[1]).getroot().iter("item"):
    e = item.find("enclosure")
    if e is not None and e.get(S + "os", "").startswith("windows"):
        entries.append((int(item.findtext(S + "version")), e.get("url"), e.get("length"),
                        item.findtext(S + "shortVersionString")))
build, url, length, version = max(entries)
print(url, length, version, build)
PY
)
EOF
echo "feed $feed; newest Windows entry: $version, build $build, $url, $length bytes"
cp "$out/to/appcast.xml" "$out/appcast.txt"
case $feed$url in https://github.com/*https://github.com/*) ;; *) fail "the feed or the entry isn't on github.com" ;; esac
[ "$(stat -c %s "$out/to/ilo-windows-x64-setup.exe")" = "$length" ] ||
  fail "the entry's length isn't the installer's"
for path in "${feed#https://github.com}" "${url#https://github.com}"; do mkdir -p "$out/site$(dirname "$path")"; done
cp "$out/to/appcast.xml" "$out/site${feed#https://github.com}"
cp "$out/to/ilo-windows-x64-setup.exe" "$out/site${url#https://github.com}"

note "1. $from installs"
"$out/from/ilo-windows-x64-setup.exe" //VERYSILENT //SUPPRESSMSGBOXES //NORESTART //SP- ||
  fail "$from didn't install"
was=$(installed_version)
[ "$was" = "${from#v}" ] || fail "$from installed $was"
if [ "$own" = from-own ]; then
  [ ! -e "$installed/WinSparkle.dll" ] || fail "$from still has WinSparkle.dll"
  ok "$from ($was) installed per user, with ilo's own updater"
else
  [ -f "$installed/WinSparkle.dll" ] || fail "$from has no WinSparkle.dll"
  ok "$from ($was) installed per user, with WinSparkle"
fi

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
MSYS2_ARG_CONV_EXCL='*' certutil -addstore -f Root "$(cygpath -w "$out/tls.pem")" > /dev/null ||
  fail "the certificate couldn't be trusted"
cp "$hosts" "$out/hosts.orig"
printf '\r\n127.0.0.1 github.com\r\n' >> "$hosts"
ipconfig //flushdns > /dev/null
python -I -u -c '
import functools, http.server, ssl, sys
root, cert, key = sys.argv[1:4]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=root)
server = http.server.ThreadingHTTPServer(("127.0.0.1", 443), handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(cert, key)
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
' "$(cygpath -w "$out/site")" "$(cygpath -w "$out/tls.pem")" "$(cygpath -w "$out/tls-key.pem")" \
  > "$out/server.log" 2>&1 &
server=$!
for _ in $(seq 50); do
  curl -sf --cacert "$out/tls.pem" "$feed" > /dev/null && break
  sleep 0.2
done
curl -sf --cacert "$out/tls.pem" "$feed" | grep -q "$version" || fail "the feed isn't served"
ok "github.com is this run's server, with $to's appcast and installer"

if [ "$own" = from-own ]; then
  note "3. $from launches, and its own updater gets $version ready"
  reg_ delete 'HKCU\Software\ilo\Updates' /v LastCheck /f > /dev/null 2>&1 || true
  export BROWSER_URL='data:text/html,<title>Proof tab</title><h1>Proof tab</h1>'
  export MSYS2_ENV_CONV_EXCL=BROWSER_URL
  "$installed/chrome.exe" > "$out/from.log" 2>&1 &
  pid=$!
  sleep 1
  winpid=$(cat "/proc/$pid/winpid")
  await '^browser: updater started' "$out/from.log" 60 || fail "$from's updater didn't start"
  await "GET ${feed#https://github.com}" "$out/server.log" 120 || fail "$from didn't check at launch"
  await "^browser: update $version ready" "$out/from.log" 300 ||
    fail "$from didn't get $version ready: $(grep 'browser: update' "$out/from.log" | tail -3)"
  grep -q "GET ${url#https://github.com}" "$out/server.log" || fail "$from didn't download $version"
  screen 3-ready
  close_ilo
  pid=
  await '^browser: update installing as ilo quits' "$out/from.log" 30 ||
    fail "$from didn't start installing $version as it quit: $(grep 'browser: update' "$out/from.log" | tail -3)"
  profile=$(find "$(cygpath -u "$LOCALAPPDATA")" -maxdepth 4 -name "Local State" -path "*ilo*" 2>/dev/null | head -1)
  [ -n "$profile" ] || fail "$from left no profile"
  echo "profile: $profile"
  ok "$from's updater downloaded $version, checked its signature, and started its installer as ilo quit"
  for _ in $(seq 600); do
    [ "$(installed_version)" = "$version" ] && [ -z "$(ui dump '^Setup - ilo' | head -1)" ] && break
    sleep 0.5
  done
  [ "$(installed_version)" = "$version" ] || fail "$(installed_version) is installed, not $version"
  [ -z "$(ilo_ids)" ] || fail "the installer relaunched ilo after a plain quit"
  ok "the installer put $version in silently, without relaunching ilo"
else
note "3. $from launches, and WinSparkle offers $version"
export BROWSER_URL='data:text/html,<title>Proof tab</title><h1>Proof tab</h1>'
export MSYS2_ENV_CONV_EXCL=BROWSER_URL
"$installed/chrome.exe" > "$out/from.log" 2>&1 &
pid=$!
sleep 1
winpid=$(cat "/proc/$pid/winpid")
await '^browser: updater started' "$out/from.log" 60 || fail "WinSparkle didn't start"
await "GET ${feed#https://github.com}" "$out/server.log" 120 || fail "WinSparkle didn't check at launch"
pressed=0
for tick in $(seq 240); do
  if ui click "$winpid" '^&?Install' >> "$out/presses.txt"; then
    pressed=$((pressed + 1))
    sleep 1
    screen "3-winsparkle-$pressed"
  fi
  [ "$pressed" -gt 0 ] && ! kill -0 "$pid" 2>/dev/null && break
  [ $tick = 20 ] && screen 3-winsparkle-waiting
  sleep 0.5
done
cat "$out/presses.txt"
[ "$pressed" -gt 0 ] || fail "WinSparkle didn't offer $version"
grep -q "GET ${url#https://github.com}" "$out/server.log" || fail "WinSparkle didn't download $version"
! kill -0 "$pid" 2>/dev/null || fail "ilo didn't quit for the installer"
pid=
profile=$(find "$(cygpath -u "$LOCALAPPDATA")" -maxdepth 4 -name "Local State" -path "*ilo*" 2>/dev/null | head -1)
[ -n "$profile" ] || fail "$from left no profile"
echo "profile: $profile"
ok "WinSparkle offered $version, downloaded it, checked its signature and quit ilo for its installer"

note "the installer's wizard"
steps=0
for tick in $(seq 600); do
  if [ "$(installed_version)" = "$version" ] && [ -z "$(ui dump '^Setup - ilo' | head -1)" ]; then
    break
  fi
  for button in '^&?Install$' '^&?Finish$' '^&?Next' '^&?OK$'; do
    if ui click '^Setup - ilo' "$button" >> "$out/presses.txt"; then
      steps=$((steps + 1))
      screen "3-setup-$steps"
      sleep 1
      break
    fi
  done
  sleep 0.5
done
tail -n +$((pressed + 1)) "$out/presses.txt"
[ "$(installed_version)" = "$version" ] || fail "$(installed_version) is installed, not $version"
ok "the installer's wizard installed $version"
fi

note "4. $version"
sleep 5
screen 4-after
close_ilo
[ -s "$profile" ] || fail "the profile's Local State is gone"
ok "the profile $from made is kept"
if [ "$own" != own ] && [ "$own" != from-own ]; then
  ok "$version installed; it still runs WinSparkle, so there's no own updater to check"
  exit 0
fi
[ ! -e "$installed/WinSparkle.dll" ] || fail "WinSparkle.dll is still installed"
! reg_ query 'HKCU\Software\ilo\WinSparkle' > /dev/null 2>&1 || fail "WinSparkle's settings are still there"
reg_ delete 'HKCU\Software\ilo\Updates' /v LastCheck /f > /dev/null 2>&1 || true
"$installed/chrome.exe" > "$out/to.log" 2>&1 &
pid=$!
await '^browser: updater started' "$out/to.log" 60 || fail "$version's updater didn't start"
await "^browser: update: none newer than build $build" "$out/to.log" 60 ||
  fail "$version didn't find itself current: $(grep 'browser: update' "$out/to.log" | tail -3)"
screen 4-current
close_ilo
pid=
ok "$version is installed without WinSparkle, and its own updater finds nothing newer than build $build"
