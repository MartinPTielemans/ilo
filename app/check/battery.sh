# check.sh runs this at 530.
# Chrome's Battery Saver as ilo shows it (#686), on the check's battery (--battery-for-testing,
# core/battery_saver.h), which a run on mains needs.
# At 50% on battery it stays off. Below 20% on battery it turns on: the toast says so with Turn Off,
# and a leaf shows among the URL field's buttons, whose press says so again. Turn Off Battery Saver
# for Now in the command bar turns it off until the computer's plugged in, and it stays off lower
# still; plugged in and out again it's back. While it's on, a page busy out of sight freezes, which
# its row's hover card says, and thaws once shown. Plugged in, it goes, the leaf with it. Settings'
# switch turns it off at once, and on again below 20%.

serve
# Each page is named by its query, "battery" without one. "o" opens the next page of the names,
# with no opener, so each is a browsing instance of its own: Chrome freezes a page only with every
# page its opener ties it to, and the one shown keeps them all. "b" keeps a worker busy, as a page
# that uses the CPU out of sight does.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
const names = ["battery", "busy", "other"];
const name = location.search.slice(1) || "battery";
requestAnimationFrame(() => setTimeout(() => document.title = name));
let worker;
onkeydown = e => {
  if (e.key == "o") {
    open("?" + names[names.indexOf(name) + 1], "_blank", "noopener");
  } else if (e.key == "b") {
    worker = new Worker(URL.createObjectURL(new Blob(["for (;;) {}"])));
    document.title = "spinning";
  }
};
</script>' > "$out/site/battery.html"

log="$out/battery.log"
step=0
rm -f "$saved" "$launch_frame"
# A page out of sight for 5 s and silent for 5 s may freeze, not Chrome's 5 minutes, with the app's
# own feature beside them, as this switch would otherwise replace it.
BROWSER_DRIVE=1 BROWSER_URL="${site}battery.html" "$browser" --battery-for-testing \
  --enable-features=PartitionAllocAdaptiveMemoryReclaimInterval,AperitifHelpers,CPUMeasurementInFreezingPolicy:freezing_visible_protection_time/5s/freezing_audio_protection_time/5s \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* battery$' || fail "the battery page didn't draw"
a=$(today | cut -d' ' -f1)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: battery. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

fifty=$((step + 1))
send battery 50 unplugged
send battery 19 unplugged
expect 'browser: battery saver on at 19%$' "Battery Saver didn't turn on at 19% on battery"
none_since 'browser: battery saver on at 50%' "$fifty" || fail "Battery Saver turned on at 50%"
expect '^browser: toast shown Battery Saver is on at 19% \[turn off battery saver\]$' \
  "the toast didn't say Battery Saver is on, with Turn Off"
# Captured once it has come in, not as it grows in.
expect '^browser: toast settled$' "the toast didn't settle"
await '^browser: battery saver leaf at ' || fail "the URL field didn't draw Battery Saver's leaf"
../tools/window-screenshot.sh "$pid" "$out/battery-notice.png" || fail "no window to capture"
read -r lx ly lw lh << EOF
$(sed -n 's/^browser: battery saver leaf at //p' "$log" | tail -1)
EOF
send click $((${lx%.*} + ${lw%.*} / 2)) $((${ly%.*} + ${lh%.*} / 2))
expect '^browser: toast shown Battery Saver is on at 19% \[turn off battery saver\]$' \
  "the leaf's press didn't say Battery Saver is on again"
bar_command turn-off-battery-saver "Turn Off Battery Saver for Now" "battery saver"
expect 'browser: battery saver off$' "Turn Off Battery Saver for Now didn't turn it off"
expect '^browser: toast shown Battery Saver is off until you plug in$' \
  "the toast didn't say Battery Saver is off until plugged in"
expect '^browser: battery saver leaf hidden$' "the leaf stayed with Battery Saver off"
eighteen=$((step + 1))
send battery 18 unplugged
send battery 18 plugged
send battery 17 unplugged
expect 'browser: battery saver on at 17%$' "plugged in and out again, Battery Saver didn't come back"
none_since 'browser: battery saver on at 18%' "$eighteen" ||
  fail "Battery Saver came back on, turned off for now"

# Settings' Battery Saver switch: off, it goes at once and stays off lower still; on again, it's
# back, below 20% on battery.
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show General"
settings_drew General
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
in_sight "switch battery saver setting" || fail "Settings didn't draw Battery Saver's switch"
../tools/window-screenshot.sh "$pid" "$out/battery-setting.png" all || fail "no window to capture"
send clickkey $at
expect '^browser: battery saver setting off$' "a click on Battery Saver's switch didn't turn it off"
expect 'browser: battery saver off$' "Battery Saver stayed on with its switch off"
expect '^browser: battery saver leaf hidden$' "the leaf stayed with Battery Saver's switch off"
sixteen=$((step + 1))
send battery 16 unplugged
send clickkey $at
expect '^browser: battery saver setting on$' "a click on Battery Saver's switch didn't turn it on"
expect 'browser: battery saver on at 16%$' "Battery Saver's switch on didn't turn it on at 16%"
# 16% read, it turned on only once the switch was.
! since "$sixteen" | sed '/^browser: battery saver setting on$/q' | grep -q 'browser: battery saver on' ||
  fail "Battery Saver came on with its switch off"
settings_close
post clickkey $close
expect '^browser: settings hidden$' "the close button didn't hide Settings"

# On Battery Saver a page busy out of sight freezes once Chrome has measured it, a minute at a time,
# and says so in its row's hover card; shown, it thaws.
send click $page_point
send key o
expect "^browser: title [0-9]* busy\$" "the page didn't open the busy page"
b=$(grep '^browser: title [0-9]* busy$' "$log" | tail -1 | cut -d' ' -f3)
send click $page_point
send key b
expect "^browser: title $b spinning\$" "the busy page's worker didn't start"
send key o
expect "^browser: title [0-9]* other\$" "the busy page didn't open another"
busy=$step
for _ in $(seq 2400); do
  logged "browser: tab $b frozen\$" "$busy" && break
  kill -0 "$pid" 2>/dev/null || gone
  sleep 0.1
done
logged "browser: tab $b frozen\$" "$busy" ||
  fail "the page busy out of sight didn't freeze on Battery Saver in 4 minutes"
send move $(row "$(($(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1) - 1))")
expect "^browser: hover card $b says paused\$" "the frozen tab's hover card didn't say it's paused"
expect '^browser: hover card settled$' "the frozen tab's hover card didn't settle"
../tools/window-screenshot.sh "$pid" "$out/battery-paused.png" || fail "no window to capture"
send move $page_point
n=$(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1)
switch "$((n + $(pinned | wc -w))) cmd" "$b"
expect "browser: tab $b thawed\$" "the frozen tab didn't thaw once shown"

send battery 21 plugged
expect 'browser: battery saver off$' "Battery Saver stayed on once plugged in"
expect '^browser: battery saver leaf hidden$' "the leaf stayed once plugged in"
exec 3>&-
quit
echo "ok: at 50% on battery Battery Saver stayed off; at 19% it turned on, the toast said so with" \
  "Turn Off and the URL field drew its leaf, whose press said so again; the command bar turned it" \
  "off until plugged in, and it came back plugged in and out; Settings' switch turned it off and" \
  "on again; a page busy out of sight froze," \
  "its hover card said so, and it thawed once shown; plugged in, Battery Saver and the leaf went." \
  "Windows: $out/battery-notice.png, $out/battery-setting.png, $out/battery-paused.png"
