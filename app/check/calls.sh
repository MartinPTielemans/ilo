# check.sh runs this at 520.
# A tab in a call keeps its page however long it's unseen, as Arc never puts one to sleep (#686):
# one using the camera and microphone, answered with Allow as the user would, and one holding a
# WebRTC connection that takes nothing and makes no sound, two peers of its page on a data channel.
# A tab in no call, unseen as long, lets go of its page beside them (core/idle_unload.cc). And each
# time the system says memory is short, the tab with a page unseen longest that's in no call lets go
# of its page, whatever its age, one a notification: DevTools' Memory domain says it, as the driver
# can't (check/cdp.py). A tab opened behind, waiting to load until it's shown, has no page to let
# go of, so it's passed over, though it's unseen longer.

serve
# Each page is named by its query, "call" without one. "c" calls with the camera and microphone,
# "w" connects two peers of the page on a data channel, and "o" opens the next page of the names.
# The call page is a link to the page named behind, which a ⌘-click, Ctrl off the Mac, opens in a tab
# behind; a click alone does nothing.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
const names = ["call", "listening", "quiet", "last", "extra"];
const name = location.search.slice(1) || "call";
requestAnimationFrame(() => setTimeout(() => document.title = name));
const held = [];
if (name == "call") {
  const link = document.createElement("a");
  link.href = "?behind";
  link.style = "position: fixed; inset: 0";
  link.onclick = e => e.metaKey || e.ctrlKey || e.preventDefault();
  document.body.append(link);
}
const gathered = peer => new Promise(done => {
  const check = () => peer.iceGatheringState == "complete" && done();
  peer.onicegatheringstatechange = check;
  check();
});
onkeydown = async e => {
  try {
    if (e.key == "c") {
      held.push(await navigator.mediaDevices.getUserMedia({video: true, audio: true}));
      document.title = "in a call";
    } else if (e.key == "w") {
      // Each peer makes the channel, so neither is handed it by the other: Blink takes a channel
      // the other peer made as open before WebRTC has opened it, opens it again when WebRTC does,
      // and an engine with DCHECKs aborts the renderer on that (#2395).
      const a = new RTCPeerConnection(), b = new RTCPeerConnection();
      const channel = a.createDataChannel("check", {negotiated: true, id: 0});
      channel.onopen = () => document.title = "connected";
      held.push(a, b, channel, b.createDataChannel("check", {negotiated: true, id: 0}));
      await a.setLocalDescription();
      await gathered(a);
      await b.setRemoteDescription(a.localDescription);
      await b.setLocalDescription();
      await gathered(b);
      await a.setRemoteDescription(b.localDescription);
    } else if (e.key == "o") {
      open("?" + names[names.indexOf(name) + 1]);
    }
  } catch (error) {
    document.title = "failed " + error.name;
  }
};
</script>' > "$out/site/calls.html"

log="$out/calls.log"
step=0
# A profile of the run's own, whose WebRTC may use the computer's own addresses, so the page's two
# peers find each other: ungoogled-chromium's default sends WebRTC only through a proxy, and the
# runner has none. Without mDNS names for those addresses, as a runner has no mDNS resolver, and
# with the app's own disabled feature, which this switch would otherwise replace.
profile_dir="$PWD/$out/calls/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir/Default"
printf '%s' '{"webrtc": {"ip_handling_policy": "default"}}' > "$profile_dir/Default/Preferences"
cdp_log="$out/calls-cdp.log"
scratch=$(mktemp -d)
start_with_cdp "${site}calls.html" "$profile_dir" /calls.html --use-fake-device-for-media-stream \
  --disable-features=MacAppCodeSignClone,WebRtcHideLocalIpsWithMdns
await_load
await '^browser: title [0-9]* call$' || fail "the call page didn't draw"
a=$(today | cut -d' ' -f1)
send click $page_point
if [ "$active" != 1 ]; then
  # Quits before cdp.py's pipe closes: the browser ends by itself as the pipe goes, and a quit
  # asked then finds it ending, which the Mac answers with an error (#3870).
  quit
  exec 5>&-
  rm -rf "$scratch"
  echo "skip: calls. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
