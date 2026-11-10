#!/bin/bash
# The app a check built, kept as an artifact named for its commit and OS, so that another check of
# the same commit takes it rather than building it again (#2199). Only the app's own files are kept:
# what the app has that the engine doesn't, and the names of the engine's files build.sh took out,
# as the job fetches the engine either way. The app is build/Browser, or build/Browser.app on the
# Mac. Run in app/.
# `app-artifact.sh find <name>` prints the run that has the artifact for this commit, or nothing.
# `app-artifact.sh pack <engine> <tar>` packs the files of the app the engine lacks or differs in.
# `app-artifact.sh unpack <engine> <tar>` makes the app again: a copy of the engine, with them.
set -euo pipefail
# Git Bash's tar takes D:\… for a remote host's path.
posix() { if command -v cygpath >/dev/null; then cygpath -u "$1"; else printf '%s\n' "$1"; fi; }
app=build/Browser copy="cp -R" tar=tar
# A clone of the engine, as build.sh makes, and no AppleDouble files in the tar.
[ "$(uname -s)" != Darwin ] || app=build/Browser.app copy="cp -cR" tar="tar --no-mac-metadata"
gone=.app-artifact-gone
case ${1:-} in
find)
  gh api "repos/$GITHUB_REPOSITORY/actions/artifacts?name=$2&per_page=100" --jq \
    '[.artifacts[] | select((.expired | not) and .workflow_run.head_sha == env.GITHUB_SHA)][0].workflow_run.id // empty'
  ;;
pack)
  engine=$(posix "$2") out=$(posix "$3") root=$PWD/$app
  (cd "$app" && find . -type f | while IFS= read -r file; do
    cmp -s "$file" "$engine/$file" || printf '%s\n' "$file"
  done) >"$out.list"
  # Such as the Mac's Chromium executable, which build.sh replaces with Browser.
  (cd "$engine" && find . -type f | while IFS= read -r file; do
    [ -e "$root/$file" ] || [ -L "$root/$file" ] || printf '%s\n' "$file"
  done) >"$app/$gone"
  echo "./$gone" >>"$out.list"
  $tar -cf "$out" -C "$app" -T "$out.list"
  rm "$app/$gone"
  echo "app-artifact: $(wc -l <"$out.list") files, $(wc -c <"$out") bytes"
  ;;
unpack)
  engine=$(posix "$2") out=$(posix "$3")
  rm -rf "$app"
  mkdir -p build
  $copy "$engine" "$app"
  tar -xf "$out" -C "$app"
  if [ -f "$app/$gone" ]; then
    (cd "$app" && while IFS= read -r file; do rm -f "$file"; done <"$gone" && rm "$gone")
  fi
  ;;
*) echo "usage: app-artifact.sh find <name> | pack <engine> <tar> | unpack <engine> <tar>" >&2 && exit 2 ;;
esac
