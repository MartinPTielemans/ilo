#include "browser_core/link_preview_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr char kLink[] = "https://example.com/a";

TEST(LinkPreviewRuleTest, ThePreviewOfTheLinkAskedIsThePagesOwn) {
  LinkPreviewAsk ask;
  ask.Keep(kLink);
  EXPECT_TRUE(ask.Take(kLink));
}

TEST(LinkPreviewRuleTest, APreviewNoPageAskedForIsNoPagesLink) {
  LinkPreviewAsk ask;
  EXPECT_FALSE(ask.Take(kLink));
  EXPECT_FALSE(ask.Take(""));
}

TEST(LinkPreviewRuleTest, APreviewOfAnotherAddressIsNoPagesLink) {
  LinkPreviewAsk ask;
  ask.Keep(kLink);
  EXPECT_FALSE(ask.Take("https://example.com/b"));
}

// Nor is the link asked, once another preview has opened from the tab.
TEST(LinkPreviewRuleTest, OneAskIsOnePreview) {
  LinkPreviewAsk ask;
  ask.Keep(kLink);
  EXPECT_TRUE(ask.Take(kLink));
  EXPECT_FALSE(ask.Take(kLink));
  ask.Keep(kLink);
  EXPECT_FALSE(ask.Take("https://example.com/b"));
  EXPECT_FALSE(ask.Take(kLink));
}

}  // namespace
}  // namespace browser_core
