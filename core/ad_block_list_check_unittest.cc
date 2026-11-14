#include "browser_core/ad_block_list_check.h"

#include <string>

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
