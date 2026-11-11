#!/bin/bash
# Looks for a Chromium security point release, for .github/workflows/engine-point-release.yml.
# For each platform section in pins.ini it finds the newest tag on the pinned major and bumps
# pins.ini to it, then dry-runs every patch the engine applies (ungoogled-chromium's, the
# platform's and our series) on only the files they touch, from Chromium's source tarball. It
# leaves what the workflow posts in the work directory: pr-title, pr-branch and pr-body when
# pins.ini changed, and major-<n>.md for each new major, which needs the owner's go.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: point-release.sh <work directory>" >&2 && exit 2; }
mkdir -p "$1"
work=$(cd "$1" && pwd)
. "$(dirname "$0")/tree.sh"
pins=$engine/pins.ini
# Tests point this at local repositories with made-up tags.
upstream=${POINT_RELEASE_UPSTREAM:-https://github.com/ungoogled-software}

fail() {
  echo "point-release: $*" >&2
  exit 1
}

for tool in git curl jq patch python3 sha256sum tar xz; do
  command -v "$tool" >/dev/null || fail "$tool is missing"
done

# A tag names the Chromium version it builds, then the platform's revision: 156.0.8078.47-1.1.
version_of() { echo "${1%%-*}"; }
major_of() { echo "${1%%.*}"; }
newer() { [ "$1" != "$2" ] && [ "$(printf '%s\n' "$1" "$2" | sort -V | tail -1)" = "$1" ]; }
# The newest tag of a repository that builds a version.
tag_for() { awk -v v="$2" '$0 == v || index($0, v "-") == 1' "$work/$1.tags" | sort -V | tail -1; }
# A quilt series' patches, in order, as ungoogled-chromium's utils/_common.py reads them.
series() { sed -e '/^#/d' -e 's/ #.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$1" | grep -v '^$' || true; }
# What Chrome's version history calls a section's platform.
chrome_platform() { case $1 in *-macos) echo mac ;; *-windows) echo win ;; *) echo linux ;; esac; }

version=$(git config -f "$pins" chromium.version)
git ls-remote --tags --refs "$upstream/ungoogled-chromium.git" | sed 's#.*refs/tags/##' \
  >"$work/ungoogled-chromium.tags"
sections=() shared=()
for section in ungoogled-chromium-macos ungoogled-chromium-portablelinux ungoogled-chromium-windows; do
  tag=$(git config -f "$pins" "$section.tag") || continue
  sections+=("$section")
  git ls-remote --tags --refs "$upstream/$section.git" | sed 's#.*refs/tags/##' >"$work/$section.tags"
  # The sections on [chromium]'s version move together, with those ilo rebased onto it (tree.sh);
  # one on its own alone.
  { [ "$(version_of "$tag")" = "$version" ] || rebased_platform "$section"; } && shared+=("$section")
done

# A new major is the owner's call: one file per major, listing the sections that have it.
rm -f "$work"/major-*.md "$work"/pr-*
for section in "${sections[@]}"; do
  pinned=$(major_of "$(git config -f "$pins" "$section.tag")")
  newest=$(sed 's/\..*//' "$work/$section.tags" | sort -n | tail -1)
  [ "$newest" -gt "$pinned" ] || continue
  file=$work/major-$newest.md
  [ -e "$file" ] || printf '%s\n\n%s\n\n' "Chromium $newest is out: does the engine move to it?" \
    "Point releases update the engine by themselves (#299); a new major waits for the owner's go. On a go, a lane bumps \`engine/pins.ini\`, rebases the patches, and the builds run overnight, as with 155 → 156." >"$file"
  echo "- $section $(awk -v m="$newest" 'index($0, m ".") == 1' "$work/$section.tags" | sort -V | tail -1), pinned at $(git config -f "$pins" "$section.tag")" >>"$file"
done

# The bump follows the Mac, which ships, so its patch lag never waits on another platform: the
# newest version on the pinned major that it has a tag for, or ungoogled-chromium has while ilo
# rebases the Mac's patches. Sections that have one too move with it, and rebased ones stay on their
# tag with the new ungoogled-chromium. Each change is "<section> <old tag> <new tag> <version>".
changes=() lagging=()
target=
leader=${shared[0]:-}
[[ " ${shared[*]:-} " == *" ungoogled-chromium-macos "* ]] && leader=ungoogled-chromium-macos
leader_tags=$work/$leader.tags
! rebased_platform "$leader" || leader_tags=$work/ungoogled-chromium.tags
if [ -n "$leader" ]; then
  for v in $(sed 's/-.*//' "$leader_tags" | sort -V -u -r); do
    [ "$(major_of "$v")" = "$(major_of "$version")" ] && newer "$v" "$version" || continue
    target=$v
    break
  done
