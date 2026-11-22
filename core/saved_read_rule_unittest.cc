#include "browser_core/saved_read_rule.h"

#include <string>

#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

std::string Refusal(const char* url) {
  return SavedReadRefusal(GURL(url), /*off_the_record=*/false, 1000, 1000);
}

TEST(SavedReadRuleTest, AnHttpOrHttpsAddressIsRead) {
  EXPECT_EQ(Refusal("https://www.example.test/inbox?page=2"), "");
  EXPECT_EQ(Refusal("http://example.test:8080/"), "");
}

TEST(SavedReadRuleTest, APrivateWindowsSessionIsRefused) {
  EXPECT_NE(SavedReadRefusal(GURL("https://example.test/"), /*off_the_record=*/true, 1000, 1000),
            "");
}

TEST(SavedReadRuleTest, AnythingButHttpOrHttpsIsRefused) {
  for (const char* url :
       {"file:///etc/hosts", "data:text/plain,hello", "blob:https://example.test/1",
        "chrome://settings", "ftp://example.test/", "javascript:void(0)", "about:blank",
        "not an address", ""}) {
    EXPECT_EQ(Refusal(url), "Only an http or https address is read.") << url;
  }
}

TEST(SavedReadRuleTest, AnAddressWithAUserNameOrPasswordIsRefused) {
  EXPECT_NE(Refusal("https://someone@example.test/"), "");
  EXPECT_NE(Refusal("https://someone:made-up@example.test/"), "");
  EXPECT_NE(Refusal("https://:made-up@example.test/"), "");
}

TEST(SavedReadRuleTest, AnAddressWithAFragmentIsRefused) {
  EXPECT_NE(Refusal("https://example.test/page#part"), "");
  EXPECT_NE(Refusal("https://example.test/page#"), "");
}

TEST(SavedReadRuleTest, AReadWithNoSizeOrTimeIsRefused) {
  const GURL url("https://example.test/");
  EXPECT_NE(SavedReadRefusal(url, false, 0, 1000), "");
  EXPECT_NE(SavedReadRefusal(url, false, 1000, 0), "");
  EXPECT_NE(SavedReadRefusal(url, false, 1000, -1), "");
}

TEST(SavedReadRuleTest, TheLimitsAreWhatWasAskedButNoMoreThanTheMost) {
  EXPECT_EQ(SavedReadBytes(1000), 1000u);
  EXPECT_EQ(SavedReadBytes(kSavedReadMaxBytes), kSavedReadMaxBytes);
  EXPECT_EQ(SavedReadBytes(size_t{1} << 40), kSavedReadMaxBytes);
  EXPECT_EQ(kSavedReadMaxBytes, size_t{4} * 1024 * 1024);
  EXPECT_EQ(SavedReadMilliseconds(200), 200);
  EXPECT_EQ(SavedReadMilliseconds(60'000), kSavedReadMaxMilliseconds);
  EXPECT_EQ(kSavedReadMaxMilliseconds, 30'000);
}

TEST(SavedReadRuleTest, ARedirectIsFollowedOnlyOnTheAddresssOwnOrigin) {
  const GURL asked("https://www.example.test/inbox");
  EXPECT_TRUE(SavedReadFollows(asked, GURL("https://www.example.test/inbox/2")));
  EXPECT_TRUE(SavedReadFollows(asked, GURL("https://www.example.test:443/")));
  // Another host of the same site, such as its sign-in, is told.
  EXPECT_FALSE(SavedReadFollows(asked, GURL("https://login.example.test/")));
  EXPECT_FALSE(SavedReadFollows(asked, GURL("https://www.example.test:8443/")));
  EXPECT_FALSE(SavedReadFollows(asked, GURL("https://other.test/")));
  // The upgrade too: the app can save the https address.
  EXPECT_FALSE(
      SavedReadFollows(GURL("http://www.example.test/"), GURL("https://www.example.test/")));
  EXPECT_FALSE(
      SavedReadFollows(GURL("https://www.example.test/"), GURL("http://www.example.test/")));
  EXPECT_FALSE(SavedReadFollows(asked, GURL("data:text/plain,hello")));
  EXPECT_FALSE(SavedReadFollows(asked, GURL()));
}

}  // namespace
}  // namespace browser_core
