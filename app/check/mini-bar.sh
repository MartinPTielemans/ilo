# Arrange's mini bar, the window over the thing selected on a page (#2347), drawn by each OS's host:
# BROWSER_MINI_BAR shows it over a made-up thing selected on the first page that loads, with Hide
# and Put back (selected), the pin as the thing stays on screen (float), and as it scrolls with the
# page (scrolls), and each is captured over the window with the host's material, corners and
# shadow. The core's mode drives the bar once its slice lands, so this is where its host is seen.

page='data:text/html;charset=utf-8,<title>ready</title>
<body style="margin:0;height:100vh;background:linear-gradient(%23d9e2e6,%23f3f5f6)">'
shots=
for kind in selected float scrolls; do
  log="$out/mini-bar-$kind.log"
  rm -f "$saved" "$launch_frame"
  BROWSER_MINI_BAR=$kind BROWSER_URL="$page" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  await '^browser: mini bar shown over [0-9]*$' || fail "the mini bar wasn't shown ($kind)"
  await '^browser: mini bar drawn over [0-9]*$' || fail "the mini bar didn't draw ($kind)"
  ../tools/window-screenshot.sh "$pid" "$out/mini-bar-$kind.png" || fail "no window to capture"
  shots="$shots $out/mini-bar-$kind.png"
  quit
done
echo "ok: the mini bar showed and drew over a thing selected, with Hide and Put back, and the pin" \
  "either way. Windows:$shots"
