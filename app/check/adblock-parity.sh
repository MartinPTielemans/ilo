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
# load. It needs the network and the sites, so it runs only when named: ./check.sh adblock-parity.
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
  # Off everywhere, as Settings' Ad Blocking switch turns it off, and the probe's ad script loads.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  send clickkey $(drawn "toolbar Ad Blocking")
  expect '^browser: settings tab Ad Blocking$' "a click on Ad Blocking didn't show its section"
  settings_drew "Ad Blocking"
  send clickkey $(drawn "switch ad blocking")
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
# Each site: when Return went, in ms since the epoch, then the site, then the load's time and what
# loaded, or "none" for a load that didn't come within 45 s.
: > "$starts"
n=0
# The sites come on fd 4, as the steps' commands may read their standard input.
while read -r url <&4; do
  case $url in '' | '#'*) continue ;; esac
  n=$((n + 1))
  send key l cmd
  expect '^browser: command bar shown$' "⌘L didn't open the command bar"
  send type "$url"
  started=$(ms)
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
  echo "$started $url $loaded" >> "$starts"
  # Ads load after the page does, from its scripts. adblock-cost, whose two passes have to fit a
  # run's 20 minutes on Windows' Check, needs no pictures, so it waits only after the last site.
  [ -n "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] && continue
  sleep 8
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
python3 -I - "$requests" "$starts" "$dir/sites.tsv" "${ADBLOCK_PARITY_BASELINE:-}" > "$result" << 'PY' || fail "$(cat "$result")"

import json, sys
from urllib.parse import urlsplit

recorded, starts, table, baseline = sys.argv[1:]
# Each request, by its id: its address, when it went, and whether the ad blocker blocked it or
# answered it with a stand-in, an extension's resource it redirected the request to.
requests = {}
for line in open(recorded, encoding="utf-8"):
    event = json.loads(line)
    request = requests.get(event["id"])
    if event["event"] == "sent" and not request:
        if event["url"].startswith("http"):
            requests[event["id"]] = {"url": event["url"], "at": event["at"], "how": "allowed"}
    elif event["event"] == "sent" and not event["url"].startswith("http"):
        request["how"] = "stand-in"
    elif event["event"] == "failed" and request and "BLOCKED_BY_CLIENT" in event["error"]:
        request["how"] = "blocked"
    elif event["event"] == "answered" and request and request["how"] == "allowed":
        request["how"] = "stand-in"

sites = []
for line in open(starts, encoding="utf-8"):
    started, url, rest = line.rstrip("\n").split(" ", 2)
    load_ms, _, landed = rest.partition(" ")
    ok = load_ms != "none" and not landed.startswith("chrome-error:")
    sites.append({"at": int(started), "url": url, "loaded": ok, "load_ms": load_ms if ok else ""})
for i, s in enumerate(sites):
    end = sites[i + 1]["at"] if i + 1 < len(sites) else float("inf")
    mine = [r for r in requests.values() if s["at"] <= r["at"] < end]
    host = lambda r: urlsplit(r["url"]).hostname or "?"
    s["requests"] = len(mine)
    s["blocked"] = sorted({host(r) for r in mine if r["how"] != "allowed"})
    s["allowed"] = sorted({host(r) for r in mine if r["how"] == "allowed"})
    s["blocked_n"] = sum(r["how"] == "blocked" for r in mine)
    s["standin_n"] = sum(r["how"] == "stand-in" for r in mine)

with open(table, "w", encoding="utf-8") as out:
    out.write("site\tloaded\tload_ms\trequests\tblocked\tstand_ins\tblocked_hosts\tallowed_hosts\n")
    for s in sites:
        out.write("\t".join([s["url"], "1" if s["loaded"] else "0", s["load_ms"], str(s["requests"]),
                             str(s["blocked_n"]), str(s["standin_n"]), ",".join(s["blocked"]),
                             ",".join(s["allowed"])]) + "\n")

worse = []
if baseline:
    rows = [line.rstrip("\n").split("\t") for line in open(baseline, encoding="utf-8")][1:]
    before = {r[0]: r for r in rows}
    for s in sites:
        b = before.get(s["url"])
        if not b:
            continue
        if b[1] == "1" and not s["loaded"]:
            worse.append(f"{s['url']} loaded with the baseline and doesn't now")
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
      f"{sum(s['standin_n'] for s in sites)} given a stand-in, of {sum(s['requests'] for s in sites)}")
if worse:
    sys.exit(1)
PY
echo "$run: $(cat "$result")$memory. Per site: $dir/sites.tsv"
# uBlock Origin's, for adblock-native's to compare with in a check of its own.
[ "$engine" != ublock ] || cp "$dir/sites.tsv" build/adblock-parity-sites.tsv
captured="with the window captured 8 s after each site loaded in $dir"
[ -z "${ADBLOCK_PARITY_NO_CAPTURE:-}" ] || captured="its sites in $dir"
echo "ok: $run, ${ADBLOCK_PARITY_BASELINE:+no worse than $ADBLOCK_PARITY_BASELINE, }$captured"