fi
problems=()
if [ -n "$target" ]; then
  git config -f "$pins" chromium.version "$target"
  uc_tag=$(tag_for ungoogled-chromium "$target")
  if [ -n "$uc_tag" ]; then
    git config -f "$pins" ungoogled-chromium.tag "$uc_tag"
  else
    problems+=("ungoogled-chromium has no tag for $target.")
  fi
  for section in ${shared[@]+"${shared[@]}"}; do
    old=$(git config -f "$pins" "$section.tag")
    tag=$(tag_for "$section" "$target")
    if [ -n "$tag" ]; then
      changes+=("$section $old $tag $target")
      git config -f "$pins" "$section.tag" "$tag"
      [ ! -d "$engine/patches/$section" ] || [ "$(version_of "$old")" = "$version" ] ||
        problems+=("$section has tagged $target, so ilo's rebased copies of its patches in \`engine/patches/$section\` go in this PR.")
    elif rebased_platform "$section"; then
      changes+=("$section $old $old $target")
    else
      lagging+=("$section")
    fi
  done
fi
# A section off [chromium]'s version, such as a platform whose tag lags, follows on its own: to its
# newest version on its major that isn't past [chromium]'s.
cap=$(git config -f "$pins" chromium.version)
for section in "${sections[@]}"; do
  old=$(git config -f "$pins" "$section.tag")
  [ "$(version_of "$old")" != "$cap" ] && ! rebased_platform "$section" || continue
  v=$(sed 's/-.*//' "$work/$section.tags" | awk -v m="$(major_of "$old")" 'index($0, m ".") == 1' |
    while read -r v; do newer "$v" "$cap" || echo "$v"; done | sort -V | tail -1)
  newer "$v" "$(version_of "$old")" || continue
  changes+=("$section $old $(tag_for "$section" "$v") $v")
  git config -f "$pins" "$section.tag" "$(tag_for "$section" "$v")"
