#!/bin/bash
# Publishes the engine engine/build.sh built, in out/Release or the out/ BUILD_OUT names as
# build.sh takes it, as the Mac's prebuilt engine: packs its Chromium.app into
# Chromium-mac-arm64.tar.xz, and once local_dsyms below allows, its dSYMs into
# Chromium-mac-arm64-dSYMs.tar.xz (tools/mac-dsyms.sh), and uploads them, each with its SHA-256,
# as the release engine-mac-<commit> of the repository engine/prebuilt.ini names, named for the
# commit of ilo's (app/branding/repository) it was built from, as engine-linux.yml publishes
# Linux's. engine/fetch.sh on a Mac takes the one nearest a checkout's history. Runs on the Mac after
# engine/build.sh, as tools/mac-nightly.sh does. The first engine uploaded for a commit stays,
# whether this script or Engine on macOS (.github/workflows/engine-mac.yml) built it, since a check
# may already have run on it: a second build of the commit is never uploaded. It reads nothing of
# the owner's but gh's login, which uploads the release.
# `symbols` adds the dSYMs alone to a release that has its engine and none: the build in its out/
# isn't rebuilt, and its framework's UUID must be the published framework's, which is all it reads
# of the published engine (#2951).
# Usage: mac-engine-publish.sh [symbols]
set -euo pipefail
symbols=
[ "${1:-}" != symbols ] || symbols=1
# A dSYM built here holds this Mac's build directory, the account's name in it, as each unit's
# compilation directory, and the release is public. So none is published from here until the owner
# says yes, when this becomes home-ok, which tools/mac-dsyms.sh takes for that yes (#2951).
local_dsyms=
root=$(cd "$(dirname "$0")/.." && pwd)
engine=$root/engine
out=$engine/.deps/src/out/${BUILD_OUT:-Release}

fail() {
  echo "mac-engine-publish: $*" >&2
  exit 1
}

[ "$(uname -sm)" = "Darwin arm64" ] || fail "the Mac's engine is built on a Mac with Apple silicon."
# The release names the commit as the engine's source, so the build can't hold anything else: a
# clean engine/ and core/, a commit GitHub has, and the tree at the Chromium pins.ini names.
[ -z "$(git -C "$root" status --porcelain -- engine core)" ] ||
  fail "engine/ or core/ has uncommitted changes. Commit them first."
