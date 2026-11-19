#include "browser_core/pinned_url.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

constexpr char kPinned[] = "https://mail.example.org/";
constexpr char kLanded[] = "https://mail.example.org/u/0/inbox";
constexpr char kSignIn[] = "https://accounts.example.org/signin?continue=mail";
constexpr char kElsewhere[] = "https://example.org/news";

// Whether a page at |url|, whose load started at |requested|, is at kPinned.
bool Home(const char* url, const char* requested) {
  return AtPinnedUrl(GURL(kPinned), GURL(url), GURL(requested));
}

TEST(PinnedUrl, AtThePinnedUrlIsHome) {
  EXPECT_TRUE(Home(kPinned, kPinned));
}

TEST(PinnedUrl, WhereThePinnedUrlRedirectsIsHome) {
  // Back to Pinned URL loaded the pinned URL, and the server sent it on: the entry keeps the
  // request it started from.
  EXPECT_TRUE(Home(kLanded, kPinned));
  // Through a sign-in hop too, wherever the hop ends.
  EXPECT_TRUE(Home(kSignIn, kPinned));
}

TEST(PinnedUrl, TheRedirectsPageReachedAnotherWayIsAway) {
  // A link to the page the pinned URL lands on made an entry of its own, which ⌘↩ would load
  // again from the pinned URL.
  EXPECT_FALSE(Home(kLanded, kLanded));
}

TEST(PinnedUrl, GoneElsewhereIsAwayAndBackIsHome) {
  EXPECT_FALSE(Home(kElsewhere, kElsewhere));
  // A link followed from home, which the server redirected, started from where the link went.
  EXPECT_FALSE(Home(kLanded, kElsewhere));
  // Back in its history to home's entry, or a link to the pinned URL that redirects again.
  EXPECT_TRUE(Home(kLanded, kPinned));
  EXPECT_TRUE(Home(kPinned, kPinned));
}

TEST(PinnedUrl, APageThatReplacedItsOwnAddressIsStillHome) {
  EXPECT_TRUE(Home("https://mail.example.org/#inbox", kPinned));
}

TEST(PinnedUrl, NoPinnedUrlOrNoPageIsNeverHome) {
  EXPECT_FALSE(AtPinnedUrl(GURL(), GURL(), GURL()));
  EXPECT_FALSE(AtPinnedUrl(GURL(), GURL(kPinned), GURL(kPinned)));
  EXPECT_FALSE(Home("", ""));
}

}  // namespace
}  // namespace browser_core