done
if [ ${#changes[@]} -eq 0 ]; then
  echo "point-release: no point release since the pins" >&2
  exit 0
fi

# Each moved section's repository at its new tag, and the files its patches and ours touch.
conflicts=()
for change in "${changes[@]}"; do
  read -r section old tag v <<<"$change"
  dir=$work/$section
  rm -rf "$dir"
  git clone -q -c advice.detachedHead=false --depth 1 --branch "$tag" --recurse-submodules \
    --shallow-submodules "$upstream/$section.git" "$dir"
  uc=$dir/ungoogled-chromium
  msg=$( (rebase_platform "$dir" "$section" && check_ungoogled "$uc" "$section $tag") 2>&1 >/dev/null) ||
    problems+=("$msg.")
  {
    series "$uc/patches/series" | sed "s#^#$uc/patches/#"
    series "$dir/patches/series" | sed "s#^#$dir/patches/#"
  } >"$dir.upstream"
  # Our series as apply_series in tree.sh takes it for this platform's tree.
  platform=
  [ "$section" != ungoogled-chromium-macos ] || platform=mac
  ours=$(series_for "$(major_of "$v")" "$platform") || fail "engine/series doesn't read"
  sed "s#^#$engine/patches/#" <<<"$ours" >"$dir.ours"
  # -p1 drops each path's first part, whatever it's called; a patch that adds a file has nothing
  # to extract.
  cat "$dir.upstream" "$dir.ours" | xargs cat | sed -n 's#^[-+]\{3\} [^/]*/\([^[:space:]]*\).*#\1#p' |
    grep -v '^dev/null$' >>"$work/files-$v" || true
done

# Applies a patch as its series applies it, or lists it as a conflict and leaves the tree alone.
try_patch() {
  local tree=$1 label=$2 file=$3
  shift 3
  if patch -d "$tree" -p1 -s --dry-run "$@" <"$file" >/dev/null 2>&1; then
    patch -d "$tree" -p1 -s "$@" <"$file" >/dev/null
  else
    conflicts+=("$label")
  fi
}

for v in $(for change in "${changes[@]}"; do read -r _ _ _ v <<<"$change" && echo "$v"; done | sort -u); do
  tarball=$work/chromium-$v-lite.tar.xz
  if [ ! -e "$tarball" ]; then
    url=https://commondatastorage.googleapis.com/chromium-browser-official/${tarball##*/}
    curl -fsSL --retry 3 -o "$tarball.partial" "$url"
    sum=$(curl -fsSL --retry 3 "$url.hashes" | awk '$1 == "sha256" { print $2 }')
    [ "$(sha256sum <"$tarball.partial" | cut -d' ' -f1)" = "$sum" ] || fail "${tarball##*/} doesn't match its hash"
    mv "$tarball.partial" "$tarball"
  fi
  base=$work/src-$v
  rm -rf "$base"
  mkdir -p "$base"
  sort -u "$work/files-$v" | sed "s#^#chromium-$v/#" >"$base.list"
  # Files a patch changes but the tarball lacks come from an earlier patch, or are a conflict the
  # dry-run reports.
  tar -xJf "$tarball" -C "$base" --strip-components=1 -T "$base.list" 2>"$base.log" ||
    ! grep -v -e 'Not found in archive' -e 'Exiting with failure status' -e 'Error exit delayed' "$base.log" >&2 ||
    fail "${tarball##*/} didn't extract"
  for change in "${changes[@]}"; do
    read -r section _ _ change_v <<<"$change"
    [ "$change_v" = "$v" ] || continue
    tree=$work/src-$section
    uc=$work/$section/ungoogled-chromium
    rm -rf "$tree"
    cp -R "$base" "$tree"
    while read -r file; do
      try_patch "$tree" "$section: ${file#"$work/$section/"}" "$file" --ignore-whitespace --forward --no-backup-if-mismatch
    done <"$work/$section.upstream"
    python3 "$uc/utils/domain_substitution.py" apply -r "$uc/domain_regex.list" \
      -f "$uc/domain_substitution.list" "$tree" >"$tree.domains.log" 2>&1
    while read -r file; do
      try_patch "$tree" "$section: engine/patches/${file#"$engine/patches/"}" "$file" -F0 -N
    done <"$work/$section.ours"
  done
done

# The PR. Patch lag is the days from Chrome's stable release of the version to this bump.
versions=$(for change in "${changes[@]}"; do read -r _ _ _ v <<<"$change" && echo "$v"; done | sort -uV)
if [ -n "$target" ]; then
  echo "fix(engine): the engine is on Chromium $target, a security point release" >"$work/pr-title"
else
  echo "fix(engine): ${changes[0]%% *} is on Chromium $(echo "$versions" | head -1), a security point release" >"$work/pr-title"
fi
# Named for each section's new version, so a lagging platform's later bump gets a branch of its own.
for change in "${changes[@]}"; do
  read -r section _ _ v <<<"$change"
  echo "${section#ungoogled-chromium-}-$v"
done | paste -sd+ - | sed 's#^#engine/point-#' >"$work/pr-branch"
{
  echo "## Problem"
  echo
  echo "A Chromium security point release is out on the pinned major. Point releases update the engine without asking (#299)."
  echo
  echo "## Change"
  echo
  for change in "${changes[@]}"; do
    read -r section old tag _ <<<"$change"
    if [ "$old" = "$tag" ]; then
      echo "- $section has no tag for $target yet, so it stays on $old with ilo's rebased patches (\`engine/patches/$section\`), on ungoogled-chromium $(git config -f "$pins" ungoogled-chromium.tag)."
    else
      echo "- $section $old → $tag"
    fi
  done
  [ -z "$target" ] || echo "- Chromium $version → $target, ungoogled-chromium $(git config -f "$pins" ungoogled-chromium.tag)"
  for section in ${lagging[@]+"${lagging[@]}"}; do
    old=$(git config -f "$pins" "$section.tag")
    echo "- $section has no tag for $target yet, so it stays on $old and follows in its own PR."
  done
  echo
  for change in "${changes[@]}"; do
    read -r section _ _ v <<<"$change"
    platform=$(chrome_platform "$section")
    lag=$(curl -fsS "https://versionhistory.googleapis.com/v1/chrome/platforms/$platform/channels/stable/versions/$v/releases" |
      jq -r --arg p "$platform" '[.releases[].serving.startTime | sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601] |
        min // empty | "reached \($p) stable on \(strftime("%Y-%m-%d")), \((now - .) / 86400 | floor) days before this bump"' ||
      true)
    echo "**Patch lag:** Chrome $v ${lag:-has no stable release on $platform in the version history}."
  done
  echo
  echo "## Verification"
  echo
  echo "The scheduled Action extracted the files the patches touch from each version's source tarball and dry-ran ungoogled-chromium's, the platform's and our series on them."
  echo
  if [ ${#conflicts[@]} -eq 0 ] && [ ${#problems[@]} -eq 0 ]; then
    echo "**Applies cleanly.**"
  else
    for c in ${conflicts[@]+"${conflicts[@]}"}; do echo "- Conflicts: \`$c\`"; done
    for p in ${problems[@]+"${problems[@]}"}; do echo "- $p"; done
    echo
    echo "A lane rebases the patches before the builds, as for a new major."
  fi
  echo
  echo "Not yet run: the incremental Dev and Release builds (\`engine/setup.sh update\`, then \`engine/build.sh\`), the full check, and a \`good/…\` tag. The Action opens this with \`GITHUB_TOKEN\`, so no workflow ran on it; the integrator's builds and check are the gate."
} >"$work/pr-body"
echo "point-release: $(cat "$work/pr-title")" >&2
