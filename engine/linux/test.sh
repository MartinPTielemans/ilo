#!/bin/bash
# Builds and runs the core's unit tests, browser_core_unittests, in the tree build.sh built. On
# Linux base's test support loads Chromium's test fonts, which its source tarball leaves out, so
# they're fetched first, once, into the tree. Only the tests read them: package.sh packs a list of
# files that doesn't name them. Flags after the work directory go to the tests, such as
# --gtest_filter (.github/workflows/core-tests.yml).
set -euo pipefail
[ $# -ge 1 ] || { echo "usage: test.sh <work directory> [test flags...]" >&2 && exit 2; }

fail() {
  echo "test: $*" >&2
  exit 1
}

# The object DEPS names for src/third_party/test_fonts/test_fonts at pins.ini's Chromium version.
object=9c07d19d9c5ee1ff94f717e6fb17e0c8c354e6f9
sha256=f0e9628f9e43e3f3476cde06a1849058de460e0e037b7449ce0d42b9a73c37d5
fonts=$1/src/third_party/test_fonts/test_fonts
if [ "$(cat "$fonts/.object" 2>/dev/null)" != "$object" ]; then
  archive=$(mktemp)
  trap 'rm -f "$archive"' EXIT
  curl -sSfL --retry 3 -o "$archive" "https://storage.googleapis.com/chromium-fonts/$object"
  [ "$(sha256sum <"$archive" | cut -d' ' -f1)" = "$sha256" ] || fail "the test fonts don't match their hash"
  rm -rf "$fonts"
  mkdir -p "$fonts"
  tar -xzf "$archive" -C "$fonts" --no-same-owner
  echo "$object" >"$fonts/.object"
fi

"$(dirname "$0")/build.sh" "$1" browser_core_unittests
"$1/src/out/Default/browser_core_unittests" "${@:2}"
