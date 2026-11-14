# check.sh runs this at 190.
# The browser blocks ads and trackers from a new profile's first page, with nothing to install:
# uBlock Origin, in the framework, loads into every profile, and no extension list shows it, as
# Settings › Ad Blocking is all of its settings (#2107). Filters given for the check as My filters,
# through the same managed settings the browser gives it its own, block the served page's ad script
# and hide its ad. The run reports how long that first launch let requests through. Its own lists
# take fbclid out of an address. The Site Controls' shield, the action row's last tile, says how
# many requests it blocked on the page, and turns it off for the site, reloading the page, which
# then shows the ad, and on again, counting afresh, by clicks and then by Tab's ring and space and
# return. Off
# for the site, Settings › Ad Blocking lists it, and its Turn On turns it on there again. Its Block
# ads and trackers turns it off in every profile, which takes it out of the Site Controls, and on
# again.
# A filter list switched on there, found by Settings' search, blocks at once: uBlock filters –
# Annoyances, off as it comes, which --ad-block-lists makes a list on the check's server. A rule
# added in My filters hides an element, and its Remove shows it again. A private window blocks
# too. The app ships the version ublock.ini pins, trusting no site, YouTube neither,
# and launched again on the same profile, it blocks there too, from each launch's first page, which
# the core holds until uBlock Origin is ready. The browser's own engine, which blocks in its place
# with --ad-block-engine=native (#2111), never loads, nor keeps anything in the profile.
# Where the browser's own is the default, Settings' Block with uBlock Origin goes back to the
# extension at the next launch and forward again, and the browser's own leaves its storage as it was.

# The served ad page: its title says whether its script ran and whether its ad shows. uBlock Origin
# hides the ad from a stylesheet of its own, which the page can't hear arrive, so it looks again
# every 100 ms.
serve
printf '%s' 'self.adLoaded = true;' > "$out/site/check-ad.js"
printf '%s' '<title>loading</title>
<div class=check-ad style="height:60px;background:#c33;color:#fff;font:20px system-ui">an ad</div>
<script src="check-ad.js"></script>
<script>
const report = () => {
  const hidden = getComputedStyle(document.querySelector(".check-ad")).display == "none";
  const title = "ad script " + (self.adLoaded ? "loaded" : "blocked") +
    ", element " + (hidden ? "hidden" : "shown");
  if (document.title != title) {
    document.title = title;
  }
};
requestAnimationFrame(() => setTimeout(report));
setInterval(report, 100);
</script>' > "$out/site/ads.html"
ads="${site}ads.html"
# A page whose script only the annoyance list blocks, and one whose element only My filters' rule
# added below hides.
printf '%s' 'self.listedLoaded = true;' > "$out/site/listed-ad.js"
printf '%s' '<title>loading</title>
<script src="listed-ad.js"></script>
<script>
requestAnimationFrame(() => setTimeout(() => {
  document.title = "listed script " + (self.listedLoaded ? "loaded" : "blocked");
}));
</script>' > "$out/site/listed.html"
listed="${site}listed.html"
printf '%s' '<title>loading</title>
<div class=check-filtered style="height:60px">filtered</div>
<script>
const report = () => {
  const hidden = getComputedStyle(document.querySelector(".check-filtered")).display == "none";
  const title = "filtered element " + (hidden ? "hidden" : "shown");
  if (document.title != title) {
    document.title = title;
  }
};
requestAnimationFrame(() => setTimeout(report));
setInterval(report, 100);
</script>' > "$out/site/filtered.html"
filtered="${site}filtered.html"
# The lists' source: uBlock Origin's assets.json with every remote address taken out, so nothing
# reaches the network, as in adblock-lists, and uBlock filters – Annoyances, off as it comes, the
# check's list blocking listed-ad.js.
if [ -d "$app/ublock" ]; then
  ublock_dir="$app/ublock"
else
  ublock_dir="$(echo "$app"/Contents/Frameworks/*Framework.framework)/Resources/ublock"
fi
mkdir -p "$out/site/lists"
printf '%s\n' '! Title: Check annoyances' '/listed-ad.js' > "$out/site/lists/annoyances.txt"
python3 -I - "$ublock_dir/assets/assets.json" "${site}lists/annoyances.txt" \
  > "$out/site/lists/assets.json" << 'EOF'
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    assets = json.load(f)
listed = {}
for key, entry in assets.items():
    urls = entry.get("contentURL", [])
    local = [url for url in ([urls] if isinstance(urls, str) else urls)
             if not url.startswith(("http:", "https:"))]
    if key == "ublock-annoyances":
        local = [sys.argv[2]]
    if local:
        entry["contentURL"] = local
        entry.pop("cdnURLs", None)
        entry.pop("patchURLs", None)
        listed[key] = entry
