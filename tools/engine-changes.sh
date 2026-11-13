#!/bin/bash
# Which of this checkout's core and engine patch files the engine app/ links wasn't built with: a
# line naming the engine, then one path per file, from the checkout's root. check/lib.sh's
# engine_has reads it, and app/build.sh keeps it as build/engine-changes for a copy of the tree with
# no repository, as a check VM's (#2554). Exits non-zero when it can't tell.
# A prebuilt engine was built at a commit of ilo's, which git compares with. A local build, Release
# or Dev, was built from the engine tree, which links a checkout's core and keeps a copy of each
# patch it applied (engine/tree.sh): a file that differs from the tree's, or that changed there
# after the build began (browser_core.h.built, engine/build.sh), isn't in it.
# Usage: tools/engine-changes.sh
set -euo pipefail
cd "$(dirname "$0")/.."

case $(uname -s) in
Darwin) link=app/.deps/Chromium.app ;;
*) link=app/.deps/engine ;;
esac
engine=$(cd "$link" 2> /dev/null && pwd -P) || engine=
out=${engine%/*}
case $engine in
*/prebuilt/*) out= ;;
esac
if [ -n "$engine" ] && [ -n "$out" ] && [ -f "$out/browser_core.h.built" ]; then
  tree=$out/../..
  built=$out/browser_core.h.built
  echo "$out"
  find core -type f | while read -r file; do
    there=$tree/browser_core/${file#core/}
    { cmp -s "$file" "$there" && [ ! "$there" -nt "$built" ]; } || echo "$file"
  done
  find engine/patches -type f | while read -r file; do
    there=$tree/.browser_core-series/${file#engine/patches/}
    { cmp -s "$file" "$there" && [ ! "$there" -nt "$built" ]; } || echo "$file"
  done
else
  commit=$(engine/fetch.sh commit 2> /dev/null)
  echo "$commit"
  git diff --name-only "$commit" -- core engine/patches
fi
