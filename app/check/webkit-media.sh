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
  cp ../spikes/webkit-media-session/index.html "$spike/"
  # Two looped tones, 2 s each.
  python3 -I -c '
import math, struct, sys, wave
for name, pitch in (("tone.wav", 440), ("tone2.wav", 660)):
    with wave.open(sys.argv[1] + "/" + name, "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(22050)
        out.writeframes(b"".join(struct.pack("<h", int(8000 * math.sin(2 * math.pi * pitch * i / 22050)))
                                 for i in range(22050 * 2)))
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
  "$spike/probe" "http://localhost:$spike_port/index.html" > "$log" 2>&1
  probed=$?
  kill "$spike_server" 2> /dev/null
  wait "$spike_server" 2> /dev/null
  cat "$log"
  [ "$probed" = 0 ] || fail "the probe exited $probed"
  echo "ok: the WebKit media probe ran; its answers are the webkit-media lines above"
else
  echo "skip: webkit-media: a spike for Safari's engine, which only the Mac has"
fi
