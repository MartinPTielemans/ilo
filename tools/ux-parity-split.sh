#!/bin/bash
# Splits docs/ux-parity.md into one file per area under docs/ux-parity/, so lanes working on
# different areas stop editing the same file. Each "## " heading is an area; headings that differ
# only in a parenthetical, such as the two Later sections, share a file. The text above the first
# heading becomes docs/ux-parity/README.md, with the list of areas. Then every reference to the
# old file in the repo points at the folder, and the check below runs.
# It splits by headings, not line numbers, so it re-runs on the newest main.
#
# Usage, from the repo root:
#   tools/ux-parity-split.sh          split, rewrite the links, check
#   tools/ux-parity-split.sh check    check a split already made against the file it replaced
#
# The check proves a pure move: every line of the old file, each table row included, appears in
# the new files exactly as often as it did in the old one, with only the area titles, the
# pointer lines and the README list added; and no file in the repo still names the old one.
set -euo pipefail

old=docs/ux-parity.md
dir=docs/ux-parity
self=tools/ux-parity-split.sh
pointer='Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.'
intro='One file per area, so lanes working on different areas edit different files. A new area gets its own file and a line here.'

cd "$(git rev-parse --show-toplevel)"

# Lines that are neither blank nor only in the split's own scaffolding, with links made relative
# to docs/ again, sorted so comm compares them as multisets.
lines() {
  sed -e 's#](\.\./#](#g' -e 's/^# /## /' "$@" | grep -v '^[[:space:]]*$' | LC_ALL=C sort
}

check() {
  local before=$1 tmp
  tmp=$(mktemp -d)
  lines "$before" >"$tmp/old"
  lines "$dir"/*.md >"$tmp/new"
  LC_ALL=C comm -23 "$tmp/old" "$tmp/new" >"$tmp/missing"
  LC_ALL=C comm -13 "$tmp/old" "$tmp/new" >"$tmp/added"
  grep '^| \[' "$before" | LC_ALL=C sort >"$tmp/old-rows" || true
  cat "$dir"/*.md | grep '^| \[' | LC_ALL=C sort >"$tmp/new-rows" || true
  local fail=0
  echo "rows: $(wc -l <"$tmp/old-rows" | tr -d ' ') in $old, $(wc -l <"$tmp/new-rows" | tr -d ' ') in $dir/"
  echo "rows in more than one place: $(uniq -d "$tmp/new-rows" | wc -l | tr -d ' ')"
  if ! cmp -s "$tmp/old-rows" "$tmp/new-rows"; then
    echo "rows differ:"
    diff "$tmp/old-rows" "$tmp/new-rows" || true
    fail=1
  fi
  echo "lines of $old missing from $dir/: $(wc -l <"$tmp/missing" | tr -d ' ')"
  if [ -s "$tmp/missing" ]; then cat "$tmp/missing"; fail=1; fi
  echo "lines added by the split:"
  uniq -c "$tmp/added"
  if grep -q '^|' "$tmp/added"; then
    echo "a table line was added"
    fail=1
  fi
  local refs
  refs=$(git grep -n 'ux-parity\.md' -- . ":!$self" ":!$old" || true)
  echo "references to $old: $(printf '%s' "$refs" | grep -c . || true)"
  if [ -n "$refs" ]; then echo "$refs"; fail=1; fi
  echo "files: $(ls "$dir" | tr '\n' ' ')"
  rm -r "$tmp"
  if [ "$fail" = 0 ]; then echo "ux-parity-split: check passed"; else echo "ux-parity-split: check FAILED"; exit 1; fi
}

if [ "${1:-}" = check ]; then
  # The file as it was in the commit before the one that removed it.
  gone=$(git log -1 --format=%H --diff-filter=D -- "$old")
  [ -n "$gone" ] || { echo "no commit removes $old"; exit 1; }
  before=$(mktemp)
  git show "$gone^:$old" >"$before"
  echo "old file: $old at $(git rev-parse --short "$gone^")"
  check "$before"
  rm "$before"
  exit 0
fi

[ -f "$old" ] || { echo "no $old to split"; exit 1; }
[ ! -e "$dir" ] || { echo "$dir already exists"; exit 1; }
# Links into the file by anchor, or within it by anchor, need a hand: say so rather than guess.
if git grep -n 'ux-parity\.md#' -- . ":!$self" ":!$old"; then echo "links to an anchor above: point each at its area file by hand"; exit 1; fi
if grep -n '](#' "$old"; then echo "links within $old above: they would break when split"; exit 1; fi

before=$(mktemp)
cp "$old" "$before"
mkdir "$dir"

awk -v dir="$dir" -v pointer="$pointer" -v intro="$intro" '
  function slug(h, s) {
    s = h; sub(/ *\(.*$/, "", s); s = tolower(s)
    gsub(/[^a-z0-9]+/, "-", s); sub(/^-/, "", s); sub(/-$/, "", s)
    return s
  }
  # Links relative to docs/ move one folder down.
  function relink(l, out, i) {
    out = ""
    while ((i = index(l, "](")) > 0) {
      out = out substr(l, 1, i + 1); l = substr(l, i + 2)
      if (l !~ /^(https?:|mailto:|#|\/|\.\.\/)/) out = out "../"
    }
    return out l
  }
  /^## / {
    h = substr($0, 4); s = slug(h)
    if (!(s in count)) { order[++areas] = s; title[s] = h; sub(/ *\(.*$/, "", title[s]) }
    n = ++count[s]; head[s, n] = $0; cur = s SUBSEP n
    next
  }
  { if (cur == "") pre[++npre] = relink($0); else body[cur] = body[cur] relink($0) "\n" }
  function trim(t) { sub(/\n+$/, "\n", t); sub(/^\n+/, "", t); return t }
  END {
    while (npre > 0 && pre[npre] == "") npre--
    f = dir "/README.md"
    for (i = 1; i <= npre; i++) print pre[i] > f
    print "\n## Areas\n\n" intro "\n" > f
    for (a = 1; a <= areas; a++) print "- [" title[order[a]] "](" order[a] ".md)" > f
    close(f)
    for (a = 1; a <= areas; a++) {
      s = order[a]; f = dir "/" s ".md"
      if (count[s] == 1) {
        printf "# %s\n\n%s\n\n%s", substr(head[s, 1], 4), pointer, trim(body[s SUBSEP 1]) > f
      } else {
        printf "# %s\n\n%s\n", title[s], pointer > f
        for (n = 1; n <= count[s]; n++) printf "\n%s\n\n%s", head[s, n], trim(body[s SUBSEP n]) > f
      }
      close(f)
    }
  }
' "$old"
rm "$old"

# The folder replaces the file: a link opens its README, a mention names the folder.
git grep -l 'ux-parity\.md' -- . ":!$self" ":!$old" | while read -r f; do
  sed -e 's#[]](\([./]*\)ux-parity\.md)#](\1ux-parity/README.md)#g' -e 's#ux-parity\.md#ux-parity/#g' "$f" >"$f.split"
  mv "$f.split" "$f"
done

check "$before"
rm "$before"