if not listed.get("ublock-annoyances", {}).get("off"):
    sys.exit("uBlock filters - Annoyances isn't off as uBlock Origin comes")
json.dump(listed, sys.stdout)
EOF
[ -s "$out/site/lists/assets.json" ] || fail "the run couldn't make its lists' source"
lists="${site}lists/assets.json"
ublock=oghkpboejbohceocapipmnjifoklonij
# A profile of the run's own, so uBlock Origin starts as on a new install: it keeps the managed
# settings it last read, and reads its lists into the profile on its first launch.
profile_dir="$PWD/$out/adblock/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/adblock.log"
step=0
filters="$(printf '/check-ad.js\n127.0.0.1##.check-ad')"
launched=$(python3 -I -c 'import time; print(time.time())')
BROWSER_DRIVE=1 BROWSER_URL="$ads" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" --ad-block-filters="$filters" --ad-block-lists="$lists" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab ad script " || fail "the ad page didn't report"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: adblock. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
# Reloads the tab until its page says $1, for at most 20 reloads, each waiting up to 2 s for the
# page to say it. On a new profile, uBlock Origin lets everything through until it has read its
# lists, a few seconds in. The page reports in titles starting $report.
report="ad script "
reload_until() {
  for _ in $(seq 20); do
    send key r cmd
    expect "^browser: title $tab $report" "the page didn't report after ⌘R"
    for _ in $(seq 20); do
      none_since "^browser: title $tab $1\$" || return 0
      sleep 0.1
    done
  done
  fail "$2: $(grep "^browser: title $tab " "$log" | tail -1)"
}
blocked="ad script blocked, element hidden"
reload_until "$blocked" "the check's filter didn't block the ad script and hide the ad"
# How long the first launch let requests through, to the reload that found the ad blocked: the
# spec's about 3 s, which this bounds from above by the reload's wait.
first_block=$(python3 -I -c 'import sys, time; print("%.1f" % (time.time() - float(sys.argv[1])))' \
  "$launched")
reloads=$(grep -c "^browser: title $tab ad script " "$log" || true)
echo "adblock: a new profile's first launch blocked within $first_block s, over $reloads loads"
../tools/window-screenshot.sh "$pid" "$out/adblock-blocked.png" || fail "no window to capture"
# No extension list shows it, so its popup and dashboard never open: the Extensions menu leaves
# it out, as the URL field's actions do (#2107).
listed_ublock() {
  grep -e "^browser: extensions menu .*$ublock" -e "^browser: extension actions .*$ublock" "$log" |
    head -1
}
[ -z "$(listed_ublock)" ] || fail "an extension list showed uBlock Origin: $(listed_ublock)"

# Its own lists take Facebook's click id out of an address before the page loads, and leave the
# site's own parameters.
go_from_bar "${ads#http://}?id=1&fbclid=2"
expect "^browser: loaded $tab ${ads}.id=1\$" \
  "the tab kept fbclid in its address: $(grep "^browser: loaded $tab " "$log" | tail -1)"
expect "^browser: title $tab $blocked\$" "the page without fbclid didn't block its ad"

# Off for the site with the Site Controls' shield, which reloads the page with its ad, and on again.
# The panel logs where it drew the shield.
controls="^browser: site controls http://127.0.0.1:$port: [^;]*; "
open_site_controls() {
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type site controls
  expect '^browser: command bar commands "site controls": Site Controls$' \
    "the command bar didn't find Site Controls"
  send key return
  expect '^browser: site controls settled$' "the Site Controls didn't open"
}
open_site_controls
# With how many requests it blocked on the page, the ad script among them, as its badge counts.
expect "${controls}Ad blocking On ([0-9>][0-9k]* blocked)" \
  "the Site Controls didn't list ad blocking as on, first, with what it blocked"
blocked_count=$(grep "^${controls#^}" "$log" | sed -n 's/.*Ad blocking On (\([^)]*\) blocked).*/\1/p' |
  tail -1)
click_placed send shield
expect '^browser: site controls set ad blocking off$' "a click on the shield didn't run"
expect "^browser: title $tab ad script loaded, element shown\$" \
  "off for the site, the page didn't load its ad script and show its ad"
