#!/bin/bash
# The owner's daily copy of the rework, refreshed each morning on their Mac: the newest commit on
# dev/rework whose CI (the UI on macOS among its jobs) and UI on Linux runs are green, built on the
# published engine (app/setup.sh prebuilt) with `app/build.sh rework` and installed with
# tools/install.sh --rework as its own app beside the daily one. A local build, as no hosted run
# keeps a rework build: check-mac's artifacts are dev builds, on the mock Keychain under the dev id.
# It builds in a worktree of its own, ~/.cache/woven-rework/tree, and remembers the commit it
# installed there, so a morning with nothing new does nothing.
# It never launches the app, and does nothing while the copy is running. The copy's profile, in
# ~/Library/Application Support/<branding/rework/name>, and its Keychain items stay as they are:
# only the bundle is replaced. Scheduling it (launchd) is a step of its own.
# Usage: tools/rework-refresh.sh [--dry-run]. --dry-run says what it would install where.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
repo_url=$(cat "$here/app/branding/repository")
repo=${repo_url#https://github.com/}
branch=${REWORK_BRANCH:-dev/rework}
to=${REWORK_APPS:-/Applications}
work=${REWORK_WORK:-$HOME/.cache/woven-rework}
name=$(cat "$here/app/branding/rework/name")
id=$(cat "$here/app/branding/rework/id")
dry=
[ "${1:-}" != --dry-run ] || dry=1

say() { echo "rework-refresh: $(date -u +%FT%RZ) $*"; }
skip() {
  say "skipped: $*"
  exit 0
}

# Swapping the bundle under a running Chromium would mix its helpers' versions, and quitting it
# is the owner's to do.
! pgrep -f "$to/$name.app/Contents" >/dev/null || skip "$name is running"

# The commits whose run of a workflow passed, newest first.
green() {
  gh run list --repo "$repo" --branch "$branch" --workflow "$1" --status success --limit 30 \
    --json headSha --jq '.[].headSha'
}
ui=$(green app-linux.yml)
commit=
for sha in $(green ci.yml); do
  if grep -qx "$sha" <<<"$ui"; then
    commit=$sha
    break
  fi
done
[ -n "$commit" ] || skip "no commit on $branch has CI and UI on Linux green"
installed=$(cat "$work/installed" 2>/dev/null || true)
if [ "$installed" = "$commit" ] && [ -d "$to/$name.app" ]; then
  skip "$to/$name.app is $commit already"
fi

if [ -n "$dry" ]; then
  say "would build $branch at $commit in $work/tree with app/setup.sh prebuilt, app/build.sh rework"
  say "would install it ($id) as $to/$name.app, replacing ${installed:-nothing}"
  say "its profile: ~/Library/Application Support/$name, its Keychain item: $name Safe Storage, both kept"
  exit 0
fi

[ "$(uname -sm)" = "Darwin arm64" ] || { echo "rework-refresh: the copy is built on a Mac with Apple silicon." >&2; exit 1; }
say "building $branch at $commit"
git -C "$here" fetch -q "$repo_url.git" "+refs/heads/$branch:refs/woven-rework/head"
git -C "$here" merge-base --is-ancestor "$commit" refs/woven-rework/head ||
  { echo "rework-refresh: $commit isn't on $branch." >&2; exit 1; }
mkdir -p "$work"
[ -d "$work/tree" ] || git -C "$here" worktree add -q --detach "$work/tree" "$commit"
git -C "$work/tree" checkout -q --detach "$commit"
(cd "$work/tree/app" && ./setup.sh prebuilt && nice -n 10 ./build.sh rework)
"$work/tree/tools/install.sh" --rework "$to"
echo "$commit" >"$work/installed"
say "installed $commit as $to/$name.app"
