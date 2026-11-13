# check.sh runs this at 550.
# An administrator's policies, set the system's way under ilo's own names, reach the browser
# (#684, docs/policies.md): URLBlocklist blocks one of the run's pages, whose request the run's
# server never gets, and whose tab commits Chromium's error page in its place (net error -22,
# ERR_BLOCKED_BY_ADMINISTRATOR, where the core logs it). Where the run can, it also sets Chromium's
# own names to block another page, which loads, as ilo doesn't read them. Without the policies, a
# launch loads the blocked page. Each system's way:
# - Linux: /etc/ilo/policies/managed, written with sudo, on a machine where that needs no password,
#   as GitHub's runners; Chromium's /etc/chromium/policies/managed beside it.
# - The Flatpak (BROWSER_FLATPAK): an unmanaged extension for app.ilo.browser.Extension, in
#   $XDG_DATA_HOME/flatpak/extension, as Flathub's Chromium takes them.
# - The Mac: the forced level, /Library/Managed Preferences/<the app's bundle id>.plist, written
#   with sudo, only in a check VM, never on someone's own Mac; org.chromium.Chromium's beside it.
# - Windows: HKCU\SOFTWARE\Policies\ilo, where the Windows engine reads them (#1810).
# The policies come off as the run ends, a failed one too (run_undo).

if [ "$(uname)" = Darwin ] && [ "$(sysctl -n kern.hv_vmm_present)" != 1 ]; then
  echo "skip: policies. It writes the Mac's managed preferences, so it runs only in a check VM."
  return 0
fi
if [ "$(uname)" = Linux ] && [ -z "${BROWSER_FLATPAK:-}" ] && ! sudo -n true 2>/dev/null; then
  echo "skip: policies. It writes /etc/ilo/policies, which needs sudo without a password."
  return 0
fi
serve
printf '%s' '<title>blocked page</title>' > "$out/site/policy-blocked.html"
printf '%s' '<title>other page</title>' > "$out/site/policy-other.html"
blocked="${site}policy-blocked.html"
blocked_filter="127.0.0.1:$port/policy-blocked.html"
other_filter="127.0.0.1:$port/policy-other.html"
log="$out/policies.log"
step=0
extension="${XDG_DATA_HOME:-$HOME/.local/share}/flatpak/extension/app.ilo.browser.Extension.check"
# Takes every policy the run set off the system again.
forget_policies() {
  if [ -n "${BROWSER_FLATPAK:-}" ]; then
    rm -rf "$extension"
  elif [ "$(uname)" = Linux ]; then
    sudo -n rm -f /etc/ilo/policies/managed/check.json /etc/chromium/policies/managed/check.json
  elif [ "$(uname)" = Darwin ]; then
    sudo -n rm -f "/Library/Managed Preferences/$app_id.plist" \
      "/Library/Managed Preferences/org.chromium.Chromium.plist"
    sudo -n killall cfprefsd 2>/dev/null || true
  else
    MSYS2_ARG_CONV_EXCL='*' reg delete 'HKCU\SOFTWARE\Policies\ilo\URLBlocklist' /f \
      > /dev/null 2>&1 || true
  fi
}
run_undo=forget_policies
# Sets URLBlocklist to block the page $1 under ilo's names and, where the system has them apart,
# the page $2 under Chromium's.
set_policies() {
  forget_policies
  json() { printf '{"URLBlocklist": ["%s"]}\n' "$1"; }
  plist() {
    python3 -I -c 'import plistlib, sys; sys.stdout.buffer.write(plistlib.dumps({"URLBlocklist": [sys.argv[1]]}))' "$1"
  }
  if [ -n "${BROWSER_FLATPAK:-}" ]; then
    arch=$(flatpak --default-arch)
    mkdir -p "$extension/$arch/1/policies/managed"
    json "$1" > "$extension/$arch/1/policies/managed/check.json"
    names=app.ilo.browser.Extension chromium_names=
  elif [ "$(uname)" = Linux ]; then
    sudo -n mkdir -p /etc/ilo/policies/managed /etc/chromium/policies/managed
    json "$1" | sudo -n tee /etc/ilo/policies/managed/check.json > /dev/null
    json "$2" | sudo -n tee /etc/chromium/policies/managed/check.json > /dev/null
    names=/etc/ilo/policies chromium_names=/etc/chromium/policies
  elif [ "$(uname)" = Darwin ]; then
    sudo -n mkdir -p "/Library/Managed Preferences"
    plist "$1" | sudo -n tee "/Library/Managed Preferences/$app_id.plist" > /dev/null
    plist "$2" | sudo -n tee "/Library/Managed Preferences/org.chromium.Chromium.plist" > /dev/null
    sudo -n killall cfprefsd 2>/dev/null || true
    names=$app_id chromium_names=org.chromium.Chromium
  else
    MSYS2_ARG_CONV_EXCL='*' reg add 'HKCU\SOFTWARE\Policies\ilo\URLBlocklist' /v 1 /t REG_SZ \
      /d "$1" /f > /dev/null || fail "reg couldn't set URLBlocklist"
    names='SOFTWARE\Policies\ilo' chromium_names=
  fi
}
launch() {
  BROWSER_DRIVE=1 BROWSER_URL="$1" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
}

set_policies "$blocked_filter" "$other_filter"
launch "${site}policy-other.html"
await_load
await '^browser: title [0-9]* other page$' ||
  fail "the page Chromium's names block, which ilo doesn't read, didn't load"
go_from_bar "${blocked#http://}"
await "^browser: page [0-9]* .*loading 0 $blocked\$" || fail "going to the blocked page didn't end"
grep -q '^browser: title [0-9]* blocked page$' "$log" &&
  fail "URLBlocklist under $names didn't block the page"
grep -q '/policy-blocked.html' "$out/site.log" &&
  fail "the browser asked the run's server for the page URLBlocklist blocks"
# The core logs the tab's net error where its engine has the line (#684).
error=$(sed -n 's/.*browser: tab [0-9]* error \(-[0-9]*\)$/\1/p' "$log" | tail -1)
[ -z "$error" ] || [ "$error" = -22 ] ||
  fail "the blocked page's error was $error, not -22, ERR_BLOCKED_BY_ADMINISTRATOR"
../tools/window-screenshot.sh "$(app_pid)" "$out/policies-blocked.png" || fail "no window to capture"
exec 3>&-
quit

# The way back: without the policy, the page loads.
forget_policies
step=0
launch "$blocked"
await_load
await '^browser: title [0-9]* blocked page$' || fail "with the policy gone, the page didn't load"
exec 3>&-
quit
run_undo=
echo "ok: URLBlocklist under $names blocked $blocked_filter, which the run's server never saw" \
  "requested${error:+, with net error $error}${chromium_names:+, and $chromium_names's policy didn't apply}," \
  "and without it the page loaded. Log: $log, the window: $out/policies-blocked.png"
