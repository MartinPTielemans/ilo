#!/bin/bash
# Proves tools/install.sh --rework and tools/rework-refresh.sh in a scratch copy, with codesign,
# ditto, security, pgrep and gh stood in for: the rework's copy installs as its own app beside the
# daily one, and never as it. It refuses a rework branding naming the daily app's name or id, a
# build of the daily id, the product's name or an app.<product>.* id but .rework, a folder whose app
# of that name is another's, and a folder it can't write, and leaves the daily app as it was each
# time. The morning refresh does nothing while the copy runs or the heavy slot is held, prefers the
# commit look-build marked green, else the newest both workflows passed, and its dry run changes
# nothing. Runs on Linux and the Mac.
# Usage: tools/rework-copy-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
failed=0
fail() {
  echo "FAIL: $*"
  failed=1
}

mkdir -p "$scratch/tools" "$scratch/app/branding/rework" "$scratch/bin" "$scratch/Applications"
cp "$here/tools/install.sh" "$here/tools/rework-refresh.sh" "$here/tools/requirement.sh" "$scratch/tools/"
echo ilo >"$scratch/app/branding/name"
echo app.ilo.browser >"$scratch/app/branding/id"
echo woven >"$scratch/app/branding/product"
echo https://github.com/example/ilo >"$scratch/app/branding/repository"
rework() {
  echo "$1" >"$scratch/app/branding/rework/name"
  echo "$2" >"$scratch/app/branding/rework/id"
}
rework "woven Rework" app.woven.rework

