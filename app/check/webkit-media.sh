# check.sh runs this at 990, only when named.
# A spike for #3432, not a check: spikes/webkit-media-session/probe.m in a WKWebView of its own,
# with no build of the app or the engine, says what WebKit tells its host about a page's media and
# what a script world of the host's can read and do. Its lines are "webkit-media: ...".
if [ "$(uname)" = Darwin ]; then
  spike="$out/webkit-media"
  rm -rf "$spike"
  mkdir -p "$spike"
  log="$spike/probe.log"
  : > "$log"
  cp ../spikes/webkit-media-session/index.html ../spikes/webkit-media-session/detached.html "$spike/"
  # Two looped tones, 30 s each, long enough for WebKit to count them as Now Playing.
  python3 -I -c '
import math, struct, sys, wave
for name, pitch in (("tone.wav", 440), ("tone2.wav", 660)):
    with wave.open(sys.argv[1] + "/" + name, "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(22050)
        out.writeframes(b"".join(struct.pack("<h", int(8000 * math.sin(2 * math.pi * pitch * i / 22050)))
                                 for i in range(22050 * 30)))
' "$spike"
  clang -fobjc-arc -Wall -framework Cocoa -framework WebKit -framework MediaPlayer \
    -o "$spike/probe" ../spikes/webkit-media-session/probe.m 2> "$spike/build.log" ||
    fail "the probe didn't build: $(head -20 "$spike/build.log")"
  spike_port=$(python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
  python3 -I -m http.server --bind 127.0.0.1 --directory "$spike" "$spike_port" > "$spike/server.log" 2>&1 &
  spike_server=$!
  for _ in $(seq 50); do
    curl -sf "http://127.0.0.1:$spike_port/index.html" > /dev/null && break
    sleep 0.1
  done
  # The runs go under set -e, so a status the probe or the stopped server ends with is kept.
  probed=0
  "$spike/probe" "http://localhost:$spike_port/index.html" > "$log" 2>&1 || probed=$?
  # Then music no document holds, on a muted page, as the app's media check plays it.
  detached=0
  "$spike/probe" "http://localhost:$spike_port/detached.html" detached > "$spike/probe-detached.log" 2>&1 ||
    detached=$?
  kill "$spike_server" 2> /dev/null || true
  wait "$spike_server" 2> /dev/null || true
  cat "$log" "$spike/probe-detached.log"
  [ "$probed" = 0 ] || fail "the probe exited $probed"
  [ "$detached" = 0 ] || fail "the detached probe exited $detached"
  echo "ok: the WebKit media probe ran; its answers are the webkit-media lines above"
else
  echo "skip: webkit-media: a spike for Safari's engine, which only the Mac has"
fi
