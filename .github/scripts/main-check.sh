#!/bin/bash
# The verdicts of the full checks (check-linux.yml, check-windows.yml, check-mac.yml), which run each
# night and when the integrator dispatches them on main after a merge train (#871).
#
# `main-check.sh seen` writes seen=true to GITHUB_OUTPUT when another run of this workflow on this
# branch already checked this commit, so a night with no new merge costs no runner. A run checked
# it when its job named "check", the one with the default runs, failed, or passed with its "Runs"
# step done: one with no engine for the commit, or cancelled, checked nothing.
#
# `main-check.sh report <result> <found>` keeps one open issue per workflow, "<workflow> fails on
# main": a run that fails opens it, or comments on it if it's open, and one that passes with an
# engine found (<found> true) closes it. Other results, such as cancelled, leave it as it is.
# MAIN_CHECK_DRY=1 prints the issue it would write instead.
set -euo pipefail
repo=$GITHUB_REPOSITORY
runs=$GITHUB_SERVER_URL/$repo/actions/runs
run=$runs/$GITHUB_RUN_ID
gh() {
  if [ -n "${MAIN_CHECK_DRY:-}" ] && [ "$1" = issue ] && [ "$2" != list ]; then
    printf 'would run: gh'
    printf ' %q' "$@"
    echo
  else
    command gh "$@"
  fi
}

case ${1:-} in
seen)
  workflow=$(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID" --jq .workflow_id)
  for id in $(gh api "repos/$repo/actions/workflows/$workflow/runs?head_sha=$GITHUB_SHA&branch=$GITHUB_REF_NAME&status=completed&per_page=100" \
    --jq ".workflow_runs[] | select(.id != $GITHUB_RUN_ID) | .id"); do
    checked=$(gh api "repos/$repo/actions/runs/$id/jobs" --jq 'any(.jobs[]; .name == "check" and
      (.conclusion == "failure" or (.conclusion == "success" and any(.steps[]; .name == "Runs" and .conclusion == "success"))))')
    if [ "$checked" = true ]; then
      echo "$runs/$id already checked ${GITHUB_SHA:0:12} on $GITHUB_REF_NAME." | tee -a "$GITHUB_STEP_SUMMARY"
      echo "seen=true" >> "$GITHUB_OUTPUT"
      exit 0
    fi
  done
  echo "No run has checked ${GITHUB_SHA:0:12} on $GITHUB_REF_NAME yet."
  echo "seen=false" >> "$GITHUB_OUTPUT"
  ;;
report)
  result=$2 found=$3
  title="$GITHUB_WORKFLOW fails on main"
  issue=$(gh issue list --repo "$repo" --state open --search "in:title \"$title\"" --json number,title \
    --jq "map(select(.title == \"$title\"))[0].number // empty")
  case $result in
  failure)
    # What each failed job said went wrong, or else its failed steps.
    failed=
    for job in $(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID/jobs" --jq '.jobs[] | select(.conclusion == "failure") | .id'); do
      said=$(gh api "repos/$repo/check-runs/$job/annotations" --jq '.[] | select(.annotation_level == "failure"
        and (.message | startswith("Process completed with exit code") | not)) | "- " + .message')
      # shellcheck disable=SC2016 # $job is jq's.
      [ -n "$said" ] || said=$(gh api "repos/$repo/actions/jobs/$job" --jq '.name as $job | .steps[]
        | select(.conclusion == "failure") | "- \($job): the \(.name) step failed"')
      failed+=$said$'\n'
    done
    body="[A run]($run) ($GITHUB_EVENT_NAME) failed on main at $GITHUB_SHA.

$failed"
    if [ -n "$issue" ]; then
      gh issue comment "$issue" --repo "$repo" --body "$body"
    else
      gh issue create --repo "$repo" --title "$title" --body "$body
The next run of $GITHUB_WORKFLOW on main that passes closes this issue (.github/scripts/main-check.sh)."
    fi
    ;;
  success)
    if [ "$found" = true ] && [ -n "$issue" ]; then
      gh issue close "$issue" --repo "$repo" --comment "[It passes]($run) on main at $GITHUB_SHA."
    fi
    ;;
  esac
  ;;
*) echo "usage: main-check.sh seen | report <result> <found>" >&2 && exit 2 ;;
esac
