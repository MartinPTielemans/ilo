#include "browser_core/ad_block_list_check.h"

#include <optional>
#include <string>
#include <string_view>

#include "base/time/time.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::ad_block_list_check {
namespace {

// A list of |rules| rules, under a header and comments as uBlock Origin's lists have them.
std::string List(int rules) {
  std::string list = "[Adblock Plus 2.0]\n! Title: A list\n! Expires: 5 days\n\n";
  for (int rule = 0; rule < rules; ++rule) {
    list += "||ads" + std::to_string(rule) + ".example^\n";
  }
  return list;
}

// |list|, under a header saying it was updated at |updated|, as URLhaus's says it.
std::string Dated(std::string_view updated, std::string_view list) {
  return "! Title: A list\n! Updated: " + std::string(updated) + "\n" + std::string(list);
}

// Whether |downloaded| takes |last_good|'s place, for the rules, which ask nothing more.
bool Takes(std::string_view downloaded, std::optional<std::string_view> last_good) {
  return Judge(downloaded, last_good) == Download::kTaken;
}

base::Time Utc(int year, int month, int day, int hour, int minute, int second) {
  base::Time time;
  EXPECT_TRUE(base::Time::FromUTCExploded({year, month, 0, day, hour, minute, second, 0}, &time));
  return time;
}

TEST(AdBlockListCheck, CountsRulesNotCommentsAsUBlockOriginReadsThem) {
  EXPECT_EQ(Rules(List(3)), 3u);
  EXPECT_EQ(Rules("# a hosts file\n0.0.0.0 ads.example\r\n#\n  \n"), 1u);
  EXPECT_EQ(Rules("example.com##.ad\nexample.com#@#.ad\nexample.com#?#.ad:has(a)\n"
                  "example.com#$#.ad { display: none }\nexample.com#%#//scriptlet('x')\n"),
            5u)
      << "cosmetic filters are rules, though they start with # where a site is left out";
  EXPECT_EQ(Rules("!#if env_chromium\n||ads.example^\n!#endif"), 1u);
}

TEST(AdBlockListCheck, ADownloadThatDoesntReadAsAListIsRefused) {
  const std::string good = List(100);
  EXPECT_TRUE(Takes(List(100), good));
  EXPECT_TRUE(Takes(List(100), std::nullopt)) << "a list the browser has no copy of";
  EXPECT_FALSE(Takes("<!doctype html><title>502 Bad Gateway</title>", good))
      << "a server's page about an error";
  EXPECT_FALSE(Takes("\n  <html><body>Moved</body></html>", std::nullopt));
  EXPECT_FALSE(Takes("", good));
  EXPECT_FALSE(Takes(List(0), std::nullopt)) << "a header with no rules";
  EXPECT_FALSE(Takes(List(100) + "\xff\xfe", good)) << "not text";
  EXPECT_FALSE(Takes(std::string(List(100)).append(1, '\0'), good)) << "not text";
}

TEST(AdBlockListCheck, ADownloadThatLostMoreThanHalfTheLastGoodRulesIsRefused) {
  const std::string good = List(100);
  EXPECT_TRUE(Takes(List(120), good)) << "rules added";
  EXPECT_TRUE(Takes(List(90), good)) << "rules taken out, as a list's authors do";
  EXPECT_TRUE(Takes(List(50), good)) << "half kept";
  EXPECT_FALSE(Takes(List(49), good)) << "more than half lost, as a download cut short";
  EXPECT_FALSE(Takes(List(1), good));
}

TEST(AdBlockListCheck, ReadsWhenAListSaysItWasUpdated) {
  EXPECT_EQ(Updated("! Title: Online Malicious URL Blocklist\n! Updated: 2026-11-19T00:10:18Z\n"
                    "! Expires: 12 hours (update frequency)\n1.70.76.88\n"),
            Utc(2026, 11, 19, 0, 10, 18))
      << "URLhaus's";
  EXPECT_EQ(Updated("[Adblock Plus 2.0]\n! Version: 202611181215\n! Title: EasyList\n"
                    "! Last modified: Wed, 18 Nov 2026 12:15:39 +0000\n||ads.example^\n"),
            Utc(2026, 11, 18, 12, 15, 39))
      << "EasyList's and uBlock Origin's";
  EXPECT_EQ(Updated("# Peter Lowe's list\n#\n# last updated:   Wed, 18 Nov 2026 10:50:36 GMT\n"
                    "127.0.0.1 ads.example\n"),
            Utc(2026, 11, 18, 10, 50, 36))
      << "a hosts file's";
  EXPECT_EQ(Updated(Dated("2026-11-19", "||ads.example^\n")), Utc(2026, 11, 19, 0, 0, 0));
  EXPECT_EQ(Updated(List(3)), std::nullopt) << "a list that says no date";
  EXPECT_EQ(Updated("! Last modified: %timestamp%\n||ads.example^\n"), std::nullopt)
      << "a source copy whose date its build fills in";
  EXPECT_EQ(Updated(Dated("2026-11-19T00:10:18+01:00", "||ads.example^\n")), std::nullopt)
      << "an offset this doesn't read";
  EXPECT_EQ(Updated("||ads.example^\n! Updated: 2026-11-19T00:10:18Z\n"), std::nullopt)
      << "a comment after the first rule isn't the header";
}

// A mirror that stopped updating serves a copy older than the one held: refused, so the next
// address is tried, whatever its rules (#3505).
TEST(AdBlockListCheck, ADownloadOlderThanTheCopyHeldIsRefused) {
  const std::string held = Dated("2026-11-19T00:10:18Z", List(100));
  EXPECT_EQ(Judge(Dated("2026-10-14T00:10:05Z", List(100)), held), Download::kRefused)
      << "a mirror a month behind, with as many rules";
  EXPECT_EQ(Judge(Dated("2026-11-19T00:10:17Z", List(200)), held), Download::kRefused)
      << "a second older";
  EXPECT_EQ(Judge(Dated("2026-11-19T12:10:18Z", List(100)), held), Download::kTaken)
      << "a newer copy";
  EXPECT_EQ(
      Judge(Dated("2026-10-14T00:10:05Z", List(100)), Dated("2026-09-16T00:10:16Z", List(100))),
      Download::kTaken)
      << "the stale mirror's copy is newer than the one the browser came with";
  EXPECT_EQ(Judge(Dated("2026-11-20T00:10:18Z", List(49)), held), Download::kRefused)
      << "a newer copy that lost more than half the rules is still cut short";
  EXPECT_EQ(Judge("! Last modified: Thu, 19 Nov 2026 12:15:39 +0000\n" + List(100),
                  "! Last modified: Wed, 18 Nov 2026 12:15:39 +0000\n" + List(100)),
            Download::kTaken)
      << "EasyList's date reads as well as URLhaus's";
  EXPECT_EQ(Judge("! Last modified: Tue, 17 Nov 2026 12:15:39 +0000\n" + List(100),
                  "! Last modified: Wed, 18 Nov 2026 12:15:39 +0000\n" + List(100)),
            Download::kRefused);
}

// The list's first address is a mirror that stopped where the copy held came from, so each
// update's first download says the same date as the held copy. That's nothing new, not a copy to
// take, so the next address gets its turn and gives the newer one (#3505).
TEST(AdBlockListCheck, AFirstMirrorThatStoppedGivesTheNextItsTurn) {
  const std::string held = Dated("2026-10-14T00:10:05Z", List(100));
  EXPECT_EQ(Judge(Dated("2026-10-14T00:10:05Z", List(100)), held), Download::kNothingNew)
      << "the first mirror, stopped at the held copy";
  EXPECT_EQ(Judge(Dated("2026-11-19T00:10:18Z", List(100)), held), Download::kTaken)
      << "the second, current";
  EXPECT_EQ(Judge("! Last modified: Wed, 18 Nov 2026 12:15:39 +0000\n" + List(101),
                  "! Last modified: Wed, 18 Nov 2026 12:15:39 +0000\n" + List(100)),
            Download::kNothingNew)
      << "EasyList's date, the same, though a mirror's copy differs";
  EXPECT_EQ(Judge(Dated("2026-10-14T00:10:05Z", List(49)), held), Download::kRefused)
      << "half the rules lost is a fault, whatever the date: that check comes first";
}

// A list that hasn't changed since the last update, the common day: every address says the
// held copy's date. Each is nothing new, never refused, so the held copy stays with no line in
// the log.
TEST(AdBlockListCheck, AListWithNothingNewAtAnyAddressIsNoFault) {
  const std::string held = Dated("2026-11-19T00:10:18Z", List(100));
  for (const std::string& address :
       {Dated("2026-11-19T00:10:18Z", List(100)), Dated("2026-11-19T00:10:18Z", List(100)),
        Dated("2026-11-19T00:10:18Z", List(99)), Dated("2026-11-19T00:10:18Z", List(100))}) {
    EXPECT_EQ(Judge(address, held), Download::kNothingNew);
  }
}

// With no date to go by on either side, a download is judged by its rules, as before.
TEST(AdBlockListCheck, ACopyThatSaysNoDateIsJudgedByItsRulesAlone) {
  const std::string held = Dated("2026-11-19T00:10:18Z", List(100));
  EXPECT_EQ(Judge(List(100), held), Download::kTaken) << "a download that says no date";
  EXPECT_EQ(Judge(List(49), held), Download::kRefused);
  EXPECT_EQ(Judge(Dated("2026-10-14T00:10:05Z", List(100)), List(100)), Download::kTaken)
      << "a held copy that says no date";
  EXPECT_EQ(Judge(Dated("not a date", List(100)), held), Download::kTaken)
      << "a date that doesn't read";
  EXPECT_EQ(Judge(List(100), List(100)), Download::kTaken)
      << "neither says a date, so nothing tells a copy as new from a newer one";
}

// The first download of a list, with no copy held, has nothing to be older than: it's taken if
// it reads as a list, whatever its date.
TEST(AdBlockListCheck, AFirstDownloadIsTakenWhateverItsDate) {
  EXPECT_EQ(Judge(Dated("2026-10-13T00:10:05Z", List(100)), std::nullopt), Download::kTaken);
  EXPECT_EQ(Judge(Dated("2026-11-19T00:10:18Z", List(1)), std::nullopt), Download::kTaken);
  EXPECT_EQ(Judge(Dated("2026-11-19T00:10:18Z", List(0)), std::nullopt), Download::kRefused)
      << "no rules";
  EXPECT_EQ(Judge("<!doctype html><title>404</title>", std::nullopt), Download::kRefused);
}

TEST(AdBlockListCheck, APageLooksAtTheListsAnHourAfterTheLastLook) {
  const base::TimeTicks launch = base::TimeTicks::Now();
  EXPECT_FALSE(PageLooks(base::TimeTicks(), launch + base::Days(1)))
      << "launch's look, 37 s in, comes first";
  EXPECT_FALSE(PageLooks(launch, launch + base::Minutes(59)));
  EXPECT_TRUE(PageLooks(launch, launch + base::Hours(1)));
  EXPECT_TRUE(PageLooks(launch, launch + base::Days(8)))
      << "a browser left idle a week looks with its next page, not on a timer";
}

}  // namespace
}  // namespace browser_core::ad_block_list_check
