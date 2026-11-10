#!/bin/bash
# Whether a night's engine build has anything to build (#871). It compares this commit's engine
# inputs with those of the newest engine-<os> release built from its history, in the repository
# engine/prebuilt.ini names, the one engine/fetch.sh would download. It exits 3 when they're the
# same, so the night builds nothing, and 0 when they differ, when there's no such release or its
# package isn't uploaded, or when it can't tell: a night it can't read builds.
# The inputs are each workflow's push paths, and the core, which the night builds too. Comments in
# the core's C++ don't count.
# engine-linux.yml and engine-windows-cross.yml ask it on their schedules; a push or a dispatch
# always builds. It's in tools/, not engine/, so a change to it doesn't start Core on Linux
# (ci-changes.cjs). mac-nightly.sh beside it makes the same call for the Mac's.
# Usage: engine-changed.sh linux|windows
build() {
  echo "engine-changed: $*"
  exit 0
}
same() {
  echo "engine-changed: $*"
  exit 3
}

case ${1:-} in
linux) own=engine/linux asset=Chromium-linux-x64.tar.xz ;;
windows) own=engine/windows/cross asset=Chromium-windows-x64.zip ;;
*) build "usage: engine-changed.sh linux|windows; building, as it can't tell." ;;
esac
platform=$1
inputs=(core engine/pins.ini engine/series engine/patches engine/tree.sh "$own" engine/icons.py
  app/platform/windows/package/Assets/ilo.ico ':(exclude)*.md')

cd "$(dirname "$0")/.." || build "no checkout to read."
repo=$(git config -f engine/prebuilt.ini prebuilt.repository) || build "no engine/prebuilt.ini."
tags=$(git ls-remote --tags "$repo.git" "refs/tags/engine-$platform-*") ||
  build "couldn't list $repo's engine-$platform releases."
# The nearest one in this history wins, as in fetch.sh.
commit=
best=
while read -r _ ref; do
  case $ref in "" | *"^{}") continue ;; esac
  tagged=$(git rev-parse -q --verify "${ref#refs/tags/engine-"$platform"-}^{commit}" 2>/dev/null) ||
    continue
  git merge-base --is-ancestor "$tagged" HEAD 2>/dev/null || continue
  distance=$(git rev-list --count "$tagged..HEAD")
  if [ -z "$best" ] || [ "$distance" -lt "$best" ]; then
    commit=$tagged
    best=$distance
  fi
done <<<"$tags"
head=$(git rev-parse --short=12 HEAD)
[ -n "$commit" ] || build "no engine-$platform release was built from $head's history."
tag=engine-$platform-${commit:0:12}
# A build that failed between making its release and uploading to it left a tag with no engine.
curl -fsSIL -o /dev/null "$repo/releases/download/$tag/$asset.sha256" ||
  build "$tag has no $asset."
# A change to the core's comments alone is the same to the compiler (core-same.sh, #2195).
tools/core-same.sh "$commit" HEAD "${inputs[@]}"
case $? in
0) same "$head has the core and engine of $tag, to the compiler." ;;
1) build "$head's core or engine differs from $tag's." ;;
*) build "couldn't compare $head with $tag." ;;
esac
