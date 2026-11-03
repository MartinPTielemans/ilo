# uBlock Origin's filter lists come from the source the core gives it (kFilterListsSource in
# core/ad_block.cc), here the check's own, through --ad-block-lists: uBlock Origin's assets.json
# with every remote address taken out, so nothing reaches the network, and one list more, on the
# check's server. A new profile fetches that list and blocks the script its rule names. Stale
# 20 s after it's fetched, the list is fetched again 37 s after the launch, and not before.
# Not here: the weekly update while the browser stays open, a week after the last.

serve
if [ -d "$app/ublock" ]; then
  ublock="$app/ublock"
else
  ublock="$(echo "$app"/Contents/Frameworks/*Framework.framework)/Resources/ublock"
fi
mkdir "$out/site/lists"
printf '%s\n' '! Title: Check list' '/listed-ad.js' > "$out/site/lists/check-list.txt"
python3 -I - "$ublock/assets/assets.json" "${site}lists/check-list.txt" \
  > "$out/site/lists/assets.json" << 'EOF'
import json, sys
# uBlock Origin's file is UTF-8, which Windows' Python doesn't read by default (#872).
with open(sys.argv[1], encoding="utf-8") as f:
    assets = json.load(f)
listed = {}
for key, entry in assets.items():
    urls = entry.get("contentURL", [])
    local = [url for url in ([urls] if isinstance(urls, str) else urls)
             if not url.startswith(("http:", "https:"))]
    if local:
        entry["contentURL"] = local
        entry.pop("cdnURLs", None)
        entry.pop("patchURLs", None)
        listed[key] = entry
# A day is 86400 s, so 20 s is stale by uBlock Origin's launch check 37 s in.
listed["check-list"] = {
    "content": "filters",
    "group": "default",
    "title": "Check list",
    "contentURL": [sys.argv[2]],
    "updateAfter": 20 / 86400,
}
json.dump(listed, sys.stdout)
EOF
printf '%s' 'self.listedLoaded = true;' > "$out/site/listed-ad.js"
printf '%s' '<title>loading</title>
<script src="listed-ad.js"></script>
<script>
requestAnimationFrame(() => setTimeout(() => {
  document.title = "listed script " + (self.listedLoaded ? "loaded" : "blocked");
}));
</script>' > "$out/site/listed.html"
listed="${site}listed.html"
# How many times the server has sent the list.
fetches() {
  grep -c "^GET chromium 127.0.0.1:$port/lists/check-list.txt" "$out/site.log" || true
}
profile_dir="$PWD/$out/adblock-lists/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
log="$out/adblock-lists.log"
step=0
launched=$(date +%s)
BROWSER_DRIVE=1 BROWSER_URL="$listed" BROWSER_PROFILE_DIR="$profile_dir" \
  "$browser" --ad-block-lists="${site}lists/assets.json" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
served "/lists/assets.json" "uBlock Origin didn't fetch the check's assets.json"
served "/lists/check-list.txt" "uBlock Origin didn't fetch the check's list"
fetched_at="$(($(date +%s) - launched))"
first=$(fetches)
tab=$(today | cut -d' ' -f1)
await "^browser: title $tab listed script " || fail "the list's page didn't report"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: adblock-lists. Browser wasn't the active app, so the page didn't get its keys."
else
  # uBlock Origin lets the page through until it has read the list, then blocks its script at the
  # next load, so the tab reloads once its page has reported, until it's blocked.
  for _ in $(seq 100); do
    send key r cmd
    expect "^browser: title $tab listed script " "the list's page didn't report after ⌘R"
    grep "^browser: title $tab listed script " "$log" | tail -1 | grep -q ' blocked$' && break
  done
  grep "^browser: title $tab listed script " "$log" | tail -1 | grep -q ' blocked$' ||
    fail "the check's list didn't block the script its rule names"
  ../tools/window-screenshot.sh "$pid" "$out/adblock-lists.png" || fail "no window to capture"
  # The launch's check, 37 s in, by the server's log, and up to 60 s for a slow start. A fetch
  # since the first, sooner, would be one uBlock Origin's 37 s doesn't allow.
  for _ in $(seq 600); do
    [ "$(fetches)" -gt "$first" ] && break
    [ $(($(date +%s) - launched)) -lt 60 ] || break
    kill -0 "$pid" 2>/dev/null || fail "the app exited"
    sleep 0.1
  done
  refetched_at="$(($(date +%s) - launched))"
  [ "$(fetches)" -gt "$first" ] ||
    fail "the stale list wasn't fetched again within 60 s of the launch"
  # Whole seconds, so 36 is the earliest the 37 s can read.
  [ "$refetched_at" -ge 36 ] ||
    fail "the stale list was fetched again $refetched_at s after the launch, before the 37 s"
  exec 3>&-
  quit
  echo "ok: uBlock Origin read its lists' source from --ad-block-lists, the check's assets.json," \
    "fetched the check's list $fetched_at s after the launch and blocked the script it names," \
    "and fetched the stale list again $refetched_at s after the launch, 37 s in, not before." \
    "Window: $out/adblock-lists.png"
fi
