# check.sh runs this at 670.
# The foot bar, the window over the page's bottom centre that Arrange and pointing show (#2380),
# drawn by each OS's host: BROWSER_FOOT_BAR shows it with made-up words over the first page that
# loads, with one line (arrange), two (point), and a question past its two lines (long), and each is
# captured over the window with the host's material, corners and shadow. Nothing uses the bar yet,
# so this is where its host is seen.

page='data:text/html;charset=utf-8,<title>ready</title>
<body style="margin:0;height:100vh;background:linear-gradient(%23d9e2e6,%23f3f5f6)">'
shots=
for kind in arrange point long; do
  log="$out/foot-bar-$kind.log"
  rm -f "$saved" "$launch_frame"
  BROWSER_FOOT_BAR=$kind BROWSER_URL="$page" "$browser" < /dev/null > "$log" 2>&1 &
  pid=$!
  await_load
  await '^browser: foot bar shown over [0-9]*$' || fail "the foot bar wasn't shown ($kind)"
  await '^browser: foot bar drawn over [0-9]*$' || fail "the foot bar didn't draw ($kind)"
  ../tools/window-screenshot.sh "$pid" "$out/foot-bar-$kind.png" || fail "no window to capture"
  shots="$shots $out/foot-bar-$kind.png"
  quit
done
echo "ok: the foot bar showed and drew over the page with one line, two, and a long question." \
  "Windows:$shots"
