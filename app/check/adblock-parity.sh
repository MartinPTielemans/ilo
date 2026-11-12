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
#   - a host blocked on a site there that this run let through on that site.
# A host neither run requested counts for neither, as the ads a site serves change from load to
# load. A site that's still Cloudflare's bot check 8 s after it loads gets one more go. If the check
# still stands in either run, the site isn't compared, and the result names it, as a bot check has
# no ads to block. On a YouTube watch page the run reads the player from the load on, to say
# whether an ad showed before the video played 5 s, and it fails where the baseline's played with
# no ad and this run's showed one or didn't play. It needs the network and the sites, so it runs
# only when named: ./check.sh adblock-parity.
# It leaves adblock-parity/sites.tsv, a capture a site, and the requests recorded.
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
# A site's ask to show notifications or the like would hold the keyboard from the next ⌘L, so
# Chrome answers each with no; without uBlock Origin's lists, sites' scripts ask (nypost.com).
start_with_cdp "${site}parity.html" "$PWD/$dir/profile" /parity.html --deny-permission-prompts $engine_switch
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab ad script " || fail "the probe didn't say whether the ad script loaded"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&- 5>&-
  quit
  echo "skip: adblock-parity. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
if [ "$engine" = off ]; then
  # Off everywhere, as Settings' Privacy switch turns it off, and the probe's ad script loads.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  send clickkey $(drawn "toolbar Privacy")
  expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
  settings_drew Privacy
  click_in_sight "switch ad blocking"
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
  exec 3>&- 5>&-
  quit
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
  for _ in $(seq 100); do
    [ "$(grep -c '^cdp: hidden ' "$cdp_log" || true)" -gt "$asked" ] && return
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
# A watch page's line in $videos: the site, "ad" if its player showed an ad or "no-ad", and the
# seconds its video played, or none for no video, as YouTube's bot check has none. An ad plays
# before the video does, so the player is read each half second from the load until the video
# has played 5 s, or for 30 s. Chrome lets a page start a video with sound only once the user has
# acted on it, so a video whose start Chrome refused, as YouTube's player says, gets k, YouTube's
# play key, once, as a user's press of play. Before the player has asked to play, the key would
# start the video, and the player's own start would then pause it.
videos="$dir/videos.txt"
: > "$videos"
watch_video() {
  site=$1 ad=no-ad played=none pressed= since=$(date +%s)
  while [ $(($(date +%s) - since)) -lt 30 ]; do
    # The seconds, playing or paused, ad or no-ad, refused or -.
    set -- $(video https://www.youtube.com/watch)
    if [ $# = 4 ]; then
      if [ "$4" = refused ] && [ -z "$pressed" ]; then
        send key k
        pressed=1
      fi
      if [ "$3" = ad ]; then
        ad=ad
      else
        played=$1
        [ "${played%.*}" -lt 5 ] || break
      fi
    fi
    sleep 0.5
  done
  echo "$site $ad $played" >> "$videos"
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
  ../tools/window-screenshot.sh "$pid" "$dir/$(printf %02d "$n").png" ||
    fail "no window to capture"
done 4< check/adblock-sites.txt
# Without pictures, the last site's ads, which load after its page does, get the 8 s here alone.
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || sleep 8
[ -s "$starts" ] || fail "no site opened"
# The most memory a process took, in MB.
peak() {
  if windows; then
    ID=$1 powershell.exe -NoProfile -NonInteractive -Command '$p = Get-Process -Id $env:ID;
      "$([math]::Round($p.PeakWorkingSet64 / 1MB))"' | tr -d '\r'
  else
    awk '/^VmHWM:/ { print int($2 / 1024) }' "/proc/$1/status"
  fi
}
python3 -I check/usage.py snap "$procs" "$dir/memory.json" kinds || fail "the browser's memory didn't read"
summed=$(python3 -I check/usage.py memory "$dir/memory.json")
memory=". Its processes held ${summed% *} MiB, ${summed#* } processes ($(python3 -I check/usage.py kinds "$dir/memory.json"))"
echo "${summed% *}" > "$dir/memory.txt"
if ! [ "$(uname)" = Darwin ]; then
  memory="$memory; the browser's process at most $(peak "$pid") MB"
  ublock=$(pgrep -f "$procs.*--extension-process" | head -1)
  [ -z "$ublock" ] || memory="$memory, uBlock Origin's at most $(peak "$ublock") MB"
fi
exec 3>&- 5>&-
quit
rm -rf "$scratch"
[ -s "$requests" ] || fail "DevTools recorded no requests"
# What's wrong, if anything, as a here-document in a command substitution trips the Mac's bash 3.2.
result="$dir/result.txt"
python3 -I - "$requests" "$starts" "$videos" "$dir/sites.tsv" "${ADBLOCK_PARITY_BASELINE:-}" > "$result" << 'PY' || fail "$(cat "$result")"

import json, sys
from urllib.parse import urlsplit

recorded, starts, watched, table, baseline = sys.argv[1:]
# Each request: its address, when it went, and whether the ad blocker blocked it or answered it
# with a stand-in, an extension's resource it redirected the request to. Its later events come by
# its id, which a later request can take again, once a renderer's process id comes back: an id sent
# again to an http(s) address, not as a redirect, is a new request.
requests = []
live = {}
for line in open(recorded, encoding="utf-8"):
    event = json.loads(line)
    request = live.get(event["id"])
    if event["event"] == "sent" and event["url"].startswith("http") and not event.get("redirect"):
        live[event["id"]] = {"url": event["url"], "at": event["at"], "how": "allowed"}
        requests.append(live[event["id"]])
    elif not request:
        continue
    elif event["event"] == "sent" and not event["url"].startswith("http"):
        request["how"] = "stand-in"
    elif event["event"] == "failed" and request and "BLOCKED_BY_CLIENT" in event["error"]:
        request["how"] = "blocked"
    elif event["event"] == "answered" and request and request["how"] == "allowed":
        request["how"] = "stand-in"

sites = []
for line in open(starts, encoding="utf-8"):
    started, url, load_ms, *rest = line.rstrip("\n").split(" ")
    landed = rest[0] if rest and load_ms != "none" else ""
    ok = load_ms != "none" and not landed.startswith("chrome-error:")
    sites.append({"at": int(started), "url": url, "loaded": ok, "load_ms": load_ms if ok else "",
                  "bot": "bot-check" in rest})
# A watch page's video: "ad" or "no-ad", then the seconds it played or none.
videos = {}
for line in open(watched, encoding="utf-8"):
    url, ad, played = line.split()
    videos[url] = {"ad": ad == "ad", "played": None if played == "none" else float(played)}


def plays(video):
    return bool(video) and not video["ad"] and (video["played"] or 0) >= 5


def video_said(video):
    if video["played"] is None:
        return "had no video"
    return (f"played {video['played']:.1f} s" + (" after an ad" if video["ad"] else " with no ad"))


for i, s in enumerate(sites):
    s["video"] = videos.get(s["url"])
    end = sites[i + 1]["at"] if i + 1 < len(sites) else float("inf")
    mine = [r for r in requests if s["at"] <= r["at"] < end]
    host = lambda r: urlsplit(r["url"]).hostname or "?"
    s["requests"] = len(mine)
    s["blocked"] = sorted({host(r) for r in mine if r["how"] != "allowed"})
    s["allowed"] = sorted({host(r) for r in mine if r["how"] == "allowed"})
    s["blocked_n"] = sum(r["how"] == "blocked" for r in mine)
    s["standin_n"] = sum(r["how"] == "stand-in" for r in mine)

def video_cell(video):
    if not video:
        return ""
    played = "none" if video["played"] is None else f"{video['played']:.1f}"
    return ("ad " if video["ad"] else "no-ad ") + played


def video_of(cell):
    ad, _, played = cell.partition(" ")
    return {"ad": ad == "ad", "played": None if played in ("", "none") else float(played)} if cell else None


with open(table, "w", encoding="utf-8") as out:
    out.write("site\tloaded\tload_ms\trequests\tblocked\tstand_ins\tblocked_hosts\tallowed_hosts"
              "\tbot_check\tvideo\n")
    for s in sites:
        out.write("\t".join([s["url"], "1" if s["loaded"] else "0", s["load_ms"], str(s["requests"]),
                             str(s["blocked_n"]), str(s["standin_n"]), ",".join(s["blocked"]),
                             ",".join(s["allowed"]), "1" if s["bot"] else "0",
                             video_cell(s["video"])]) + "\n")

worse = []
# A site that was Cloudflare's bot check in this run or the baseline's, which an older table
# doesn't say.
unchecked = [s["url"] for s in sites if s["bot"]]
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
        if len(b) > 8 and b[8] == "1" and not s["bot"]:
            unchecked.append(s["url"])
        if s["url"] in unchecked:
            continue
        if b[1] == "1" and not s["loaded"]:
            worse.append(f"{s['url']} loaded with the baseline and doesn't now")
        was = video_of(named[s["url"]].get("video", ""))
        if plays(was) and s["video"] and s["video"]["ad"]:
            worse.append(f"{s['url']} showed an ad before its video, where the baseline's "
                         f"{video_said(was)}")
        elif plays(was) and not plays(s["video"]):
            now = video_said(s["video"]) if s["video"] else "wasn't watched"
            worse.append(f"{s['url']}'s video {now}, where the baseline's {video_said(was)}")
        through = sorted(set(b[6].split(",")) & set(s["allowed"]) - set(s["blocked"]) - {""})
        if through:
            worse.append(f"{s['url']} let through what the baseline blocked: {', '.join(through)}")
loaded = [s for s in sites if s["loaded"]]
times = sorted(int(s["load_ms"]) for s in loaded)
median = times[len(times) // 2] if times else 0
# The worse sites go first, on one line, since the run's FAIL line, the summary's row and a
# quarantine line's match read only the first.
if worse:
    print("worse than the baseline: " + "; ".join(worse))
print(f"{len(loaded)} of {len(sites)} sites loaded, in {median} ms at the median; "
      f"{sum(s['blocked_n'] for s in sites)} requests blocked and "
      f"{sum(s['standin_n'] for s in sites)} given a stand-in, of {sum(s['requests'] for s in sites)}"
      + "".join(f"; {s['url']}'s video {video_said(s['video'])}" for s in sites if s["video"])
      + (f"; not compared, as Cloudflare's bot check stood in: {', '.join(unchecked)}" if unchecked else ""))
if worse:
    sys.exit(1)
PY
echo "$run: $(cat "$result")$memory. Per site: $dir/sites.tsv"
# uBlock Origin's, for adblock-native's to compare with in a check of its own.
if [ "$engine" = ublock ]; then
  cp "$dir/sites.tsv" build/adblock-parity-sites.tsv
  rm -rf build/adblock-parity-hidden
  cp -R "$hides" build/adblock-parity-hidden
fi
captured="with the window captured 8 s after each site loaded in $dir"
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || captured="its sites in $dir"
echo "ok: $run, ${ADBLOCK_PARITY_BASELINE:+no worse than $ADBLOCK_PARITY_BASELINE, }$captured"
