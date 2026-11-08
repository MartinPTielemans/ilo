# The parity gate a native ad block engine has to pass before it takes uBlock Origin's place
# (#2111), run with the engine the build has: uBlock Origin, until there's another. In a new
# profile, once its lists block, each site in check/adblock-sites.txt opens in one tab. For each,
# the run gives the time from Return in the command bar to the page's load, the requests it made,
# and those blocked or answered with a stand-in, with their hosts, which DevTools records (check/
# cdp.py), as it sees what an extension's webRequest blocks where the net log doesn't. It
# captures the window 8 s after the load, to show the ads that got through. Off the Mac it gives the
# most memory uBlock Origin's process took. With ADBLOCK_PARITY_BASELINE naming the sites.tsv an
# earlier run left, it fails where this run is worse:
#   - a site that loaded there and doesn't here;
#   - a host blocked on a site there that this run let through on that site.
# A host neither run requested counts for neither, as the ads a site serves change from load to
# load. It needs the network and the sites, so it runs only when named: ./check.sh adblock-parity.
# It leaves adblock-parity/sites.tsv, a capture a site, and the requests recorded.

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
rm -rf "$dir"
mkdir -p "$dir/profile"
log="$out/adblock-parity.log"
cdp_log="$dir/cdp.log"
requests="$dir/requests.jsonl"
starts="$dir/starts.txt"
step=0
ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
scratch=$(mktemp -d)
start_with_cdp "${site}parity.html" "$PWD/$dir/profile" /parity.html
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
# A new profile's lists block once they're compiled, so the probe reloads until they do, for up to
# 120 s, as adblock-live's does.
since=$(date +%s)
while ! grep -q "^browser: title $tab ad script blocked" "$log"; do
  [ $(($(date +%s) - since)) -lt 120 ] || fail "the lists didn't block Google's ad script within 120 s"
  send key r cmd
  expect "^browser: title $tab ad script " "the probe didn't report after ⌘R"
  for _ in $(seq 20); do
    none_since "^browser: title $tab ad script blocked" || break
    sleep 0.1
  done
done
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
  # Ads load after the page does, from its scripts.
  sleep 8
  ../tools/window-screenshot.sh "$pid" "$dir/$(printf %02d "$n").png" ||
    fail "no window to capture"
done 4< check/adblock-sites.txt
[ -s "$starts" ] || fail "no site opened"
if ! [ "$(uname)" = Darwin ]; then
  ublock=$(pgrep -f "$procs.*--extension-process" | head -1)
  if windows; then
    peaks=$(ID=$ublock powershell.exe -NoProfile -NonInteractive -Command '$p = Get-Process -Id $env:ID;
      "$([math]::Round($p.PeakWorkingSet64 / 1MB))"' | tr -d '\r')
  else
    peaks=$(awk '/^VmHWM:/ { print int($2 / 1024) }' "/proc/$ublock/status")
  fi
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
print(f"{len(loaded)} of {len(sites)} sites loaded, in {median} ms at the median; "
      f"{sum(s['blocked_n'] for s in sites)} requests blocked and "
      f"{sum(s['standin_n'] for s in sites)} given a stand-in, of {sum(s['requests'] for s in sites)}")
if worse:
    print("worse than the baseline:\n  " + "\n  ".join(worse))
    sys.exit(1)
PY
memory=
[ -z "${peaks:-}" ] || memory=". uBlock Origin's process held at most $peaks MB"
echo "adblock-parity: $(cat "$result")$memory. Per site: $dir/sites.tsv"
echo "ok: adblock-parity, ${ADBLOCK_PARITY_BASELINE:+no worse than $ADBLOCK_PARITY_BASELINE, }with the window captured 8 s after each site loaded in $dir"
