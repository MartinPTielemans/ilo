#!/bin/sh
# Fetches the app's dependencies into .deps/ (gitignored): Zed at a pinned revision, with our
# GPUI patches applied, and links our engine, Chromium.app with the core in its framework: the one
# engine/build.sh built, or else the prebuilt one engine/fetch.sh downloads. `setup.sh dev` links
# the dev build's, `setup.sh prebuilt` the prebuilt one even beside a local build, and `setup.sh ui`
# none, for the UI alone: the tests, the preview and the browser checked, not linked (app-mac.yml).
#
# Off CI, Zed's source and the pinned archives come from a cache shared by every checkout,
# ~/.cache/ilo-setup or $ILO_SETUP_CACHE, and the patched GPUI and accessibility crates are one
# read-only tree per pin and patch set there, linked from .deps/ (#3917). A checkout whose
# patches/ has changes not yet committed, or set up with ILO_SETUP_LOCAL=1 to edit GPUI and make
# its patch again, gets a checkout of its own instead, as does one already holding one, CI and
# Windows' Git Bash, which has no links. Nothing in the cache is ever deleted here.
set -e
cd "$(dirname "$0")"
ZED_REV=b36b283021c48f1712714b8a0e2ad6ffb578920b
ZED_URL=https://github.com/zed-industries/zed
# One patch a crate: gpui_macos's, gpui_windows's and gpui_linux's embed GPUI in Chromium's window
# and loop, gpui's scales what's painted, lets the tests stand in for an assistive app, names a
# window's root node for one apart from the window's title, adds the Edit menu's Paste and Match
# Style, and caps a window's frame rate only while the app isn't the active one, gpui_wgpu's draws
# with Vulkan alone on Linux, builds each pipeline once for all windows, the first time one draws
# with it, resizes a canvas under WebGL without waiting on the GPU, which WebGL can't, and lets a
# browser's WebGPU canvas be see-through. gpui_apple's, like the other renderers', starts a
# window's atlases small, and it and gpui_macos's say on stderr, a line at most every few seconds,
# when a frame doesn't reach the screen (#3270).
PATCHES="gpui-macos-embedded.patch gpui-windows-embedded.patch gpui-linux-embedded.patch \
  gpui-scale.patch gpui-wgpu.patch gpui-apple-atlas.patch"
mkdir -p .deps
# Git for Windows has sha256sum but no shasum, which the Mac has alone.
sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }

cache=${ILO_SETUP_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/ilo-setup}
[ -z "${GITHUB_ACTIONS:-}" ] || cache=
case $(uname -s) in MINGW*) cache= ;; esac
shared=$cache
[ -z "${ILO_SETUP_LOCAL:-}" ] || shared=
changed=$(git status --porcelain -- patches 2>/dev/null || echo unknown)
if [ -n "$shared" ] && [ -n "$changed" ]; then
  echo "setup: patches/ has changes not yet committed, so GPUI is a checkout of its own." >&2
  shared=
fi
if [ -n "$shared" ] && [ -d .deps/zed ] && [ ! -L .deps/zed ]; then
  echo "setup: .deps/zed is a checkout of its own, kept; remove it to share the cache's." >&2
  shared=
fi

# One setup at a time changes the cache; the others wait. The lock is a link naming its holder's
# process, made in one step, so a setup stopped at any point leaves none of its own. One left by a
# setup that was killed outright is named, not taken, as only the user knows no setup still runs.
locked=
lock() {
  [ -z "$locked" ] || return 0
  mkdir -p "$cache"
  trap unlock EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM HUP
  waited=0
  until ln -s "$$" "$cache/lock" 2>/dev/null; do
    holder=$(readlink "$cache/lock" 2>/dev/null || true)
    if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then
      echo "setup: process $holder, which has ended, holds $cache/lock." >&2
      echo "setup: remove it if no setup is running." >&2
      exit 1
    fi
    [ "$waited" -gt 0 ] ||
      echo "setup: waiting for the setup${holder:+ in process $holder} using $cache" >&2
    waited=$((waited + 1))
    [ "$waited" -lt 1800 ] || { echo "setup: $cache/lock was held for 30 minutes" >&2; exit 1; }
    sleep 1
  done
  locked=1
}
unlock() {
  if [ -n "$cache" ] && [ "$(readlink "$cache/lock" 2>/dev/null)" = "$$" ]; then
    rm -f "$cache/lock"
  fi
  locked=
}
# The archive $1 from the URL $2, which has the SHA-256 $3: downloaded once, into the cache if
# there is one.
archive() {
  echo "$3  $1" | sha256 -c --status 2>/dev/null && return
  [ -z "$cache" ] || lock
  if ! echo "$3  $1" | sha256 -c --status 2>/dev/null; then
    curl -sSfL --retry 5 "$2" -o "$1.part"
    echo "$3  $1.part" | sha256 -c --status
    mv "$1.part" "$1"
  fi
}
archives=.deps
if [ -n "$cache" ]; then
  archives=$cache/archives
  mkdir -p "$archives"
