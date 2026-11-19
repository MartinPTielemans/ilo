# check.sh runs this at 610.
# A page's WebRTC tells it no address its HTTP doesn't (decision 10, #1774), read from getStats.
# With no setting and no proxy, it gathers only server-reflexive UDP candidates, on the default
# route's interface, none of the computer's own, and two peers of the page connect through STUN
# with no TURN relay. With Settings › Privacy's Relay calls on it gathers no host or
# server-reflexive UDP candidate, so they don't connect, and off again they do. A profile that uses
# a proxy gathers none either, the switch off.
# The STUN server is the run's own (check/stun.py), on the default route's address, so the
# server-reflexive candidate is that address and the peers reach each other without a NAT.

serve
# The page connects two of its peers on a data channel through the STUN server its query names,
# each sending the other its description once it has gathered, then says which local candidates
# getStats lists, by type, protocol and address, and whether they connected, within 5 s. It leaves
# out the active TCP candidates WebRTC lists at 0.0.0.0 and ::, port 9, which hold no address and
# listen on nothing (RFC 6544). The channel is negotiated, each peer making its own: one the other
# announces is Blink's remote channel, opened without its event, which a state change posted before
# can take back to connecting, and opening again it fails a DCHECK on a Dev engine
# (rtc_data_channel.cc:873, upstream; the IP handling policy only filters candidates).
printf '%s' '<title>loading</title><body style="background:#333">
<script>
const servers = {iceServers: [{urls: "stun:" + location.search.slice(1)}]};
const gathered = peer => new Promise(done => {
  const check = () => peer.iceGatheringState == "complete" && done();
  peer.onicegatheringstatechange = check;
  setTimeout(done, 5000);
  check();
});
(async () => {
  try {
    const a = new RTCPeerConnection(servers), b = new RTCPeerConnection(servers);
    const channel = a.createDataChannel("check", {negotiated: true, id: 0});
    b.createDataChannel("check", {negotiated: true, id: 0});
    const open = new Promise(done => {
      channel.onopen = () => done(true);
      setTimeout(() => done(false), 5000);
    });
    await a.setLocalDescription();
    await gathered(a);
    await b.setRemoteDescription(a.localDescription);
    await b.setLocalDescription();
    await gathered(b);
    await a.setRemoteDescription(b.localDescription);
    const connected = await open;
    const kinds = new Set();
    (await a.getStats()).forEach(stat => {
      if (stat.type == "local-candidate" && !["0.0.0.0", "::"].includes(stat.address)) {
        kinds.add(stat.candidateType + "/" + stat.protocol + " " + stat.address);
      }
    });
    document.title = "candidates " + ([...kinds].sort().join(" ") || "none") +
      (connected ? ", connected" : ", not connected");
  } catch (error) {
    document.title = "failed " + error.name;
  }
})();
</script>' > "$out/site/webrtc.html"

# The default route's address, found as WebRTC finds it: a UDP socket connected out, which sends
# nothing.
route=$(python3 -I -c 'import socket
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.connect(("8.8.8.8", 53))
print(s.getsockname()[0])' 2> /dev/null || true)
if [ -z "$route" ]; then
  echo "skip: webrtc. The computer has no default route, which WebRTC gathers on."
  exit 0
fi
rm -f "$out/stun-port"
python3 -I check/stun.py "$route" "$out/stun-port" > "$out/stun.log" 2>&1 &
helper=$!
for _ in $(seq 50); do
  [ -s "$out/stun-port" ] && break
  sleep 0.1
done
[ -s "$out/stun-port" ] || fail "the STUN server didn't start: $(cat "$out/stun.log")"
page="${site}webrtc.html?$route:$(cat "$out/stun-port")"
direct="candidates srflx/udp $route, connected"
relayed="candidates none, not connected"

# Sets $said to what the page in the tab $tab says, once it has: after a reload with $1.
said() {
  if [ "${1:-}" = reload ]; then
    send click $page_point
    send key r cmd
    expect_late "^browser: title $tab \(candidates\|failed\) " "the page didn't report after ⌘R"
  else
    await "^browser: title $tab \(candidates\|failed\) " || fail "the page didn't report"
  fi
  said=$(grep "^browser: title $tab \(candidates\|failed\) " "$log" | tail -1 | cut -d' ' -f4-)
}
# relay_switch <on|off>: Settings › Privacy's Relay calls, turned to <on|off> by a click.
relay_switch() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Privacy$'; then
    settings_drew
    send clickkey $(drawn "toolbar Privacy")
    expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
  fi
  settings_drew Privacy
  click_in_sight "switch relay calls"
  expect "^browser: relay calls $1\$" "a click on Relay calls' switch didn't turn it $1"
  [ "$1" = off ] ||
    ../tools/window-screenshot.sh "$pid" "$out/webrtc-settings.png" all || fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
}

log="$out/webrtc.log"
step=0
profile_dir="$PWD/$out/webrtc/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
said
[ "$said" = "$direct" ] ||
  fail "with no setting and no proxy, the page didn't gather server-reflexive candidates alone and connect: $said"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: webrtc. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
relay_switch on
said reload
[ "$said" = "$relayed" ] || fail "with Relay calls on, the page still gathered or connected: $said"
relay_switch off
said reload
[ "$said" = "$direct" ] || fail "with Relay calls off again, the page didn't connect directly: $said"
exec 3>&-
quit

# A profile whose proxy, a SOCKS one nothing answers at, its page never uses, as it's on this
# computer, which a proxy never takes.
log="$out/webrtc-proxy.log"
step=0
rm -rf "$profile_dir"
mkdir -p "$profile_dir/Default"
printf '%s' '{"proxy": {"mode": "fixed_servers", "server": "socks5://127.0.0.1:9"}}' \
  > "$profile_dir/Default/Preferences"
BROWSER_DRIVE=1 BROWSER_URL="$page" BROWSER_PROFILE_DIR="$profile_dir" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
tab=$(today | cut -d' ' -f1)
said
[ "$said" = "$relayed" ] || fail "in a profile with a proxy, the page gathered or connected: $said"
exec 3>&-
quit
rm -rf "$profile_dir"
echo "ok: with no setting and no proxy, a page's WebRTC gathered server-reflexive UDP candidates" \
  "alone and its two peers connected through STUN, with no relay. With Relay calls on it gathered" \
  "no host or server-reflexive candidate and they didn't connect, and off again they did. A" \
  "profile with a proxy gathered none. Settings: $out/webrtc-settings.png"
