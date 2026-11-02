#!/bin/bash
# Packs the Windows app for a release (#454), in Git Bash on Windows, from Browser.dll, which
# `cargo build --release --features prod` in app/windows makes, the agents' bridge, which
# `cargo build --release --features prod --manifest-path ../Cargo.toml --bin mcp-bridge
# --target-dir target` there makes, and the engine setup.sh puts in .deps/engine, whose chrome.exe
# runs it:
#   <out>/ilo-<version>-x64.msix, for the Microsoft Store, unsigned, as the Store signs it, with
#     the identity in branding/windows-store.ini, which the Store updates;
#   <out>/ilo-<version>-x64-setup.exe, the per-user installer (ilo.iss) for winget and direct
#     downloads, unsigned for 0.1.0, which ilo's own updater (updates.cc) updates.
# Without an engine it packs Browser.dll alone, and with the identity's placeholders a test MSIX,
# each named for it, to try the packaging: neither installs a browser that runs. --release stops on
# either instead. The version is Cargo.toml's, with the 0 the Store wants last in the MSIX's.
# Usage: package.sh [--release] <out directory>
set -euo pipefail
release=
if [ "${1:-}" = --release ]; then
  release=1
  shift
fi
[ $# -eq 1 ] || { echo "usage: package.sh [--release] <out directory>" >&2 && exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
app=$(cd "$here/../../.." && pwd)
mkdir -p "$1"
out=$(cd "$1" && pwd)

fail() {
  echo "package: $*" >&2
  exit 1
}

name=$(cat "$app/branding/name")
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$app/Cargo.toml" | head -1)
publisher=$(git config -f "$app/branding/winget.ini" winget.publisher)
store() { git config -f "$app/branding/windows-store.ini" "store.$1"; }
dll=$app/windows/target/release/Browser.dll
engine=$app/.deps/engine
[ -f "$dll" ] || fail "no $dll. Build it with cargo build --release --features prod in app/windows."
bridge=$app/windows/target/release/mcp-bridge.exe
[ -f "$bridge" ] || fail "no $bridge, the agents' bridge. Build it as this file's head says."

alone=
if [ ! -f "$engine/chrome.exe" ]; then
  [ -z "$release" ] || fail "no engine in $engine, which setup.sh fetches."
  alone=-app-only
  echo "package: no engine in $engine, so Browser.dll is packed alone, which doesn't run."
fi
test_identity=
if store identity-name | grep -q PLACEHOLDER || store publisher | grep -q PLACEHOLDER ||
  store publisher-display-name | grep -q PLACEHOLDER; then
  [ -z "$release" ] || fail "branding/windows-store.ini still holds placeholders, not Partner Center's identity."
  test_identity=-placeholder-identity
  echo "package: the MSIX has branding/windows-store.ini's placeholders, which the Store refuses."
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
# What both install: the engine but the import library the app links, and Browser.dll, the bridge
# an agent's MCP client runs, which Connect names beside chrome.exe, uBlock Origin and the licences
# beside it.
mkdir "$stage/app"
if [ -z "$alone" ]; then
  cp -r "$engine/." "$stage/app/"
  rm -f "$stage/app/chrome.dll.lib" "$stage/app/.from"
fi
cp "$dll" "$bridge" "$here/Assets/ilo.ico" "$stage/app/"
# uBlock Origin beside chrome.exe, where the core loads it, and the licences About opens, as
# build.sh puts them beside the engine.
cp -R "$("$app/fetch-ublock.sh")" "$stage/app/ublock"
"$app/../tools/licences.sh" "$stage/app" "$app/windows/Cargo.toml" \
  "$(rustc -vV | sed -n 's/^host: //p')" --features prod

# The Store's: Windows reads the manifest and its logos from the package's top.
cp -r "$stage/app" "$stage/msix"
mkdir "$stage/msix/Assets"
cp "$here/Assets/"*.png "$stage/msix/Assets/"
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
printf '%s\n' "$manifest" >"$stage/msix/AppxManifest.xml"
! grep -q '@[A-Z_]*@' "$stage/msix/AppxManifest.xml" || fail "AppxManifest.xml has a value package.sh doesn't fill."

# The newest SDK's makeappx. Packed alone, there's no chrome.exe for the manifest to name, so the
# check that it's there is skipped.
makeappx=$(ls -d "/c/Program Files (x86)/Windows Kits/10/bin/"10.*/x64/makeappx.exe 2>/dev/null | sort -V | tail -1)
[ -n "$makeappx" ] || fail "no makeappx.exe: install the Windows SDK."
msix=$out/$name-$version-x64$alone$test_identity.msix
MSYS2_ARG_CONV_EXCL='*' "$makeappx" pack /o ${alone:+/nv} /d "$(cygpath -w "$stage/msix")" \
  /p "$(cygpath -w "$msix")"

iscc=$(command -v iscc || echo "/c/Program Files (x86)/Inno Setup 6/ISCC.exe")
[ -x "$iscc" ] || fail "no ISCC.exe: install Inno Setup 6."
MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q "/DAppName=$name" "/DAppPublisher=$publisher" \
  "/DAppVersion=$version" "/DSuffix=$alone" "/DSource=$(cygpath -w "$stage/app")" \
  "/DOutput=$(cygpath -w "$out")" "$(cygpath -w "$here/ilo.iss")"

cd "$out"
sha256sum -- *.msix *.exe >SHA256SUMS
cat SHA256SUMS
