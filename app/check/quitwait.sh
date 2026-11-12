# #2439's probe, not to merge: whether the 2-10 s quits after Notion's page are its compression
# dictionaries, which the network service commits one by one in a sequence quitting waits for.
# Twelve launches open Notion's new page and quit as it loads, every other one with Chromium's
# CompressionDictionaryTransport off, and two quit a local page, for the floor. Twelve more open a
# local page that registers new dictionaries, 17 a launch until the profile holds 99, so every OS
# gets the same count. Each says how long its threads took to stop and how many dictionaries the
# profile then holds.
[ "$(uname)" != Darwin ] || fail "quitwait is Windows' and Linux's probe"
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
# Fetches /dict/<launch>-<i> for each i below the query's number, then names itself.
echo '<title>dicts</title><script>
const n = +location.search.slice(1), launch = location.hash.slice(1);
Promise.all([...Array(n).keys()].map(i => fetch(`/dict/${launch}-${i}`).then(r => r.text())))
  .then(() => document.title = "dicts " + n, () => document.title = "dicts failed");
</script>' > "$out/site/dicts.html"
for n in $(seq 26); do
  log="$out/quitwait-$n.log"
  step=0
  rm -f "$saved"
  case $n in
    1 | 14) url="${site}floor.html" how=floor ;;
    1[5-9] | 2[0-6])
      held=$(dictionaries)
      case $held in none*) held=0 ;; esac
      more=$((99 - held))
      [ "$more" -le 17 ] || more=17
      url="${site}dicts.html?$more#$n"
      case $n in *[13579]) how=off ;; *) how=on ;; esac ;;
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
  case $url in
    *floor.html) await '^browser: title [0-9]* floor$' || fail "quitwait $n: the local page didn't load" ;;
    *dicts.html*) await '^browser: title [0-9]* dicts [0-9]*$' || fail "quitwait $n: the dictionaries page didn't finish" ;;
    *) await '^browser: page [0-9]* back 0 forward 0 loading 1 https://app\.notion\.com/' ||
      fail "quitwait $n: Notion's page didn't start loading" ;;
  esac
  quit 30
  echo "quitwait $n $how${more:+ +$more}: $(grep '^browser: quit: threads stopped' "$log" || echo 'no threads stopped'); dictionaries $(dictionaries)"
  more=
done
echo "ok: quitwait's quits are in $out/quitwait-*.log"
