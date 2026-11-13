#!/bin/sh
# Builds build/Browser.app: the GPUI executable in a copy of our engine's Chromium.app, which holds
# Chrome's framework with the core in it, and Chromium's helper apps. `build.sh prod` builds the
# copy for daily use instead, which tools/install.sh installs: the user's profile, the real
# Keychain and the product's bundle id, on the Release engine. `build.sh updates` makes the updates
# and nightly runs' copy of a built app, with an updater and a feed on this computer (below).
# APP_TRACK=nightly builds a nightly, which About names so and whose switch starts on.
set -e
cd "$(dirname "$0")"
# Which core changes the linked engine wasn't built with, for check/lib.sh's engine_has in a copy
# of the tree with no repository, as a check VM's (#2554): compared here, where git, the engine's
# tags and its tree are. A build that can't compare leaves none, so no stale answer outlives it.
engine_changes() {
  mkdir -p build
  if changes=$(../tools/engine-changes.sh 2> /dev/null); then
    printf '%s\n' "$changes" > build/engine-changes
  else
    rm -f build/engine-changes
  fi
}
# On Linux, build/Browser: a copy of the engine, chrome and what it runs with, and libBrowser.so
# beside it, which chrome loads (linux/Cargo.toml). Hard links, as chrome finds its files and the
# app's beside its own path, which a symbolic link would leave in the engine's folder.
if [ "$(uname -s)" = Linux ]; then
  [ -e .deps/engine/chrome ] || ./setup.sh
  [ -e .deps/engine/chrome ] || { echo "No Linux engine in .deps/engine." >&2; exit 1; }
  engine_changes
  features=
  [ "${1:-}" != prod ] || features="--features prod"
  # In the browser crate's target, which has built the same dependencies for the tests.
  # shellcheck disable=SC2086 # no features, or one flag and its value
  cargo build --release --manifest-path linux/Cargo.toml --target-dir target $features
  # The bridge an agent's MCP client runs, which finds the browser's socket (src/agents/bridge.rs).
  cargo build --release --bin mcp-bridge
  rm -rf build/Browser
  mkdir -p build
  cp -al "$(cd .deps/engine && pwd -P)" build/Browser
  cp target/release/libBrowser.so target/release/mcp-bridge build/Browser/
  # uBlock Origin goes beside chrome, Chromium's assets folder off the Mac, where the core loads it.
  cp -R "$(./fetch-ublock.sh)" build/Browser/ublock
  # shellcheck disable=SC2086
  ../tools/licences.sh build/Browser linux/Cargo.toml "$(rustc -vV | sed -n 's/^host: //p')" $features
  exit 0
