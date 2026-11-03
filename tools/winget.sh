#!/bin/bash
# Writes winget's manifests for an ilo release, from app/platform/windows/package/winget, the
# installer package.sh made and the URL it's published at, into
# <out>/manifests/<letter>/<publisher>/<package>/<version>/, where microsoft/winget-pkgs keeps
# them. The owner opens the pull request there; nobody else does (#454). On Windows, `winget
# validate` checks them. The identifier and publisher are branding/winget.ini's, the version
# Cargo.toml's. The installer is Inno Setup's (ilo.iss), which winget runs silently with its own
# switches and knows installed by its uninstall key, app.ilo.browser_is1.
# Usage: tools/winget.sh <ilo-<version>-x64-setup.exe> <its https download URL> <out directory>
set -euo pipefail
[ $# -eq 3 ] || { echo "usage: winget.sh <installer.exe> <download URL> <out directory>" >&2 && exit 2; }
installer=$1
url=$2
root=$(cd "$(dirname "$0")/.." && pwd)
app=$root/app
templates=$app/platform/windows/package/winget

fail() {
  echo "winget: $*" >&2
  exit 1
}

name=$(cat "$app/branding/name")
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$app/Cargo.toml" | head -1)
id=$(git config -f "$app/branding/winget.ini" winget.identifier)
publisher=$(git config -f "$app/branding/winget.ini" winget.publisher)
repository=$(cat "$app/branding/repository")
[ "$(basename "$installer")" = "$name-$version-x64-setup.exe" ] ||
  fail "$installer isn't $name-$version-x64-setup.exe, a release's installer from package.sh."
case $url in
https://*) ;;
*) fail "winget downloads the installer over https, not $url." ;;
esac
# From stdin: given a Windows path, as RUNNER_TEMP is in Git Bash, sha256sum escapes its
# backslashes and marks the line with a leading one.
sha256=$(sha256sum <"$installer" | cut -d' ' -f1)

dir=$3/manifests/$(echo "${id:0:1}" | tr '[:upper:]' '[:lower:]')/${id//.//}/$version
mkdir -p "$dir"
fill() {
  sed -e "s|@ID@|$id|g" -e "s|@VERSION@|$version|g" -e "s|@NAME@|$name|g" \
    -e "s|@PUBLISHER@|$publisher|g" -e "s|@REPOSITORY@|$repository|g" -e "s|@URL@|$url|g" \
    -e "s|@SHA256@|$sha256|g" -e "s|@DATE@|$(date -u +%F)|g" "$templates/$1" >"$dir/$2"
  ! grep -q '@[A-Z0-9]*@' "$dir/$2" || fail "$1 has a value winget.sh doesn't fill."
}
fill version.yaml "$id.yaml"
fill installer.yaml "$id.installer.yaml"
fill locale.en-US.yaml "$id.locale.en-US.yaml"
if command -v winget >/dev/null; then
  winget validate --manifest "$dir"
fi
echo "$dir"
