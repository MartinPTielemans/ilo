#!/bin/bash
# The commit release.yml's nightly mode builds, from HEAD, main's newest commit: the newest on its
# first-parent line whose core/ and engine/ are those of the Mac engine engine/fetch.sh takes there,
# so a nightly never pairs the app with an older core. Windows joins when its engine fits that
# commit too; otherwise its previous build stays in the feed. The appcast is signed beside the
# Mac's build, as a release's is, so while the Mac engine is behind main's core, the night's commit
# stays where it was, and the feed, which has it already, waits for the engine.
# Prints commit=<sha> and windows=true|false, for GITHUB_OUTPUT, and says why on stderr.
# Usage: tools/nightly-commit.sh
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(git config -f engine/prebuilt.ini prebuilt.repository)
tags=$(git ls-remote --tags "$repo.git" 'refs/tags/engine-mac-*' 'refs/tags/engine-windows-*')

# The commit the $1 engine that engine/fetch.sh takes at commit $2 was built from: the nearest one
# in $2's history.
engine() {
  local ref tagged distance best='' found=''
  while read -r _ ref; do
    case $ref in refs/tags/engine-"$1"-*) ;; *) continue ;; esac
    case $ref in *"^{}") continue ;; esac
    tagged=$(git rev-parse -q --verify "${ref#refs/tags/engine-"$1"-}^{commit}" 2> /dev/null) ||
      continue
    git merge-base --is-ancestor "$tagged" "$2" || continue
    distance=$(git rev-list --count "$tagged..$2")
    if [ -z "$best" ] || [ "$distance" -lt "$best" ]; then
      found=$tagged best=$distance
    fi
  done <<< "$tags"
  [ -n "$found" ] && echo "$found"
}
# Whether the $1 engine at commit $2 has $2's core and engine, to the compiler: comments in the
# core's C++ don't count (core-same.sh, #2195), as they don't for the nights that build engines.
fits() {
  local built
  built=$(engine "$1" "$2") && tools/core-same.sh "$built" "$2" core engine
}

mac=$(engine mac HEAD) || { echo "No Mac engine in $repo was built from main's history." >&2; exit 1; }
# The first commit since the Mac engine's whose core/ or engine/ isn't the engine's, which it
# doesn't have.
changed=
for each in $(git rev-list --first-parent --reverse "$mac..HEAD" -- core engine); do
  tools/core-same.sh "$mac" "$each" core engine 2> /dev/null || { changed=$each; break; }
done
if [ -n "$changed" ]; then
  commit=$(git rev-parse "$changed^")
  echo "The Mac engine (${mac:0:12}) is from before ${changed:0:12}, which changed core/ or" \
    "engine/, so the nightly is ${commit:0:12}, the commit before it." >&2
else
  commit=$(git rev-parse HEAD)
  echo "The Mac engine (${mac:0:12}) has the core of ${commit:0:12}, the newest commit." >&2
fi
fits mac "$commit" || {
  echo "The Mac engine at ${commit:0:12} has another core: it was built on a branch main merged." >&2
  exit 1
}
windows=true
if ! fits windows "$commit"; then
  windows=false
  echo "No Windows engine has the core of ${commit:0:12}, so the feed keeps Windows' previous" \
    "build." >&2
fi
echo "commit=$commit"
echo "windows=$windows"
