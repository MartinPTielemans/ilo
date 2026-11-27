# check.sh runs this at 740, only when named.
# The parity gate a native ad block engine has to pass before it takes uBlock Origin's place
# (#2111), run with uBlock Origin, or with the browser's own engine where engine is native, as
# adblock-native's is. In a new profile, once its lists block, each site in check/adblock-sites.txt
# opens in one tab. For each,
# the run gives the time from Return in the command bar to the page's load, the requests it made,
# and those blocked or answered with a stand-in, with their hosts, which DevTools records (check/
# cdp.py), as it sees what an extension's webRequest blocks where the net log doesn't. It
# captures the window 8 s after the load, to show the ads that got through. After the last, it gives
# the memory of every process of the browser's (check/usage.py), and off the Mac the most the
# browser's process took, where the native engine runs, and uBlock Origin's. Where engine is off,
# as adblock-cost's second pass is, Settings' Block ads and trackers is turned off before the
# first site, and the probe waits for the ad script to load rather than be blocked. With
# ADBLOCK_PARITY_BASELINE naming the sites.tsv an earlier run left, it fails where this run is worse:
#   - a site that loaded there and doesn't here;
#   - a host blocked on a site there that this run let through on that site. Where the baseline
#     both blocked and let through a host's requests on the site, the host alone can't say, so
#     its blocked addresses, host and path, are held to this run's instead (#2111);
#   - 3 or more elements' selectors on a site, or 10 in the run, whose every element the baseline's
#     run hid and some of which this run shows, as an ad's box the engine's hiding missed. Each
#     selector is an element's tag, id and classes, read 8 s after the load (cdp.py's hidden), and
#     doesn't count where the baseline's page hid it itself, by no rule of its blocker's style
#     sheet, as a video player its controls before it plays (#3268);
#   - a site this run breaks, or a wall uBlock Origin kept away: text under half the baseline's,
#     where two loads of a page with the same engine differ by a seventh at most, or 70% of the
#     viewport covered by an element fixed in place, a wall or a modal, where the baseline's page
#     had nothing as large. A site's own banner covers half at most, in every run so far; a wall
#     73% to all of it, as it draws in.
#     Both are read 8 s after the load (cdp.py's content), and the table says them, with the
#     page's pictures, which vary too much between loads to hold a run to, a site's count from 4
#     to 38. A watch page's text is as far as its player and comments got, so the video says
#     whether it works;
#   - fewer than 45 of the sites compared, as bot checks stood in or the baseline's table has no
#     row for them: a run that compared too few says nothing of the engine (#2111).
# A host or address neither run requested counts for neither, as the ads a site serves change from
# load to load. A site that's still Cloudflare's bot check 8 s after it loads gets one more go. If
# the check still stands in either run, DataDome's captcha loaded over the page, the site sent
# the run on to Google's unusual-traffic page, or the site's own wall stood in, a page that shows
# under 500 characters and at most one picture and loaded Turnstile's widget, which the run's
# blocker let through (#3383), the site isn't compared. The result names it and what stood in, as
# a bot check has no ads to block, and where only one of the two runs got a bot check, which. A
# Turnstile check is told by how little the page shows, as a page the blocker broke would be, so
# one in this run alone fails it, and a widget the blocker blocked makes none. On a YouTube watch
# page the run reads the player from the load on, to say whether an ad showed before the video
# played 5 s, and it fails where the baseline's played with no ad and this run's showed one or
# didn't play. It needs the network and the sites, so it runs only when named:
# ./check.sh adblock-parity.
# Both engines block with one copy of each list, downloaded once by the baseline's run, or with
# ADBLOCK_PARITY_LISTS naming the lists/ another run left, that copy: a Check dispatched with
# lists_from takes an earlier run's, so the three OSes' runs block with one snapshot (#2111).
# It leaves adblock-parity/sites.tsv, a capture a site, the requests recorded, in hidden/ what
# each site hid, and in lists/ the lists it blocked with, which a run with uBlock Origin copies
# beside its table for the next to compare.
engine=${engine:-ublock}

serve
# The probe's title says whether Google's ad script loaded, was blocked, or was blocked by uBlock
# Origin's stand-in, whose adsbygoogle.push does nothing, as adblock-live's does.
printf '%s' '<title>loading</title>
<script>
function loaded() {
  const push = String(self.adsbygoogle && self.adsbygoogle.push);
  document.title = /^function *\(\) *\{\s*;\s*\}$/.test(push) ?
    "ad script blocked by a stand-in" : "ad script loaded";
}
</script>
<script src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js" onload="loaded()"
  onerror="document.title = &quot;ad script blocked&quot;"></script>' > "$out/site/parity.html"
dir="$out/adblock-parity"
log="$out/adblock-parity.log"
[ "$engine" = ublock ] || dir="$out/adblock-$engine" log="$out/adblock-$engine.log"
rm -rf "$dir"
mkdir -p "$dir/profile"
cdp_log="$dir/cdp.log"
requests="$dir/requests.jsonl"
starts="$dir/starts.txt"
step=0
ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
scratch=$(mktemp -d)
engine_switch=
case $engine in ublock | off) ;; *) engine_switch=--ad-block-engine=$engine ;; esac
# Both engines block with the same copy of each list, or a list's authors' edits between the two
# runs read as one engine's miss: foxnews let rockspreading.com through on the Mac, which only that
# day's EasyPrivacy named (#2111). The baseline's run downloads each list it starts with once, into
# lists/ beside its table, and a run held to it takes those: uBlock Origin from the check's server,
# through --ad-block-lists, and the browser's own engine as the copies it downloaded itself, too
# new to download again.
if [ -d "$app/ublock" ]; then
  ublock="$app/ublock"
