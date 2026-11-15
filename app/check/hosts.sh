# check.sh runs this at 560.
# Every host the browser connects to on its own is one ilobrowser.com/privacy names (#684): a run
# that launches with Chromium's net log on, idles until the security components' first check
# (about a minute in) shows in it, opens one of its own pages and types in the command bar, so
# search suggestions go out if they do, then quits. Each host in the net log, from the requests'
# addresses and the names the resolver looked up, must match check/hosts.txt, which says why each
# is there. Sync points at a port of the run's own where nothing listens, so nothing reaches the
# hosted server, and nothing may ask it. A dev build has no update feed, crash collector or daily
# count's server, which only the prod build names (engine/README.md, decision 11), so none shows
# here. It leaves the hosts it saw, with how often, in hosts-seen.txt, and each address it
# connected to, with the name that led to it or none, which fails it, in hosts-addresses.txt.

serve
log="$out/hosts.log"
netlog="$out/hosts-net.json"
step=0
rm -f "$netlog"
sync_port=$(python3 -I -c '
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])')
BROWSER_DRIVE=1 BROWSER_URL="data:text/html,<title>hosts</title>" "$browser" \
  --sync-url="http://127.0.0.1:$sync_port" --log-net-log="$(app_path "$PWD/$netlog")" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
# The net log is written as it goes, so the first security check shows in it once it's made.
for _ in $(seq 1800); do
  grep -q 'update\.googleapis\.com' "$netlog" 2>/dev/null && break
  kill -0 "$pid" 2>/dev/null || gone
  sleep 0.1
done
grep -q 'update\.googleapis\.com' "$netlog" ||
  fail "no security components' check in the net log 3 minutes after launch"
go_from_bar "${site#http://}scheme.html"
expect '^browser: title [0-9]* scheme ' "the run's own page didn't load"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type "weather in lisbon"
expect '^browser: command bar ' "the command bar didn't take what was typed"
# A suggestion request goes out as the typing does, if it does at all; a page loads in the time
# the run waits for one, and none coming is an answer too.
suggested="not sent"
for _ in $(seq 100); do
  grep -q 'lisbon' "$netlog" && { suggested=sent; break; }
  sleep 0.1
done
send key escape
exec 3>&-
quit
[ -s "$netlog" ] || fail "Chromium wrote no net log"
# What's wrong, if anything, goes in hosts-result.txt, as a here-document in a command substitution
# trips the Mac's bash 3.2.
result="$out/hosts-result.txt"
python3 -I - "$netlog" check/hosts.txt "$out" "$sync_port" > "$result" << 'PY' || fail "$(cat "$result")"

import fnmatch, ipaddress, json, re, sys
from collections import Counter

netlog, listed, out, sync_port = sys.argv[1:]
text = open(netlog, encoding="utf-8", errors="replace").read()
# The hosts it may ask, and those the page names only for what the user does (a ! before them),
# which it mustn't.
patterns, never = [], {}
for line in open(listed, encoding="utf-8"):
    line = line.split("#", 1)[0].strip()
    if line.startswith("!"):
        name, _, why = line[1:].partition(" ")
        never[name] = why.strip()
    elif line:
        patterns.append(line.split()[0])

hosts = Counter()
# Only what goes over the network: a chrome-extension: or data: URL names no host.
for url in re.findall(r'"(?:url|original_url|new_location)":"((?:https?|wss?|ftp)://[^"]*)"', text):
    host = re.match(r'[a-z]+://(?:[^@/]*@)?(\[[^\]]*\]|[^/:?#]*)', url)
    if host and host.group(1):
        hosts[host.group(1).lower()] += 1
for host in re.findall(r'"host":"(?:[a-z]+://)?([^":/]+)', text):
    hosts[host.lower()] += 1

def local(name):
    try:
        address = ipaddress.ip_address(name.strip("[]"))
    except ValueError:
        return name == "localhost"
    return address.is_loopback or address.is_private or address.is_link_local or address.is_multicast

# Each address a socket connected to outside the machine and its network, and whether a name the
# resolver looked up gave it, from the events, as a quit leaves the net log whole: an address no
# name led to would be a connection the hosts above don't account for. The system's own resolver,
# on port 53, is the one exception.
log = json.loads(text)
types = {number: name for name, number in log["constants"]["logEventTypes"].items()}
named, nameservers, addresses = set(), set(), Counter()
for event in log["events"]:
    kind = types.get(event["type"], "")
    params = event.get("params", {})
    if kind.startswith("HOST_RESOLVER"):
        for found in re.findall(r"[0-9a-fA-F:.]*[:.][0-9a-fA-F:.]+", json.dumps(params)):
            try:
                named.add(str(ipaddress.ip_address(found.strip(":") if "::" not in found else found)))
            except ValueError:
                pass
    if kind == "DNS_CONFIG_CHANGED":
        nameservers.update(params.get("nameservers", []))
    if kind in ("TCP_CONNECT_ATTEMPT", "TCP_CONNECT", "UDP_CONNECT") and params.get("address"):
        addresses[params["address"]] += 1
unnamed = []
for address, count in addresses.items():
    ip = address.rsplit(":", 1)[0].strip("[]")
    if local(ip) or address in nameservers or address.endswith(":53"):
        continue
    if str(ipaddress.ip_address(ip)) not in named:
        unnamed.append(address)

with open(f"{out}/hosts-seen.txt", "w") as seen:
    for host, count in sorted(hosts.items()):
        seen.write(f"{host} {count}\n")
with open(f"{out}/hosts-addresses.txt", "w") as seen:
    for address, count in sorted(addresses.items()):
        ip = address.rsplit(":", 1)[0].strip("[]")
        ip = str(ipaddress.ip_address(ip)) if not local(ip) else ip
        seen.write(f"{address} {count} {'named' if ip in named else 'local' if local(ip) else 'resolver' if address.endswith(':53') else 'no name'}\n")

asked = [f"{h} ({why})" for h in sorted(hosts) for p, why in never.items() if fnmatch.fnmatch(h, p)]
unlisted = [h for h in sorted(hosts) if not any(fnmatch.fnmatch(h, p) for p in patterns + list(never))]
# A request to it, not the switch naming it, which the net log's own command line holds.
sync = re.findall(r'"(?:url|original_url)":"[a-z]+://(?:127\.0\.0\.1|localhost):' + sync_port + r'[/"]', text)
if asked:
    print("hosts asked that the page names only for what the user does: " + ", ".join(asked))
    sys.exit(1)
if unlisted:
    print("hosts check/hosts.txt doesn't name: " + " ".join(unlisted))
    sys.exit(1)
if unnamed:
    print("connections to addresses no name led to: " + " ".join(sorted(unnamed)))
    sys.exit(1)
if sync:
    print("signed out, the browser asked the sync server")
    sys.exit(1)
PY
echo "ok: every host the browser asked, through a launch, the security components' first check," \
  "a page and a search typed in the command bar (suggestions $suggested)," \
  "is one check/hosts.txt names: $(cut -d' ' -f1 "$out/hosts-seen.txt" | tr '\n' ' ')" \
  "Net log: $netlog, hosts: $out/hosts-seen.txt, addresses: $out/hosts-addresses.txt"
