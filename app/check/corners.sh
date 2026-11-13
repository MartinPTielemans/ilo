# check.sh runs this at 990, only when named.
# Proof only (#2662): a dark page alone in the window, captured with the window light and then dark,
# so a page's corners can be compared before and after on a dark page as well as the light ones the
# other runs capture.

page='data:text/html,<title>loading</title>
<body style="margin:0;height:100vh;background:%23161616;color:%23e8e8e8">
<h1 style="font:bold 48px system-ui;margin:40px">A dark page</h1>
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = "dark"));</script>'
log="$out/corners.log"
step=0
rm -f "$saved" "$launch_frame"
forget_appearance
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* dark$' || fail "the dark page didn't draw"
../tools/window-screenshot.sh "$pid" "$out/corners-dark-page-light-window.png" ||
  fail "no window to capture"
send menu "Appearance › Dark"
expect '^browser: window light false$' "View › Appearance › Dark didn't turn the window dark"
../tools/window-screenshot.sh "$pid" "$out/corners-dark-page-dark-window.png" ||
  fail "no window to capture"
send menu "Appearance › Automatic"
exec 3>&-
quit
echo "ok: proof only (#2662): a dark page in a light window and in a dark one." \
  "Windows: $out/corners-dark-page-light-window.png, $out/corners-dark-page-dark-window.png"