else
  ublock="$(echo "$app"/Contents/Frameworks/*Framework.framework)/Resources/ublock"
fi
lists="$dir/lists"
# Lists named and not there fail the run: a download in their place would be another snapshot.
held_lists=${ADBLOCK_PARITY_LISTS:-}
[ -z "$held_lists" ] || [ -s "$held_lists/snapshot.txt" ] ||
  fail "ADBLOCK_PARITY_LISTS names no lists another run left: no $held_lists/snapshot.txt"
[ -z "${ADBLOCK_PARITY_BASELINE:-}" ] || held_lists="${ADBLOCK_PARITY_BASELINE%sites.tsv}lists"
if [ -n "$held_lists" ] && [ -s "$held_lists/snapshot.txt" ]; then
  cp -R "$held_lists" "$lists"
else
  python3 -I - "$ublock/assets/assets.json" "$lists" > "$scratch/lists.txt" 2>&1 << 'PY' ||
import json, os, sys, time, urllib.request

assets_json, folder = sys.argv[1:]
os.makedirs(folder)
# uBlock Origin's file is UTF-8, which Windows' Python doesn't read by default (#872).
with open(assets_json, encoding="utf-8") as f:
    assets = json.load(f)
kept = []
for key, entry in assets.items():
    if entry.get("content") != "filters" or entry.get("off"):
        continue
    urls = entry.get("contentURL", [])
    urls = entry.get("cdnURLs", []) + ([urls] if isinstance(urls, str) else urls)
    for url in (u for u in urls if u.startswith("https:")):
        try:
            body = urllib.request.urlopen(url, timeout=30).read()
        except OSError:
            continue
        # A server's page about an error isn't a list, as the core's own download checks.
        if body.lstrip()[:1] not in (b"", b"<"):
            with open(os.path.join(folder, key + ".txt"), "wb") as f:
                f.write(body)
            kept.append(key)
            break
# Lines end in \n alone: Windows' Python would end them in \r\n, and a list's name that keeps its
# \r in the loop below names no file to copy.
with open(os.path.join(folder, "snapshot.txt"), "w", newline="\n") as f:
    f.write(time.strftime("%Y-%m-%dT%H:%M:%SZ\n", time.gmtime()) + "\n".join(kept) + "\n")
print(f"{len(kept)} lists downloaded: {', '.join(kept)}")
PY
    fail "the lists didn't download for both engines to share: $(tail -3 "$scratch/lists.txt")"
fi
[ "$(sed 1d "$lists/snapshot.txt" | grep -c .)" -gt 0 ] ||
  fail "no list downloaded for both engines to share: $(cat "$scratch/lists.txt")"
mkdir -p "$out/site/parity-lists" "$dir/profile/Ad Blocking/Lists"
python3 -I - "$ublock/assets/assets.json" "$lists" "${site}parity-lists/" \
  > "$out/site/parity-lists/assets.json" << 'PY' || fail "the lists' assets.json wasn't written"
import json, os, sys

assets_json, folder, served = sys.argv[1:]
with open(assets_json, encoding="utf-8") as f:
    assets = json.load(f)
with open(os.path.join(folder, "snapshot.txt")) as f:
    kept = set(f.read().split()[1:])
# Every list the snapshot has only from the check's server, as uBlock Origin would block with the
# copy it comes with first and the newer one only once it updates, partway through the sites; the
# rest only from the copy it comes with. Nothing it blocks with comes from the network.
for key, entry in assets.items():
    urls = entry.get("contentURL", [])
    local = [u for u in ([urls] if isinstance(urls, str) else urls)
             if not u.startswith(("http:", "https:"))]
    entry.pop("cdnURLs", None)
    entry["contentURL"] = [served + key + ".txt"] if key in kept else local
json.dump(assets, sys.stdout, indent=1)
PY
for list in $(sed 1d "$lists/snapshot.txt"); do
  cp "$lists/$list.txt" "$out/site/parity-lists/"
  # Copied now, so younger than any list's updateAfter: the core's update finds none due.
  cp "$lists/$list.txt" "$dir/profile/Ad Blocking/Lists/"
done
# Its own switch: a set engine_switch is what says this run asked for an engine of the browser's.
lists_switch="--ad-block-lists=${site}parity-lists/assets.json"
# A site's ask to show notifications or the like would hold the keyboard from the next ⌘L, so
# Chrome answers each with no; without uBlock Origin's lists, sites' scripts ask (nypost.com).
start_with_cdp "${site}parity.html" "$PWD/$dir/profile" /parity.html --deny-permission-prompts $engine_switch "$lists_switch"
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab ad script " || fail "the probe didn't say whether the ad script loaded"
send click $page_point
if [ "$active" != 1 ]; then
  # Quits before cdp.py's pipe closes: the browser ends by itself as the pipe goes, and a quit
  # asked then finds it ending, which the Mac answers with an error (#3870).
  quit
  exec 5>&-
  echo "skip: adblock-parity. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