expect "${controls}Ad blocking Off" "the Site Controls didn't show ad blocking as off for the site"
../tools/window-screenshot.sh "$pid" "$out/adblock-off-for-site.png" || fail "no window to capture"
click_placed send shield
expect '^browser: site controls set ad blocking on$' "a second click didn't turn it on again"
expect "^browser: title $tab $blocked\$" "on again, the page didn't block its ad"
# The open panel hears the badge of the page it reloaded, which counts afresh.
expect "${controls}Ad blocking On ([0-9>][0-9k]* blocked)" \
  "on again, the open Site Controls didn't say what the reloaded page blocked"
# From the keyboard: Tab takes the ring along the tiles to the shield, the row's last, however
# many tiles the system has, and space and return turn it off and on as clicks do (#1382).
keyed() { grep '^browser: site controls keyed ' "$log" | tail -1 | cut -d' ' -f5-; }
for _ in 1 2 3 4 5 6 7 8; do
  send key tab
  expect '^browser: site controls keyed ' "Tab didn't move the Site Controls' ring"
  [ "$(keyed)" = AdBlocking ] && break
done
[ "$(keyed)" = AdBlocking ] || fail "Tab didn't reach the shield: the ring is on $(keyed)"
../tools/window-screenshot.sh "$pid" "$out/site-controls-keyed.png" || fail "no window to capture"
send key space
expect '^browser: site controls set ad blocking off$' "space on the ringed shield didn't turn ad blocking off"
expect "^browser: title $tab ad script loaded, element shown\$" \
  "off from the keyboard, the page didn't load its ad script and show its ad"
send key return
expect '^browser: site controls set ad blocking on$' "return on the ringed shield didn't turn it on again"
expect "^browser: title $tab $blocked\$" "on again from the keyboard, the page didn't block its ad"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"

# Off for the site again, Settings lists it under Block ads and trackers, and its Turn On turns it
# on again there.
open_site_controls
click_placed send shield
expect '^browser: site controls set ad blocking off$' "a third click on the shield didn't run"
expect "^browser: title $tab ad script loaded, element shown\$" \
  "off for the site again, the page didn't load its ad script and show its ad"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show General"
settings_drew General
send clickkey $(drawn "toolbar Ad Blocking")
expect '^browser: settings tab Ad Blocking$' "a click on Ad Blocking didn't show its section"
settings_drew "Ad Blocking"
expect '^browser: settings ad blocking off on 127\.0\.0\.1$' \
  "Settings didn't list the site ad blocking is off on, alone"
expect '^browser: settings turn on 127\.0\.0\.1 at ' "Settings didn't draw the site's Turn On"
turn_on=$(sed -n "s/^browser: settings turn on 127\.0\.0\.1 at //p" "$log" | tail -1)
../tools/window-screenshot.sh "$pid" "$out/adblock-off-sites.png" all || fail "no window to capture"
send clickkey $turn_on
expect '^browser: ad blocking on for 127\.0\.0\.1$' "a click on the site's Turn On didn't run"
expect '^browser: settings ad blocking off on $' "turned on, Settings still listed the site"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
reload_until "$blocked" "turned on again in Settings' list, the page didn't block its ad"

# Off everywhere in Settings, whose Ad Blocking, where it opens again, leads with Block ads and
# trackers.
send menu Settings…
expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking again"
settings_drew "Ad Blocking"
click_in_sight "switch ad blocking"
expect '^browser: ad blocking off$' "a click on Block ads and trackers' switch didn't turn it off"
../tools/window-screenshot.sh "$pid" "$out/adblock-settings.png" all || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
send key r cmd
expect "^browser: title $tab ad script loaded, element shown\$" \
  "turned off in Settings, the page didn't load its ad"
open_site_controls
expect "$controls" "the Site Controls didn't list the site's rows"
none_since "${controls}Ad blocking" || fail "turned off in Settings, the Site Controls listed it"
send key escape
expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
send menu Settings…
expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking a third time"
settings_drew "Ad Blocking"
click_in_sight "switch ad blocking"
expect '^browser: ad blocking on$' "a second click on the switch didn't turn it on"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
reload_until "$blocked" "turned on again in Settings, the page didn't block its ad"

