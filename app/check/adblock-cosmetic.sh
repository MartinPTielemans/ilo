# The browser's own ad blocker (--ad-block-engine=native, #2111) hides what a page's filters hide
# and runs their scriptlets from the page's start, as uBlock Origin does: a site's own rule, and a
# generic one that isn't a class or an id, hide their elements before the page's first frame, over
# the page's own !important, and a scriptlet sets a value before the page's first script reads it.
# Filters for another site do neither there.

# An engine from before the page's rules has no agent to send them to.
if ! engine_has ../core/ad_block_page_agent.cc ../core/native_ad_block.cc \
  ../engine/patches/ours/chromium-core-ad-block-page.patch; then
  echo "skip: adblock-cosmetic. This engine is from before the native ad blocker's page rules."
  exit 0
fi
serve
printf '%s' '<title>loading</title>
<style>.check-ad { display: block !important }</style>
<div class=check-ad style="height:60px;background:#c33">an ad</div>
<div data-check-ad style="height:60px;background:#c33">a generic ad</div>
<script>
self.checkAdsShown = true;
const scriptlet = self.checkAdsShown === false ? "ran" : "didn'"'"'t run";
const hidden = (selector) => getComputedStyle(document.querySelector(selector)).display == "none";
const shown = (selector) => (hidden(selector) ? "hidden" : "shown");
requestAnimationFrame(() => setTimeout(() => {
  document.title = "site ad " + shown(".check-ad") + ", generic ad " + shown("[data-check-ad]") +
    ", scriptlet " + scriptlet;
}));
</script>' > "$out/site/cosmetic.html"
page="${site}cosmetic.html"
# localhost is another site, where the site's rule and scriptlet don't apply.
other="http://localhost:$port/cosmetic.html"
# EasyList turns generic rules off on 127.0.0.1 and localhost, for uBlock Origin too, so the
# filters take those exceptions back with $badfilter for the generic rule to apply here.
filters="$(printf '%s\n' '127.0.0.1##.check-ad' '##[data-check-ad]' \
  '127.0.0.1##+js(set, checkAdsShown, false)' \
  '@@://127.0.0.1$generichide,badfilter' '@@://localhost:$generichide,badfilter')"
# Launches the app at $1 on a new profile, logging to $2, and sets $reported to the page's first
# report, from its first frame, so nothing the page showed before the blocker's rules came in
# passes. No input, so the app needn't be the active one.
report_at() {
  profile_dir="$PWD/$out/$2/profile"
  rm -rf "$profile_dir"
  mkdir -p "$profile_dir"
  log="$out/$2.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="$1" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" --ad-block-engine=native --ad-block-filters="$filters" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
  tab=$(today | cut -d' ' -f1)
  await "^browser: title $tab site ad " || fail "the page at $1 didn't report"
  reported=$(grep "^browser: title $tab site ad " "$log" | head -1 | cut -d' ' -f4-)
}
report_at "$page" adblock-cosmetic
[ "$reported" = "site ad hidden, generic ad hidden, scriptlet ran" ] ||
  fail "the page's filters didn't hide and run from its start: $reported"
../tools/window-screenshot.sh "$pid" "$out/adblock-cosmetic.png" || fail "no window to capture"
quit
report_at "$other" adblock-cosmetic-other
[ "$reported" = "site ad shown, generic ad hidden, scriptlet didn't run" ] ||
  fail "on another site, the site's filters applied or the generic one didn't: $reported"
quit
echo "ok: the browser's own ad blocker hid the site's ad and a generic one from the page's first" \
  "frame, over its own !important, and ran its scriptlet before the page's first script; on" \
  "another site, only the generic rule applied. Window: $out/adblock-cosmetic.png"
