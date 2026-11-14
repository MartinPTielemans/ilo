#!/bin/sh
# Fetches the app's dependencies into .deps/ (gitignored): Zed at a pinned revision, with our
# GPUI patches applied, and links our engine, Chromium.app with the core in its framework: the one
# engine/build.sh built, or else the prebuilt one engine/fetch.sh downloads. `setup.sh dev` links
# the dev build's, `setup.sh prebuilt` the prebuilt one even beside a local build, and `setup.sh ui`
# none, for the UI alone: the tests, the preview and the browser checked, not linked (app-mac.yml).
set -e
cd "$(dirname "$0")"
ZED_REV=b36b283021c48f1712714b8a0e2ad6ffb578920b
mkdir -p .deps

if [ ! -d .deps/zed ]; then
  git clone -c core.autocrlf=false -c core.longpaths=true --filter=blob:none --no-checkout \
    https://github.com/zed-industries/zed .deps/zed
  git -C .deps/zed checkout -q "$ZED_REV"
fi
# Every change in the checkout goes, not only in the crates this checkout patches: one a branch
# patched that this one doesn't would stay patched and break its build. The files a patch adds go
# too; `git add -N` them to see them in a regenerated patch.
git -C .deps/zed reset -q
git -C .deps/zed checkout -q -- .
git -C .deps/zed clean -fdq
# A checkout made for an earlier pin moves to this one.
if [ "$(git -C .deps/zed rev-parse HEAD)" != "$ZED_REV" ]; then
  git -C .deps/zed fetch -q origin "$ZED_REV"
  git -C .deps/zed checkout -q "$ZED_REV"
fi
# Downloaded code can bring instructions for coding agents (Zed's tell agents to edit its
# README). They're written for that project's contributors and don't apply to work here. The reset
# above brings Zed's back, so they go before anything else can fail.
find .deps/zed \( -name AGENTS.md -o -name CLAUDE.md -o -name GEMINI.md -o -name .rules \
  -o -name .cursorrules -o -name .windsurfrules -o -name .clinerules -o -name copilot-instructions.md \
  -o -name .agents -o -name .claude -o -name .codex -o -name .cursor -o -name .factory \
  -o -name .gemini -o -name .junie \) -prune -exec rm -rf {} +
# One patch a crate: gpui_macos's, gpui_windows's and gpui_linux's embed GPUI in Chromium's window
# and loop, gpui's scales what's painted, lets the tests stand in for an assistive app, adds the
# Edit menu's Paste and Match Style, caps a window's frame rate only while the app isn't the
# active one and leaves the site's demo only the image decoders and regex tables it uses,
# gpui_wgpu's draws with Vulkan alone on Linux, builds each pipeline once for all windows, the
# first time one draws with it, resizes a canvas under WebGL without waiting on the GPU, which
# WebGL can't, and lets a browser's WebGPU canvas be see-through, and gpui_web's draws the site's
# demo in the site's canvas, see-through where it draws nothing, leaving the page its keys and
# scrolling. gpui_apple's, like the other renderers', starts a window's atlases small.
git -C .deps/zed apply "$PWD/patches/gpui-macos-embedded.patch" "$PWD/patches/gpui-windows-embedded.patch" \
  "$PWD/patches/gpui-linux-embedded.patch" "$PWD/patches/gpui-scale.patch" \
  "$PWD/patches/gpui-wgpu.patch" "$PWD/patches/gpui-web-page.patch" \
  "$PWD/patches/gpui-apple-atlas.patch"

# GPUI's accessibility adapters for the Mac and for AT-SPI, at the versions Cargo.lock has, with
# what upstream lacks: VoiceOver's menu action and a folder's open state, and AT-SPI's expanded
# state. Each a repository of its own, so a change to one becomes its patch again with
# `git -C .deps/<crate> diff`.
# Git for Windows has sha256sum but no shasum, which the Mac has alone.
sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi; }
# The crate $1 at version $2, whose .crate has the SHA-256 $3, in .deps/$1 with patches/$4 applied.
vendor() {
  crate=.deps/$1-$2.crate
  if ! echo "$3  $crate" | sha256 -c --status 2>/dev/null; then
    curl -sSfL --retry 5 "https://static.crates.io/crates/$1/$1-$2.crate" -o "$crate"
    echo "$3  $crate" | sha256 -c --status
  fi
  rm -rf ".deps/$1" "${crate%.crate}"
  tar xzf "$crate" -C .deps
  mv "${crate%.crate}" ".deps/$1"
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
  archive=.deps/Sparkle-$SPARKLE.tar.xz
  if ! echo "$SPARKLE_SHA256  $archive" | sha256 -c --status 2>/dev/null; then
    curl -sSfL --retry 5 "https://github.com/sparkle-project/Sparkle/releases/download/$SPARKLE/Sparkle-$SPARKLE.tar.xz" -o "$archive"
    echo "$SPARKLE_SHA256  $archive" | sha256 -c --status
  fi
  rm -rf .deps/Sparkle
  mkdir .deps/Sparkle
  tar xJf "$archive" -C .deps/Sparkle
fi

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