fi
# On Windows, under Git Bash, the same: build/Browser, a copy of the engine, with Browser.dll beside
# chrome.exe, which loads it (windows/Cargo.toml).
case "$(uname -s)" in
  MINGW* | MSYS*)
    [ -e .deps/engine/chrome.exe ] || ./setup.sh
    [ -e .deps/engine/chrome.exe ] || { echo "No Windows engine in .deps/engine." >&2; exit 1; }
    engine_changes
    features=
    [ "${1:-}" != prod ] || features="--features prod"
    # The nightly run's copy of build/Browser, build/Updates beside it, whose updater checks
    # UPDATE_FEED, and UPDATE_FEED_NIGHTLY while it takes nightly builds, feeds on this computer,
    # for updates signed with UPDATE_PUBLIC_KEY, all the run's own (check/nightly.sh). build.rs
    # gives a build an updater only for such a feed, or for the product's in a prod build.
    if [ "${1:-}" = updates ]; then
      case ${UPDATE_FEED:-} in
        http://127.0.0.1:*/*) ;;
        *)
          echo "build.sh updates takes UPDATE_FEED here: http://127.0.0.1:<port>/…" >&2
          exit 1
          ;;
      esac
      [ -n "${UPDATE_PUBLIC_KEY:-}" ] || {
        echo "build.sh updates takes UPDATE_PUBLIC_KEY" >&2
        exit 1
      }
      [ -d build/Browser ] || { echo "No build/Browser. Build it with ./build.sh" >&2; exit 1; }
      cargo build --release --manifest-path windows/Cargo.toml --target-dir target
      rm -rf build/Updates
      cp -al build/Browser build/Updates
      # Its own, not a link to build/Browser's.
      rm build/Updates/Browser.dll
      cp target/release/Browser.dll build/Updates/
      echo build/Updates
      exit 0
    fi
    # shellcheck disable=SC2086 # no features, or one flag and its value
    cargo build --release --manifest-path windows/Cargo.toml --target-dir target $features
    # The bridge an agent's MCP client runs, which finds the browser's pipe (src/agents/bridge.rs).
    # shellcheck disable=SC2086
    cargo build --release --bin mcp-bridge $features
    rm -rf build/Browser
    mkdir -p build
    cp -al .deps/engine build/Browser
    cp target/release/Browser.dll target/release/mcp-bridge.exe build/Browser/
    cp -R "$(./fetch-ublock.sh)" build/Browser/ublock
    # shellcheck disable=SC2086
    ../tools/licences.sh build/Browser windows/Cargo.toml "$(rustc -vV | sed -n 's/^host: //p')" $features
    exit 0
    ;;
esac
[ -e .deps/Chromium.app ] || ./setup.sh
engine_changes
# setup.sh vendors it since the app patched it; a checkout set up before that hasn't got it.
[ -d .deps/accesskit_macos ] || {
  echo "No .deps/accesskit_macos. Run ./setup.sh again, with dev for the dev engine." >&2
  exit 1
}
# Sparkle in the app at $1, checking the feed $2 every 8 hours for updates signed with the key $3,
# or the nightly feed $4, if there's one, while the user takes nightly builds (src/updates.rs), and
# installing what it downloads when the app quits (docs/ux-spec.md › Updates). Its helpers for
# sandboxed apps stay out, as this one isn't.
add_sparkle() {
  ditto .deps/Sparkle/Sparkle.framework "$1/Contents/Frameworks/Sparkle.framework"
  rm -rf "$1/Contents/Frameworks/Sparkle.framework/XPCServices" \
    "$1/Contents/Frameworks/Sparkle.framework/Versions/B/XPCServices"
  plutil -replace SUFeedURL -string "$2" "$1/Contents/Info.plist"
  plutil -replace SUPublicEDKey -string "$3" "$1/Contents/Info.plist"
  plutil -replace SUEnableAutomaticChecks -bool YES "$1/Contents/Info.plist"
  plutil -replace SUAutomaticallyUpdate -bool YES "$1/Contents/Info.plist"
  plutil -replace SUScheduledCheckInterval -integer 28800 "$1/Contents/Info.plist"
  [ -z "${4:-}" ] || plutil -replace NightlyFeedURL -string "$4" "$1/Contents/Info.plist"
}
# A nightly's Info.plist says so, for About.
mark_nightly() {
  [ "${APP_TRACK:-}" != nightly ] || plutil -replace NightlyBuild -bool YES "$1/Contents/Info.plist"
}
# Fails unless there's Sparkle to add and UPDATE_PUBLIC_KEY is an EdDSA public key.
check_update_key() {
  [ -d .deps/Sparkle/Sparkle.framework ] || { echo "No .deps/Sparkle. Run ./setup.sh again." >&2; exit 1; }
  python3 -I -c 'import base64, sys; sys.exit(len(base64.b64decode(sys.argv[1], validate=True)) != 32)' \
    "$UPDATE_PUBLIC_KEY" 2>/dev/null || {
    echo "The update key isn't an EdDSA public key: generate_keys -p prints one." >&2
    exit 1
  }
}
# The updates run's copy of build/Browser.app, build/Updates.app beside it, where the dev engine's
# libraries are: Sparkle checks UPDATE_FEED, a feed on this Mac, for updates signed with
# UPDATE_PUBLIC_KEY, both the run's own (check/updates.sh), and UPDATE_FEED_NIGHTLY, if it's set,
# while it takes nightly builds (check/nightly.sh). Only the prod build carries the product's feeds
# and key, and only it reaches users. Its own id keeps Sparkle's settings apart from every other
# build's, and a quit sent to it from launching another.
if [ "${1:-}" = updates ]; then
  for feed in "${UPDATE_FEED:-}" "${UPDATE_FEED_NIGHTLY-http://127.0.0.1:0/}"; do
    case $feed in
      http://127.0.0.1:*/*) ;;
      *)
        echo "build.sh updates takes feeds on this Mac: http://127.0.0.1:<port>/…" >&2
        exit 1
        ;;
    esac
  done
  [ -d build/Browser.app ] || { echo "No build/Browser.app. Build it with ./build.sh" >&2; exit 1; }
  check_update_key
  rm -rf build/Updates.app
  cp -cR build/Browser.app build/Updates.app
  add_sparkle build/Updates.app "$UPDATE_FEED" "$UPDATE_PUBLIC_KEY" "${UPDATE_FEED_NIGHTLY:-}"
  mark_nightly build/Updates.app
  plutil -replace CFBundleIdentifier -string dev.browser.app.updates \
    build/Updates.app/Contents/Info.plist
  echo build/Updates.app
  exit 0
