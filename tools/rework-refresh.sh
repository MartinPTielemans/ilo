#!/bin/bash
# The owner's daily copy of the rework, refreshed each morning on their Mac, built on the published
# engine (app/setup.sh prebuilt) with `app/build.sh rework` and installed with
# tools/install.sh --rework as its own app beside the daily one. Its commit is the newest on
# dev/rework the look-build lane marked green (a "look-build" commit status of success), or, while
# none is, the newest whose CI (the UI on macOS among its jobs) and UI on Linux runs passed. A local
# build, as no hosted run keeps a rework build: check-mac's artifacts are dev builds, on the mock
# Keychain under the dev id.
# It builds in a worktree of its own, ~/.cache/woven-rework/tree, and remembers the commit it
# installed there, so a morning with nothing new does nothing.
# It never launches the app, and does nothing while the copy is running. The copy's profile, in
# ~/Library/Application Support/<branding/rework/name>, and its Keychain items stay as they are:
# only the bundle is replaced. Scheduling it (launchd) is a step of its own.
# The Mac takes one heavy job at a time, so it skips the morning, never waiting, when the check
# VM's lock (~/.cache/tart/locks) is held or a VM, an engine build, a bench or another cargo build
# runs, while memory is under pressure, or with under 60 GB free; and stops between steps once
# memory comes under pressure or swap grows by 1.5 GB. It builds under nice with 4 jobs, Release
# as a prod build is: the fast profile's copy would be slower than the one the owner judges.
# Usage: tools/rework-refresh.sh [--dry-run]. --dry-run says what it would install where.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
repo_url=$(cat "$here/app/branding/repository")
repo=${repo_url#https://github.com/}
branch=${REWORK_BRANCH:-dev/rework}
to=${REWORK_APPS:-/Applications}
work=${REWORK_WORK:-$HOME/.cache/woven-rework}
locks=${REWORK_VM_LOCKS:-$HOME/.cache/tart/locks}
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

# What holds the Mac's heavy slot, if anything: the same marks the other lanes go by.
held() {
  local lock pattern level free
  for lock in "$locks"/*/; do
    [ -d "$lock" ] || continue
    echo "the check VM's lock $(basename "$lock") is held ($(cat "$lock/who" 2>/dev/null || echo "by no one named"))"
    return
  done
  # One pattern a call: the Mac's pgrep takes no -E.
  for pattern in "tart run" "engine/build.sh" "autoninja" "ninja -C" "mind2web" "ilo-bench" \
    "cargo build" "cargo test" "cargo run" "rustc"; do
    if pgrep -f "$pattern" >/dev/null; then
      echo "a heavy job runs ($pattern)"
      return
    fi
  done
  if [ "$(uname -s)" = Darwin ]; then
    level=$(sysctl -n kern.memorystatus_vm_pressure_level)
    [ "$level" = 1 ] || { echo "memory is under pressure (level $level)"; return; }
  fi
  free=$(df -Pk "$HOME" | awk 'NR == 2 { print int($4 / 1048576) }')
  [ "$free" -ge 60 ] || echo "the disk has $free GB free, under 60"
}
# Swap in use, in MB, which the Mac's rule measures a heavy job's growth by.
swap() {
  [ "$(uname -s)" = Darwin ] || { echo 0; return; }
  sysctl -n vm.swapusage | sed -n 's/.*used = \([0-9]*\).*/\1/p'
}
# Between steps: stops once memory is critical or swap has grown 1.5 GB since the start.
between() {
  [ "$(uname -s)" = Darwin ] || return 0
  local level
  level=$(sysctl -n kern.memorystatus_vm_pressure_level)
  [ "$level" != 4 ] || skip "stopped before $1: memory is critical; nothing was installed"
  [ "$(swap)" -le $((swap0 + 1536)) ] || skip "stopped before $1: swap grew past 1.5 GB; nothing was installed"
}

# The commit the look-build lane marked green last, from one query of the branch's history.
look_green() {
  gh api graphql -F owner="${repo%%/*}" -F name="${repo#*/}" -F ref="refs/heads/$branch" -f query='
    query($owner: String!, $name: String!, $ref: String!) {
      repository(owner: $owner, name: $name) { ref(qualifiedName: $ref) { target { ... on Commit {
        history(first: 50) { nodes { oid status { context(name: "look-build") { state } } } }
      } } } }
    }' --jq '[.data.repository.ref.target.history.nodes[] |
      select(.status.context.state == "SUCCESS") | .oid][0] // empty'
}
# The commits whose run of a workflow passed, newest first.
green() {
  gh run list --repo "$repo" --branch "$branch" --workflow "$1" --status success --limit 30 \
    --json headSha --jq '.[].headSha'
}
commit=$(look_green) source="look-build marked it green"
if [ -z "$commit" ]; then
  source="CI and UI on Linux passed on it, as no commit has a look-build mark"
  ui=$(green app-linux.yml)
  for sha in $(green ci.yml); do
    if grep -qx "$sha" <<<"$ui"; then
      commit=$sha
      break
    fi
  done
fi
[ -n "$commit" ] || skip "no commit on $branch is marked look-build green or has CI and UI on Linux green"
installed=$(cat "$work/installed" 2>/dev/null || true)
if [ "$installed" = "$commit" ] && [ -d "$to/$name.app" ]; then
  skip "$to/$name.app is $commit already"
fi
holder=$(held)

if [ -n "$dry" ]; then
  say "would build $branch at $commit ($source) in $work/tree with app/setup.sh prebuilt, app/build.sh rework"
  say "would install it ($id) as $to/$name.app, replacing ${installed:-nothing}"
  say "its profile: ~/Library/Application Support/$name, its Keychain item: $name Safe Storage, both kept"
  [ -z "$holder" ] || say "would skip now: $holder"
  exit 0
fi
[ -z "$holder" ] || skip "$holder"

[ "$(uname -sm)" = "Darwin arm64" ] || { echo "rework-refresh: the copy is built on a Mac with Apple silicon." >&2; exit 1; }
# The tree is checked out afresh each morning, so this script can't be the tree's own.
[ "$here" != "$(cd "$work/tree" 2>/dev/null && pwd)" ] ||
  { echo "rework-refresh: run it from a checkout other than $work/tree." >&2; exit 1; }
swap0=$(swap)
say "building $branch at $commit ($source)"
git -C "$here" fetch -q "$repo_url.git" "+refs/heads/$branch:refs/woven-rework/head"
git -C "$here" merge-base --is-ancestor "$commit" refs/woven-rework/head ||
  { echo "rework-refresh: $commit isn't on $branch." >&2; exit 1; }
mkdir -p "$work"
[ -d "$work/tree" ] || git -C "$here" worktree add -q --detach "$work/tree" "$commit"
git -C "$work/tree" checkout -q --detach "$commit"
(cd "$work/tree/app" && ./setup.sh prebuilt)
between "the build"
(cd "$work/tree/app" && CARGO_BUILD_JOBS=4 nice -n 10 ./build.sh rework)
between "the install"
"$work/tree/tools/install.sh" --rework "$to"
echo "$commit" >"$work/installed"
say "installed $commit as $to/$name.app"
