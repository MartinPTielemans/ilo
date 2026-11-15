# check.sh runs this at 202.
# The browser's own ad blocker (--ad-block-engine=native, #2111) hides what a page's filters hide
# and runs their scriptlets from the page's start, as uBlock Origin does: a site's own rule, and a
# generic one that isn't a class or an id, hide their elements before the page's first frame, over
# the page's own !important, and a scriptlet sets a value before the page's first script reads it.
# Filters for another site do neither there. Generic rules that hide by a class or an id hide
# their elements soon after the page has them, on both sites, as uBlock Origin's do: one in the
# page, and one the page adds once it has loaded. The site's procedural filters apply too, out of
# the page's reach: :has-text() hides a post the page adds, :upward() styles an ad's box, and
# :style() a plain selector's.

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
<div class=check-class-ad style="height:60px;background:#c33">an ad by class</div>
<div class=check-box><div><span class=check-boxed-ad>an ad in a box</span></div></div>
<div class=check-styled style="height:60px">styled</div>
<script>
self.checkAdsShown = true;
const scriptlet = self.checkAdsShown === false ? "ran" : "didn'"'"'t run";
const hidden = (selector) => getComputedStyle(document.querySelector(selector)).display == "none";
const shown = (selector) => (hidden(selector) ? "hidden" : "shown");
requestAnimationFrame(() => setTimeout(() => {
  document.title = "site ad " + shown(".check-ad") + ", generic ad " + shown("[data-check-ad]") +
    ", scriptlet " + scriptlet;
  const late = document.createElement("div");
  late.id = "check-late-ad";
  late.style.cssText = "height:60px;background:#c33";
  late.textContent = "an ad by id, added late";
  document.body.append(late);
  const post = document.createElement("div");
  post.className = "check-post";
  post.textContent = "Sponsored post, added late";
  document.body.append(post);
  const opacity = (selector) => getComputedStyle(document.querySelector(selector)).opacity;
  // Reports once both are hidden, or what they are after 10 s.
  const since = performance.now();
  const report = () => {
    // The procedural filters are 127.0.0.1's alone, so elsewhere it doesn't wait for them.
    const done = hidden(".check-class-ad") && hidden("#check-late-ad") &&
      (location.hostname != "127.0.0.1" ||
        hidden(".check-post") && opacity(".check-box") == "0" && opacity(".check-styled") == "0");
    if (!done && performance.now() - since < 10000) {
      setTimeout(report, 50);
      return;
    }
    document.title = "class ad " + shown(".check-class-ad") + ", late id ad " +
      shown("#check-late-ad") + ", late post " + shown(".check-post") + ", box opacity " +
      opacity(".check-box") + ", styled opacity " + opacity(".check-styled");
  };
  report();
}));
</script>' > "$out/site/cosmetic.html"
page="${site}cosmetic.html"
# localhost is another site, where the site's rule and scriptlet don't apply.
other="http://localhost:$port/cosmetic.html"
# EasyList turns generic rules off on 127.0.0.1 and localhost, for uBlock Origin too, so the
# filters take those exceptions back with $badfilter for the generic rule to apply here.
filters="$(printf '%s\n' '127.0.0.1##.check-ad' '##[data-check-ad]' '##.check-class-ad' \
  '###check-late-ad' \
  '127.0.0.1##+js(set, checkAdsShown, false)' \
  '127.0.0.1##.check-post:has-text(Sponsored)' '127.0.0.1##.check-boxed-ad:upward(2):style(opacity: 0)' \
  '127.0.0.1##.check-styled:style(opacity: 0 !important)' \
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
  await "^browser: title $tab class ad " || fail "the page at $1 didn't report its later ads"
  later=$(grep "^browser: title $tab class ad " "$log" | head -1 | cut -d' ' -f4-)
}
report_at "$page" adblock-cosmetic
[ "$reported" = "site ad hidden, generic ad hidden, scriptlet ran" ] ||
  fail "the page's filters didn't hide and run from its start: $reported"
[ "$later" = "class ad hidden, late id ad hidden, late post hidden, box opacity 0, styled opacity 0" ] ||
  fail "the generic rules by class and id or the site's procedural filters didn't apply: $later"
../tools/window-screenshot.sh "$pid" "$out/adblock-cosmetic.png" || fail "no window to capture"
quit
report_at "$other" adblock-cosmetic-other
[ "$reported" = "site ad shown, generic ad hidden, scriptlet didn't run" ] ||
  fail "on another site, the site's filters applied or the generic one didn't: $reported"
[ "$later" = "class ad hidden, late id ad hidden, late post shown, box opacity 1, styled opacity 1" ] ||
  fail "on another site, the generic rules didn't hide, or the site's procedural filters applied: $later"
quit
echo "ok: the browser's own ad blocker hid the site's ad and a generic one from the page's first" \
  "frame, over its own !important, and ran its scriptlet before the page's first script; it hid" \
  "an ad by a generic class rule and one the page added later by a generic id rule, and its" \
  "procedural filters hid a post the page added and styled an ad's box and a plain selector; on" \
  "another site, only the generic rules applied. Window: $out/adblock-cosmetic.png"