fi
engine_out=$(dirname "$(cd .deps/Chromium.app && pwd -P)")
id=dev.browser.app
features=
if [ "${1:-}" = prod ]; then
  # The dev engine stops the browser on a failed DCHECK, where Release goes on.
  if [ -e "$engine_out/libchrome_dll.dylib" ]; then
    echo "A prod build needs the Release engine: ./setup.sh" >&2
    exit 1
  fi
  id=$(cat branding/id)
  features="--features prod"
  # A release build updates itself with Sparkle, which setup.sh fetches, when it has the key the
  # updates are signed with: the public half of the owner's (tools/appcast.sh), in the branding a
  # fork replaces. A fork that empties branding/update-key builds a copy with no updater.
  if [ -z "${UPDATE_PUBLIC_KEY:-}" ] && [ -f branding/update-key ]; then
    UPDATE_PUBLIC_KEY=$(tr -d '[:space:]' < branding/update-key)
  fi
  [ -z "${UPDATE_PUBLIC_KEY:-}" ] || check_update_key
fi
# The version the user sees is the crate's. Sparkle orders updates by the build number, the count
# of commits, which only grows on main; a shallow clone's count would be wrong.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
  [ -z "$features" ] || { echo "A prod build needs the whole history: git fetch --unshallow" >&2; exit 1; }
fi
build=$(git rev-list --count HEAD)
# The build number, for Settings › About to show beside the version.
APP_BUILD=$build cargo build --release --bin Browser --bin mcp-bridge $features

