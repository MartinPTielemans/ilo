# With the lists uBlock Origin ships and nothing given for the check, a page's script from Google's
# ad network doesn't load, and an ad server's own page doesn't open: uBlock Origin shows its page
# saying it blocked it instead. uBlock Origin fails the script's request, or answers it with a
# stand-in of its own that loads, so the page's ads find nothing missing (its lists' filter for it
# says redirect=googlesyndication_adsbygoogle.js). It needs the network and those servers, so it
# runs only when named: ./check.sh adblock-live.

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
step=0
BROWSER_DRIVE=1 BROWSER_URL="${site}live.html" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
# The time to blocking counts from here, as uBlock Origin is loaded into the profile.
await "^browser: extensions 1 .*$ublock\$" || fail "uBlock Origin wasn't loaded"
loaded=$(date +%s)
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
../tools/window-screenshot.sh "$pid" "$out/adblock-live-script.png" || fail "no window to capture"
go_from_bar doubleclick.net
expect "^browser: loaded $tab chrome-extension://$ublock/document-blocked\.html" \
  "doubleclick.net opened, where uBlock Origin's page should have said it blocked it"
../tools/window-screenshot.sh "$pid" "$out/adblock-live-page.png" || fail "no window to capture"
quit
echo "ok: with the lists uBlock Origin ships, Google's ad script doesn't load, and doubleclick.net shows uBlock Origin's page saying it blocked it. Windows: $out/adblock-live-script.png, $out/adblock-live-page.png"
