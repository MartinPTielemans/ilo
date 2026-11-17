#include "browser_core/link_card_rule.h"

#include <string>

#include "net/base/net_errors.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

TEST(LinkCardRule, AsksOnlyHttpAndHttps) {
  EXPECT_TRUE(LinkCardAsks(GURL("https://example.com/a")));
  EXPECT_TRUE(LinkCardAsks(GURL("http://example.com/")));
  for (const char* other :
       {"file:///etc/passwd", "ftp://example.com/", "data:text/html,<title>x</title>",
        "javascript:alert(1)", "chrome://settings", "blob:https://example.com/1", "about:blank",
        "ws://example.com/", "mailto:a@example.com", ""}) {
    EXPECT_FALSE(LinkCardAsks(GURL(other))) << other;
  }
}

TEST(LinkCardRule, EveryRedirectIsHttpOrHttpsAndAtMostFive) {
  EXPECT_TRUE(LinkCardFollows(GURL("https://www.example.com/"), 1));
  EXPECT_TRUE(LinkCardFollows(GURL("http://example.com/"), kLinkCardMaxRedirects));
  EXPECT_FALSE(LinkCardFollows(GURL("https://example.com/"), kLinkCardMaxRedirects + 1));
  EXPECT_FALSE(LinkCardFollows(GURL("file:///etc/passwd"), 1));
  EXPECT_FALSE(LinkCardFollows(GURL("data:text/html,x"), 1));
  EXPECT_FALSE(LinkCardFollows(GURL("chrome://settings"), 2));
}

TEST(LinkCardRule, OnlyADirectConnectionIsAsked) {
  EXPECT_TRUE(LinkCardDirect(net::OK, true, true));
  // A proxy, a failed lookup or none at all: the private-network check can't see past a proxy.
  EXPECT_FALSE(LinkCardDirect(net::OK, true, false));
  EXPECT_FALSE(LinkCardDirect(net::ERR_FAILED, true, true));
  EXPECT_FALSE(LinkCardDirect(net::OK, false, false));
}

TEST(LinkCardRule, ReadsPagesAndTheImagesTheAppDecodes) {
  EXPECT_TRUE(LinkCardPage("text/html"));
  EXPECT_TRUE(LinkCardPage("Application/XHTML+XML"));
  EXPECT_FALSE(LinkCardPage("application/pdf"));
  EXPECT_FALSE(LinkCardPage("application/octet-stream"));
  EXPECT_FALSE(LinkCardPage("text/plain"));
  EXPECT_FALSE(LinkCardPage(""));
  for (const char* image : {"image/png", "image/jpeg", "image/gif", "image/webp"}) {
    EXPECT_TRUE(LinkCardPicture(image)) << image;
  }
  // AVIF and SVG need decoders the app doesn't link.
  EXPECT_FALSE(LinkCardPicture("image/avif"));
  EXPECT_FALSE(LinkCardPicture("image/svg+xml"));
  EXPECT_FALSE(LinkCardPicture("text/html"));
}

TEST(LinkCardRule, KeepsThePageUpToItsHeadsEnd) {
  EXPECT_EQ(LinkCardKeep("<title>a</title></head><body>b"),
            std::string("<title>a</title></head>").size());
  EXPECT_EQ(LinkCardKeep("<TITLE>a</TITLE></HEAD><body>"),
            std::string("<TITLE>a</TITLE></HEAD>").size());
  // No end yet: all of it, and the read goes on.
  EXPECT_EQ(LinkCardKeep("<head><title>a</title></he"),
            std::string("<head><title>a</title></he").size());
}

TEST(LinkCardRule, KeepsAtMostTheCap) {
  const std::string big(kLinkCardPageCap + 100, 'x');
  EXPECT_EQ(LinkCardKeep(big), kLinkCardPageCap);
  // A head ending past the cap is cut at the cap.
  EXPECT_EQ(LinkCardKeep(big + "</head>"), kLinkCardPageCap);
}

}  // namespace
}  // namespace browser_core
