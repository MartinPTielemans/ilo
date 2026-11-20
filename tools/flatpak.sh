#!/bin/bash
# Builds ilo's Flatpak, app.ilo.browser, from the prod build `app/build.sh prod` makes on the prebuilt
# Linux engine, into app/build/ilo.flatpak, a bundle that fetches its runtime from Flathub. It needs
# flatpak and flatpak-builder, and puts the runtime, SDK and Chromium base in the user's install.
# Try it with `flatpak install --user app/build/ilo.flatpak` and `flatpak run app.ilo.browser`.
# Usage: tools/flatpak.sh
set -euo pipefail
app=$(cd "$(dirname "$0")/../app" && pwd)
[ "$(uname -s)" = Linux ] || { echo "A Flatpak builds on Linux." >&2; exit 1; }
for tool in flatpak flatpak-builder; do
  command -v "$tool" >/dev/null || { echo "Needs flatpak and flatpak-builder: no $tool." >&2; exit 1; }
done

# Planted for #3632's proof, never to land: Zed's URL can't resolve, and setup.sh clones it again.
plant=$(mktemp -d)
mkdir -p "$plant/git"
printf '[url "https://zed.invalid/zed-industries/zed"]\n\tinsteadOf = https://github.com/zed-industries/zed\n' > "$plant/git/config"
export XDG_CONFIG_HOME=$plant
rm -rf "$app/.deps/zed"
"$app/setup.sh"
"$app/build.sh" prod
out=$app/build/flatpak
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak-builder --user --install-deps-from=flathub --force-clean --disable-rofiles-fuse \
  --state-dir="$out/state" --repo="$out/repo" "$out/app" "$app/platform/linux/flatpak/app.ilo.browser.yml"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$out/repo" "$app/build/ilo.flatpak" app.ilo.browser
echo "$app/build/ilo.flatpak"