fi
# The trees built in the cache aren't repositories, so git apply patches files there as patch would,
# even when a repository holds the cache, as one holding a home directory would.
apply() {
  (cd "$1" && shift && GIT_CEILING_DIRECTORIES=$cache git apply "$@")
}
# A tree built in $1.part becomes the read-only $1 at once, so no setup ever sees half of one.
seal() {
  chmod -R a-w "$1.part"
  mv "$1.part" "$1"
}
# Removes what a setup killed while building $1 left, before building it again.
unbuilt() {
  if [ -e "$1.part" ]; then
    chmod -R u+w "$1.part"
    rm -rf "$1.part"
  fi
}
# How setup builds a tree in the cache, numbered and in both trees' keys: raised whenever what goes
# into a tree changes, such as a name added below, so no checkout links a tree built the old way.
RECIPE=1
# The instructions downloaded code brings for coding agents, under $1. Zed's tell agents to edit
# its README: they're written for that project's contributors and don't apply to work here.
unagent() {
  find "$1" \( -name AGENTS.md -o -name CLAUDE.md -o -name GEMINI.md -o -name .rules \
    -o -name .cursorrules -o -name .windsurfrules -o -name .clinerules -o -name copilot-instructions.md \
    -o -name .agents -o -name .claude -o -name .codex -o -name .cursor -o -name .factory \
    -o -name .gemini -o -name .junie \) -prune -exec rm -rf {} +
}
# The cache's copy of Zed's repository holds each pinned revision, fetched once and tagged, as a
# clone takes only what a ref names.
pin=refs/tags/pin-$ZED_REV
mirror() {
  lock
  [ -d "$cache/zed.git" ] || git init -q --bare "$cache/zed.git"
  git -C "$cache/zed.git" rev-parse -q --verify "$pin" >/dev/null && return
  git -C "$cache/zed.git" cat-file -e "$ZED_REV^{commit}" 2>/dev/null ||
    git -C "$cache/zed.git" fetch -q --depth 1 "$ZED_URL" "$ZED_REV"
  git -C "$cache/zed.git" update-ref "$pin" "$ZED_REV"
}

if [ -n "$shared" ]; then
  # Named by the revision and by the patches, so a branch with other patches gets its own tree.
  key=$({ echo "zed $ZED_REV $RECIPE"; for p in $PATCHES; do cat "patches/$p"; done; } | sha256)
  key=$(echo "$key" | cut -c1-16)
  tree=$cache/zed/$(echo "$ZED_REV" | cut -c1-12)-$key
  [ -d "$tree" ] || mirror
  # Looked for again once locked, as the setup waited on may have built it.
  if [ ! -d "$tree" ]; then
    mkdir -p "$cache/zed"
    unbuilt "$tree"
    mkdir "$tree.part"
    GIT_INDEX_FILE=$cache/zed.index git -C "$cache/zed.git" -c core.autocrlf=false \
      --work-tree="$tree.part" read-tree "$ZED_REV"
    GIT_INDEX_FILE=$cache/zed.index git -C "$cache/zed.git" -c core.autocrlf=false \
      --work-tree="$tree.part" checkout-index -a
    rm -f "$cache/zed.index"
    unagent "$tree.part"
    (set --; for p in $PATCHES; do set -- "$@" "$PWD/patches/$p"; done; apply "$tree.part" "$@")
    seal "$tree"
  fi
  rm -f .deps/zed
  ln -s "$tree" .deps/zed
else
  # Cloned from the cache's copy of the repository when there is one: a copy of its one revision,
  # which needs nothing from the cache after.
  if [ ! -d .deps/zed ] || [ -L .deps/zed ]; then
    rm -f .deps/zed
    if [ -n "$cache" ]; then
      mirror
      git clone -q -c core.autocrlf=false --no-checkout "$cache/zed.git" .deps/zed
    else
      git clone -c core.autocrlf=false -c core.longpaths=true --filter=blob:none --no-checkout \
        "$ZED_URL" .deps/zed
    fi
    git -C .deps/zed checkout -q "$ZED_REV"
  fi
  # Every change in the checkout goes, not only in the crates this checkout patches: one a branch
  # patched that this one doesn't would stay patched and break its build. The files a patch adds go
  # too; `git add -N` them to see them in a regenerated patch.
  git -C .deps/zed reset -q
  git -C .deps/zed checkout -q -- .
  git -C .deps/zed clean -fdq
  # A checkout made for an earlier pin moves to this one, from the cache's copy if it came from it.
  if [ "$(git -C .deps/zed rev-parse HEAD)" != "$ZED_REV" ]; then
    origin=$(git -C .deps/zed config remote.origin.url || true)
    if [ -n "$cache" ] && [ "$origin" = "$cache/zed.git" ]; then
      mirror
      git -C .deps/zed fetch -q --depth 1 origin "$pin"
    else
      git -C .deps/zed fetch -q origin "$ZED_REV"
    fi
    git -C .deps/zed checkout -q "$ZED_REV"
  fi
  # Downloaded code can bring instructions for coding agents (Zed's tell agents to edit its
  # README). They're written for that project's contributors and don't apply to work here. The reset
  # above brings Zed's back, so they go before anything else can fail.
  unagent .deps/zed
  (set --; for p in $PATCHES; do set -- "$@" "$PWD/patches/$p"; done; git -C .deps/zed apply "$@")
