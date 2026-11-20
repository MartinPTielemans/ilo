#!/bin/bash
# What a new checkout of the iPhone app needs before xcodebuild, which the tree doesn't hold: the
# blocking rules Blocking/build.sh makes, and ilo.xcodeproj, which XcodeGen writes from project.yml.
# Xcode Cloud runs a script of this name, from a folder of this name beside the project, after it
# clones the repository and before it resolves packages, once for each action of a workflow
# (Apple's "Writing custom build scripts"). GitHub's job runs it too
# (.github/workflows/mobile-ios.yml), so every pull request proves it while Xcode Cloud isn't
# connected: a tool or a made file the build comes to need goes here, for both.
# It reads no secret and sends nothing: Xcode Cloud signs and uploads by itself.
set -euo pipefail

# A test action's second phase runs the tests its first built, on a machine with this folder and
# none of the source, so there's nothing to make there.
if [ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ]; then
  exit 0
fi
cd "$(dirname "$0")/.."

# Xcode Cloud's machines have Homebrew, and neither XcodeGen nor Rust, which Blocking/build.sh
# builds its converter with. GitHub's images have Rust. Without sudo, as Xcode Cloud gives none.
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1
command -v xcodegen > /dev/null || brew install xcodegen
command -v cargo > /dev/null || brew install rust

# The rules a cache brought back, on GitHub, are the pins' own, so they stay.
[ -d Blocking/Rules ] || Blocking/build.sh

# Apple asks for a project that's in the repository, and says one a tool writes during the build
# may fail (its "Setting up your project to use Xcode Cloud"). So a project that is there, should
# it come to be committed, is the one that's built, and only a checkout without one writes it.
[ -d ilo.xcodeproj ] || xcodegen generate
