# #2439's probe, not to merge: ten launches each open Notion's new page and quit as it loads, as
# profiles' last quit does after ⌃⇧N, which twice took 10 s to stop the app's threads. A quit whose
# threads haven't stopped 3 s after Chromium said it was stopping gets a minidump of the app, and
# its threads' names, in quitwait-<n>.dmp and quitwait-<n>.threads, from lib.sh's exited().
windows || fail "quitwait is Windows' probe"
for n in 1 2 3 4 5 6 7 8 9 10; do
  log="$out/quitwait-$n.log"
  step=0
  rm -f "$saved"
  BROWSER_DRIVE=1 BROWSER_URL=https://notion.new "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: page [0-9]* back 0 forward 0 loading 1 https://app\.notion\.com/' ||
    fail "quitwait $n: Notion's page didn't start loading"
  quit 30
  echo "quitwait $n: $(grep '^browser: quit: threads stopped' "$log" || echo 'no threads stopped')"
done
echo "ok: quitwait's ten quits are in $out/quitwait-*.log"
