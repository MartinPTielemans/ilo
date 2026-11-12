# #2439's probe, not to merge: whether the 2-10 s quits after Notion's page are its compression
# dictionaries, which the network service commits one by one in a sequence quitting waits for.
# Twelve launches open Notion's new page and quit as it loads, every other one with Chromium's
# CompressionDictionaryTransport off, and two quit a local page, for the floor. Each says how long
# its threads took to stop and how many dictionaries the profile then holds.
windows || fail "quitwait is Windows' probe"
probe_off=1
serve
dictionaries() {
  python3 -I -c 'import sqlite3, sys
try:
    print(sqlite3.connect("file:%s?mode=ro" % sys.argv[1], uri=True).execute("select count(*) from dictionaries").fetchone()[0])
except Exception as error:
    print("none (%s)" % error)' "$(native_path "$profile_dir/Default/Shared Dictionary/db")"
}
echo '<title>floor</title>floor' > "$out/site/floor.html"
for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14; do
  log="$out/quitwait-$n.log"
  step=0
  rm -f "$saved"
  case $n in
    1 | 14) url="${site}floor.html" how=floor ;;
    *[13579]) url=https://notion.new how=off ;;
    *) url=https://notion.new how=on ;;
  esac
  if [ "$how" = off ]; then
    BROWSER_DRIVE=1 BROWSER_URL=$url "$browser" \
      --disable-features=MacAppCodeSignClone,CompressionDictionaryTransport \
      < "$out/drive" > "$log" 2>&1 &
  else
    BROWSER_DRIVE=1 BROWSER_URL=$url "$browser" < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
  if [ "$how" = floor ]; then
    await '^browser: title [0-9]* floor$' || fail "quitwait $n: the local page didn't load"
  else
    await '^browser: page [0-9]* back 0 forward 0 loading 1 https://app\.notion\.com/' ||
      fail "quitwait $n: Notion's page didn't start loading"
  fi
  quit 30
  echo "quitwait $n $how: $(grep '^browser: quit: threads stopped' "$log" || echo 'no threads stopped'); dictionaries $(dictionaries)"
done
echo "ok: quitwait's quits are in $out/quitwait-*.log"
