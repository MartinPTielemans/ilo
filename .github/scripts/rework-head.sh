#!/bin/bash
# Whether a commit is on the rework line: whether it holds dev/rework's own line, the oldest commit
# of dev/rework's first parents that main lacks. Its name doesn't decide it, as the rework's lanes
# push fix/, feat/ and core/ branches too; nor does holding commits dev/rework took in, as a branch
# for main that the rework merged has its commits in dev/rework but none of dev/rework's own.
# CI skips the iPhone app's and the Mac app's jobs on a push of such a commit
# (.github/actions/ci-changes): Check on macOS covers the Mac at each build's cut
# (tools/cut-checks.sh), and the iPhone app isn't reworked. Once main has the rework, dev/rework's
# line is main's too, so nothing is on the rework line and the jobs run again. Exits 0 for a commit
# on the line, saying so, 1 for one that isn't, and 2 when it can't tell, which CI takes as not on
# the line, so the jobs run.
# Usage: rework-head.sh [<commit>] (HEAD by default), in a clone with origin/main and origin/dev/rework.
set -uo pipefail
head=${1:-HEAD}
git rev-parse -q --verify origin/dev/rework > /dev/null ||
  git fetch -q --no-tags --filter=blob:none origin +refs/heads/dev/rework:refs/remotes/origin/dev/rework ||
  { echo "rework-head: no origin/dev/rework to compare with" >&2; exit 2; }
# A pipe closed by head early would fail rev-list under pipefail, so the whole list is read.
own=$(git rev-list --first-parent origin/dev/rework ^origin/main) ||
  { echo "rework-head: couldn't list dev/rework's own line" >&2; exit 2; }
first=$(printf '%s\n' "$own" | tail -n 1)
short=$(git rev-parse --short=12 "$head") || exit 2
if [ -n "$first" ] && git merge-base --is-ancestor "$first" "$head"; then
  echo "$short is on the rework line: it has $(git rev-parse --short=12 "$first"), where dev/rework's own line leaves main."
  exit 0
fi
echo "$short isn't on the rework line."
exit 1
