# What setup.sh and linux/prepare.sh both do to a prepared Chromium tree. Sourced; each function
# takes the tree's path.

engine=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# Chromium's instructions and skills for its own contributors' agents. Ours are the repository's
# AGENTS.md. Some are symlinks to an AGENTS.md beside them. Chrome's own `skills` folders are a
# browser feature, not agent skills, so they stay.
remove_agent_instructions() {
  local src=$1
  find "$src" \( -name AGENTS.md -o -name CLAUDE.md -o -name GEMINI.md -o -name SKILL.md \
    -o -name .cursorrules -o -name .rules -o -name copilot-instructions.md \) \
    \( -type f -o -type l \) -delete
  find "$src" \( -name .claude -o -name .gemini -o -name .cursor -o -name .agents \) -type d -prune \
    -exec rm -rf {} +
  rm -rf "$src/agents"
}

# ilo's icon in place of Chromium's logo in a Windows build (#1350): chrome.exe's first icon, which
# Explorer, Task Manager and its shortcuts show, chrome.dll's IDR_MAINFRAME, which the browser
# windows take for the taskbar's thumbnails, Alt+Tab and the title bar, and the exe's HTML and PDF
# document icons, which the installer gives ilo's icon too. Copied only when it differs, so a kept
# tree doesn't relink chrome.dll for nothing. icons.py reads them back from a build.
windows_icons() {
  local theme=$1/chrome/app/theme/chromium/win name
  local ico=$engine/../app/platform/windows/package/Assets/ilo.ico
  for name in chromium chromium_doc chromium_pdf; do
    cmp -s "$ico" "$theme/$name.ico" || cp "$ico" "$theme/$name.ico"
  done
}

# The same in a Linux build, whose logos are PNGs in its .pak files: IDR_PRODUCT_LOGO_128 is the
# windows' icon, which X11 and Wayland get, and the rest are logos Chromium's own UI, notifications
# and shortcuts show. Each is the size Chromium's file is, from ilo.ico's PNGs, its 128 the 256
# halved, as the .ico has none (icons.py png). icons.py reads them back from a build.
linux_icons() {
  local theme=$1/chrome/app/theme size file png
  local ico=$engine/../app/platform/windows/package/Assets/ilo.ico
  png=$(mktemp)
  while read -r size file; do
    python3 -I "$engine/icons.py" png "$ico" "$size" "$png"
    cmp -s "$png" "$theme/$file" || cp "$png" "$theme/$file"
  done <<'LOGOS'
16 chromium/product_logo_16.png
24 chromium/product_logo_24.png
64 chromium/product_logo_64.png
128 chromium/product_logo_128.png
64 chromium/linux/product_logo_64.png
128 chromium/linux/product_logo_128.png
256 chromium/linux/product_logo_256.png
16 default_100_percent/chromium/linux/product_logo_16.png
32 default_100_percent/chromium/product_logo_32.png
64 default_200_percent/chromium/product_logo_32.png
LOGOS
  rm -f "$png"
}

