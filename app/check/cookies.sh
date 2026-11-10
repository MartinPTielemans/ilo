# With ad blocking on, the ad blocker hides a page's cookie notice from a new profile's first page,
# with uBlock Origin's own cookie-notice lists, which the browser turns on for it (#2323). Settings'
# Hide cookie notices, under Block ads and trackers, turns that off and on again in every profile,
# without a relaunch, and the Site Controls' Block ads and trackers, off for the site, shows the
# notice there. The lists come from their sites the first time they're on, so it needs the network
# and runs only when named: ./check.sh cookies.

# The served page holds the container OneTrust's consent tool draws its notice in, which EasyList's
# Cookie Notices list hides on every site: real notices are regional, and a runner may get none.
# The page's title says whether it shows. uBlock Origin hides it from a stylesheet of its own,
# which the page can't hear arrive, so it looks again every 100 ms.
serve
printf '%s' '<title>loading</title>
<div id=onetrust-consent-sdk style="height:60px;background:#36c;color:#fff;font:20px system-ui">
  we use cookies</div>
<script>
const report = () => {
  const hidden = getComputedStyle(document.getElementById("onetrust-consent-sdk")).display == "none";
  const title = "cookie notice " + (hidden ? "hidden" : "shown");
  if (document.title != title) {
    document.title = title;
  }
};
requestAnimationFrame(() => setTimeout(report));
setInterval(report, 100);
</script>' > "$out/site/cookies.html"
profile_dir="$PWD/$out/cookies/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/cookies.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}cookies.html" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab cookie notice " || fail "the cookie page didn't report"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: cookies. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
# Reloads the tab until its page says $1, for at most 60 reloads, each waiting up to 2 s for the
# page to say it: uBlock Origin lets everything through until it has read its lists, and fetches
# the two cookie-notice lists the first time they're on.
reload_until() {
  for _ in $(seq 60); do
    send key r cmd
    expect "^browser: title $tab cookie notice " "the cookie page didn't report after ⌘R"
    for _ in $(seq 20); do
      none_since "^browser: title $tab cookie notice $1\$" || return 0
      sleep 0.1
    done
  done
  fail "$2: $(grep "^browser: title $tab " "$log" | tail -1)"
}
launched=$(python3 -I -c 'import time; print(time.time())')
reload_until hidden "on by default, the ad blocker didn't hide the page's cookie notice"
first_hidden=$(python3 -I -c 'import sys, time; print("%.1f" % (time.time() - float(sys.argv[1])))' \
  "$launched")
../tools/window-screenshot.sh "$pid" "$out/cookies-hidden.png" || fail "no window to capture"

# Off in Settings, under Block ads and trackers, then on again.
switch_cookie_notices() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Privacy$'; then
    settings_drew
    send clickkey $(drawn "toolbar Privacy")
    expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
  fi
  settings_drew Privacy
  send clickkey $(drawn "switch cookie notices")
  expect "^browser: cookie notices $1\$" "a click on Hide cookie notices' switch didn't turn it $1"
  [ "$1" = on ] || ../tools/window-screenshot.sh "$pid" "$out/cookies-settings.png" ||
    fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
}
switch_cookie_notices off
reload_until shown "turned off in Settings, the page's cookie notice stayed hidden"
switch_cookie_notices on
reload_until hidden "turned on again in Settings, the page's cookie notice didn't hide"

# Ad blocking off for the site in the Site Controls, which reloads the page: its notice shows, and
# hides again with blocking back on. With no extension turned on but the ad blocker, the panel's
# first row's middle is at y 262, as in the adblock run.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type site controls
expect '^browser: command bar commands "site controls": Site Controls$' \
  "the command bar didn't find Site Controls"
send key return
expect '^browser: site controls settled$' "the Site Controls didn't open"
send click 207 262
expect '^browser: site controls set ad blocking off$' "a click on Block ads and trackers didn't run"
expect "^browser: title $tab cookie notice shown\$" \
  "with ad blocking off for the site, the page's cookie notice stayed hidden"
../tools/window-screenshot.sh "$pid" "$out/cookies-off-for-site.png" || fail "no window to capture"
send click 207 262
expect '^browser: site controls set ad blocking on$' "a second click didn't turn it on again"
expect "^browser: title $tab cookie notice hidden\$" \
  "with ad blocking on again, the page's cookie notice didn't hide"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
quit
echo "ok: a new profile hides a page's cookie notice with uBlock Origin's cookie-notice lists, the first time within $first_hidden s of its first load. Settings' Hide cookie notices shows it again and hides it again without a relaunch, and the Site Controls' Block ads and trackers, off for the site, shows it there. Windows: $out/cookies-hidden.png, $out/cookies-settings.png, $out/cookies-off-for-site.png"
