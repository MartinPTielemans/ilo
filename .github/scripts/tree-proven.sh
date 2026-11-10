#!/bin/bash
# Prints the id of a run in which job <job> passed on tree <tree>, as .github/actions/tree-proof
# recorded it, or nothing, and says which on stderr. A push to main whose tree a branch or a pull
# request already tested skips the job (#2198): main's merges make new commits, but a merge of an
# up-to-date branch has the branch's tree. Only a record from this repository's own code counts,
# never a fork's. It reads the artifacts' list, so it needs GH_TOKEN with actions: read, and
# GITHUB_REPOSITORY and GITHUB_REPOSITORY_ID, which Actions sets. When it can't tell, it prints
# nothing, so the job runs. In a run, what it says goes in the run's summary too.
# Usage: tree-proven.sh <job> <tree>
set -uo pipefail
job=${1:-} tree=${2:-}
say() {
  echo "tree-proven: $*" >&2
  [ -z "${GITHUB_STEP_SUMMARY:-}" ] || echo "$*" >> "$GITHUB_STEP_SUMMARY"
}
[ -n "$job" ] && [ -n "$tree" ] || { say "no job or tree was given, so the job runs."; exit 0; }
run=$(gh api -X GET "repos/$GITHUB_REPOSITORY/actions/artifacts" -f name="proven-$job-$tree" \
  -f per_page=100 --jq "[.artifacts[] | .workflow_run
    | select(.head_repository_id == $GITHUB_REPOSITORY_ID)][0].id // empty") || {
  say "$job runs, as the artifacts' list couldn't be read."
  exit 0
}
if [ -n "$run" ]; then
  say "$job is skipped: it passed on this tree, ${tree:0:12}, in" \
    "${GITHUB_SERVER_URL:-https://github.com}/$GITHUB_REPOSITORY/actions/runs/$run."
  echo "$run"
else
  say "$job runs: no run has passed it on this tree, ${tree:0:12}."
fi