# A rule added in My filters hides an element at once, and its Remove shows it again: the page's
# element shows until then. The filters given for the check are My filters' first two.
go_from_bar "${filtered#http://}"
expect "^browser: loaded $tab $filtered\$" "the tab didn't load the filtered page"
expect "^browser: title $tab filtered element shown\$" "the filtered page's element didn't show first"
send menu Settings…
expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking for My filters"
settings_drew "Ad Blocking"
send clickkey $(drawn "filter field")
send insert "127.0.0.1##.check-filtered"
send key return
expect '^browser: ad block filter added 127\.0\.0\.1##\.check-filtered$' \
  "return in My filters' field didn't add the rule"
expect '^browser: settings remove filter 2 at ' "My filters didn't list the rule added, third"
../tools/window-screenshot.sh "$pid" "$out/adblock-my-filters.png" all || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
report="filtered element "
reload_until "filtered element hidden" "My filters' rule didn't hide the page's element"
send menu Settings…
expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking to remove the rule"
settings_drew "Ad Blocking"
send clickkey $(drawn "remove filter 2")
expect '^browser: ad block filter removed 127\.0\.0\.1##\.check-filtered$' \
  "My filters' Remove didn't remove the rule"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
reload_until "filtered element shown" "My filters' rule removed, the page's element stayed hidden"

# uBlock filters – Annoyances, off as it comes, lets the listed page's script load; switched on in
# Settings, found by its search, it's fetched from the check's server and blocks it.
go_from_bar "${listed#http://}"
expect "^browser: loaded $tab $listed\$" "the tab didn't load the listed page"
expect "^browser: title $tab listed script loaded\$" \
  "with the annoyance list off, the listed page's script didn't load"
send menu Settings…
expect '^browser: settings shown Ad Blocking$' "Settings… didn't show Ad Blocking for its lists"
settings_drew "Ad Blocking"
send clickkey $(drawn "search")
send type ublock annoyances
expect '^browser: settings search "ublock annoyances" finds uBlock filters – Annoyances$' \
  "Settings' search didn't find uBlock filters – Annoyances alone"
send clickkey $(drawn "switch ad block list ublock-annoyances")
expect '^browser: ad block list ublock-annoyances on$' "a click on the list's switch didn't turn it on"
served "/lists/annoyances.txt" "uBlock Origin didn't fetch the list switched on"
../tools/window-screenshot.sh "$pid" "$out/adblock-list-on.png" all || fail "no window to capture"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $page_point
report="listed script "
reload_until "listed script blocked" "the list switched on in Settings didn't block its script"
report="ad script "
go_from_bar "${ads#http://}"
expect "^browser: loaded $tab $ads\$" "the tab didn't load the ad page again"
expect "^browser: title $tab $blocked\$" "back on the ad page, it didn't block its ad"

# A private window blocks too.
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
new_window_bar
send window 2
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
private_ads="$ads?private"
send insert "$private_ads"
send key return
expect "^browser: loaded [0-9]* $private_ads\$" "the private window didn't load the ad page"
tab=$(sed -n "s|^browser: loaded \([0-9]*\) $private_ads\$|\1|p" "$log" | tail -1)
expect "^browser: title $tab ad script " "the private window's ad page didn't report"
# Its uBlock Origin is another, which reads its lists as it first starts.
send window 2
send click $page_point
reload_until "$blocked" "the private window didn't block the ad"
../tools/window-screenshot.sh "$pid" "$out/adblock-private.png" all || fail "no window to capture"

quit
# Its housekeeping timers that re-armed themselves, waking an idle browser, as the app ships it
# (#252): fetch-ublock.sh takes them out.
if [ -d "$app/ublock" ]; then
  ublock_js="$app/ublock/js"
else
  ublock_js="$(echo "$app"/Contents/Frameworks/*Framework.framework)/Resources/ublock/js"
fi
rearming=$(cat "$ublock_js/tab.js" "$ublock_js/traffic.js" | grep -cxF \
  -e '        gcTimer.on(30011);' -e '            this.gcTimer.on(gcPeriod);' \
  -e '        pageStoreJanitorTimer.on(pageStoreJanitorPeriod);' || true)
[ "$rearming" = 0 ] || fail "uBlock Origin ships with $rearming timers that wake an idle browser"
echo "adblock: uBlock Origin timers that wake an idle browser: $rearming"
# The version ublock.ini pins, which fetch-ublock.sh checked against its sha256.
pinned_version=$(git config -f ublock.ini ublock.version)
shipped_version=$(sed -n 's/^ *"version": "\([^"]*\)",*$/\1/p' "$ublock_js/../manifest.json")
[ "$shipped_version" = "$pinned_version" ] ||
  fail "the app ships uBlock Origin $shipped_version, not ublock.ini's $pinned_version"