fi

# GPUI's accessibility adapters for the Mac and for AT-SPI, at the versions Cargo.lock has, with
# what upstream lacks: VoiceOver's menu action and a folder's open state, and AT-SPI's expanded
# state. In a checkout of its own, each is a repository, so a change to one becomes its patch
# again with `git -C .deps/<crate> diff`.
# The crate $1 at version $2, whose .crate has the SHA-256 $3, in .deps/$1 with patches/$4 applied:
# a link to the cache's tree for them, or a repository of its own.
vendor() {
  crate=$archives/$1-$2.crate
  archive "$crate" "https://static.crates.io/crates/$1/$1-$2.crate" "$3"
  rm -rf ".deps/$1" ".deps/$1-$2"
  if [ -n "$shared" ]; then
    built=$cache/crates/$1-$2-$({ echo "$3 $RECIPE"; cat "patches/$4"; } | sha256 | cut -c1-16)
    # Looked for again once locked, as the setup waited on may have built it.
    [ -d "$built" ] || lock
    if [ ! -d "$built" ]; then
      mkdir -p "$cache/crates"
      unbuilt "$built"
      mkdir "$built.part"
      tar xzf "$crate" -C "$built.part" --strip-components 1
      apply "$built.part" "$PWD/patches/$4"
      seal "$built"
    fi
    ln -s "$built" ".deps/$1"
    return
  fi
  tar xzf "$crate" -C .deps
  mv ".deps/$1-$2" ".deps/$1"
  git -C ".deps/$1" init -q
  git -C ".deps/$1" config core.autocrlf false
  git -C ".deps/$1" add -A
  git -C ".deps/$1" -c user.name=setup -c user.email=setup@localhost commit -s -qm upstream
  git -C ".deps/$1" apply "$PWD/patches/$4"
}
vendor accesskit_macos 0.26.3 ce02dc63b43f0c9296af9ac946312a2dc8814427d7a64d2d600971dac55b6076 \
  accesskit-macos-actions.patch
vendor accesskit_atspi_common 0.19.1 \
  023da0e5097f46df7092d5280b02efb9bbf8d93298daeced42652463e357d636 accesskit-atspi-expanded.patch

# Sparkle, the Mac's updater: its headers for platform/mac/updates.mm, the framework a release
# build carries, and generate_keys and generate_appcast for tools/appcast.sh.
if [ "$(uname -s)" = Darwin ]; then
  SPARKLE=2.10.1
  SPARKLE_SHA256=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
  sparkle=$archives/Sparkle-$SPARKLE.tar.xz
  archive "$sparkle" \
    "https://github.com/sparkle-project/Sparkle/releases/download/$SPARKLE/Sparkle-$SPARKLE.tar.xz" \
    "$SPARKLE_SHA256"
  rm -rf .deps/Sparkle
  mkdir .deps/Sparkle
  tar xJf "$sparkle" -C .deps/Sparkle
fi

unlock

# The engine: the Mac's, linked here; on Linux and Windows, the prebuilt one the engine's CI
# publishes, when there is one for this checkout. Without one there is the UI alone: the tests and
# the preview (AGENTS.md, Dev).
case $(uname -s) in
  Darwin)
    engines=$(cd .. && pwd)/engine
    case ${1:-} in
      "")
        engine=$engines/.deps/src/out/Release/Chromium.app
        [ -d "$engine" ] || engine=$("$engines/fetch.sh")
        ;;
      dev) engine=$engines/.deps/src/out/Dev/Chromium.app ;;
      prebuilt) engine=$("$engines/fetch.sh") ;;
      ui) ;;
      *) echo "usage: setup.sh [dev|prebuilt|ui]" >&2 && exit 2 ;;
    esac
    if [ "${1:-}" = ui ]; then
      rm -f .deps/Chromium.app
    else
      [ -d "$engine" ] || { echo "No engine at $engine. Build it with engine/build.sh ${1:-}." >&2; exit 1; }
      ln -sfn "$engine" .deps/Chromium.app
    fi
    ;;
  Linux)
    engines=$(cd .. && pwd)/engine
    if engine=$("$engines/fetch.sh"); then
      ln -sfn "$engine" .deps/engine
    else
      echo "No Linux engine for this checkout yet, so only the tests and the preview build." >&2
    fi
    ;;
  MINGW*)
    # Git Bash's ln copies a directory without being asked to, so the copy is made here, once per
    # engine, with the engine it came from noted in it.
    engines=$(cd .. && pwd)/engine
    if engine=$("$engines/fetch.sh"); then
      if [ "$(cat .deps/engine/.from 2>/dev/null)" != "$engine" ]; then
        rm -rf .deps/engine
        cp -r "$engine" .deps/engine
        echo "$engine" >.deps/engine/.from
      fi
    else
      echo "No Windows engine for this checkout yet, so only the tests and the preview build." >&2
    fi
    ;;
esac
