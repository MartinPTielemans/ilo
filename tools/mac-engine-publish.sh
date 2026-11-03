#!/bin/bash
# Publishes the Release engine engine/build.sh built as the Mac's prebuilt engine: packs its
# Chromium.app into Chromium-mac-arm64.tar.xz and uploads it, with its SHA-256, as the release
# engine-mac-<commit> of the repository engine/prebuilt.ini names, named for the commit of ilo's
# (app/branding/repository) it was built from, as engine-linux.yml publishes Linux's.
# engine/fetch.sh on a Mac takes the one nearest a checkout's history. Runs on the Mac after
# engine/build.sh, as tools/mac-nightly.sh does. It reads nothing of the owner's but gh's login,
# which uploads the release.
# Usage: mac-engine-publish.sh
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
engine=$root/engine
out=$engine/.deps/src/out/Release

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
# build.sh relinks this checkout's core, applies its series and builds what changed, which takes
# seconds when the build is already this commit's.
"$engine/build.sh"

tag=engine-mac-${commit:0:12}
mkdir -p "$engine/.deps/publish"
archive=$engine/.deps/publish/Chromium-mac-arm64.tar.xz
# Without the Mac's metadata, which tar would otherwise add as ._ files.
tar -cJf "$archive" --no-mac-metadata --options xz:threads=0 -C "$out" Chromium.app
shasum -a 256 "$archive" | cut -d' ' -f1 >"$archive.sha256"
# Never GitHub's Latest, which releases/latest/download/<name> links mean as a release of its
# repository's own (#804). The tag is on that repository's main, which hasn't this commit: the
# name carries it.
gh release view "$tag" -R "$repo" >/dev/null 2>&1 ||
  gh release create "$tag" -R "$repo" --target main --title "Mac engine ${commit:0:12}" --latest=false \
    --notes "The Mac engine (Chromium $version, Apple silicon) built from $source/commit/$commit, which holds its source."
gh release upload "$tag" -R "$repo" --clobber "$archive" "$archive.sha256"
rm "$archive" "$archive.sha256"
echo "mac-engine-publish: released $tag."
