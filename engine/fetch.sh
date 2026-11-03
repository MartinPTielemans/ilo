#!/bin/bash
# Downloads the prebuilt engine, checked against its hash, into .deps/prebuilt, and prints the
# path of its Chromium.app, or on Linux and Windows its Chromium directory. It's the newest
# engine-mac, engine-linux or engine-windows release built from a commit in this checkout's
# history, in the repository prebuilt.ini names: the owner's Mac publishes the Mac's
# (tools/mac-engine-publish.sh), and .github/workflows/engine-linux.yml and engine-windows-cross.yml
# the others. It exits with 2 when no engine fits this checkout, which CI takes for a reason to
# skip, and 1 on any other failure.
# `fetch.sh symbols` does the same for the Windows engine's PDBs, which its release has beside it,
# for crash reports (tools/symbols.sh). `fetch.sh commit` only prints the commit that engine was
# built from, without the header's check or a download (tools/mac-nightly.sh).
# Usage: fetch.sh [symbols|commit]
set -euo pipefail
symbols=
[ "${1:-}" != symbols ] || symbols=1
only_commit=
[ "${1:-}" != commit ] || only_commit=1
cd "$(dirname "$0")"

fail() {
  echo "fetch: $*" >&2
  exit 1
}
none() {
  echo "fetch: $*" >&2
  exit 2
}

case $(uname -sm) in
"Darwin arm64")
  [ -z "$symbols" ] || fail "the Mac engine's symbols are its dSYMs, beside Chromium.app."
  platform=mac asset=Chromium-mac-arm64.tar.xz app=Chromium.app
  publisher="tools/mac-engine-publish.sh on the Mac, after engine/build.sh"
  sha256sum() { shasum -a 256 "$@"; }
  ;;
"Linux x86_64" | MINGW*" x86_64")
  case $(uname -s) in
  Linux) platform=linux asset=Chromium-linux-x64.tar.xz ;;
  *) platform=windows asset=Chromium-windows-x64${symbols:+-symbols}.zip ;;
  esac
  [ -z "$symbols" ] || [ "$platform" = windows ] ||
    fail "only the Windows engine keeps its symbols apart; the Linux engine's are in its files."
  app=Chromium
  publisher="the engine-$platform workflow"
  ;;
*) fail "there's a prebuilt engine for Macs with Apple silicon, x86-64 Linux and x64 Windows only." ;;
esac

repo=$(git config -f prebuilt.ini prebuilt.repository)
# A release is named for the commit it was built from, which is ilo's, not the engine repository's,
# where its tag is. The nearest one in this history wins.
commit=
best=
while read -r _ ref; do
  case $ref in *"^{}") continue ;; esac
  tagged=$(git rev-parse -q --verify "${ref#refs/tags/engine-"$platform"-}^{commit}" 2>/dev/null) ||
    continue
  git merge-base --is-ancestor "$tagged" HEAD 2>/dev/null || continue
  distance=$(git rev-list --count "$tagged..HEAD")
  if [ -z "$best" ] || [ "$distance" -lt "$best" ]; then
    commit=$tagged
    best=$distance
  fi
done < <(git ls-remote --tags "$repo.git" "refs/tags/engine-$platform-*")
[ -n "$commit" ] ||
  none "no prebuilt $platform engine in $repo was built from this checkout's history. Fetch the history with git fetch --unshallow, or publish one with $publisher."
if [ -n "$only_commit" ]; then
  echo "$commit"
  exit 0
fi
url=$repo/releases/download/engine-$platform-${commit:0:12}/$asset
sha256=$(curl -fsSL "$url.sha256") || none "the release engine-$platform-${commit:0:12} has no $asset yet."

# The engine holds the core of the commit it was built from. A different header would bind the app
# to functions or layouts the engine doesn't have.
git cat-file -e "$commit^{commit}" 2>/dev/null ||
  fail "the prebuilt engine's commit ${commit:0:12} isn't in this clone. Fetch the history with git fetch --unshallow."
git diff --quiet "$commit" -- ../core/public/browser_core.h ||
  none "core/public/browser_core.h has changed since the prebuilt engine (${commit:0:12}). Build the engine with engine/build.sh."

dir=$PWD/.deps/prebuilt/$sha256
if [ ! -d "$dir" ]; then
  mkdir -p .deps/prebuilt
  name=${url##*/}
  archive=$dir.${name#*.}
  echo "fetch: downloading the prebuilt engine (${commit:0:12})" >&2
  curl -fL --progress-bar -o "$archive" "$url"
  echo "$sha256  $archive" | sha256sum -c --status ||
    { rm -f "$archive"; fail "the download doesn't match its sha256."; }
  rm -rf "$dir.part"
  mkdir "$dir.part"
  case $archive in
  *.zip) unzip -q "$archive" -d "$dir.part" ;;
  *) tar -xf "$archive" -C "$dir.part" ;;
  esac
  rm "$archive"
  mv "$dir.part" "$dir"
  # Earlier prebuilt engines go, but not for the symbols, which go with the next engine. A built
  # app keeps its own copy.
  [ -n "$symbols" ] || find .deps/prebuilt -mindepth 1 -maxdepth 1 ! -name "$sha256" -exec rm -rf {} +
fi
echo "$dir/$app"