send key c
expect "^browser: page $a asks ${site%/} to " "the call didn't ask for the camera and microphone"
expect "^browser: question $a at " "the call's question wasn't drawn"
expect "^browser: question $a takes input\$" "the call's question never took input"
send layout
read -r qx qy qw qh << EOF
$(grep "^browser: question $a at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
expect "^browser: title $a in a call\$" "Allow didn't give the call the camera and microphone"
expect "^browser: page $a camera 1 microphone 1 sharing 0 shared 0\$" \
  "the call's tab didn't show it uses the camera"
send click $page_point cmd
expect "^browser: tab [0-9]* opened by $a in the background\$" \
  "a ⌘-click on the call page's link didn't open a tab behind"
# opened <name>: the page shown opens the page named <name>, which is shown, and sets $tab to it.
opened() {
  send click $page_point
  send key o
  expect "^browser: title [0-9]* $1\$" "the page didn't open the page $1"
  tab=$(grep "^browser: title [0-9]* $1\$" "$log" | tail -1 | cut -d' ' -f3)
  expect "^browser: selected $tab\( in .*\)\{0,1\}\$" "the page $1 wasn't shown"
  send click $page_point
}
opened listening
b=$tab
send key w
expect "^browser: title $b connected\$" \
  "the page's two peers didn't connect: $(grep "^browser: title $b " "$log" | tail -1)"
opened quiet
c=$tab
opened last
d=$tab
opened extra
e=$tab

# Sends Memory.simulatePressureNotification at critical, as the system says memory is short,
# through check/cdp.py, which logs "cdp: pressure critical" once the browser has taken it.
pressure() {
  taken=$(grep -c '^cdp: pressure critical$' "$cdp_log" || true)
  echo "pressure critical" >&5
  for _ in $(seq 100); do
    [ "$(grep -c '^cdp: pressure critical$' "$cdp_log" || true)" -gt "$taken" ] && return 0
    kill -0 "$helper" 2>/dev/null || fail "cdp.py stopped: $(tail -3 "$cdp_log")"
    sleep 0.1
  done
  fail "DevTools didn't take the memory pressure: $(tail -3 "$cdp_log")"
}
# The tabs that let go of their pages since the driver's command $1, by their ids.
unloaded_since() {
  sed -n "/^browser: drive $1: /,\$p" "$log" |
    sed -n 's/^.*browser: tab \([0-9]*\) without its page$/\1/p' | tr '\n' ' '
}
# Each notification lets go of one page, the unseen longest's in no call, at any age, passing over
# the tab opened behind: the quiet tab's, then the last's, then none, as the calls and the tab shown
# keep theirs.
send layout
pressed=$step
pressure
expect "browser: tab $c without its page\$" "memory short, the tab unseen longest in no call kept its page" \
  "$pressed"
[ "$(unloaded_since "$pressed")" = "$c " ] ||
  fail "memory short, the tabs $(unloaded_since "$pressed")let go of their pages, not $c alone"
send layout
pressed=$step
pressure
expect "browser: tab $d without its page\$" "memory short again, the next tab in no call kept its page" \
  "$pressed"
[ "$(unloaded_since "$pressed")" = "$d " ] ||
  fail "memory short again, the tabs $(unloaded_since "$pressed")let go of their pages, not $d alone"
send layout
pressed=$step
pressure
send layout
[ -z "$(unloaded_since "$pressed")" ] ||
  fail "memory short a third time, the tabs $(unloaded_since "$pressed")let go of their pages"
# Unseen 2 hours, each tab but the one shown lets go of its page once another is switched to,
# unless it's in a call. The tab named extra, which the pressure left with its page, is hidden by
# showing the quiet tab, and the last tab is switched to once all are aged, each by its ⌘ number, as
# a page's own tab shows without the switch that checks.
n=$(today | tr ' ' '\n' | grep -nx "$c" | cut -d: -f1)
switch "$((n + $(pinned | wc -w))) cmd" "$c"
send age 7200
aged=$step
n=$(today | tr ' ' '\n' | grep -nx "$d" | cut -d: -f1)
switch "$((n + $(pinned | wc -w))) cmd" "$d"
expect 'browser: idle tabs checked$' "showing a tab didn't check for tabs unseen 2 hours"
sed -n "/^browser: drive $aged: /,\$p" "$log" | grep -q "browser: tab $e without its page\$" ||
  fail "the tab in no call, unseen 2 hours, kept its page"
for kept in "$a using the camera and microphone" "$b holding a WebRTC connection"; do
  sed -n "/^browser: drive $aged: /,\$p" "$log" | grep -q "browser: tab ${kept%% *} without its page\$" &&
    fail "the tab ${kept#* } let go of its page"
done
../tools/window-screenshot.sh "$pid" "$out/calls.png" || fail "no window to capture"
quit
exec 5>&-
rm -rf "$profile_dir" "$scratch"
echo "ok: memory short, the tab with a page unseen longest in no call let go of its page, one each" \
  "time, passing over the tab opened behind, and none once only the calls and the tab shown had" \
  "theirs; unseen 2 hours, a tab using the camera and microphone and one holding a WebRTC" \
  "connection that takes nothing and makes no sound kept their pages, and a tab in no call let" \
  "go of its page. Window: $out/calls.png"