# It trusts no site to show its ads, YouTube neither, as Arc's did: by default it trusts only
# extension pages, and the core's managed settings add none.
trusted=$(sed -n '/^ *netWhitelistDefault: \[$/,/^ *\],$/p' "$ublock_js/background.js")
[ -n "$trusted" ] || fail "the run found no trusted sites in uBlock Origin's background.js"
echo "$trusted" | grep -qi youtube && fail "uBlock Origin trusts YouTube by default: $trusted"

# An existing profile blocks from its first page too, at each launch: launched again on it, uBlock
# Origin reads the lists it kept, and the core holds the first page until it's ready (#744). Three
# launches, as it raced the first page at every one. Each reports how long the page was held and
# when the UI drew its first frame, which the hold mustn't move.
for n in 2 3 4; do
  log="$out/adblock-launch-$n.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$ads?$n" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" --ad-block-filters="$filters" --ad-block-lists="$lists" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: loaded [0-9]* $ads?$n\$" ||
    fail "launch $n on its profile: the app didn't load the ad page"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) $ads?$n\$|\1|p" "$log" | head -1)
  await "^browser: title $tab ad script " || fail "launch $n on its profile: the ad page didn't report"
  first_page=$(grep "^browser: title $tab ad script " "$log" | head -1 | cut -d' ' -f4-)
  case $first_page in
  "ad script blocked"*) ;;
  *) fail "launch $n on its profile: the first page loaded its ad script ($first_page)" ;;
  esac
  await "^browser: title $tab $blocked\$" ||
    fail "launch $n on its profile: the ad wasn't hidden: $(grep "^browser: title $tab " "$log" | tail -1)"
  loads=$(grep -c "^browser: loaded $tab " "$log")
  [ "$loads" = 1 ] || fail "launch $n on its profile: the page loaded $loads times, not once"
  [ -z "$(listed_ublock)" ] ||
    fail "launch $n on its profile: an extension list showed uBlock Origin: $(listed_ublock)"
  held=$(sed -n 's/^browser: ad blocking held a page \([0-9]*\) ms, ready$/\1/p' "$log" | head -1)
  frame=$(sed -n 's/^browser: first frame at \([0-9]*\).*/\1/p' "$log" | head -1)
  echo "adblock: launch $n on its profile blocked from its first page, held ${held:-0} ms; UI's first frame at $frame ms"
  [ "$n" != 4 ] || ../tools/window-screenshot.sh "$pid" "$out/adblock-existing.png" ||
    fail "no window to capture"
  [ "$n" = 4 ] || quit
done
quit
# uBlock Origin blocks unless --ad-block-engine=native, and then the browser's own engine never
# loads, compiles or downloads a list, nor keeps anything in the profile.
! grep -h "^browser: ad block engine " "$out/adblock.log" "$out"/adblock-launch-*.log ||
  fail "without --ad-block-engine=native, the browser's own ad block engine loaded"
[ ! -e "$profile_dir/Ad Blocking" ] ||
  fail "without --ad-block-engine=native, the profile has the native engine's folder:" \
    "$(ls "$profile_dir/Ad Blocking")"
