#!/bin/bash
# Downloads the prebuilt engine, checked against its hash, into .deps/prebuilt, and prints the path
# of its Chromium.app, or on Linux and Windows its Chromium directory. It's the nearest engine-mac,
# engine-linux or engine-windows release built from a commit in this checkout's history with this
# commit's core, or else the nearest, in the repository prebuilt.ini names: the owner's Mac
# publishes the Mac's (tools/mac-engine-publish.sh), and .github/workflows/engine-linux.yml and
# engine-windows-cross.yml the others. It exits with 2 when no engine fits this checkout, which CI
# takes for a reason to skip, and 1 on any other failure.
# `fetch.sh symbols` does the same for the engine's symbols, for crash reports (tools/symbols.sh):
# the Windows engine's PDBs, which its release has beside it, and on the Mac the engine with its
# dSYMs unpacked beside Chromium.app, where app/build.sh looks, printing their directory. A release
# without them exits with 2, as one without an engine does. `fetch.sh commit` only prints the
# commit that engine was built from, without the header's check or a download
# (tools/mac-nightly.sh).
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
# where its tag is. The nearest one in this history whose core is this commit's wins, or with none,
# the nearest: by commit count alone, an engine built on main after a batch branched is nearer than
# the batch's own once the batch lands, and lacks its core (#2913). tools/engine-changed.sh picks
# the same way. An engine the nearest was built over has this commit's core, when the nearest
# hasn't, only if a later commit reverted the core, where the nearest serves anyway, so only
# engines off the nearest's history are compared: on a core branch, which no engine has yet, they
# are the few a landing brought in, not every engine since the first.
# A failed listing says so, rather than that no engine was built from this history (#4027).
tags=$(git ls-remote --tags "$repo.git" "refs/tags/engine-$platform-*") ||
  fail "couldn't list the prebuilt engines in $repo: git ls-remote failed, as above."
# Each engine's commit, in one git call rather than three a tag: a Windows runner took 12 to 20 s a
# call for 39 tags (#4014). The count from each to HEAD is HEAD's commits less the engine's, as
# all of the engine's are HEAD's, counted over one listing of HEAD's history, the same count as
# git rev-list --count <engine>..HEAD.
tagged=$(printf '%s\n' "$tags" |
  sed -n "s|^[0-9a-f]*[[:space:]]*refs/tags/engine-$platform-\([^^]*\)\$|\1^{commit}|p" |
  git cat-file --batch-check='%(objectname)' | grep -E '^[0-9a-f]{40,64}$' || true)
built=
if [ -n "$tagged" ]; then
  built=$(git rev-list --parents HEAD | awk '
    NR == FNR { parents[$1] = substr($0, length($1) + 2); total++; next }
    ($1 in parents) {
      split("", seen)
      seen[$1] = 1
      stack[1] = $1
      top = 1
      count = 0
      while (top > 0) {
        commit = stack[top--]
        count++
        n = split(parents[commit], up, " ")
        for (i = 1; i <= n; i++)
          if (!(up[i] in seen)) { seen[up[i]] = 1; stack[++top] = up[i] }
      }
      print total - count, $1
    }' - <(printf '%s\n' "$tagged"))
fi
[ -n "$built" ] ||
  none "no prebuilt $platform engine in $repo was built from this checkout's history. Fetch the history with git fetch --unshallow, or publish one with $publisher."
nearest=
commit=
while read -r _ tagged; do
  if [ -z "$nearest" ]; then
    nearest=$tagged commit=$tagged
  elif git merge-base --is-ancestor "$tagged" "$nearest"; then
    continue
  fi
  if ../tools/core-same.sh "$tagged" HEAD core engine/series engine/patches ':(exclude)*.md' 2>/dev/null; then
    commit=$tagged
    break
  fi
done < <(printf '%s\n' "$built" | sort -n)
if [ -n "$only_commit" ]; then
  echo "$commit"
  exit 0
fi
url=$repo/releases/download/engine-$platform-${commit:0:12}/$asset
# --retry tries a host's 5xx again, as GitHub's 503 that stopped a whole Check on Linux (#2074),
# but not a 404, a release without its engine yet.
sha256=$(curl -fsSL --retry 5 "$url.sha256") || none "the release engine-$platform-${commit:0:12} has no $asset yet."

# The engine holds the core of the commit it was built from. A different header would bind the app
# to functions or layouts the engine doesn't have. One that differs only in its comments is the same
# to the compiler (tools/core-same.sh, #2195).
git cat-file -e "$commit^{commit}" 2>/dev/null ||
  fail "the prebuilt engine's commit ${commit:0:12} isn't in this clone. Fetch the history with git fetch --unshallow."
status=0
../tools/core-same.sh "$commit" - core/public/browser_core.h || status=$?
case $status in
0) ;;
1) none "core/public/browser_core.h has changed since the prebuilt engine (${commit:0:12}). Build the engine with engine/build.sh." ;;
*) fail "couldn't compare core/public/browser_core.h with the prebuilt engine's (${commit:0:12})." ;;
esac

