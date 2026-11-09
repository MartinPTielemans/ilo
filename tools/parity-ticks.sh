#!/bin/bash
# Lists the parity rows that PRs merged since the last parity-ticks PR name on their "Parity:"
# line (.github/pull_request_template.md), one PR a line, for the day's bookkeeping PR (#2204).
# The integrator, once a day: run this, then on a docs/parity-ticks-<date> branch tick each row it
# lists in docs/ux-parity/ with the run that proved it, update docs/roadmap.md's counts, and open the PR.
# Usage: tools/parity-ticks.sh [since], an ISO date or time; by default when the last
# docs/parity-ticks-* PR merged. Needs gh, signed in.
set -euo pipefail
repo=${PARITY_REPO:-MartinPTielemans/ilo}
since=${1:-$(gh pr list -R "$repo" --state merged --base main --limit 50 \
  --search 'head:docs/parity-ticks- sort:updated-desc' --json headRefName,mergedAt \
  --jq '[.[] | select(.headRefName | startswith("docs/parity-ticks-")) | .mergedAt] | max // empty')}
if [ -z "$since" ]; then
  echo "no parity-ticks PR has merged yet: give the date to start from, such as $0 2026-11-09" >&2
  exit 2
fi
# The template's comment under the line goes first, so an untouched template names nothing.
gh pr list -R "$repo" --state merged --base main --limit 500 --search "merged:>$since" \
  --json number,title,body --jq '
    .[] | .number as $n | (.body // "" | gsub("<!--(.|\n)*?-->"; "") | split("\n")[] | sub("\r$"; ""))
    | select(test("^\\s*Parity:")) | sub("^\\s*Parity:\\s*"; "") | select(test("^(none\\b|\\s*$)"; "i") | not)
    | "#\($n)\t\(.)"'
