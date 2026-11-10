# Probe for #2322, not a check: what a page in Safari's engine asks of Apple's site-safety service,
# at the check's own 127.0.0.1 and at a public host. It records and never fails on what it finds.

serve
ssb_filter='subsystem CONTAINS[c] "safebrowsing" OR process CONTAINS[c] "safebrowsing" OR eventMessage CONTAINS[c] "safebrowsing" OR eventMessage CONTAINS[c] "safe browsing" OR eventMessage CONTAINS[c] "SSBLookup" OR eventMessage CONTAINS[c] "fraudulent"'
for host in 127.0.0.1 example.com; do
  case $host in
    127.0.0.1) url="http://127.0.0.1:$port/engine.html" ;;
    *) url="https://$host/" ;;
  esac
  log="$out/ssb-$host.log"
  step=0
  rm -f "$saved"
  forget_engine_sites
  start=$(date '+%Y-%m-%d %H:%M:%S')
  echo "ssb $host: before, processes: $(pgrep -fl -i safebrowsing | tr '\n' ';')"
  BROWSER_DRIVE=1 BROWSER_STREAMING_TEST_HOST=$host BROWSER_URL="$url" "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  await '^browser: system engine loaded for ' || echo "ssb $host: no system engine line"
  grep '^browser: \(loaded\|title\|system engine\)' "$log" | head -8 | sed "s/^/ssb $host: /"
  # The probe's window for a lookup the load set off to show in the unified log.
  sleep 15
  echo "ssb $host: after, processes: $(pgrep -fl -i safebrowsing | tr '\n' ';')"
  lsof -nP -i 2> /dev/null | grep -i -e safebrows -e 'WebKit' | sed "s/^/ssb $host lsof: /" | head -20
  log show --start "$start" --style compact --info --debug --predicate "$ssb_filter" > "$out/ssb-$host-unified.txt" 2>&1
  echo "ssb $host: unified log lines: $(wc -l < "$out/ssb-$host-unified.txt")"
  head -60 "$out/ssb-$host-unified.txt" | sed "s/^/ssb $host log: /"
  log show --start "$start" --style compact --info --predicate 'process BEGINSWITH "com.apple.WebKit"' > "$out/ssb-$host-webkit.txt" 2>&1
  echo "ssb $host: WebKit process log lines: $(wc -l < "$out/ssb-$host-webkit.txt")"
  grep -i -e safe -e ssb -e lookup -e phish -e malware "$out/ssb-$host-webkit.txt" | head -30 | sed "s/^/ssb $host webkit: /"
  exec 3>&-
  quit
done
echo "ssb: recorded, see the lines above"