# Off CI, the engines are in a cache every checkout shares, with app/setup.sh's GPUI (#3917), and
# one fetch or setup at a time changes it, as there. Each is made whole in .part, then moved, and
# never changed or deleted after: not made read-only, as the Mac's build signs its copy of the
# engine, which keeps the engine's modes. CI and Git Bash keep theirs in .deps/prebuilt.
cache=${ILO_SETUP_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/ilo-setup}
[ -z "${GITHUB_ACTIONS:-}" ] || cache=
[ "$platform" != windows ] || cache=
prebuilt=$PWD/.deps/prebuilt
[ -z "$cache" ] || prebuilt=$cache/prebuilt
# The lock is a link naming its holder's process, made in one step. One left by a process that was
# killed outright is named, not taken, as only the user knows none still runs.
locked=
lock() {
  [ -n "$cache" ] && [ -z "$locked" ] || return 0
  mkdir -p "$cache"
  trap unlock EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM HUP
  local waited=0 holder
  until ln -s "$$" "$cache/lock" 2>/dev/null; do
    holder=$(readlink "$cache/lock" 2>/dev/null || true)
    if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then
      fail "process $holder, which has ended, holds $cache/lock. Remove it if no setup is running."
    fi
    [ "$waited" -gt 0 ] ||
      echo "fetch: waiting for the setup or fetch${holder:+ in process $holder} using $cache" >&2
    waited=$((waited + 1))
    [ "$waited" -lt 1800 ] || fail "$cache/lock was held for 30 minutes."
    sleep 1
  done
  locked=1
}
unlock() {
  if [ -n "$cache" ] && [ "$(readlink "$cache/lock" 2>/dev/null)" = "$$" ]; then
    rm -f "$cache/lock"
  fi
  locked=
}

dir=$prebuilt/$sha256
# Looked for again once locked, as the fetch waited on may have made it.
[ -d "$dir" ] || lock
if [ ! -d "$dir" ]; then
  mkdir -p "$prebuilt"
  name=${url##*/}
  archive=$dir.${name#*.}
  echo "fetch: downloading the prebuilt engine (${commit:0:12})" >&2
  curl -fL --retry 5 --progress-bar -o "$archive" "$url"
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
  # Earlier prebuilt engines go, but not for the symbols, which go with the next engine, and not
  # from the cache. A built app keeps its own copy.
  [ -n "$symbols" ] || [ -n "$cache" ] ||
    find .deps/prebuilt -mindepth 1 -maxdepth 1 ! -name "$sha256" -exec rm -rf {} +
fi
if [ -n "$symbols" ] && [ "$platform" = mac ]; then
  [ -d "$dir/Chromium Framework.dSYM" ] || lock
  if ! [ -d "$dir/Chromium Framework.dSYM" ]; then
    url=${url%/*}/Chromium-mac-arm64-dSYMs.tar.xz
    sha256=$(curl -fsSL --retry 5 "$url.sha256") ||
      none "the release engine-mac-${commit:0:12} has no dSYMs, so its framework's frames can't be named."
    echo "fetch: downloading the prebuilt engine's dSYMs (${commit:0:12})" >&2
    archive=$dir.dSYMs.tar.xz
    curl -fL --retry 5 --progress-bar -o "$archive" "$url"
    echo "$sha256  $archive" | sha256sum -c --status ||
      { rm -f "$archive"; fail "the dSYMs' download doesn't match its sha256."; }
    rm -rf "$dir.dSYMs"
    mkdir "$dir.dSYMs"
    tar -xf "$archive" -C "$dir.dSYMs"
    rm "$archive"
    # Each moves in whole, the framework's last, as it's what says they're all there. One a killed
    # fetch already moved stays.
    for each in "$dir.dSYMs"/*.dSYM; do
      [ "${each##*/}" = "Chromium Framework.dSYM" ] || [ -e "$dir/${each##*/}" ] || mv "$each" "$dir"
    done
    [ ! -d "$dir.dSYMs/Chromium Framework.dSYM" ] || mv "$dir.dSYMs/Chromium Framework.dSYM" "$dir"
    rm -rf "$dir.dSYMs"
  fi
  echo "$dir"
  exit 0
fi
echo "$dir/$app"
