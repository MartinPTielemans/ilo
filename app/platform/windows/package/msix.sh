#!/bin/bash
# Packs an app folder, the engine with Browser.dll and what package.sh puts beside it, as an MSIX
# for the Microsoft Store, unsigned, as the Store signs it: the identity from
# branding/windows-store.ini, the name from branding/name and the version from Cargo.toml with the 0
# the Store wants last. package.sh packs a release's; check-windows.yml packs build/Browser and
# installs it, signed for its runner alone, for the links run's activation of the package (#1427).
# A folder without chrome.exe, Browser.dll packed alone, skips makeappx's check that the manifest's
# executable is there.
# Usage: msix.sh <app folder> <out.msix>, in Git Bash on Windows.
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: msix.sh <app folder> <out.msix>" >&2 && exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
app=$(cd "$here/../../.." && pwd)
from=$(cd "$1" && pwd)

fail() {
  echo "msix: $*" >&2
  exit 1
}

name=$(cat "$app/branding/name")
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$app/Cargo.toml" | head -1)
store() { git config -f "$app/branding/windows-store.ini" "store.$1"; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
# Windows reads the manifest and its logos from the package's top.
cp -r "$from/." "$stage/"
mkdir -p "$stage/Assets"
cp "$here/Assets/"*.png "$stage/Assets/"
# Bash 5.2 reads & in a replacement as what it replaces.
shopt -u patsub_replacement 2>/dev/null || true
xml() {
  local value=${1//&/&amp;}
  value=${value//</&lt;}
  value=${value//>/&gt;}
  printf '%s' "${value//\"/&quot;}"
}
manifest=$(<"$here/AppxManifest.xml")
manifest=${manifest//@IDENTITY_NAME@/$(xml "$(store identity-name)")}
manifest=${manifest//@PUBLISHER_DISPLAY_NAME@/$(xml "$(store publisher-display-name)")}
manifest=${manifest//@PUBLISHER@/$(xml "$(store publisher)")}
manifest=${manifest//@VERSION@/$version.0}
manifest=${manifest//@NAME@/$(xml "$name")}
printf '%s\n' "$manifest" >"$stage/AppxManifest.xml"
! grep -q '@[A-Z_]*@' "$stage/AppxManifest.xml" || fail "AppxManifest.xml has a value msix.sh doesn't fill."

# The newest SDK's makeappx.
makeappx=$(ls -d "/c/Program Files (x86)/Windows Kits/10/bin/"10.*/x64/makeappx.exe 2>/dev/null | sort -V | tail -1)
[ -n "$makeappx" ] || fail "no makeappx.exe: install the Windows SDK."
alone=
[ -f "$stage/chrome.exe" ] || alone=1
MSYS2_ARG_CONV_EXCL='*' "$makeappx" pack /o ${alone:+/nv} /d "$(cygpath -w "$stage")" \
  /p "$(cygpath -w "$2")"
