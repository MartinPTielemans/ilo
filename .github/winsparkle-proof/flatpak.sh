#!/bin/bash
# The proof flatpak-update-proof.yml runs, from the checkout's root: FROM's Flatpak, from its GitHub
# release, goes into a repository tools/flatpak-repo.sh makes and installs from its flatpakref, as
# the website's does; ilo starts and makes its profile. Then TO's build goes into the same
# repository, flatpak update takes it, and TO starts on the profile FROM made.
# Usage: flatpak.sh <out directory> <FROM tag> release:<TO tag>|run:<release.yml run id>
set -uo pipefail
out=$1 from=$2 to=$3
mkdir -p "$out"
out=$(cd "$out" && pwd)
repo=$out/repo
url=http://localhost:8123/
id=app.ilo.browser
data=$HOME/.var/app/$id
server=

fail() {
  echo "FAIL: $*"
  exit 1
}
ok() { echo "ok: $*"; }
note() { echo "-- $*"; }
trap '[ -z "$server" ] || kill "$server" 2>/dev/null' EXIT
version() { flatpak info --user "$id" | sed -n 's/^ *Version: *//p'; }
commit() { flatpak info --user "$id" | sed -n 's/^ *Commit: *//p'; }
# Starts the installed ilo under Xvfb until its UI shows, captures it and stops it.
start() {
  dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24' bash -c '
    flatpak run '"$id"' > "$1.log" 2>&1 &
    pid=$!
    for _ in $(seq 120); do
      grep -q "browser: UI shown" "$1.log" && break
      kill -0 $pid 2>/dev/null || break
      sleep 1
    done
    sleep 2
    import -window root "$1.png" || true
    flatpak kill '"$id"' || true
    wait $pid || true' _ "$out/$1"
  grep -q "browser: UI shown" "$out/$1.log" || fail "$1: ilo's UI didn't show: $(tail -5 "$out/$1.log")"
}

note "downloads"
gh release download "$from" -R "$GITHUB_REPOSITORY" -p ilo-linux-x86_64.flatpak -D "$out/from" ||
  fail "no Flatpak in $from"
case $to in
  release:*) gh release download "${to#release:}" -R "$GITHUB_REPOSITORY" -p ilo-linux-x86_64.flatpak -D "$out/to" ;;
  run:*) gh run download "${to#run:}" -R "$GITHUB_REPOSITORY" -n linux -D "$out/to" ;;
  *) fail "TO is release:<tag> or run:<id>, not $to" ;;
esac
[ -f "$out/to/ilo-linux-x86_64.flatpak" ] || fail "no Flatpak for $to"

GNUPGHOME=$(mktemp -d)
export GNUPGHOME
gpg --batch --quiet --passphrase '' --quick-gen-key "ilo's proof Flatpak repository" rsa3072 sign never
FLATPAK_GPG_KEY=$(gpg --batch --armor --export-secret-keys)
export FLATPAK_GPG_KEY
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

note "1. $from from the repository"
tools/flatpak-repo.sh "$out/from/ilo-linux-x86_64.flatpak" "$repo" "$url" "$out/out-from" > "$out/repo-from.log" 2>&1 ||
  fail "flatpak-repo.sh took no $from: $(tail -3 "$out/repo-from.log")"
(cd "$repo" && exec python3 -m http.server 8123 --bind 127.0.0.1 > "$out/server.log" 2>&1) &
server=$!
sleep 1
flatpak install --user -y --noninteractive --from "$out/out-from/ilo.flatpakref" > "$out/install.log" 2>&1 ||
  fail "$from didn't install: $(tail -3 "$out/install.log")"
was=$(version) before=$(commit)
echo "$from installed: version $was, commit $before"
[ "$was" = "${from#v}" ] || fail "the install says $was, not ${from#v}"
start 1-from
state=$(find "$data" -name 'Local State' 2>/dev/null | head -1)
[ -n "$state" ] || fail "$from made no profile under $data"
ok "$from installed from its flatpakref, started and made its profile"

note "2. $to into the repository, and flatpak update"
tools/flatpak-repo.sh "$out/to/ilo-linux-x86_64.flatpak" "$repo" "$url" "$out/out-to" > "$out/repo-to.log" 2>&1 ||
  fail "flatpak-repo.sh took no $to: $(tail -3 "$out/repo-to.log")"
flatpak update --user -y --noninteractive "$id" > "$out/update.log" 2>&1 ||
  fail "flatpak update failed: $(tail -3 "$out/update.log")"
now=$(version) after=$(commit)
echo "after the update: version $now, commit $after"
[ "$after" != "$before" ] || fail "flatpak update kept $from's commit"
[ "$now" != "$was" ] || fail "the update still says $was"
start 2-to
[ -s "$state" ] || fail "the profile's Local State is gone"
ok "flatpak update took $now from the repository, and it started on $from's profile"
