#include "browser_core/typed_http.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

TEST(TypedHttp, AnHttpAddressWrittenOutIsKept) {
  EXPECT_TRUE(TypedHttp("http://example.com", GURL("http://example.com/")));
  EXPECT_TRUE(TypedHttp("  HTTP://Example.com/a?b ", GURL("http://example.com/a?b")));
  EXPECT_TRUE(TypedHttp("http://203.0.113.5:8080/", GURL("http://203.0.113.5:8080/")));
}

TEST(TypedHttp, AHostWithoutASchemeIsntAChoiceOfHttp) {
  // The bar's address for "example.com" is http too, which HTTPS-Upgrades tries over https first.
  EXPECT_FALSE(TypedHttp("example.com", GURL("http://example.com/")));
  EXPECT_FALSE(TypedHttp("", GURL("http://example.com/")));
}

TEST(TypedHttp, OnlyTheAddressTypedNotASuggestionStartingWithIt) {
  EXPECT_FALSE(TypedHttp("http://example.com", GURL("http://example.com/page")));
  EXPECT_FALSE(TypedHttp("http://exam", GURL("http://example.com/")));
}

TEST(TypedHttp, HttpsAndOtherSchemesAreUntouched) {
  EXPECT_FALSE(TypedHttp("https://example.com", GURL("https://example.com/")));
  EXPECT_FALSE(TypedHttp("http://example.com", GURL("https://example.com/")));
  EXPECT_FALSE(TypedHttp("ftp://example.com", GURL("ftp://example.com/")));
}

}  // namespace
}  // namespace browser_core
