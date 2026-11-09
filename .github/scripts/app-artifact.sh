#!/bin/bash
# The app a check built, kept as an artifact named for its commit and OS, so that another check of
# the same commit takes it rather than building it again (#2199). Only the app's own files are kept:
# what build/Browser has that the engine doesn't, as the job fetches the engine either way. Run in
# app/.
# `app-artifact.sh find <name>` prints the run that has the artifact for this commit, or nothing.
# `app-artifact.sh pack <engine> <tar>` packs the files of build/Browser the engine lacks or differs in.
# `app-artifact.sh unpack <engine> <tar>` makes build/Browser again: a copy of the engine, with them.
set -euo pipefail
# Git Bash's tar takes D:\… for a remote host's path.
posix() { if command -v cygpath >/dev/null; then cygpath -u "$1"; else printf '%s\n' "$1"; fi; }

case ${1:-} in
find)
  gh api "repos/$GITHUB_REPOSITORY/actions/artifacts?name=$2&per_page=100" --jq \
    '[.artifacts[] | select((.expired | not) and .workflow_run.head_sha == env.GITHUB_SHA)][0].workflow_run.id // empty'
  ;;
pack)
  engine=$(posix "$2") tar=$(posix "$3")
  (cd build/Browser && find . -type f | while IFS= read -r file; do
    cmp -s "$file" "$engine/$file" || printf '%s\n' "$file"
  done) >"$tar.list"
  tar -cf "$tar" -C build/Browser -T "$tar.list"
  echo "app-artifact: $(wc -l <"$tar.list") files, $(wc -c <"$tar") bytes"
  ;;
unpack)
  engine=$(posix "$2") tar=$(posix "$3")
  rm -rf build/Browser
  mkdir -p build
  cp -r "$engine" build/Browser
  tar -xf "$tar" -C build/Browser
  ;;
*) echo "usage: app-artifact.sh find <name> | pack <engine> <tar> | unpack <engine> <tar>" >&2 && exit 2 ;;
esac
