# check.sh runs this at 905, only when named.
# A probe for #2802, not a check: the Mini Player with its controls showing, at its narrowest and
# its widest, from a short host and a long one, for the captures its PR shows. Under Wayland, where
# the driver's scroll over it leaves the pointer there, so its controls stay shown.

serve
[ -n "${WAYLAND_DISPLAY:-}" ] || fail "the probe needs Wayland, where the pointer stays over the Mini Player"

# mini_player_at <condition> <failure>, as media.sh's.
mini_player_at() {
  for _ in $(seq 100); do
    at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: pip at ' | tail -1 |
      cut -d' ' -f4-)
    if [ -n "$at" ] && echo "$at" | awk -v px="$px" -v py="$py" -v pw="$pw" -v ph="$ph" \
      "{ x = \$1; y = \$2; w = \$3; h = \$4; exit !($1) }"; then
      read -r px py pw ph <<EOF
$at
EOF
      return 0
    fi
    sleep 0.1
  done
  fail "$2"
}

# media.sh's video, a canvas's colours.
printf '%s' '<title>loading</title><body style="margin:0;background:#333">
<video id=v muted autoplay style="width:100vw;height:100vh"></video>
<script>
const canvas = Object.assign(document.createElement("canvas"), {width: 320, height: 180});
const paint = canvas.getContext("2d");
const frame = time => {
  paint.fillStyle = `hsl(${time / 20 % 360} 50% 40%)`;
  paint.fillRect(0, 0, 320, 180);
  requestAnimationFrame(frame);
};
requestAnimationFrame(frame);
v.srcObject = canvas.captureStream();
v.onplaying = () => document.title = "playing";
v.onenterpictureinpicture = () => document.title = "floating";
v.onleavepictureinpicture = () => document.title = "landed";
onkeydown = e => e.key == "x" && document.exitPictureInPicture();
</script>' > "$out/site/look.html"

# Chromium takes any name under .localhost for this machine, so the site answers a long one too.
port=${site##*:}
long="http://the-origin-of-a-floating-video-on-a-long-host-name.localhost:$port"

# looks <name> <site>: floats the video from <site>, captures it at its narrowest and its widest
# with its controls showing, and puts it back in its tab.
looks() {
  send key t cmd
  expect '^browser: command bar shown$' "the command bar didn't open for $2"
  send type "${2}look.html"
  send key return
  expect "^browser: loaded [0-9]* ${2}look.html\$" "the video's page didn't load from $2"
  v=$(grep "^browser: loaded [0-9]* ${2}look.html\$" "$log" | tail -1 | cut -d' ' -f3)
  expect "^browser: title $v playing\$" "the video from $2 didn't play"
  post rightclick $page_point
  expect "^browser: menu shown .*$(cased 'Picture in Picture')" "the menu had no Picture in Picture"
  send choose "$(cased 'Picture in Picture')"
  expect "^browser: title $v floating\$" "the video from $2 didn't float"
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at 1 "the video from $2 had no Mini Player"
  # Smaller than its smallest, where it stays, the pointer over it.
  send pip wheel $((pw / 2)) $((ph / 2)) 3 cmd
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/look-$1-narrowest.png" || fail "no window to capture"
  echo "$1 narrowest: $(grep 'mini player look ' "$log" | tail -1 | sed 's/.*mini player look //')" >> "$out/looks.txt"
  send pip
  mini_player_at 1 "the Mini Player from $2 went"
  send pip wheel $((pw / 2)) $((ph / 2)) -40 cmd
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/look-$1-widest.png" || fail "no window to capture"
  echo "$1 widest: $(grep 'mini player look ' "$log" | tail -1 | sed 's/.*mini player look //')" >> "$out/looks.txt"
  send click 300 200
  send key x
  expect "^browser: title $v landed\$" "the video from $2 didn't land"
}
looks short "$site"
looks long "$long"
cat "$out/looks.txt"
echo "ok: probe: the Mini Player's look at its narrowest and widest, from a short host and a long one."
