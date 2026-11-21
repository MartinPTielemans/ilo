# check.sh runs this at 720, only when named.
# With the lists uBlock Origin ships and nothing given for the check, a page's script from Google's
# ad network doesn't load, and an ad server's own page doesn't open: uBlock Origin shows its page
# saying it blocked it instead. uBlock Origin fails the script's request, or answers it with a
# stand-in of its own that loads, so the page's ads find nothing missing (its lists' filter for it
# says redirect=googlesyndication_adsbygoogle.js). Off the Mac it reports the most memory uBlock
# Origin's process took as it compiled its lists and wrote its selfie (#2069). It needs the network
# and those servers, so it runs only when named: ./check.sh adblock-live. The browser's own is the
# default (#2111), so the run gives --ad-block-engine=ublock.

# The served page's title says whether Google's ad script loaded, was blocked, or was blocked by
# uBlock Origin's stand-in, whose adsbygoogle.push does nothing, where Google's queues an ad.
serve
printf '%s' '<title>loading</title>
<script>
function loaded() {
  const push = String(self.adsbygoogle && self.adsbygoogle.push);
  document.title = /^function *\(\) *\{\s*;\s*\}$/.test(push) ?
    "ad script blocked by a stand-in" : "ad script loaded";
}
</script>
<script src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js" onload="loaded()"
  onerror="document.title = &quot;ad script blocked&quot;"></script>' > "$out/site/live.html"
ublock=oghkpboejbohceocapipmnjifoklonij
profile_dir="$PWD/$out/adblock-live/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/adblock-live.log"
memory="$out/adblock-live/ublock-memory.txt"
rm -f "$memory"
step=0
# uBlock Origin's process, once there is one, within $1 tenths of a second.
ublock_process() {
  for _ in $(seq "$1"); do
    pgrep -f "$procs.*--extension-process" | head -1 | grep . && return 0
    sleep 0.1
  done
  return 1
}
# Adds uBlock Origin's memory to $memory every quarter second until its process ends, a line each:
# the time in ms, then on Windows its working set and its commit in MB, on Linux its resident set.
# Commit is what ran out on Windows (#2069), and counts memory reserved and never touched, as the
# 3 bytes a character TextEncoder.encode takes for its worst case.
sample_ublock() {
  if windows; then
    ID=$1 OUT=$(cygpath -w "$memory") powershell.exe -NoProfile -NonInteractive -Command '
      $p = Get-Process -Id $env:ID
      while (-not $p.HasExited) {
        $p.Refresh()
        [IO.File]::AppendAllText($env:OUT, ("{0} {1} {2}`n" -f
          [DateTimeOffset]::Now.ToUnixTimeMilliseconds(),
          [math]::Round($p.WorkingSet64 / 1MB), [math]::Round($p.PrivateMemorySize64 / 1MB)))
        Start-Sleep -Milliseconds 250
      }' 2> /dev/null
  elif [ "$(uname)" = Linux ]; then
    while rss=$(awk '/^VmRSS:/ { print int($2 / 1024) }' "/proc/$1/status" 2> /dev/null) &&
      [ -n "$rss" ]; do
      echo "$(date +%s%3N) $rss" >> "$memory"
      sleep 0.25
    done
  fi
}
# The process's own peaks in MB, which catch what's freed between samples: on Windows its working
# set's and its commit's, on Linux its resident set's.
ublock_peaks() {
  if windows; then
    ID=$1 powershell.exe -NoProfile -NonInteractive -Command '$p = Get-Process -Id $env:ID;
      "$([math]::Round($p.PeakWorkingSet64 / 1MB)) $([math]::Round($p.PeakPagedMemorySize64 / 1MB))"' |
      tr -d '\r'
  else
    awk '/^VmHWM:/ { print int($2 / 1024) }' "/proc/$1/status"
  fi
}
BROWSER_DRIVE=1 BROWSER_URL="${site}live.html" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" --ad-block-engine=ublock < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
# The time to blocking counts from uBlock Origin's process starting, as it's loaded into the
# profile: no extension list names it since #2113, the app's log line among them (#2650).
ublock_id=$(ublock_process 600) || fail "uBlock Origin had no process 60 s after the launch"
loaded=$(date +%s)
if ! [ "$(uname)" = Darwin ]; then
  loaded_ms=$(date +%s%3N)
  sample_ublock "$ublock_id" &
  # Not $sampler, which quit clears (lib.sh's exited).
  ublock_sampler=$!