commit=$(git -C "$root" rev-parse HEAD)
repo=$(git -C "$root" config -f engine/prebuilt.ini prebuilt.repository)
repo=${repo#https://github.com/}
source=$(cat "$root/app/branding/repository")
gh api "repos/${source#https://github.com/}/commits/$commit" --silent 2>/dev/null ||
  fail "$source has no commit ${commit:0:12}. Push it first."
version=$(git -C "$root" config -f engine/pins.ini chromium.version)
built=$(sed -n 's/^[A-Z]*=//p' "$engine/.deps/src/chrome/VERSION" | paste -sd. -)
[ "$built" = "$version" ] ||
  fail "the tree has Chromium $built and pins.ini names $version. Update it with engine/setup.sh update."
tag=engine-mac-${commit:0:12}
framework="Chromium.app/Contents/Frameworks/Chromium Framework.framework/Chromium Framework"
# Says which engine the release has, if it has one, as "<label> uploaded <time>", or with the name
# of an asset, which of it. Without the release, gh prints GitHub's answer, which isn't one.
published() {
  local there
  there=$(gh api "repos/$repo/releases/tags/$tag" --jq '.assets[] | select(.name == "'"${1:-Chromium-mac-arm64.tar.xz}"'") |
    "\(if (.label // "") == "" then .name else .label end) uploaded \(.created_at)"' 2>/dev/null) || there=
  echo "$there"
}
there=$(published)

if [ -n "$symbols" ]; then
  [ -n "$local_dsyms" ] ||
    fail "a dSYM built here holds this Mac's build directory, and $repo is public: publishing one waits on the owner's yes (#2951)."
  [ -n "$there" ] || fail "$tag has no engine to add dSYMs to."
  there=$(published Chromium-mac-arm64-dSYMs.tar.xz)
  [ -z "$there" ] || { echo "mac-engine-publish: $tag already has its dSYMs, $there."; exit 0; }
  mkdir -p "$engine/.deps/publish"
  shipped=$(mktemp -d "$engine/.deps/publish/shipped.XXXX")
  # Only the framework comes out of the published engine, and nothing of it is kept.
  curl -fsSL --retry 5 "https://github.com/$repo/releases/download/$tag/Chromium-mac-arm64.tar.xz" |
    tar -xJf - -C "$shipped" "Chromium.app/Contents/Frameworks/Chromium Framework.framework/Versions/*/Chromium Framework" ||
    { rm -rf "$shipped"; fail "couldn't take the framework out of $tag's engine."; }
  status=0
  "$root/tools/mac-dsyms.sh" "$out" "$(echo "$shipped"/Chromium.app/Contents/Frameworks/*/Versions/*/Chromium\ Framework)" \
    "$engine/.deps/publish" "$local_dsyms" || status=$?
  rm -rf "$shipped"
  [ "$status" = 0 ] || fail "the dSYMs in $out aren't the build $tag published, so none are uploaded."
  archive=$engine/.deps/publish/Chromium-mac-arm64-dSYMs.tar.xz
  gh release upload "$tag" -R "$repo" "$archive#Chromium-mac-arm64-dSYMs.tar.xz, from tools/mac-engine-publish.sh symbols" "$archive.sha256"
  rm "$archive" "$archive.sha256"
  echo "mac-engine-publish: added the dSYMs to $tag."
  exit 0
fi
[ -z "$there" ] || { echo "mac-engine-publish: $tag already has its engine, $there."; exit 0; }
# build.sh relinks this checkout's core, applies its series and builds what changed, which takes
# seconds when the build is already this commit's.
"$engine/build.sh"

mkdir -p "$engine/.deps/publish"
archive=$engine/.deps/publish/Chromium-mac-arm64.tar.xz
# Without the Mac's metadata, which tar would otherwise add as ._ files.
tar -cJf "$archive" --no-mac-metadata --options xz:threads=0 -C "$out" Chromium.app
shasum -a 256 "$archive" | cut -d' ' -f1 >"$archive.sha256"
set -- "$archive#Chromium-mac-arm64.tar.xz, from tools/mac-engine-publish.sh" "$archive.sha256"
dsyms=$engine/.deps/publish/Chromium-mac-arm64-dSYMs.tar.xz
if [ -n "$local_dsyms" ]; then
  "$root/tools/mac-dsyms.sh" "$out" "$out/$framework" "$engine/.deps/publish" "$local_dsyms"
  set -- "$@" "$dsyms#Chromium-mac-arm64-dSYMs.tar.xz, from tools/mac-engine-publish.sh" "$dsyms.sha256"
fi
# Never GitHub's Latest, which releases/latest/download/<name> links mean as a release of its
# repository's own (#804). The tag is on that repository's main, which hasn't this commit: the
# name carries it.
gh release view "$tag" -R "$repo" >/dev/null 2>&1 ||
  gh release create "$tag" -R "$repo" --target main --title "Mac engine ${commit:0:12}" --latest=false \
    --notes "The Mac engine (Chromium $version, Apple silicon) built from $source/commit/$commit, which holds its source."
# Engine on macOS may have uploaded one while this built.
there=$(published)
if [ -n "$there" ]; then
  rm -f "$archive" "$archive.sha256" "$dsyms" "$dsyms.sha256"
  echo "mac-engine-publish: $tag already has its engine, $there, so this build isn't uploaded."
  exit 0
fi
gh release upload "$tag" -R "$repo" "$@"
rm -f "$archive" "$archive.sha256" "$dsyms" "$dsyms.sha256"
echo "mac-engine-publish: released $tag."
