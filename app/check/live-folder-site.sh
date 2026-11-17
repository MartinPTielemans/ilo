# check.sh runs this at 735, only when named.
# A Live Folder made from the Space's menu asks GitHub itself, from the browser process on the
# run's new profile, which is signed out, and reads the answer as signed out (#2979). Every other
# proof of the folder's fetch is a fixture, whose shape came from GitHub's signed-out answers to
# curl: JSON for Accept: application/json, with payload.blackbirdSearchRoute and logged_in false
# (core/live_folder_rule.cc). This is the tripwire for the day the site answers otherwise: then the
# folder says it couldn't refresh, logged "answered failed", and the run fails. A failed or hung
# answer while GitHub doesn't answer curl either is the site down, not its shape changed, so that
# skips: a tripwire that trips on an outage gets quarantined, and then trips on nothing. It needs
# the network and GitHub, so check.sh runs it only when named: ./check.sh live-folder-site, and
# Check on Linux in each X11 run of its default list (each night's, each train's on main, and a
# dispatch that names no runs). It never signs in: the signed-in answer needs an account, which is
# the owner's. Then the app sits idle past the core's two minutes and must ask nothing more.

# Until the core's fetch is in this tree, and in the engine linked, a Live Folder asks nothing.
if [ ! -f ../core/live_folder.cc ] || ! engine_has ../core/live_folder.cc ../core/live_folder_rule.cc; then
  echo "skip all: live-folder-site. This tree or the engine linked has no Live Folder fetch yet (core/live_folder.cc)."
  exit 0
fi
# The outage test below needs curl: without it a changed answer would pass as an outage.
command -v curl > /dev/null || fail "curl isn't installed, so a failed answer can't be told from GitHub being down"
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
expect '^browser: live folder [0-9]* fetch on Made$' "making the Live Folder didn't ask for its lists"
folder=$(grep '^browser: live folder [0-9]* fetch on Made$' "$log" | tail -1 | cut -d' ' -f4)
# GitHub's answer comes over the network, so it's given 30 s; one that never comes is tested
# against the outage as a failed one is.
answer=hung
if await "^browser: live folder $folder answered "; then
  answer=$(grep "^browser: live folder $folder answered " "$log" | head -1 | sed 's/.* answered //')
fi
../tools/window-screenshot.sh "$pid" "$out/live-folder-site.png" || fail "no window to capture"
# Idle: with the folder there and nothing done, the app asks nothing more. The core lets an ask
# through to the site once each two minutes, so a poll would show by then: the run waits past them.
if [ "$answer" = "signed out" ]; then
  asks=$(grep -c "^browser: live folder $folder fetch on " "$log")
  sleep 130
  again=$(grep "^browser: live folder $folder fetch on " "$log" | tail -n +$((asks + 1)) | head -3)
  [ -z "$again" ] || fail "the idle app asked for the folder's lists again: $again"
  echo "ok: the idle app asked nothing for its Live Folder in 130 s"
fi
exec 3>&-
quit
if [ "$answer" != "signed out" ]; then
  # Unreachable is no route, no name or no answer in time (curl's 6, 7 and 28), or GitHub's own
  # server error. Anything else curl fails on, such as a certificate, is this runner's fault, not
  # an outage, and fails.
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 20 https://github.com/login 2> "$out/live-folder-site-curl.txt") && sent=0 || sent=$?
  case "$sent:$code" in
    6:* | 7:* | 28:* | 0:5??)
      echo "skip all: live-folder-site. GitHub wasn't reachable: the folder's fetch $answer, and curl https://github.com/login got exit $sent, HTTP $code."
      exit 0
      ;;
    0:*) ;;
    *) fail "the folder's fetch $answer, and curl https://github.com/login failed with exit $sent, which isn't GitHub being down: $(cat "$out/live-folder-site-curl.txt")" ;;
  esac
fi
[ "$answer" = "signed out" ] ||
  fail "GitHub's answer to a signed-out profile read as \"$answer\", not signed out, while GitHub answered curl: the site's answer may have changed shape. Window: $out/live-folder-site.png"
echo "ok: GitHub answered a new profile's Live Folder, read as signed out. Window: $out/live-folder-site.png"