fi
await_load
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab ad script " || fail "the page didn't say whether the ad script loaded"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: adblock-live. Browser wasn't the active app, so the page didn't get its keys."
  exit 0
fi
# On a new profile uBlock Origin lets everything through until it has read and compiled the lists
# it ships, so the page reloads until its ad script is blocked, until 120 s after uBlock Origin
# loaded, each reload waiting up to 2 s for the page to say so. Blocking began between the last
# reload that let the script through and the one that didn't.
before=0
since=0
while ! grep -q "^browser: title $tab ad script blocked" "$log"; do
  before=$since
  since=$(($(date +%s) - loaded))
  [ "$since" -lt 120 ] ||
    fail "uBlock Origin's lists didn't block Google's ad script within 120 s of its loading"
  send key r cmd
  expect "^browser: title $tab ad script " "the page didn't report after ⌘R"
  for _ in $(seq 20); do
    none_since "^browser: title $tab ad script blocked" || break
    sleep 0.1
  done
done
loads=$(grep -c "^browser: title $tab ad script " "$log")
how=$(grep -m 1 "^browser: title $tab ad script blocked" "$log" | sed 's/.* ad script //')
echo "adblock-live: $how $before to $since s after uBlock Origin loaded, on load $loads"
blocked_ms=$(date +%s%3N)
../tools/window-screenshot.sh "$pid" "$out/adblock-live-script.png" || fail "no window to capture"
go_from_bar doubleclick.net
expect "^browser: loaded $tab chrome-extension://$ublock/document-blocked\.html" \
  "doubleclick.net opened, where uBlock Origin's page should have said it blocked it"
../tools/window-screenshot.sh "$pid" "$out/adblock-live-page.png" || fail "no window to capture"
if [ -n "${ublock_sampler:-}" ]; then
  # 53 s after its lists last changed it writes its filters' selfie into its cache, the largest
  # value it writes, which ran its process out of memory on Windows (#2069). It writes each value
  # to its IndexedDB and to its extension storage, and the peaks are read once both have the
  # selfie (lib.sh's ublock_selfie), or 180 s on.
  idb="$profile_dir/Default/IndexedDB/chrome-extension_${ublock}_0.indexeddb"
  local_storage="$profile_dir/Default/Local Extension Settings/$ublock"
  selfie="no selfie within 180 s"
  for _ in $(seq 180); do
    if ublock_selfie; then
      selfie="its selfie in its cache $(($(date +%s) - loaded)) s after it loaded"
      break
    fi
    sleep 1
  done
  peaks=$(ublock_peaks "$ublock_id")
  cache=$(du -sk "$idb.leveldb" "$idb.blob" "$local_storage" 2> /dev/null |
    awk '{ kb += $1 } END { print int(kb / 1024) }')
fi
quit
if [ -n "${ublock_sampler:-}" ]; then
  wait "$ublock_sampler"
  # From the samples: the most before its lists blocked, the most after and when, and the last.
  sampled=$(awk -v t0="$loaded_ms" -v blocked="$blocked_ms" '
    $1 <= blocked && $NF > early { early = $NF }
    $1 > blocked && $NF > late { late = $NF; at = $1 }
    { last = $NF }
    END { printf "%d MB until its lists blocked, then %d MB %.1f s after it loaded, ending at %d MB", early, late, (at - t0) / 1000, last }' "$memory")
  if windows; then
    echo "adblock-live: uBlock Origin's process committed at most ${peaks#* } MB and held at most ${peaks%% *} MB, with $selfie. Its commit sampled each quarter second: $sampled. Its cache is $cache MB. The samples: $memory"
  else
    echo "adblock-live: uBlock Origin's process held at most $peaks MB, with $selfie. Its resident set sampled each quarter second: $sampled. Its cache is $cache MB. The samples: $memory"
  fi
fi
echo "ok: with the lists uBlock Origin ships, Google's ad script doesn't load, and doubleclick.net shows uBlock Origin's page saying it blocked it. Windows: $out/adblock-live-script.png, $out/adblock-live-page.png"