if [ "$engine" = off ]; then
  # Off everywhere, as Settings › Privacy's Ad Blocking switch turns it off, and the probe's ad
  # script loads.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  send clickkey $(drawn "toolbar Security & Privacy")
  expect '^browser: settings tab Security & Privacy$' "a click on Security & Privacy didn't show its section"
  settings_drew "Security & Privacy"
  # Ad Blocking is under Privacy's own rows: in_sight scrolls it up, from this show's frame.
  in_sight "switch ad blocking" '^browser: settings drew Security & Privacy$' ||
    fail "Settings didn't draw Block ads and trackers' switch"
  send clickkey $at
  expect '^browser: ad blocking off$' "a click on Block ads and trackers' switch didn't turn it off"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  send click $page_point
  send key r cmd
  expect "^browser: title $tab ad script loaded\$" "turned off in Settings, the probe's ad script didn't load"
fi
# A new profile's lists block once they're compiled, so the probe reloads until they do, for up to
# 120 s, as adblock-live's does.
since=$(date +%s)
while [ "$engine" != off ] && ! grep -q "^browser: title $tab ad script blocked" "$log"; do
  [ $(($(date +%s) - since)) -lt 120 ] || fail "the lists didn't block Google's ad script within 120 s"
  send key r cmd
  expect "^browser: title $tab ad script " "the probe didn't report after ⌘R"
  for _ in $(seq 20); do
    none_since "^browser: title $tab ad script blocked" || break
    sleep 0.1
  done
done
# A build without the engine --ad-block-engine names (#2116) blocks with uBlock Origin, so the run
# would compare uBlock Origin with itself, where live sites alone make it look worse (#2284).
engine_log='^browser: ad block engine \(loaded\|compiled\) in '
if [ -n "$engine_switch" ] && ! grep -q "$engine_log" "$log"; then
  quit
  exec 5>&-
  rm -rf "$scratch"
  echo "skip all: no $engine engine in this build, so it blocked with uBlock Origin, which" \
    "adblock-parity measures"
  exit 0
fi
echo "requests $(app_path "$PWD/$requests")" >&5
for _ in $(seq 100); do
  grep -q '^cdp: recording requests' "$cdp_log" && break
  sleep 0.1
done
grep -q '^cdp: recording requests' "$cdp_log" ||
  fail "DevTools didn't record the requests: cdp.py said $(tail -1 "$cdp_log")"