# A bundle of id $2 at $1, holding $3 to tell copies apart.
bundle() {
  rm -rf "$1"
  mkdir -p "$1/Contents"
  echo "$3" >"$1/Contents/mark"
  python3 -I -c 'import plistlib, sys; plistlib.dump({"CFBundleIdentifier": sys.argv[2]}, open(sys.argv[1], "wb"))' \
    "$1/Contents/Info.plist" "$2"
}
cat >"$scratch/bin/ditto" <<'EOF'
#!/bin/sh
cp -R "$1" "$2"
EOF
for tool in codesign security; do printf '#!/bin/sh\nexit 0\n' >"$scratch/bin/$tool"; done
# pgrep finds what $RUNNING starts and $HEAVY is.
cat >"$scratch/bin/pgrep" <<'EOF'
#!/bin/sh
[ -n "${RUNNING:-}" ] && case "$2" in "$RUNNING"*) exit 0 ;; esac
[ -n "${HEAVY:-}" ] && [ "$2" = "$HEAVY" ] && exit 0
exit 1
EOF
# df has $FREE_GB free, 100 unless set.
cat >"$scratch/bin/df" <<'EOF'
#!/bin/sh
echo "Filesystem 1024-blocks Used Available Capacity Mounted"
echo "/dev/disk 999999999 1 $(( ${FREE_GB:-100} * 1048576 )) 1% /"
EOF
# gh lists $GREEN_<workflow>'s commits, answers the look-build query with $LOOK, and notes that
# it was asked.
cat >"$scratch/bin/gh" <<'EOF'
#!/bin/sh
echo asked >>"$GH_ASKED"
[ "$1" != api ] || { [ -z "${LOOK:-}" ] || echo "$LOOK"; exit 0; }
for arg; do case $prev in --workflow) workflow=$arg ;; esac; prev=$arg; done
case $workflow in
ci.yml) printf '%s\n' $GREEN_CI ;;
app-linux.yml) printf '%s\n' $GREEN_UI ;;
esac
EOF
chmod +x "$scratch"/bin/* "$scratch"/tools/*.sh
export PATH="$scratch/bin:$PATH" GH_ASKED=$scratch/gh-asked
apps=$scratch/Applications
daily=$apps/ilo.app copy="$apps/woven Rework.app" shipped=$apps/woven.app
install() { "$scratch/tools/install.sh" "$@" >"$scratch/out" 2>&1; }
daily_kept() {
  [ "$(cat "$daily/Contents/mark" 2>/dev/null)" = daily ] || fail "$1: the daily app changed"
}

bundle "$daily" app.ilo.browser daily

# A good rework build installs as woven.app, beside the daily app.
bundle "$scratch/app/build/Browser.app" app.woven.rework rework-1
install --rework "$apps" || fail "the rework build didn't install: $(cat "$scratch/out")"
[ "$(cat "$copy/Contents/mark" 2>/dev/null)" = rework-1 ] || fail "woven.app isn't the rework build"
daily_kept "install"

# Its update replaces the copy alone; what else is in the folder stays.
bundle "$scratch/app/build/Browser.app" app.woven.rework rework-2
install --rework "$apps" || fail "the update didn't install: $(cat "$scratch/out")"
[ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the update didn't replace woven.app"
daily_kept "update"

# The dry run says where and changes nothing.
bundle "$scratch/app/build/Browser.app" app.woven.rework rework-3
install --rework --dry-run "$apps" || fail "the dry run failed: $(cat "$scratch/out")"
grep -q "would install .*(app.woven.rework) as $copy" "$scratch/out" || fail "the dry run said: $(cat "$scratch/out")"
[ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the dry run changed woven.app"

# Refused: a rework branding named as the daily app, so its target is ilo.app.
rework ilo app.woven.rework
! install --rework "$apps" || fail "a rework branding named ilo installed"
grep -q "names the daily app" "$scratch/out" || fail "the name's refusal said: $(cat "$scratch/out")"
daily_kept "the name's refusal"

# Refused: a rework branding named as the product that ships, so its target is woven.app.
rework woven app.woven.rework
! install --rework "$apps" || fail "a rework branding named woven installed"
grep -q "names the product that ships" "$scratch/out" || fail "the product name's refusal said: $(cat "$scratch/out")"
[ ! -e "$shipped" ] || fail "woven.app was made"

# Refused: an app.woven.* id that isn't app.woven.rework.
rework "woven Rework" app.woven.browser
bundle "$scratch/app/build/Browser.app" app.woven.browser shipped
! install --rework "$apps" || fail "a rework branding of app.woven.browser installed"
grep -q "names the product that ships" "$scratch/out" || fail "the product id's refusal said: $(cat "$scratch/out")"
[ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the product id's refusal changed the copy"
daily_kept "the product id's refusal"

# Refused: a rework branding with the daily app's bundle id.
rework "woven Rework" app.ilo.browser
bundle "$scratch/app/build/Browser.app" app.ilo.browser prod
! install --rework "$apps" || fail "a rework branding with the daily id installed"
grep -q "names the daily app" "$scratch/out" || fail "the id's refusal said: $(cat "$scratch/out")"
daily_kept "the id's refusal"
rework "woven Rework" app.woven.rework

# Refused: a build of the daily id, with the rework's branding right.
! install --rework "$apps" || fail "a prod build installed as the rework's copy"
grep -q "isn't a build of app.woven.rework" "$scratch/out" || fail "the build's refusal said: $(cat "$scratch/out")"
daily_kept "the build's refusal"

# Refused: a woven.app in the folder that is another app's.
bundle "$scratch/app/build/Browser.app" app.woven.rework rework-4
bundle "$copy" app.ilo.browser other
! install --rework "$apps" || fail "the copy replaced another app's woven.app"
[ "$(cat "$copy/Contents/mark")" = other ] || fail "another app's woven.app changed"
bundle "$copy" app.woven.rework rework-2

# Refused: a folder it can't write, which would need a password.
if [ "$(id -u)" != 0 ]; then
  chmod a-w "$apps"
  ! install --rework "$apps" || fail "the copy installed in a folder it can't write"
  grep -q "would need a password" "$scratch/out" || fail "the password refusal said: $(cat "$scratch/out")"
  chmod u+w "$apps"
  [ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the password refusal changed the copy"
fi

# Refused: the copy is running.
! RUNNING=$copy/Contents install --rework "$apps" || fail "the copy installed under itself running"
[ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the running copy changed"

# The daily install still refuses a rework build.
! install "$apps" || fail "the daily install took a rework build"
daily_kept "the daily install's refusal"

mkdir -p "$scratch/locks"
refresh() {
  REWORK_APPS=$apps REWORK_WORK=$scratch/work REWORK_VM_LOCKS=$scratch/locks \
    "$scratch/tools/rework-refresh.sh" "$@" >"$scratch/out" 2>&1
}

# While the copy runs, the refresh does nothing, not even ask GitHub.
rm -f "$GH_ASKED"
RUNNING=$copy/Contents GREEN_CI="c3 c2" GREEN_UI="c2" refresh --dry-run || fail "the refresh failed while running"
grep -q "skipped: woven Rework is running" "$scratch/out" || fail "the running refresh said: $(cat "$scratch/out")"
[ ! -e "$GH_ASKED" ] || fail "the refresh asked GitHub while the copy ran"

# The newest commit both passed, c2, not c3 whose UI on Linux didn't.
GREEN_CI="c3 c2 c1" GREEN_UI="c2 c1" refresh --dry-run || fail "the dry run failed: $(cat "$scratch/out")"
grep -q "would build dev/rework at c2 " "$scratch/out" || fail "the dry run picked: $(cat "$scratch/out")"
grep -q "as $copy" "$scratch/out" || fail "the dry run's target: $(cat "$scratch/out")"
[ ! -e "$scratch/work" ] || fail "the dry run made its work folder"
[ "$(cat "$copy/Contents/mark")" = rework-2 ] || fail "the dry run changed woven.app"
daily_kept "the refresh's dry run"

# The commit look-build marked green comes first.
LOOK=c1 GREEN_CI="c3 c2 c1" GREEN_UI="c2 c1" refresh --dry-run || fail "the look dry run failed"
grep -q "would build dev/rework at c1 (look-build marked it green)" "$scratch/out" || fail "the look dry run picked: $(cat "$scratch/out")"
! grep -q "would skip now" "$scratch/out" || fail "a free slot was held: $(cat "$scratch/out")"

# A held heavy slot skips the morning, never waits; the dry run says it would.
mkdir -p "$scratch/locks/192.168.64.7" && echo "engine lane" >"$scratch/locks/192.168.64.7/who"
GREEN_CI="c2" GREEN_UI="c2" refresh || fail "the refresh with the VM lock held failed"
grep -q "skipped: the check VM's lock 192.168.64.7 is held (engine lane)" "$scratch/out" || fail "the VM lock said: $(cat "$scratch/out")"
GREEN_CI="c2" GREEN_UI="c2" refresh --dry-run || fail "the dry run with the VM lock held failed"
grep -q "would skip now: the check VM's lock" "$scratch/out" || fail "the held dry run said: $(cat "$scratch/out")"
rm -r "$scratch/locks/192.168.64.7"
for heavy in "tart run" "engine/build.sh" "cargo build"; do
  HEAVY=$heavy GREEN_CI="c2" GREEN_UI="c2" refresh || fail "the refresh beside $heavy failed"
  grep -q "skipped: a heavy job runs ($heavy)" "$scratch/out" || fail "beside $heavy it said: $(cat "$scratch/out")"
done
FREE_GB=59 GREEN_CI="c2" GREEN_UI="c2" refresh || fail "the refresh on a full disk failed"
grep -q "skipped: the disk has 59 GB free, under 60" "$scratch/out" || fail "the full disk said: $(cat "$scratch/out")"
[ ! -e "$scratch/work" ] || fail "a skipped refresh made its work folder"

# Nothing green: nothing to do.
GREEN_CI="c3" GREEN_UI="c2" refresh --dry-run || fail "the refresh with nothing green failed"
grep -q "skipped: no commit" "$scratch/out" || fail "the refresh with nothing green said: $(cat "$scratch/out")"

# The commit it installed last: nothing to do.
mkdir -p "$scratch/work" && echo c2 >"$scratch/work/installed"
GREEN_CI="c2" GREEN_UI="c2" refresh || fail "the refresh with nothing new failed"
grep -q "skipped: .* is c2 already" "$scratch/out" || fail "the refresh with nothing new said: $(cat "$scratch/out")"

# Nothing of either script launches an app.
! grep -nE '^[^#]*\b(open|launchctl)\b +[^>]' "$here/tools/install.sh" "$here/tools/rework-refresh.sh" |
  grep -v 'echo "open' || fail "a script launches something"

[ "$failed" = 0 ] && echo "rework-copy-test: all passed"
exit "$failed"
