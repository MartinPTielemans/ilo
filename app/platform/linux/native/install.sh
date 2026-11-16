#!/bin/sh
# Installs ilo for you alone, as a native build that runs Chromium's own sandbox, beside any Flatpak
# of it (#2453):
#   curl -fsSL https://github.com/MartinPTielemans/ilo/releases/latest/download/install.sh | sh
# The same line again updates it in place. Add `-s -- --uninstall` after sh to remove it.
# It never asks for root, writes only in your home, and downloads two files: ilo and its checksum.
set -eu

release=${ILO_RELEASE_URL:-https://github.com/MartinPTielemans/ilo/releases/latest/download}
tarball=ilo-linux-x86_64.tar.xz
# The newest C library the build needs (tools/linux-tarball.sh checks it against the build).
needs_glibc=2.38
data=${XDG_DATA_HOME:-$HOME/.local/share}
app=$data/ilo
bin=$HOME/.local/bin/ilo
entry=$data/applications/ilo.desktop
icon=$data/icons/hicolor/scalable/apps/ilo.svg
profile=${XDG_CONFIG_HOME:-$HOME/.config}/ilo

say() { printf '%s\n' "$*"; }
stop() {
  printf 'ilo: %s\n' "$*" >&2
  exit 1
}
# Chromium starts its helpers from its own files, so they can't be swapped while ilo runs. Its
# profile's lock names the browser's process, which is running if it's still this chrome.
running() {
  lock=$(readlink "$profile/SingletonLock" 2>/dev/null) || return 1
  case $(readlink "/proc/${lock##*-}/exe" 2>/dev/null) in
  "$app/chrome"*) return 0 ;;
  *) return 1 ;;
  esac
}
ours() { [ ! -e "$bin" ] || grep -q 'install.sh' "$bin"; }

if [ "${1:-}" = --uninstall ]; then
  ! running || stop "Quit ilo first, then run this again."
  ours || stop "$bin isn't ilo's launcher, so nothing was removed."
  rm -rf "$app" "$bin" "$entry" "$icon"
  say "ilo is removed. Your profile is still in $profile: delete that folder to remove your data too."
  [ ! -f /etc/apparmor.d/ilo ] ||
    say "To remove the AppArmor profile you added for it: sudo apparmor_parser -R /etc/apparmor.d/ilo && sudo rm /etc/apparmor.d/ilo"
  exit 0
fi
[ $# -eq 0 ] || stop "usage: install.sh [--uninstall]"

# What this machine needs, before anything is downloaded.
[ "$(uname -m)" = x86_64 ] || stop "ilo is built for x86-64, and this machine is $(uname -m)."
glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null | sed -n 's/^glibc //p')
[ -n "$glibc" ] || stop "ilo needs the GNU C library, glibc, which this system doesn't use."
[ "$(printf '%s\n' "$needs_glibc" "$glibc" | sort -V | head -n1)" = "$needs_glibc" ] ||
  stop "ilo needs glibc $needs_glibc or later, and this system has $glibc. The Flatpak runs here instead."
for tool in curl tar xz sha256sum; do
  command -v $tool >/dev/null || stop "This needs $tool. Install it, then run the line again."
done

# Chromium's sandbox makes user namespaces, and ilo never runs without it.
if command -v unshare >/dev/null && ! unshare -Ur true 2>/dev/null; then
  if [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" != 1 ]; then
    stop "This system doesn't let programs make user namespaces, which Chromium's sandbox needs."
  elif ! grep -qF "\"$app/chrome\"" /etc/apparmor.d/ilo 2>/dev/null; then
    stop "AppArmor here, as on Ubuntu 23.10 and later, lets only programs with a profile make user namespaces, which Chromium's sandbox needs.
To give ilo one, run this once, then run the install line again:

  printf 'abi <abi/4.0>,\\ninclude <tunables/global>\\nprofile ilo \"%s\" flags=(unconfined) {\\n  userns,\\n}\\n' '$app/chrome' | sudo tee /etc/apparmor.d/ilo >/dev/null && sudo apparmor_parser -r /etc/apparmor.d/ilo
"
  fi
fi
ours || stop "$bin is already there and isn't ilo's launcher, so nothing was installed."

mkdir -p "$data" "${bin%/*}" "${entry%/*}" "${icon%/*}"
work=$(mktemp -d "$data/.ilo-install.XXXXXX")
trap 'rm -rf "$work"' EXIT
curl -fsSL "$release/$tarball.sha256" -o "$work/$tarball.sha256" ||
  stop "Couldn't download ilo's checksum from $release."
if [ "$(cat "$work/$tarball.sha256")" = "$(cat "$app/$tarball.sha256" 2>/dev/null)" ]; then
  say "ilo is up to date."
  exit 0
fi
! running || stop "There's a newer ilo. Quit ilo, then run the line again to update it."
say "Downloading ilo…"
curl -fL --progress-bar "$release/$tarball" -o "$work/$tarball" || stop "Couldn't download ilo from $release."
(cd "$work" && sha256sum -c --quiet "$tarball.sha256") ||
  stop "The download doesn't match its checksum, so nothing was installed. Try again later."
tar -xJf "$work/$tarball" -C "$work"
missing=$(ldd "$work/ilo/chrome" "$work/ilo/libBrowser.so" 2>&1 |
  sed -n 's/^[[:space:]]*\([^ ]*\) => not found$/\1/p; s/.*version .\([^ ]*\). not found.*/\1/p' | sort -u | tr '\n' ' ' | sed 's/ $//')
[ -z "$missing" ] || stop "ilo needs what this system doesn't have: $missing. Install it from your distribution, then run the line again."

# In place: the new build replaces the old one whole, and the profile, apart from it, stays.
cp "$work/$tarball.sha256" "$work/ilo/"
[ -d "$app" ] && was=updated || was=installed
rm -rf "$app.old"
[ ! -d "$app" ] || mv "$app" "$app.old"
mv "$work/ilo" "$app"
rm -rf "$app.old"
cat >"$bin" <<EOF
#!/bin/sh
# ilo's launcher, which install.sh put here. CHROME_DESKTOP names the entry its windows belong to.
CHROME_DESKTOP=ilo.desktop exec "$app/chrome" "\$@"
EOF
chmod 755 "$bin"
sed "s|^Exec=ilo|Exec=\"$bin\"|" "$app/ilo.desktop" >"$entry"
cp "$app/ilo.svg" "$icon"
! command -v update-desktop-database >/dev/null || update-desktop-database -q "${entry%/*}" || true

say "ilo is $was. Open it from your apps, or run ilo."
case ":$PATH:" in
*":${bin%/*}:"*) ;;
*) say "${bin%/*} isn't on your PATH, so in a terminal run $bin." ;;
esac
say "Run the same line again to update it, or with 'sh -s -- --uninstall' to remove it."
