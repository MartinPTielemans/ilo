#!/bin/bash
# Packs the Linux build `app/build.sh prod` makes, app/build/Browser, into <out>/ilo-linux-x86_64.tar.xz,
# its .sha256 and install.sh, the release's native install (#2453). The tarball is one folder, ilo,
# with the desktop entry, icon and install.sh beside chrome, which install.sh puts in place; the
# app reads install.sh there as a sign that it updates this copy (updates_host.rs).
# It fails when the build needs a newer C library than install.sh checks for.
# Usage: tools/linux-tarball.sh <app/build/Browser> <out directory>
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: linux-tarball.sh <Browser directory> <out directory>" >&2 && exit 2; }
browser=$1
out=$2
native=$(cd "$(dirname "$0")/../app/platform/linux" && pwd)
tarball=ilo-linux-x86_64.tar.xz

fail() {
  echo "linux-tarball: $*" >&2
  exit 1
}

for file in chrome libBrowser.so mcp-bridge; do
  [ -f "$browser/$file" ] || fail "no $file in $browser, so no Linux build there"
done
# The newest glibc a symbol needs, leaving out weak ones, which the loader skips when they're missing.
needs=$(sed -n 's/^needs_glibc=//p' "$native/native/install.sh")
built=$(for file in chrome libBrowser.so mcp-bridge; do objdump -T "$browser/$file"; done |
  awk '$2 != "w"' | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n1)
[ "$(printf '%s\n' "$needs" "$built" | sort -V | tail -n1)" = "$needs" ] ||
  fail "the build needs glibc $built, and install.sh checks for $needs: change needs_glibc there"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/ilo" "$out"
cp -a "$browser/." "$stage/ilo/"
cp "$native/native/install.sh" "$native/native/ilo.desktop" "$stage/ilo/"
cp "$native/flatpak/app.ilo.browser.svg" "$stage/ilo/ilo.svg"
XZ_OPT=-T0 tar -cJf "$out/$tarball" --owner=0 --group=0 --numeric-owner -C "$stage" ilo
(cd "$out" && sha256sum "$tarball" >"$tarball.sha256")
cp "$native/native/install.sh" "$out/"
ls -l "$out"