app=build/Browser.app
rm -rf "$app"
mkdir -p build
# A clone, so it's instant, and rebuilding the engine leaves a built app as it was.
cp -cR "$(cd .deps/Chromium.app && pwd -P)" "$app"
rm "$app/Contents/MacOS/Chromium"
# The bridge goes beside it, where Settings › Agents points an agent's MCP client.
cp target/release/Browser target/release/mcp-bridge "$app/Contents/MacOS/"
# uBlock Origin goes in the framework's resources, where the core loads it into every profile.
framework=$(echo "$app"/Contents/Frameworks/*Framework.framework)
cp -R "$(./fetch-ublock.sh)" "$framework/Resources/ublock"
# The dev engine's framework and helpers load its component libraries from beside the app. These
# are links, so a rebuilt dev engine's core reaches the built app at once.
rm -f build/*.dylib
if [ -e "$engine_out/libchrome_dll.dylib" ]; then
  ln -s "$engine_out"/*.dylib build/
fi

plist=$app/Contents/Info.plist
plutil -replace CFBundleExecutable -string Browser "$plist"
plutil -replace CFBundleIdentifier -string "$id" "$plist"
# The name and icon the user sees, from branding/ (icon.sh compiles the icon). Finder, the Dock
# and ⌘-Tab name the app by its file, though, so this one shows as Browser there.
name=$(cat branding/name)
plutil -replace CFBundleName -string "$name" "$plist"
plutil -replace CFBundleDisplayName -string "$name" "$plist"
plutil -replace CFBundleShortVersionString -string "$version" "$plist"
plutil -replace CFBundleVersion -string "$build" "$plist"
# macOS 15 (Sequoia) or later, the owner's call (#678). Below it macOS won't open the app, and
# Sparkle's generate_appcast copies it into the feed's entry, so an older Mac is offered no update.
plutil -replace LSMinimumSystemVersion -string 15.0 "$plist"
# No script reaches Chromium's AppleScript dictionary, which its code answers outside the core: a
# script could read every window's tabs and their URLs, and save a page, view its source or run
# JavaScript in it (#704). Quit and open-location Apple Events have handlers of their own. ilo's
# own takes its place (platform/mac/ilo.sdef), which answers only once Settings › Agents lets
# scripts in, through the UI's own paths.
rm -f "$app/Contents/Resources/scripting.sdef"
cp platform/mac/ilo.sdef "$app/Contents/Resources/"
plutil -replace NSAppleScriptEnabled -bool YES "$plist"
plutil -replace OSAScriptingDefinition -string ilo.sdef "$plist"
rm "$app/Contents/Resources/app.icns"
cp branding/Assets.car "branding/$name.icns" "$app/Contents/Resources/"
plutil -replace CFBundleIconFile -string "$name" "$plist"
plutil -replace CFBundleIconName -string "$name" "$plist"
# The licences: ilo's, NOTICE, and the Rust crates in the binary.
# shellcheck disable=SC2086
../tools/licences.sh "$app/Contents/Resources" Cargo.toml "$(rustc -vV | sed -n 's/^host: //p')" $features
# Chromium's translations of the strings below name Chromium, and macOS shows a translation over
# Info.plist's string. Emptied, they leave the English ones below in every language.
for strings in "$app"/Contents/Resources/*.lproj/InfoPlist.strings; do
  plutil -create binary1 "$strings"
done
# The framework's alert helpers post a site's alert-style notifications, so macOS names them when it
# asks whether the app may notify, and in System Settings › Notifications and on each alert: as the
# engine builds them they're "Chromium", with Chromium's icon and ungoogled-chromium's bundle id,
# which a Chromium on the same Mac shares (#2126). They're the app's, by its name, icon and id, as
# Chrome's are Chrome's; nothing in Chromium looks for the id, which its signing names the same way.
for helper in "$framework"/Helpers/*" Helper ("*"Alerts).app"; do
  [ -d "$helper" ] || continue
  helper_plist=$helper/Contents/Info.plist
  plutil -replace CFBundleIdentifier -string "$id.framework.AlertNotificationService" "$helper_plist"
  plutil -replace CFBundleName -string "$name" "$helper_plist"
  plutil -replace CFBundleDisplayName -string "$name" "$helper_plist"
  for strings in "$helper"/Contents/Resources/*.lproj/InfoPlist.strings; do
    plutil -create binary1 "$strings"
  done
  rm -f "$helper/Contents/Resources/app.icns"
  cp branding/Assets.car "branding/$name.icns" "$helper/Contents/Resources/"
  plutil -replace CFBundleIconFile -string "$name" "$helper_plist"
  plutil -replace CFBundleIconName -string "$name" "$helper_plist"
done
# Each helper is signed again, ad hoc, keeping the entitlements, flags and runtime the engine gave
# it: an alert helper's signature seals the Info.plist rewritten above, and the engine's leaves the
# GPU's Aperitif helper sealed with resources it hasn't got. Then the framework, whose signature
# seals its helpers and its resources, uBlock Origin's among them. tools/install.sh and
# tools/release.sh sign them all again with the app's identity.
for helper in "$framework"/Helpers/*.app; do
  codesign --force --sign - --preserve-metadata=entitlements,flags,runtime "$helper"
done
codesign --force --sign - --preserve-metadata=entitlements,flags,runtime "$framework"
# The prod build opens web links and files, so it can be the default browser: http, https and file
# URLs, and the documents Chrome views, such as .html. Not what's Chromium's own, which the core
# doesn't open: its chromium:// scheme, its extension and shortcut files and the types it exports
# for them, and the sign-in sheets apps ask the default browser for.
python3 -I - "$plist" << 'EOF'
import plistlib, sys
with open(sys.argv[1], "rb") as f:
    info = plistlib.load(f)
info["CFBundleURLTypes"] = [
    kind for kind in info.get("CFBundleURLTypes", [])
    if set(kind["CFBundleURLSchemes"]) <= {"http", "https", "file"}
]
info["CFBundleDocumentTypes"] = [
    kind for kind in info.get("CFBundleDocumentTypes", [])
    if not any(name.startswith("org.chromium.") for name in kind.get("LSItemContentTypes", []))
]
info.pop("UTExportedTypeDeclarations", None)
info.pop("ASWebAuthenticationSessionWebBrowserSupportCapabilities", None)
with open(sys.argv[1], "wb") as f:
    plistlib.dump(info, f)
EOF
# A dev build offers none of it: every lane's build registers with the system too, and none may
# show in its list of browsers or catch the owner's links.
if [ -z "$features" ]; then
  plutil -remove CFBundleURLTypes "$plist"
  plutil -remove CFBundleDocumentTypes "$plist"
fi
# The nightly feed is branding/update-feed-nightly's, where a fork has one.
if [ -n "$features" ] && [ -n "${UPDATE_PUBLIC_KEY:-}" ]; then
  nightly_feed=
  if [ -f branding/update-feed-nightly ]; then
    nightly_feed=$(tr -d '[:space:]' < branding/update-feed-nightly)
  fi
  add_sparkle "$app" "$(cat branding/update-feed)" "$UPDATE_PUBLIC_KEY" "$nightly_feed"
fi
mark_nightly "$app"
# Chromium touches Bluetooth, the camera, the microphone, location and the local network for
# pages and for its own metrics. Without a usage description, macOS kills the app the first time
# it does, instead of asking.
plutil -replace NSBluetoothAlwaysUsageDescription -string "Websites can ask to use Bluetooth devices." "$plist"
plutil -replace NSCameraUsageDescription -string "Websites can ask to use the camera." "$plist"
plutil -replace NSMicrophoneUsageDescription -string "Websites can ask to use the microphone." "$plist"
plutil -replace NSLocationUsageDescription -string "Websites can ask for your location." "$plist"
plutil -replace NSLocationWhenInUseUsageDescription -string "Websites can ask for your location." "$plist"
plutil -replace NSLocalNetworkUsageDescription -string "Websites and casting can reach devices on your network." "$plist"
# A prod build's crash reports name its functions only once the collector has its symbols: the
# framework's and helpers' dSYMs, which the Release engine writes beside Chromium.app, and the app's
# executable, which keeps its symbol table. tools/symbols.sh uploads them with SENTRY_AUTH_TOKEN
# and the rest set (docs/releasing.md). The app is built either way.
if [ -n "$features" ]; then
  set -- "$app/Contents/MacOS/Browser"
  for dsym in "$engine_out"/*.dSYM; do
    [ -e "$dsym" ] && set -- "$@" "$dsym"
  done
  ../tools/symbols.sh "$@" ||
    echo "symbols: none uploaded, though $app is built. Try again with: tools/symbols.sh $*" >&2
fi
echo "$app"
