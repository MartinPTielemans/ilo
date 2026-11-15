#!/bin/bash
# Packs the Release engine's dSYMs as Chromium-mac-arm64-dSYMs.tar.xz, with its SHA-256, for the
# engine's engine-mac-<commit> release: the framework and helpers ship stripped, so a crash report's
# frames in them get names only from these (#2951). A dSYM names only the build it came from, so the
# framework's must have the UUID of the framework it's for, or nothing is packed and both UUIDs are
# said. Engine on macOS and tools/mac-engine-publish.sh pack with it.
# Usage: mac-dsyms.sh <out/Release> <the Chromium Framework binary it's for> <directory to write in>
set -euo pipefail
out=$1 framework=$2 dest=$3
asset=Chromium-mac-arm64-dSYMs.tar.xz

fail() {
  echo "mac-dsyms: $*" >&2
  exit 1
}
uuid() { dwarfdump --uuid "$1" | awk '$1 == "UUID:" && $3 == "(arm64)" { print $2; exit }'; }

[ -d "$out/Chromium Framework.dSYM" ] ||
  fail "$out has no Chromium Framework.dSYM. An official build writes one (enable_dsyms)."
want=$(uuid "$framework")
got=$(uuid "$out/Chromium Framework.dSYM")
[ -n "$want" ] && [ "$want" = "$got" ] ||
  fail "Chromium Framework.dSYM is ${got:-without a UUID} and the framework is ${want:-without one}: the dSYMs are another build's."

# Only the dSYMs of what Chromium.app holds: out/Release has the build's own tools' too, such as
# mksnapshot's, which never ship and would add a gigabyte.
names=$(find "$out/Chromium.app" | sed 's|.*/||; s/\.app$//; s/\.framework$//' | sort -u)
set --
for dsym in "$out"/*.dSYM; do
  name=${dsym##*/}
  ! printf '%s\n' "$names" | grep -qxF "${name%.dSYM}" || set -- "$@" "$name"
done
mkdir -p "$dest"
archive=$dest/$asset
tar -cJf "$archive" --no-mac-metadata --options xz:threads=0 -C "$out" "$@"
# GitHub takes a release asset under 2 GiB.
size=$(stat -f %z "$archive")
[ "$size" -lt 2147483648 ] || { rm "$archive"; fail "$asset is $size bytes, over the 2 GiB a release asset can be."; }
shasum -a 256 "$archive" | cut -d' ' -f1 >"$archive.sha256"
echo "mac-dsyms: packed $asset, $((size / 1048576)) MiB, for the framework $want." >&2
