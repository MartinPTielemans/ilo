#!/bin/bash
# Marks a dev/rework commit whose look build's checks are green (tools/check-select.sh's kind=look):
# a commit status of context "look-build", state success, its target the first run named. The
# rework copy's daily refresh installs the newest dev/rework commit with that mark, so a mark is
# given only when every run named finished successfully on exactly that commit and the commit is on
# origin/dev/rework. Name every run the build's selection started, the lib tests' and the checks'.
# Usage: tools/look-build-mark.sh <commit> <run-id>...
# LOOK_BUILD_MARK_DRY=1 checks everything and prints the status it would post, posting nothing.
set -euo pipefail
[ $# -ge 2 ] || { echo "usage: $0 <commit> <run-id>..." >&2; exit 2; }
commit=$(git rev-parse --verify "$1^{commit}") || { echo "$0: no commit $1" >&2; exit 1; }
shift
repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
git fetch -q origin dev/rework
git merge-base --is-ancestor "$commit" FETCH_HEAD ||
  { echo "$0: $commit isn't on origin/dev/rework, so the refresh would never install it" >&2; exit 1; }
url=
for run in "$@"; do
  said=$(gh run view "$run" --json headSha,status,conclusion,url,workflowName \
    -q '[.headSha, .status, .conclusion, .url, .workflowName] | join("|")')
  # Not a tab: read folds runs of whitespace separators, and an unfinished run's conclusion is empty.
  IFS='|' read -r sha status conclusion run_url workflow <<< "$said"
  [ "$sha" = "$commit" ] ||
    { echo "$0: run $run ($workflow) is on $sha, not $commit" >&2; exit 1; }
  [ "$status" = completed ] && [ "$conclusion" = success ] ||
    { echo "$0: run $run ($workflow) is $status${conclusion:+, $conclusion}, not a success" >&2; exit 1; }
  url=${url:-$run_url}
done
# GitHub keeps 140 characters of a status's description.
description="Look build's checks green: runs $*"
description=${description:0:140}
if [ "${LOOK_BUILD_MARK_DRY:-}" = 1 ]; then
  echo "would post to $repo $commit: look-build success $url ($description)"
  exit 0
fi
gh api -X POST "repos/$repo/statuses/$commit" -f state=success -f context=look-build \
  -f target_url="$url" -f description="$description" > /dev/null
echo "marked $commit look-build success: $url"