# The way back (#2111): in a build whose default is the browser's own engine, which
# --ad-block-default=native makes this one for the check, Settings › Ad Blocking's Block with
# uBlock Origin goes back to the extension at the next launch, and the browser's own leaves its
# data as it was. The relaunch is the restart's, with the switches it was given, as widevine.sh's:
# a click on Restart would start an app outside the check, so the run reads that it's drawn.
store="$profile_dir/Default/Local Extension Settings/$ublock"
[ -d "$store" ] || fail "uBlock Origin kept no storage in the profile: $store"
# Its files and what they hold, as one digest.
store_digest() {
  python3 -I -c 'import hashlib, os, sys
digest = hashlib.sha256()
for name in sorted(os.listdir(sys.argv[1])):
    with open(os.path.join(sys.argv[1], name), "rb") as file:
        digest.update(name.encode() + b"\0" + file.read())
print(digest.hexdigest())' "$store"
}
kept=$(store_digest)
# The row among its neighbours in each tone, as the review of its look asks, named $1.
way_back_captures() {
  local tone light
  for tone in light dark; do
    if [ $tone = light ]; then light=true; else light=false; fi
    if ! grep '^browser: window light ' "$log" | tail -1 | grep -q " $light\$"; then
      send menu "Appearance › $([ $tone = light ] && echo Light || echo Dark)"
      expect "^browser: window light $light\$" "View › Appearance didn't make the window $tone"
    fi
    ../tools/window-screenshot.sh "$pid" "$out/adblock-way-back-$1-$tone.png" all ||
      fail "no window to capture"
  done
}
for engine in native ublock; do
  log="$out/adblock-way-back-$engine.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$ads?$engine" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" --ad-block-default=native --ad-block-filters="$filters" --ad-block-lists="$lists" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: ad blocker $engine at launch\$" ||
    fail "the way back: the launch didn't run $engine: $(grep '^browser: ad blocker ' "$log")"
  await "^browser: loaded [0-9]* $ads?$engine\$" ||
    fail "the way back: the app didn't load the ad page"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) $ads?$engine\$|\1|p" "$log" | head -1)
  await "^browser: title $tab ad script blocked" ||
    fail "the way back, $engine: the page loaded its ad script:" \
      "$(grep "^browser: title $tab " "$log" | tail -1)"
  if [ "$engine" = ublock ]; then
    ! grep -q "^browser: ad block engine " "$log" ||
      fail "the way back: with uBlock Origin chosen, the browser's own engine loaded"
    await "^browser: title $tab $blocked\$" ||
      fail "the way back: uBlock Origin didn't hide the ad:" \
        "$(grep "^browser: title $tab " "$log" | tail -1)"
  fi
  send menu Settings…
  expect '^browser: settings shown ' "the way back, $engine: Settings… didn't show Settings"
  settings_drew
  send clickkey $(drawn "toolbar Ad Blocking")
  expect '^browser: settings tab Ad Blocking$' "the way back: a click on Ad Blocking didn't show it"
  settings_drew "Ad Blocking"
  none_since '^browser: settings ad blocker restart at ' ||
    fail "the way back: the row offered a restart before a choice"
  # The row before a choice, its line saying what going back means, in each tone.
  [ "$engine" = ublock ] || way_back_captures before
  click_in_sight "switch ad blocker"
  if [ "$engine" = native ]; then
    expect '^browser: ad blocker on$' "a click on Block with uBlock Origin didn't turn it on"
    expect '^browser: ad blocker ublock chosen$' "Block with uBlock Origin didn't keep the choice"
  else
    expect '^browser: ad blocker off$' "uBlock Origin running, the switch didn't turn off"
    expect '^browser: ad blocker native chosen$' "the switch didn't keep the way forward"
  fi
  drawn "ad blocker restart" > /dev/null || fail "the way back: the row offered no restart"
  if [ "$engine" = native ]; then
    way_back_captures chosen
  else
    ../tools/window-screenshot.sh "$pid" "$out/adblock-way-back-ublock-chosen.png" all ||
      fail "no window to capture"
  fi
  [ "$engine" = ublock ] || [ "$(store_digest)" = "$kept" ] ||
    fail "the browser's own engine changed uBlock Origin's storage"
  quit
done
[ -d "$store" ] || fail "uBlock Origin's storage went with the way back"

echo "ok: a new profile blocks ads from its first page, with uBlock Origin in no extension list: My filters given through its managed settings block the page's ad script and hide its ad; its own lists take fbclid out of an address. The Site Controls' shield, the action row's last tile, says it blocked $blocked_count on the page, and turns it off for the site and on again, reloading the page and counting it afresh, and Settings › Ad Blocking lists the site while it's off, with a Turn On that lets it block there again; its switch turns it off everywhere, taking it out of the Site Controls, and on again. A rule added in My filters hides an element, and its Remove shows it again; uBlock filters – Annoyances, switched on from Settings' search, is fetched and blocks its script. A private window blocks too, and uBlock Origin ships at ublock.ini's $pinned_version without the timers that woke an idle browser, trusting no site, YouTube neither. The browser's own engine never loaded. Launched again three times, the existing profile blocks from each first page. With the browser's own as the default, Block with uBlock Origin goes back to the extension, which blocks after the restart the row offers, its storage as the browser's own left it, and turned off goes forward again. The first launch blocked within $first_block s. Windows: $out/adblock-blocked.png, $out/adblock-off-for-site.png, $out/adblock-off-sites.png, $out/adblock-settings.png, $out/adblock-my-filters.png, $out/adblock-list-on.png, $out/adblock-private.png, $out/adblock-existing.png, $out/adblock-way-back-before-light.png, $out/adblock-way-back-before-dark.png, $out/adblock-way-back-chosen-light.png, $out/adblock-way-back-chosen-dark.png, $out/adblock-way-back-ublock-chosen.png"