# Checks that a platform repository's ungoogled-chromium submodule builds the pinned tag, or a
# later commit after it (ungoogled-chromium-macos 156.0.8078.47 builds one that fixes the tag's
# patches), for the pinned Chromium version. A platform whose tag names another version lags the
# Mac's point release, and is checked against its tag alone. Takes the submodule's path and
# "<repository> <tag>".
check_ungoogled() {
  local uc=$1 platform_tag=$2 version uc_tag pinned tag_version
  version=$(git config -f "$engine/pins.ini" chromium.version)
  tag_version=${platform_tag##* }
  tag_version=${tag_version%%-*}
  if [ "$tag_version" != "$version" ]; then
    if [ "$(cat "$uc/chromium_version.txt")" != "$tag_version" ]; then
      echo "$platform_tag builds Chromium $(cat "$uc/chromium_version.txt"), not $tag_version" >&2
      exit 1
    fi
    return
  fi
  uc_tag=$(git config -f "$engine/pins.ini" ungoogled-chromium.tag)
  pinned=$(git ls-remote https://github.com/ungoogled-software/ungoogled-chromium.git \
    "refs/tags/$uc_tag^{}" | cut -f1)
  # The submodule is a shallow clone.
  git -C "$uc" fetch -q --deepen=20
  if ! git -C "$uc" merge-base --is-ancestor "$pinned" HEAD 2>/dev/null; then
    echo "$platform_tag doesn't build ungoogled-chromium $uc_tag" >&2
    exit 1
  fi
  if [ "$(cat "$uc/chromium_version.txt")" != "$version" ]; then
    echo "ungoogled-chromium $uc_tag is for Chromium $(cat "$uc/chromium_version.txt"), not $version" >&2
    exit 1
  fi
}

# Links the core into the tree as //browser_core, so the next build has every edit to it, and
# applies our series. A copy of each patch applied is kept in the tree, in .browser_core-series, so
# a later run on a kept tree takes out, newest first, the patches from the first one that changed
# or left the series, and applies the series again from there. A patch already in the tree is
# skipped, so this is safe to run again; one that neither applies cleanly nor is in the tree has
# drifted from it. In the Mac's bash 3.2, which has no mapfile and calls an empty array unbound.
apply_series() {
  local src=$1 kept=$1/.browser_core-series name patch first=0 i
  local -a names=() applied=()
  ln -sfn "$(dirname "$engine")/core" "$src/browser_core"
  while read -r name; do names+=("$name"); done < <(grep -v -e '^#' -e '^$' "$engine/series")
  if [ -f "$kept/series" ]; then
    while read -r name; do applied+=("$name"); done <"$kept/series"
  elif ! series_fits "$src" "${names[@]}"; then
    # A tree from before the copies were kept, with older versions of some patches in it.
    unapply_history "$src" "${names[@]}"
  fi
  while [ "$first" -lt "${#applied[@]}" ] && [ "${applied[first]}" = "${names[first]:-}" ] &&
    cmp -s "$kept/${applied[first]}" "$engine/patches/${applied[first]}"; do
    first=$((first + 1))
  done
  for ((i = ${#applied[@]} - 1; i >= first; i--)); do
    patch -d "$src" -p1 -F0 -R -s <"$kept/${applied[i]}" ||
      { echo "${applied[i]} won't come out of the tree" >&2 && exit 1; }
    echo "took out ${applied[i]}" >&2
  done
  mkdir -p "$kept"
  : >"$kept/series"
  for ((i = 0; i < first; i++)); do echo "${applied[i]}" >>"$kept/series"; done
  for ((i = first; i < ${#names[@]}; i++)); do
    name=${names[i]}
    patch=$engine/patches/$name
    if patch -d "$src" -p1 -F0 -N --dry-run -s <"$patch" >/dev/null 2>&1; then
      patch -d "$src" -p1 -F0 -N -s <"$patch"
      echo "applied $name" >&2
    elif ! patch -d "$src" -p1 -F0 -R --dry-run -s <"$patch" >/dev/null 2>&1; then
      echo "$name has drifted: it doesn't apply, and it isn't in the tree" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$kept/$name")"
    cp "$patch" "$kept/$name"
    echo "$name" >>"$kept/series"
  done
}

# Whether each of the series, in order, applies to the tree or is in it already, tried on a copy
# of the files they change.
series_fits() {
  local src=$1 name file try fits=0
  shift
  try=$(mktemp -d)
  for name in "$@"; do
    for file in $(sed -n 's|^+++ b/\([^[:space:]]*\).*|\1|p' "$engine/patches/$name"); do
      [ ! -e "$src/$file" ] || [ -e "$try/$file" ] ||
        { mkdir -p "$(dirname "$try/$file")" && cp "$src/$file" "$try/$file"; }
    done
  done
  for name in "$@"; do
    patch -d "$try" -p1 -F0 -N -s <"$engine/patches/$name" >/dev/null 2>&1 ||
      patch -d "$try" -p1 -F0 -R --dry-run -s <"$engine/patches/$name" >/dev/null 2>&1 ||
      { fits=1 && break; }
  done
  rm -rf "$try"
  return "$fits"
}

# Takes the series out of a tree that kept no copies, newest first, each in whichever of its
# versions in the repository's history is in the tree. Needs that history: the engine's workflow
# checks out all of it.
unapply_history() {
  local src=$1 name rev i
  shift
  local -a names=("$@")
  for ((i = ${#names[@]} - 1; i >= 0; i--)); do
    name=${names[i]}
    for rev in $(git -C "$engine" log --format=%H -- "patches/$name"); do
      if git -C "$engine" show "$rev:./patches/$name" |
        patch -d "$src" -p1 -F0 -R --dry-run -s >/dev/null 2>&1; then
        git -C "$engine" show "$rev:./patches/$name" | patch -d "$src" -p1 -F0 -R -s
        echo "took out $name as of ${rev:0:12}" >&2
        break
      fi
    done
  done
}
