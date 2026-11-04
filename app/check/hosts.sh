# Every host the browser connects to on its own is one ilobrowser.com/privacy names (#684): a run
# that launches with Chromium's net log on, idles until the security components' first check
# (about a minute in) shows in it, opens one of its own pages and types in the command bar, so
# search suggestions go out if they do, then quits. Each host in the net log, from the requests'
# addresses and the names the resolver looked up, must match check/hosts.txt, which says why each
# is there. Sync points at a port of the run's own where nothing listens, so nothing reaches the
# hosted server, and nothing may ask it. A dev build has no update feed and no crash collector,
# which only the prod build names (engine/README.md), so neither shows here. It leaves the hosts
# it saw, with how often, in hosts-seen.txt, and the addresses outside the machine that no name
# led to in hosts-addresses.txt.

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
patterns = []
for line in open(listed, encoding="utf-8"):
    line = line.split("#", 1)[0].strip()
    if line:
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

addresses = Counter()
for address in re.findall(r'"(?:address|remote_address|peer_address)":"([^"]+)"', text):
    ip = address.rsplit(":", 1)[0].strip("[]")
    try:
        ipaddress.ip_address(ip)
    except ValueError:
        continue
    if not local(ip):
        addresses[address] += 1

with open(f"{out}/hosts-seen.txt", "w") as seen:
    for host, count in sorted(hosts.items()):
        seen.write(f"{host} {count}\n")
with open(f"{out}/hosts-addresses.txt", "w") as seen:
    for address, count in sorted(addresses.items()):
        seen.write(f"{address} {count}\n")

unlisted = [h for h in sorted(hosts) if not any(fnmatch.fnmatch(h, p) for p in patterns)]
sync = [m for m in re.findall(r'127\.0\.0\.1:(\d+)', text) if m == sync_port]
if unlisted:
    print("hosts check/hosts.txt doesn't name: " + " ".join(unlisted))
    sys.exit(1)
if sync:
    print("signed out, the browser asked the sync server")
    sys.exit(1)
PY
echo "ok: every host the browser asked, through a launch, the security components' first check," \
  "a page and a search typed in the command bar (suggestions $suggested)," \
  "is one check/hosts.txt names: $(cut -d' ' -f1 "$out/hosts-seen.txt" | tr '\n' ' ')" \
  "Net log: $netlog, hosts: $out/hosts-seen.txt, addresses no name led to: $out/hosts-addresses.txt"
