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

# The newest commit on main's line that the Mac engine taken at $head fits. An engine built on a
# branch reaches main's line at the merge that brought it in, and a branch that merged main after
# its engine was built brings in a core the engine doesn't have (#2697), so where no commit of the
# line has the engine's core, the night goes on from before that merge, with the engine taken there.
head=$(git rev-parse HEAD)
while :; do
  mac=$(engine mac "$head") ||
    { echo "No Mac engine in $repo was built from main's history." >&2; exit 1; }
  # The first commit of main's line that has the engine's commit: the one after it, or a merge.
  start=
  for each in $(git rev-list --first-parent --reverse "$mac..$head"); do
    if git merge-base --is-ancestor "$mac" "$each"; then
      start=$each
      break
    fi
  done
  # From there, the first commit whose core/ or engine/ isn't the engine's, which it doesn't have.
  changed=
  if [ -n "$start" ]; then
    for each in "$start" $(git rev-list --first-parent --reverse "$start..$head" -- core engine); do
      tools/core-same.sh "$mac" "$each" core engine 2> /dev/null || { changed=$each; break; }
    done
  fi
  if [ -z "$changed" ]; then
    commit=$head
    newest=
    [ "$commit" = "$(git rev-parse HEAD)" ] && newest=", the newest commit"
    echo "The Mac engine (${mac:0:12}) has the core of ${commit:0:12}$newest." >&2
    break
  fi
  commit=$(git rev-parse "$changed^")
  if git merge-base --is-ancestor "$mac" "$commit"; then
    echo "The Mac engine (${mac:0:12}) is from before ${changed:0:12}, which changed core/ or" \
      "engine/, so the nightly is ${commit:0:12}, the commit before it." >&2
    break
  fi
  echo "The Mac engine (${mac:0:12}) was built on a branch that ${changed:0:12} merged with" \
    "another core, so no commit of main's has its core; trying the one taken at ${commit:0:12}." >&2
  head=$commit
done
fits mac "$commit" || {
  echo "The Mac engine engine/fetch.sh takes at ${commit:0:12} has another core." >&2
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
