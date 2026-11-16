# check.sh runs this at 735, only when named.
# A Live Folder made from the Space's menu asks GitHub itself, from the browser process on the
# run's new profile, which is signed out, and reads the answer as signed out (#2979). Every other
# proof of the folder's fetch is a fixture, whose shape came from GitHub's signed-out answers to
# curl: JSON for Accept: application/json, with payload.blackbirdSearchRoute and logged_in false
# (core/live_folder_rule.cc). This is the tripwire for the day the site answers otherwise: then the
# folder says it couldn't refresh, logged "answered failed", and the run fails. It needs the network
# and GitHub, so it runs only when named: ./check.sh live-folder-site, and nightly in Check on
# Linux. It never signs in: the signed-in answer needs an account, which is the owner's to give.

# Until the core's fetch is in this tree, and in the engine linked, a Live Folder asks nothing.
if [ ! -f ../core/live_folder.cc ] || ! engine_has ../core/live_folder.cc ../core/live_folder_rule.cc; then
  echo "skip: live-folder-site. This tree or the engine linked has no Live Folder fetch yet (core/live_folder.cc)."
  exit 0
fi
page='data:text/html,<title>ready</title>'
log="$out/live-folder-site.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the page didn't draw"
# The sidebar's empty space under the one Today tab, whose menu is the Space's.
post rightclick $(row 3)
expect "^browser: menu shown .*$(cased 'Live Folders › GitHub Pull Requests')" \
  "the Space's menu didn't offer Live Folders › GitHub Pull Requests"
send choose "$(cased 'GitHub Pull Requests')"
expect '^browser: live folder [0-9]* fetch on Refresh$' "making the Live Folder didn't ask for its lists"
folder=$(grep '^browser: live folder [0-9]* fetch on Refresh$' "$log" | tail -1 | cut -d' ' -f4)
# GitHub's answer comes over the network, so it may take longer than a step's 10 s.
expect_late "^browser: live folder $folder answered " "GitHub didn't answer the Live Folder's fetch"
answer=$(grep "^browser: live folder $folder answered " "$log" | head -1 | sed 's/.* answered //')
../tools/window-screenshot.sh "$pid" "$out/live-folder-site.png" || fail "no window to capture"
exec 3>&-
quit
[ "$answer" = "signed out" ] ||
  fail "GitHub's answer to a signed-out profile read as \"$answer\", not signed out: the site's answer may have changed shape. Window: $out/live-folder-site.png"
echo "ok: GitHub answered a new profile's Live Folder, read as signed out. Window: $out/live-folder-site.png"
