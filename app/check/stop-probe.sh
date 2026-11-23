# check.sh runs this at 996, only when named.
# Never to land (#3946 probe): relaunch's quit a page stops, 15 times, each on a new profile: ⌘Q,
# Return on the app's question, then the page's Leave site?, then Esc to stay. The eighteenth's Mac
# leg saw the tab close under the page's unanswered question before the Esc (check-mac 45194247447),
# once in 11 Mac legs, so one launch rarely shows it. The core's probe3946 lines in each launch's
# log say who answered the question and who closed the tab, with the stack. Fails at the end if any
# launch lost its tab, after all 15, so every launch is read.
asks='data:text/html,<title>asks</title><script>onkeydown = () => { onbeforeunload = e => e.preventDefault(); document.title = "armed" }</script>'
cp "$prefs" "$out/stop-probe-prefs"
lost=0
results=
for i in $(seq 15); do
  rm -rf "${profile_dir:?}"
  mkdir -p "$profile_dir/Default"
  cp "$out/stop-probe-prefs" "$prefs"
  log="$out/stop-probe-$i.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$asks" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: title [0-9]* asks$' || fail "launch $i: the page that asks before it's left didn't load"
  asking=$(grep '^browser: title [0-9]* asks$' "$log" | tail -1 | cut -d' ' -f3)
  send click $page_point
  send key u
  expect "^browser: title $asking armed\$" "launch $i: the page didn't ask to be asked before it's left"
  post key q cmd
  expect "^browser: dialog shown Quit $app_name?\$" "launch $i: ⌘Q didn't ask before quitting"
  expect '^browser: dialog settled$' "launch $i: the dialog asking to quit didn't settle"
  post key return
  expect "^browser: page $asking dialog Leave " "launch $i: the quit didn't ask the page"
  expect '^browser: dialog settled$' "launch $i: the page's question didn't settle"
  before=no
  ! grep -q "^browser: tab $asking closed\$" "$log" || before=yes
  post key escape
  how=
  for _ in $(seq 50); do
    if grep -q "^browser: page $asking dialog answered cancel\$" "$log"; then
      how=stayed
      break
    fi
    kill -0 "$pid" 2> /dev/null || { how=quit; break; }
    sleep 0.1
  done
  case $how in
  stayed)
    post key q cmd
    expect "^browser: dialog shown Quit $app_name?\$" "launch $i: the second ⌘Q didn't ask"
    expect '^browser: dialog settled$' "launch $i: the second quit's dialog didn't settle"
    post key return
    expect "^browser: page $asking dialog Leave " "launch $i: the second quit didn't ask the page"
    expect '^browser: dialog settled$' "launch $i: the page's second question didn't settle"
    post key return
    exited
    ;;
  quit)
    lost=$((lost + 1))
    exited
    ;;
  *)
    lost=$((lost + 1))
    how="neither stayed nor quit in 5 s"
    exec 3>&-
    kill "$pid" 2> /dev/null || true
    sleep 2
    ;;
  esac
  echo "stop-probe: launch $i: $how, tab closed before the Esc: $before"
  results="$results $i:$how/$before"
done
[ "$lost" = 0 ] || fail "stop-probe: $lost of 15 launches lost the tab under its question:$results"
echo "ok: stop-probe: 15 launches stayed on Esc:$results"
