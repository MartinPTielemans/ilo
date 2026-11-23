# check.sh runs this at 351.
# A page that steps its own history without pause, back and forth between two entries of one
# document as an app's router does, and has a beforeunload listener that answers at once: ⌘W
# closes its tab, and ⌘Q, answering Quit, quits, the first time each is asked. Chromium committed
# each step in the page's own frame and cleared there its wait for the page's answer to the close
# or the quit, then dropped the answer, so neither came (#3920). Blink lets a page make 200 such
# steps in 10 s, which this page would use up before either is asked, so the launch lifts that
# limit: a router stepping at the limit's 20 a second meets the same drop, only less often. Each
# row has a launch of its own, and the check says how each went before it fails.

serve
printf '%s' '<title>history</title>
<script>
addEventListener("beforeunload", () => {});
const step = () => {
  history.pushState(null, "", "?step");
  history.back();
};
addEventListener("popstate", () => setTimeout(step));
addEventListener("load", () => setTimeout(step));
</script>' > "$out/site/quit-history.html"

# Starts the app for the row $1, its log quit-history-$1.log, on a profile with no Spaces saved,
# with the page, and waits until the page's history is stepping. Sets $tab to the page's tab.
start() {
  row=$1
  log="$out/quit-history-$row.log"
  step=0
  rm -f "$saved"
  BROWSER_DRIVE=1 BROWSER_URL="${site}quit-history.html" \
    "$browser" --disable-ipc-flooding-protection < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await "^browser: loaded [0-9]* ${site}quit-history.html\$" || fail "$row: the page didn't load"
  tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}quit-history.html\$|\1|p" "$log" | head -1)
  await "^browser: page $tab back [0-9]* forward 1 " ||
    fail "$row: the page's history didn't step back, so there was nothing for the row to ask during"
}
# The row missed what it waited for: notes the wait with the row's lines, for the end, and ends
# the app, which a close or quit still waiting on the page would leave running.
missed() {
  missing="$missing
FAIL: $1
$(wait_report)"
  exec 3>&-
  if windows; then
    taskkill //F //T //PID "$(cat "/proc/$pid/winpid")" > /dev/null 2>&1 || true
  else
    kill -9 "$pid" 2>/dev/null || true
    pkill -9 -f "$procs" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}

missing=
start close
send key w cmd
if await "^browser: tab $tab closed\$"; then
  ! kill -0 "$pid" 2>/dev/null || quit_app
  exited
else
  missed "⌘W didn't close the tab of a page stepping its own history in 30 s: Chromium never took the page's beforeunload answer for the close"
fi

start quit
quit_by_key
if await '^browser: quit going ahead'; then
  exited
  grep -q '^browser: stopping$' "$log" || fail "quit: the app ended without stopping as a quit does"
else
  missed "⌘Q, answered Quit, didn't quit with a page stepping its own history in 30 s: Chromium never took the page's beforeunload answer for the quit"
fi

if [ -n "$missing" ]; then
  waited_for=
  log=/dev/null
  fail "a close or a quit asked while a page stepped its history didn't come the first time:$missing"
fi
echo "ok: with a page stepping its own history, ⌘W closed its tab and ⌘Q, answering Quit, quit, each the first time"