# What the ad blocker hides on the page now, in $hides, a file a site, as cdp.py's hidden writes
# it; and, where the baseline's run left its own beside its table, the selectors it hid that match
# shown elements now.
hides="$dir/hidden"
mkdir -p "$hides"
before_hides=
[ -z "${ADBLOCK_PARITY_BASELINE:-}" ] || before_hides="${ADBLOCK_PARITY_BASELINE%sites.tsv}hidden"
case $before_hides in '' | /*) ;; *) before_hides="$PWD/$before_hides" ;; esac
look_hidden() {
  name=$(printf %s "$1" | tr -c 'A-Za-z0-9' _).json
  before=
  [ ! -s "$before_hides/$name" ] || before=" $(app_path "$before_hides/$name")"
  asked=$(grep -c '^cdp: hidden ' "$cdp_log" || true)
  echo "hidden $(app_path "$PWD/$hides/$name")$before" >&5
  # Telling the blocker's hiding from the page's asks DevTools about elements, a few at a time,
  # until each is told or 40 s have gone, and its last few may take 4 s more.
  for _ in $(seq 600); do
    if [ "$(grep -c '^cdp: hidden ' "$cdp_log" || true)" -gt "$asked" ]; then
      # Where the 40 s ended first, so a site drifting over it shows before it turns a run red,
      # and where the page removed elements before they were told apart. Either way they count as
      # the blocker's.
      set -- "$1" $(grep '^cdp: hidden ' "$cdp_log" | tail -1 |
        sed -n 's/.* untold \([0-9]*\) \([0-9]*\) gone \([0-9]*\) in .*/\1 \2 \3/p')
      if [ $# = 4 ] && [ "$2" != 0 ]; then
        if [ "$4" = "$2" ]; then
          why="the page removed them before they were told apart"
        elif [ "$4" = 0 ]; then
          why="they weren't told apart in the 40 s"
        else
          why="the page removed $4 before they were told apart, and the rest weren't in the 40 s"
        fi
        echo "$run: $1: $2 hidden elements, of $3 selectors, count as the blocker's, as $why"
      fi
      return
    fi
    sleep 0.1
  done
  fail "cdp.py didn't say what $1 hid: it said $(tail -1 "$cdp_log")"
}
# What cdp.py says of the first video in the page at an address starting with $1: its seconds
# played, playing or paused, ad or no-ad, and refused or -, or none.
video() {
  asked=$(grep -c '^cdp: video ' "$cdp_log" || true)
  echo "video $1" >&5
  for _ in $(seq 50); do
    [ "$(grep -c '^cdp: video ' "$cdp_log" || true)" -gt "$asked" ] && break
    sleep 0.1
  done
  grep '^cdp: video ' "$cdp_log" | tail -1 | cut -d' ' -f3-
}
# A site's line in $contents: the site, then what cdp.py's content says the page shows 8 s after its
# load: its text's characters, its pictures, and the percent of the viewport something fixed in
# place covers.
contents="$dir/contents.txt"
: > "$contents"
look_content() {
  asked=$(grep -c '^cdp: content ' "$cdp_log" || true)
  echo content >&5
  for _ in $(seq 50); do
    [ "$(grep -c '^cdp: content ' "$cdp_log" || true)" -gt "$asked" ] && break
    sleep 0.1
  done
  [ "$(grep -c '^cdp: content ' "$cdp_log" || true)" -gt "$asked" ] ||
    fail "cdp.py didn't say what $1 shows: it said $(tail -1 "$cdp_log")"
  echo "$1 $(grep '^cdp: content ' "$cdp_log" | tail -1 | cut -d' ' -f3-)" >> "$contents"
}
# A watch page's line in $videos: the site, "ad" if its player showed an ad or "no-ad", the
# seconds its video played, or none for no video, as YouTube's bot check has none, then what got
# it playing. An ad plays before the video does, so the player is read each half second from the
# load until the video has played 5 s, or for 30 s. Chrome lets a page start a video with sound
# only once the user has acted on it, so a video whose start Chrome refused, as YouTube's player
# says, gets k, YouTube's play key, once, as a user's press of play, and so does a cued video,
# which waits for that press (#3272). Before the player has asked to play, the key would start the
# video, and the player's own start would then pause it. What got it playing is "refused" or
# "cued", which got the press, or else the player's state when last read, as cdp.py says it,
# with why that reading wasn't pressed: only printed, as nothing compares it (#3272).
videos="$dir/videos.txt"
: > "$videos"
watch_video() {
  # Not site, the check's server, which a later pass in the same shell builds its pages on.
  watched=$1 ad=no-ad played=none met= state=- why= since=$(date +%s)
  while [ $(($(date +%s) - since)) -lt 30 ]; do
    # The seconds, playing or paused, ad or no-ad, then refused, cued or the player's state, and
    # after a state why it isn't refused.
    set -- $(video https://www.youtube.com/watch)
    if [ $# -ge 4 ]; then
      state=$4
      why=$(echo "$*" | cut -d' ' -f5-)
      case $4 in
        refused | cued)
          if [ -z "$met" ]; then
            send key k
            met=$4
          fi
          ;;
      esac
      if [ "$3" = ad ]; then
        ad=ad
      else
        played=$1
        [ "${played%.*}" -lt 5 ] || break
      fi
    fi
    sleep 0.5
  done
  if [ -n "$met" ]; then why=; fi
  echo "$watched $ad $played ${met:-$state} $why" >> "$videos"
}
# Each site: when Return went, in ms since the epoch, then the site, then the load's time and what
# loaded, or "none" for a load that didn't come within 45 s, then "bot-check" where Cloudflare's
# check stood in for the page.
: > "$starts"
n=0
# The sites come on fd 4, as the steps' commands may read their standard input.
while read -r url <&4; do
  case $url in '' | '#'*) continue ;; esac
  n=$((n + 1))
  first=
  for go in 1 2; do
    send key l cmd
    expect '^browser: command bar shown$' "⌘L didn't open the command bar"
    send type "$url"
    started=$(ms)
    first=${first:-$started}
    send key return
    loaded=none
    for _ in $(seq 450); do
      line=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 "^browser: loaded $tab ") && {
        loaded="$(($(ms) - started)) ${line#browser: loaded $tab }"
        break
      }
      kill -0 "$pid" 2> /dev/null || gone
      sleep 0.1
    done
    # adblock-cost, which has no time for it, gives no site the look a watch page gets.
    case $url in
      https://www.youtube.com/watch*) [ -n "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || watch_video "$url" ;;
    esac
    # Ads load after the page does, from its scripts. adblock-cost, whose two passes have to fit a
    # run's 20 minutes on Windows' Check, needs no pictures, so it waits only after the last site,
    # and gives a bot check no second go.
    [ -n "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || sleep 8
    # A page can send itself on once it has loaded, as dailymail.co.uk does to dailymail.com,
    # which then loads itself again (#3309). DevTools asked about the page meanwhile waits on the
    # page being replaced, and the next site would take its last load for its own. So the site's
    # page is the one its tab ends on: a tab loading again is waited on to its last load, within
    # the same 45 s from Return. The load's time stays the first's, the one the user waited on.
    if [ "$loaded" != none ] && [ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ]; then
      while sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: page $tab " | tail -1 |
        grep -q ' loading 1 '; do
        [ $(($(ms) - started)) -lt 45000 ] || fail "$url loaded (${loaded#* }) and was loading again \
45 s after Return: $(sed -n "/^browser: drive $step: /,\$p" "$log" |
          grep "^browser: \(page\|loaded\) $tab " | tail -3 | tr '\n' ';')"
        kill -0 "$pid" 2> /dev/null || gone
        sleep 0.1
      done
      line=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep "^browser: loaded $tab " | tail -1)
      loaded="${loaded%% *} ${line#browser: loaded $tab }"
    fi
    # The tab's title now: its last line, as the browser logs a title only when it changes, so a
    # second go to the same check logs none.
    title=$(grep "^browser: title $tab " "$log" | tail -1)
    case ${title#browser: title $tab } in
      'Just a moment'*) bot=" bot-check" ;;
      *) bot= ;;
    esac
    [ -n "$bot" ] && [ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || break
  done
  # The requests of both goes are the site's, from the first's Return on.
  echo "$first $url $loaded$bot" >> "$starts"
  [ -n "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] && continue
  look_hidden "$url"
  look_content "$url"
  ../tools/window-screenshot.sh "$pid" "$dir/$(printf %02d "$n").png" ||
    fail "no window to capture"
done 4< check/adblock-sites.txt
# Without pictures, the last site's ads, which load after its page does, get the 8 s here alone.
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || sleep 8
[ -s "$starts" ] || fail "no site opened"
python3 -I check/usage.py snap "$procs" "$dir/memory.json" kinds || fail "the browser's memory didn't read"
summed=$(python3 -I check/usage.py memory "$dir/memory.json")
memory=". Its processes held ${summed% *} MiB, ${summed#* } processes ($(python3 -I check/usage.py kinds "$dir/memory.json"))"
echo "${summed% *}" > "$dir/memory.txt"
# The most the browser's process and uBlock Origin's took, from the same snapshot, which finds
# them by their own ids on each OS: Windows' Get-Process takes no id the shell gives (#3315).
if ! [ "$(uname)" = Darwin ]; then
  peak=$(python3 -I check/usage.py peak "$dir/memory.json" browser)
  if [ -n "$peak" ]; then
    memory="$memory; the browser's process at most $peak MiB"
  else
    memory="$memory; the browser's process's peak wasn't read"
  fi
  # An extension's process is uBlock Origin's only in its own pass.
  owner="an extension's"
  [ "$engine" != ublock ] || owner="uBlock Origin's"
  peak=$(python3 -I check/usage.py peak "$dir/memory.json" extension)
  [ -z "$peak" ] || memory="$memory, $owner at most $peak MiB"
fi
quit
exec 5>&-
rm -rf "$scratch"
[ -s "$requests" ] || fail "DevTools recorded no requests"
# What's wrong, if anything, as a here-document in a command substitution trips the Mac's bash 3.2.
result="$dir/result.txt"
# Whether each site's hidden elements were read: not under ADBLOCK_PARITY_NO_CAPTURE, as
# adblock-cost's passes skip the look with the pictures (#3928).
looked=1
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || looked=
python3 -I - "$requests" "$starts" "$videos" "$hides" "$contents" "$dir/sites.tsv" "${ADBLOCK_PARITY_BASELINE:-}" "$looked" > "$result" << 'PY' || fail "$(cat "$result")"

import json, os, re, sys
from urllib.parse import urlsplit

recorded, starts, watched, hides, shown, table, baseline, looked = sys.argv[1:]
# Each request: its address, when it went, and whether the ad blocker blocked it or answered it
# with a stand-in, an extension's resource it redirected the request to. Its later events come by
# its id, which a later request can take again, once a renderer's process id comes back. An id sent
# again as a redirect to an address not http(s) is a stand-in; sent again any other way, it's a
# later request's, as a later page's data: one is, and its answer isn't the first's (#3288).
requests = []
live = {}
for line in open(recorded, encoding="utf-8"):
    event = json.loads(line)
    request = live.get(event["id"])
    if event["event"] == "sent" and event["url"].startswith("http") and not event.get("redirect"):
        live[event["id"]] = {"url": event["url"], "at": event["at"], "how": "allowed",
                             "document": event.get("document", False)}
        requests.append(live[event["id"]])
    elif not request:
        continue
    elif event["event"] == "sent" and not event.get("redirect"):
        del live[event["id"]]
    elif event["event"] == "sent" and not event["url"].startswith("http"):
        request["how"] = "stand-in"
    elif event["event"] == "sent":
        # Where a redirect sent it, for the page's own request that fails before any answer.
        request["to"] = event["url"]
    elif event["event"] == "document":
        request["status"], request["to"] = event["status"], event["url"]
    elif event["event"] == "failed":
        request["error"] = event["error"]
        if "BLOCKED_BY_CLIENT" in event["error"]:
            request["how"] = "blocked"
    elif event["event"] == "answered" and request and request["how"] == "allowed":
        request["how"] = "stand-in"

sites = []
for line in open(starts, encoding="utf-8"):
    started, url, load_ms, *rest = line.rstrip("\n").split(" ")
    landed = rest[0] if rest and load_ms != "none" else ""
    ok = load_ms != "none" and not landed.startswith("chrome-error:")
    # What stood in for the site's page, where a bot check did.
    at = urlsplit(landed)
    bot = ("Cloudflare's bot check" if "bot-check" in rest else
           "Google's unusual-traffic page" if (at.hostname or "").endswith("google.com")
           and at.path.startswith("/sorry/") else "")
    sites.append({"at": int(started), "url": url, "loaded": ok, "load_ms": load_ms if ok else "",
                  "bot": bot})
# A watch page's video: "ad" or "no-ad", the seconds it played or none, then what got it playing.
videos = {}
for line in open(watched, encoding="utf-8"):
    url, ad, played, met, *why = line.split()
    videos[url] = {"ad": ad == "ad", "played": None if played == "none" else float(played),
                   "met": met, "why": why}

# What a site's page showed: its text's characters, its pictures, and whether it was covered.
contents = {}
for line in open(shown, encoding="utf-8"):
    url, *said = line.split()
    if len(said) == 3:
        contents[url] = {"text": int(said[0]), "images": int(said[1]), "cover": int(said[2])}


def plays(video):
    return bool(video) and not video["ad"] and (video["played"] or 0) >= 5


# YouTube's player states, as its getPlayerState gives them.
STATES = {"-1": "unstarted", "0": "ended", "1": "playing", "2": "paused", "3": "buffering",
          "-": "unknown"}


def video_said(video):
    if video["played"] is None:
        return "had no video"
    said = f"played {video['played']:.1f} s" + (" after an ad" if video["ad"] else " with no ad")
    # Which the run met, for #3272: a baseline's table doesn't say.
    met = video.get("met")
    if met in ("refused", "cued"):
        return f"{said}, {met} and pressed play"
    if met and plays(video):
        return f"{said}, by itself with no press"
    if met:
        said = f"{said}, neither refused nor cued, so not pressed, its state {STATES.get(met, met)}"
        return said + why_said(video.get("why"))
    return said


# Why a reading wasn't pressed, as cdp.py's words after the state give it (#3272): whether the
# player asked to play, whether it still says unstarted, the video's readyState, and the page.
def why_said(why):
    if not why or len(why) != 6:
        return ""
    asked, unstarted, _, data, visibility, focus = why
    return (f", asked to play {'yes' if asked == 'asked' else 'no'}, unstarted "
            f"{'yes' if unstarted == 'unstarted' else 'no'}, data {data}, page {visibility}, {focus}")


def why_not(mine):
    """Why a site's page didn't load, by its own last request: a refusal from the site's server, as
    a rate limit or a bot wall sends, isn't the engine's, but stays a red all the same (#3347)."""
    pages = [r for r in mine if r["document"]]
    if not pages:
        return "it never asked for its page"
    page = pages[-1]
    # The address it ended at, without its query, which can carry a visit's own.
    parts = urlsplit(page.get("to", page["url"]))
    at = f"{parts.scheme}://{parts.netloc}{parts.path}"
    status, error = page.get("status"), page.get("error", "")
    if "BLOCKED_BY_CLIENT" in error:
        return f"the blocker blocked its page at {at}"
    if status and status >= 400:
        return f"the site refused it, HTTP {status} at {at}"
    if status:
        then = f" then failed with {error}" if error else ""
        return f"its page answered HTTP {status} at {at}{then}"
    if error:
        return f"it failed with {error} before any answer at {at}"
    return f"its page at {at} had no answer"


TURNSTILE = "a Turnstile check"
for i, s in enumerate(sites):
    s["video"] = videos.get(s["url"])
    end = sites[i + 1]["at"] if i + 1 < len(sites) else float("inf")
    mine = [r for r in requests if s["at"] <= r["at"] < end]
    host = lambda r: urlsplit(r["url"]).hostname or "?"
    s["requests"] = len(mine)
    s["blocked"] = sorted({host(r) for r in mine if r["how"] != "allowed"})
    s["allowed"] = sorted({host(r) for r in mine if r["how"] == "allowed"})
    # Addresses, host and path, as a query changes from load to load.
    address = lambda r: host(r) + urlsplit(r["url"]).path
    s["blocked_at"] = {address(r) for r in mine if r["how"] != "allowed"}
    s["allowed_at"] = {address(r) for r in mine if r["how"] == "allowed"}
    # Where the run both blocked and let through a host's requests, those it blocked.
    s["split"] = sorted(a for a in s["blocked_at"] if a.split("/")[0] in s["allowed"])
    s["blocked_n"] = sum(r["how"] == "blocked" for r in mine)
    s["failed"] = "" if s["loaded"] else why_not(mine)
    s["standin_n"] = sum(r["how"] == "stand-in" for r in mine)
    # What the baseline hid all of and this run shows, as cdp.py's hidden says, or None where
    # there's no saying: no baseline's file for the site, or no look at it.
    hid = os.path.join(hides, re.sub("[^A-Za-z0-9]", "_", s["url"]) + ".json")
    look = json.load(open(hid, encoding="utf-8")) if os.path.exists(hid) else {}
    s["missed"] = sorted(look["missed"]) if baseline and look else None
    # The selectors the blocker's own style sheet hid and those the page hid itself, as cdp.py's
    # hidden tells them apart (#3268).
    s["blocker_hid"] = len(look.get("blocker", []))
    s["page_hid"] = len(look.get("page", []))
    s["blinks_unread"] = look.get("blinks_unread")
    # DataDome's captcha, which a site shows a runner it takes for a bot, over the page in place:
    # a bot check as Cloudflare's is, with no title to tell it by (#2111).
    if not s["bot"] and any(h.endswith("captcha-delivery.com") for h in s["allowed"]):
        s["bot"] = "DataDome's captcha"
    # A site's own wall built on Cloudflare's Turnstile, as vimeo's "Verify to continue" is, which
    # keeps the site's title. Real pages load Turnstile too, so it takes a page that is little
    # but the wall: no real page of the fifty comes near so little (#3383). Only a widget the
    # blocker let through: one it blocked, the user couldn't pass either.
    shows = contents.get(s["url"])
    if (not s["bot"] and shows and shows["text"] < 500 and shows["images"] <= 1 and
            any(a.startswith("challenges.cloudflare.com/turnstile/") for a in s["allowed_at"])):
        s["bot"] = TURNSTILE

def video_cell(video):
    if not video:
        return ""
    played = "none" if video["played"] is None else f"{video['played']:.1f}"
    return ("ad " if video["ad"] else "no-ad ") + played


def video_of(cell):
    ad, _, played = cell.partition(" ")
    return {"ad": ad == "ad", "played": None if played in ("", "none") else float(played)} if cell else None


def content_cells(content):
    if not content:
        return ["", "", ""]
    return [str(content["text"]), str(content["images"]), str(content["cover"])]


with open(table, "w", encoding="utf-8") as out:
    out.write("site\tloaded\tload_ms\trequests\tblocked\tstand_ins\tblocked_hosts\tallowed_hosts"
              "\tbot_check\tvideo\tmissed\tsplit_blocked\ttext\timages\tcover\tfailed\n")
    for s in sites:
        out.write("\t".join([s["url"], "1" if s["loaded"] else "0", s["load_ms"], str(s["requests"]),
                             str(s["blocked_n"]), str(s["standin_n"]), ",".join(s["blocked"]),
                             ",".join(s["allowed"]), "1" if s["bot"] else "0",
                             video_cell(s["video"]),
                             "" if s["missed"] is None else str(len(s["missed"])),
                             " ".join(s["split"])]
                            + content_cells(contents.get(s["url"])) + [s["failed"]]) + "\n")

worse = []
refused = []
site_missed = False
# A site that was a bot check in this run or the baseline's, which an older table
# doesn't say, with what stood in for its page.
unchecked = {s["url"]: s["bot"] for s in sites if s["bot"]}
if baseline:
    lines = [line.rstrip("\n").split("\t") for line in open(baseline, encoding="utf-8")]
    rows = lines[1:]
    before = {r[0]: r for r in rows}
    # By the header's name, as an older table has no video column.
    named = {r[0]: dict(zip(lines[0], r)) for r in rows}
    for s in sites:
        b = before.get(s["url"])
        if not b:
            continue
        # Where only one run got a bot check, which one, so a page this run's blocker emptied
        # can't pass as one.
        was_bot = len(b) > 8 and b[8] == "1"
        if was_bot and not s["bot"]:
            unchecked[s["url"]] = "a bot check in the baseline's run alone; this run had none"
        elif s["bot"] == TURNSTILE and not was_bot:
            # Told by how little the page shows, which is what a page this run's blocker broke
            # looks like, so it's never excused: a real one is an issue to file for the site.
            del unchecked[s["url"]]
            page_was, page = named[s["url"]], contents[s["url"]]
            pictures = "one picture" if page["images"] else "no picture"
            worse.append(f"{s['url']} showed a Turnstile check in this run alone, with "
                         f"{page['text']} characters of text and {pictures}, "
                         f"where the baseline's run had none and showed "
                         f"{page_was.get('text') or 'no count of'} characters and "
                         f"{page_was.get('images') or 'no count of'} pictures")
            continue
        elif s["bot"] and not was_bot:
            unchecked[s["url"]] = f"{s['bot']} in this run alone; the baseline's run had none"
        if s["url"] in unchecked:
            continue
        if b[1] == "1" and not s["loaded"]:
            # The rest of a page that didn't load says nothing more.
            worse.append(f"{s['url']} loaded with the baseline and doesn't now: {s['failed']}")
            continue
        if b[1] == "0" and s["loaded"] and named[s["url"]].get("failed"):
            # The same blindness the other way round: the baseline has nothing of the site's to
            # hold this run to.
            refused.append(f"{s['url']} loads now, where the baseline's didn't: "
                           f"{named[s['url']]['failed']}")
        was = video_of(named[s["url"]].get("video", ""))
        if plays(was) and s["video"] and s["video"]["ad"]:
            worse.append(f"{s['url']} showed an ad before its video, where the baseline's "
                         f"{video_said(was)}")
        elif plays(was) and not plays(s["video"]):
            now = video_said(s["video"]) if s["video"] else "wasn't watched"
            worse.append(f"{s['url']}'s video {now}, where the baseline's {video_said(was)}")
        through = set(b[6].split(",")) & set(s["allowed"]) - set(s["blocked"]) - {""}
        # A table from before split_blocked holds every host to the host rule.
        if "split_blocked" in named[s["url"]]:
            split = set(named[s["url"]]["split_blocked"].split())
            through -= {a.split("/")[0] for a in split}
            through |= split & s["allowed_at"] - s["blocked_at"]
        through = sorted(through)
        # What the page showed, against the baseline's, where both runs read it.
        page_was, page = named[s["url"]], contents.get(s["url"])
        if page and page_was.get("text"):
            # A watch page's text is as far as its player and comments got, and its video says
            # whether it works.
            if not s["video"] and page["text"] < int(page_was["text"]) / 2:
                worse.append(f"{s['url']} showed {page['text']} characters of text, where the "
                             f"baseline's showed {page_was['text']}")
            if page["cover"] >= 70 > int(page_was["cover"]):
                worse.append(f"{s['url']} was {page['cover']}% covered by something fixed in place, "
                             f"where the baseline's was {page_was['cover']}%")
        if through:
            worse.append(f"{s['url']} let through what the baseline blocked: {', '.join(through)}")
        # uBlock Origin against itself shows at most 2 of these a site, from a page that changes
        # between loads, as a menu button drawn for one width; an engine that hides nothing, 3 to 19
        # on half the sites.
        if s["missed"] and len(s["missed"]) >= 3:
            site_missed = True
            worse.append(f"{s['url']} showed what the baseline hid all of: {', '.join(s['missed'])}")
# A run held to a baseline that compared too few of the sites isn't a gate: each bot check is a
# site neither run was shown no worse on (#2111).
if baseline:
    compared = [s for s in sites if s["url"] in before and s["url"] not in unchecked]
    if len(compared) < 45:
        worse.append(f"only {len(compared)} of {len(sites)} sites were compared, under the floor of 45; "
                     + ", ".join([f"{url} ({what})" for url, what in unchecked.items()]
                                 + [f"{s['url']} (no row in the baseline's table)"
                                    for s in sites if s["url"] not in before]))
# Misses too few a site to fail one still add up, where an engine misses a little everywhere:
# uBlock Origin against itself shows at most 3 in a run.
missed = [(s["url"], m) for s in sites if s["url"] not in unchecked for m in s["missed"] or []]
if len(missed) >= 10 and not site_missed:
    worse.append(f"{len(missed)} selectors the baseline hid all of showed: "
                 + ", ".join(f"{m} on {url}" for url, m in missed))
loaded = [s for s in sites if s["loaded"]]
# Each blocker hides something on these sites. None hidden, where they loaded, is the telling
# apart broken, which would leave the gate blind to every element an engine fails to hide. A run
# that didn't read them has nothing to say of it.
blocker_hid = sum(s["blocker_hid"] for s in sites)
if looked and loaded and not blocker_hid:
    worse.append("no site had an element the blocker's style sheet hid, by DevTools' matched "
                 "styles: cdp.py's hidden can't tell the blocker's hiding from the page's")
# A Chromium whose own hiding rules haven't been read would count a new one as the blocker's.
unread = sorted({s["blinks_unread"] for s in sites if s["blinks_unread"]})
if unread:
    worse.append(f"the browser is {', '.join(unread)}, and Blink's own display: none !important "
                 "rules were read only for the majors in cdp.py's BLINKS_READ: read its style "
                 "sheets as the comment over BLINKS says, or a rule of Blink's counts as the "
                 "blocker's")
times = sorted(int(s["load_ms"]) for s in loaded)
median = times[len(times) // 2] if times else 0
# The worse sites go first, on one line, since the run's FAIL line, the summary's row and a
# quarantine line's match read only the first.
if worse:
    print("worse than the baseline: " + "; ".join(worse))
print(f"{len(loaded)} of {len(sites)} sites loaded, in {median} ms at the median; "
      f"{sum(s['blocked_n'] for s in sites)} requests blocked and "
      f"{sum(s['standin_n'] for s in sites)} given a stand-in, of {sum(s['requests'] for s in sites)}"
      + (f"; {blocker_hid} selectors hidden by the blocker's style sheet and "
         f"{sum(s['page_hid'] for s in sites)} by the page's own" if looked
         else "; hidden elements not read")
      + (f"; {len(missed)} selectors the baseline hid all of showed" if baseline else "")
      + "".join(f"; {s['url']}'s video {video_said(s['video'])}" for s in sites
                if s["video"] and s["url"] not in unchecked)
      + (f"; not compared, as a bot check stood in: "
         + ", ".join(f"{url} ({what})" for url, what in unchecked.items()) if unchecked else "")
      + "".join(f"; {r}" for r in refused))
if worse:
    sys.exit(1)
PY
echo "$run: $(cat "$result")$memory. Per site: $dir/sites.tsv"
# uBlock Origin's, for adblock-native's to compare with in the same check, or a later one that
# names it in ADBLOCK_PARITY_BASELINE.
if [ "$engine" = ublock ]; then
  cp "$dir/sites.tsv" build/adblock-parity-sites.tsv
  rm -rf build/adblock-parity-hidden build/adblock-parity-lists
  cp -R "$hides" build/adblock-parity-hidden
  cp -R "$lists" build/adblock-parity-lists
fi
captured="with the window captured 8 s after each site loaded in $dir"
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || captured="its sites in $dir"
echo "ok: $run, ${ADBLOCK_PARITY_BASELINE:+no worse than $ADBLOCK_PARITY_BASELINE, }on the lists downloaded at $(head -1 "$lists/snapshot.txt"), $captured"
