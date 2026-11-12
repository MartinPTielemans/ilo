# A tab in a call keeps its page however long it's unseen, as Arc never puts one to sleep (#686):
# one using the camera and microphone, answered with Allow as the user would, and one holding a
# WebRTC connection that takes nothing and makes no sound, two peers of its page on a data channel.
# A tab in no call, unseen as long, lets go of its page beside them (core/idle_unload.cc). And each
# time the system says memory is short, the tab unseen longest that's in no call lets go of its
# page, whatever its age, one a notification: DevTools' Memory domain says it, as the driver
# can't (check/cdp.py). The run says how much memory the browser's processes held before and after.

serve
# Each page is named by its query, "call" without one. "c" calls with the camera and microphone,
# "w" connects two peers of the page on a data channel, and "o" opens the next page of the names,
# with no opener, so it's a browsing instance and a process of its own, which letting go of its page
# frees. The page named quiet holds 100 MB, as a heavy page out of sight does.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
const names = ["call", "listening", "quiet", "last", "extra"];
const name = location.search.slice(1) || "call";
const held = [];
if (name == "quiet") {
  held.push(new Uint8Array(100 << 20).fill(1));
}
requestAnimationFrame(() => setTimeout(() => document.title = name));
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
      open("?" + names[names.indexOf(name) + 1], "_blank", "noopener");
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
  exec 3>&- 5>&-
  quit
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

# The browser's processes and the memory they hold, "<count> processes, <MB> MB": on Windows the
# working sets of the build's processes, elsewhere their resident sizes. Memory two processes share
# counts in each, so it's more than the browser holds, by as much before as after.
held() {
  if windows; then
    powershell.exe -NoProfile -NonInteractive -Command "Get-CimInstance Win32_Process \
      -Filter \"Name = 'chrome.exe'\" | Where-Object { \$_.ExecutablePath -like '*\\build\\Browser\\*' } |
      Measure-Object -Property WorkingSetSize -Sum | ForEach-Object { '{0} processes, {1} MB' -f \
      \$_.Count, [math]::Round(\$_.Sum / 1MB) }" | tr -d '\r'
  else
    ps -Ao rss=,args= | grep -F -e "$procs" | grep -v -e 'grep -F' |
      awk '{ n++; kb += $1 } END { printf "%d processes, %d MB\n", n, kb / 1024 }'
  fi
}
# Sends Memory.simulatePressureNotification at $1, as the system says memory is short, through
# check/cdp.py, which logs "cdp: pressure <level>" once the browser has it.
pressure() {
  taken=$(grep -c "^cdp: pressure $1\$" "$cdp_log" || true)
  echo "pressure $1" >&5
  for _ in $(seq 100); do
    [ "$(grep -c "^cdp: pressure $1\$" "$cdp_log" || true)" -gt "$taken" ] && return 0
    kill -0 "$helper" 2>/dev/null || fail "cdp.py stopped: $(tail -3 "$cdp_log")"
    sleep 0.1
  done
  fail "DevTools didn't take the memory pressure: $(tail -3 "$cdp_log")"
}
# The tabs that let go of their pages since the driver's command $1, by their ids.
unloaded_since() {
  sed -n "/^browser: drive $1: /,\$p" "$log" | sed -n 's/^.*browser: tab \([0-9]*\) without its page$/\1/p' |
    tr '\n' ' '
}
for _ in $(seq 100); do
  grep -q '^cdp: attached$' "$cdp_log" && break
  kill -0 "$helper" 2>/dev/null || fail "cdp.py stopped: $(tail -3 "$cdp_log")"
  sleep 0.1
done
grep -q '^cdp: attached$' "$cdp_log" || fail "cdp.py didn't attach to the call: $(tail -3 "$cdp_log")"
# Each notification lets go of one page, the unseen longest's in no call, at any age: the quiet
# tab's, then the last's, then none, as the calls and the tab shown keep theirs.
before=$(held)
send layout
pressed=$step
pressure critical
expect "browser: tab $c without its page\$" "memory short, the tab unseen longest in no call kept its page" \
  "$pressed"
# The page's process, once it has gone.
for _ in $(seq 50); do
  after=$(held)
  [ "${after%% *}" -lt "${before%% *}" ] && break
  sleep 0.1
done
[ "$(unloaded_since "$pressed")" = "$c " ] ||
  fail "memory short, the tabs $(unloaded_since "$pressed")let go of their pages, not $c alone"
send layout
pressed=$step
pressure critical
expect "browser: tab $d without its page\$" "memory short again, the next tab in no call kept its page" \
  "$pressed"
[ "$(unloaded_since "$pressed")" = "$d " ] ||
  fail "memory short again, the tabs $(unloaded_since "$pressed")let go of their pages, not $d alone"
send layout
pressed=$step
pressure critical
send layout
[ -z "$(unloaded_since "$pressed")" ] ||
  fail "memory short a third time, the tabs $(unloaded_since "$pressed")let go of their pages"
echo "calls: memory short: before, $before; once the quiet tab let go of its page, $after"
# Unseen 2 hours, each tab but the one shown lets go of its page once another is switched to,
# unless it's in a call. The tab named extra, which the pressure left loaded, is hidden by showing
# the quiet tab, and the last tab is switched to once all are aged, each by its ⌘ number, as a
# page's own tab shows without the switch that checks.
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
exec 3>&- 5>&-
quit
kill "$helper" 2>/dev/null || true
helper=
rm -rf "$profile_dir" "$scratch"
scratch=
echo "ok: memory short, the tab unseen longest in no call let go of its page, one each time, and" \
  "none once only the calls and the tab shown were left ($before before, $after after);" \
  "unseen 2 hours, a tab using the camera and microphone and one holding a WebRTC" \
  "connection that takes nothing and makes no sound kept their pages, and a tab in no call let" \
  "go of its page. Window: $out/calls.png"
