#include "browser_core/share_snapshot.h"

#include <array>
#include <string>
#include <vector>

#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "browser_core/public/browser_core.h"
#include "crypto/aead.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/zlib/zlib.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

ShareItem Tab(std::string title, std::string url) {
  ShareItem item;
  item.title = std::move(title);
  item.url = GURL(url);
  return item;
}

ShareItem Folder(std::string title, std::vector<ShareItem> items) {
  ShareItem item;
  item.title = std::move(title);
  item.folder = true;
  item.items = std::move(items);
  return item;
}

// A 1×1 PNG's bytes, padded past |size| if asked, as a favicon.
std::string Png(size_t size = 0) {
  std::string png("\x89PNG\r\n\x1a\n", 8);
  png += "IHDR-and-the-rest";
  if (png.size() < size) {
    png.append(size - png.size(), 'x');
  }
  return png;
}

ShareSnapshot Lisbon() {
  ShareSnapshot snapshot;
  snapshot.kind = BC_SHARE_FOLDER;
  snapshot.title = "Lisbon, March";
  snapshot.made = 1795046400;
  snapshot.items.push_back(Tab("Visit Lisboa", "https://www.visitlisboa.com/en"));
  snapshot.items.back().favicon = Png();
  snapshot.items.push_back(Tab("Lisbon — Wikipedia", "https://en.wikipedia.org/wiki/Lisbon"));
  snapshot.items.push_back(Folder("Food", {Tab("Time Out Market", "https://www.timeoutmarket.com/"),
                                           Tab("Pastéis de Belém", "https://pasteisdebelem.pt/")}));
  return snapshot;
}

// A blob of |json| as a modified client or server could make one, past what SealShare makes.
std::vector<uint8_t> RawBlob(std::string_view json, const ShareLink& link, uint8_t version = 1) {
  z_stream stream = {};
  deflateInit2(&stream, 9, Z_DEFLATED, -MAX_WBITS, 8, Z_DEFAULT_STRATEGY);
  std::string packed(deflateBound(&stream, json.size()), '\0');
  stream.next_in = reinterpret_cast<Bytef*>(const_cast<char*>(json.data()));
  stream.avail_in = json.size();
  stream.next_out = reinterpret_cast<Bytef*>(packed.data());
  stream.avail_out = packed.size();
  deflate(&stream, Z_FINISH);
  packed.resize(stream.total_out);
  deflateEnd(&stream);
  const std::array<uint8_t, 12> iv{};
  const std::string aad = base::StrCat({"ilo share v1 ", link.id});
  const std::vector<uint8_t> sealed = crypto::aead::Seal(
      crypto::aead::AES_256_GCM, link.key, base::as_byte_span(packed), iv, base::as_byte_span(aad));
  std::vector<uint8_t> blob = {version};
  blob.insert(blob.end(), iv.begin(), iv.end());
  blob.insert(blob.end(), sealed.begin(), sealed.end());
  return blob;
}

std::string Snapshot(std::string items) {
  return base::StrCat({R"({"v":1,"kind":"tabs","title":"T","made":0,"items":)", items, "}"});
}

TEST(ShareSnapshot, ALinkHasAnIdAndAKeyInItsFragment) {
  const ShareLink link = NewShareLink();
  EXPECT_EQ(link.id.size(), 22u);
  const GURL url = ShareLinkUrl(GURL("https://sync.example.org/"), link);
  EXPECT_EQ(url.path(), "/s/" + link.id);
  EXPECT_EQ(url.ref().size(), 43u);
  const std::optional<ShareLink> read = ShareLinkOf(url);
  ASSERT_TRUE(read);
  EXPECT_EQ(read->id, link.id);
  EXPECT_EQ(read->key, link.key);

  const std::string id = link.id;
  const std::string key = url.ref();
  for (const std::string& other :
       {base::StrCat({"https://sync.example.org/s/", id}),
        base::StrCat({"https://sync.example.org/s/", id, "#", key.substr(1)}),
        base::StrCat({"https://sync.example.org/s/", id.substr(1), "#", key}),
        base::StrCat({"https://sync.example.org/x/", id, "#", key}),
        base::StrCat({"https://sync.example.org/s/", id, "/data#", key}),
        base::StrCat({"https://sync.example.org/s/", id, "#", key, "A"}),
        base::StrCat({"ftp://sync.example.org/s/", id, "#", key})}) {
    EXPECT_FALSE(ShareLinkOf(GURL(other))) << other;
  }
}

TEST(ShareSnapshot, WhatIsSealedOpensWithItsLink) {
  const ShareLink link = NewShareLink();
  ShareSnapshot space = Lisbon();
  space.kind = BC_SHARE_SPACE;
  space.color = 0x5b6ad0;
  space.icon = "🔭";
  const std::optional<std::vector<uint8_t>> blob = SealShare(space, link, kShareBlobCap);
  ASSERT_TRUE(blob);
  const ShareOpened opened = OpenShare(*blob, link);
  ASSERT_TRUE(opened.snapshot);
  const ShareSnapshot& snapshot = *opened.snapshot;
  EXPECT_EQ(snapshot.kind, BC_SHARE_SPACE);
  EXPECT_EQ(snapshot.title, "Lisbon, March");
  EXPECT_EQ(snapshot.color, 0x5b6ad0u);
  EXPECT_EQ(snapshot.icon, "🔭");
  EXPECT_EQ(snapshot.made, 1795046400);
  ASSERT_EQ(snapshot.items.size(), 3u);
  EXPECT_EQ(snapshot.items[0].url, GURL("https://www.visitlisboa.com/en"));
  EXPECT_EQ(snapshot.items[0].favicon, Png());
  EXPECT_TRUE(snapshot.items[2].folder);
  EXPECT_EQ(snapshot.items[2].items[1].title, "Pastéis de Belém");
  EXPECT_EQ(CountShare(snapshot.items).tabs, 4u);
  EXPECT_EQ(CountShare(snapshot.items).folders, 1u);
}

TEST(ShareSnapshot, AnotherKeyAnotherIdOrAChangedByteOpensNothing) {
  const ShareLink link = NewShareLink();
  const std::vector<uint8_t> blob = *SealShare(Lisbon(), link, kShareBlobCap);

  ShareLink key = link;
  key.key[0] ^= 1;
  EXPECT_FALSE(OpenShare(blob, key).snapshot);
  EXPECT_EQ(OpenShare(blob, key).refusal, ShareRefusal::kKey);

  // The id is in the sealed data, so a blob served under another id doesn't open.
  ShareLink id = link;
  id.id = NewShareLink().id;
  EXPECT_EQ(OpenShare(blob, id).refusal, ShareRefusal::kKey);

  std::vector<uint8_t> changed = blob;
  changed[20] ^= 1;
  EXPECT_EQ(OpenShare(changed, link).refusal, ShareRefusal::kKey);

  std::vector<uint8_t> newer = blob;
  newer[0] = 2;
  EXPECT_EQ(OpenShare(newer, link).refusal, ShareRefusal::kNewer);
  EXPECT_EQ(OpenShare(RawBlob(R"({"v":2,"kind":"tabs","items":[]})", link), link).refusal,
            ShareRefusal::kNewer);
  EXPECT_EQ(OpenShare(base::span<const uint8_t>(), link).refusal, ShareRefusal::kKey);
  EXPECT_EQ(OpenShare(RawBlob("[1,2]", link), link).refusal, ShareRefusal::kNewer);
}

TEST(ShareSnapshot, ABlobOverTheCapIsRefusedUnread) {
  const ShareLink link = NewShareLink();
  std::vector<uint8_t> blob(kShareBlobCap + 1, 1);
  EXPECT_EQ(OpenShare(blob, link).refusal, ShareRefusal::kTooBig);
}

// A few kilobytes that inflate past the cap stop at it.
TEST(ShareSnapshot, ASmallBlobThatInflatesPastTheCapIsRefused) {
  const ShareLink link = NewShareLink();
  const std::string padded =
      Snapshot(base::StrCat({R"([{"title":")", std::string(kShareInflatedCap, 'a'),
                             R"(","url":"https://a.example/"}])"}));
  const std::vector<uint8_t> blob = RawBlob(padded, link);
  EXPECT_LT(blob.size(), 64u * 1024);
  EXPECT_EQ(OpenShare(blob, link).refusal, ShareRefusal::kTooBig);
}

TEST(ShareSnapshot, AtMostTheCapsTabsAndDepth) {
  const ShareLink link = NewShareLink();
  auto tabs = [](size_t count) {
    std::string items = "[";
    for (size_t i = 0; i < count; ++i) {
      base::StrAppend(&items, {i ? "," : "", R"({"title":"t","url":"https://a.example/)",
                               base::NumberToString(i), "\"}"});
    }
    return items + "]";
  };
  EXPECT_TRUE(OpenShare(RawBlob(Snapshot(tabs(kShareTabCap)), link), link).snapshot);
  EXPECT_EQ(OpenShare(RawBlob(Snapshot(tabs(kShareTabCap + 1)), link), link).refusal,
            ShareRefusal::kTooBig);

  auto nested = [](int depth) {
    std::string items = R"([{"title":"t","url":"https://a.example/"}])";
    for (int i = 0; i < depth; ++i) {
      items = base::StrCat({R"([{"title":"f","items":)", items, "}]"});
    }
    return items;
  };
  EXPECT_TRUE(OpenShare(RawBlob(Snapshot(nested(kShareDepthCap)), link), link).snapshot);
  EXPECT_EQ(OpenShare(RawBlob(Snapshot(nested(kShareDepthCap + 1)), link), link).refusal,
            ShareRefusal::kTooBig);
}

TEST(ShareSnapshot, TitlesAreCutAtACharacterAndOtherAddressesLeftOut) {
  const ShareLink link = NewShareLink();
  std::string long_title;
  for (size_t i = 0; i < kShareTitleCap; ++i) {
    long_title += "é";
  }
  const std::string long_url =
      base::StrCat({"https://a.example/", std::string(kShareAddressCap, 'p')});
  const std::string items = base::StrCat(
      {R"([{"title":")", long_title, R"(","url":"https://a.example/"},)",
       R"({"title":"js","url":"javascript:alert(1)"},)",
       R"({"title":"data","url":"data:text/html,<script>alert(1)</script>"},)",
       R"({"title":"file","url":"file:///etc/passwd"},)",
       R"({"title":"ilo","url":"chrome://settings"},)",
       R"({"title":"long","url":")", long_url, R"("},)",
       R"({"title":"user","url":"https://me:secret@b.example/x"}])"
});
const ShareOpened opened = OpenShare(RawBlob(Snapshot(items), link), link);
ASSERT_TRUE(opened.snapshot);
const std::vector<ShareItem>& kept = opened.snapshot->items;
ASSERT_EQ(kept.size(), 2u);
EXPECT_LE(kept[0].title.size(), kShareTitleCap);
EXPECT_TRUE(base::IsStringUTF8(kept[0].title));
EXPECT_EQ(kept[1].url, GURL("https://b.example/x"));
}

TEST(ShareSnapshot, OnlyASmallPngIsAFavicon) {
  const ShareLink link = NewShareLink();
  ShareSnapshot snapshot;
  snapshot.items.push_back(Tab("small", "https://a.example/"));
  snapshot.items.back().favicon = Png();
  snapshot.items.push_back(Tab("big", "https://b.example/"));
  snapshot.items.back().favicon = Png(kShareFaviconCap + 1);
  snapshot.items.push_back(Tab("svg", "https://c.example/"));
  snapshot.items.back().favicon = "<svg onload=alert(1)>";
  const ShareOpened opened = OpenShare(*SealShare(snapshot, link, kShareBlobCap), link);
  ASSERT_TRUE(opened.snapshot);
  EXPECT_EQ(opened.snapshot->items[0].favicon, Png());
  EXPECT_TRUE(opened.snapshot->items[1].favicon.empty());
  EXPECT_TRUE(opened.snapshot->items[2].favicon.empty());

  const std::string svg = Snapshot(
      R"([{"title":"s","url":"https://a.example/","icon":"data:image/svg+xml,<svg onload=alert(1)>"}])");
  EXPECT_TRUE(OpenShare(RawBlob(svg, link), link).snapshot->items[0].favicon.empty());
}

TEST(ShareSnapshot, FaviconsGoFirstWhenALinkWouldBeTooBig) {
  const ShareLink link = NewShareLink();
  ShareSnapshot snapshot = Lisbon();
  for (int i = 0; i < 40; ++i) {
    snapshot.items.push_back(
        Tab("t", base::StrCat({"https://a.example/", base::NumberToString(i)})));
    snapshot.items.back().favicon = Png(1000);
  }
  const size_t with = SealShare(snapshot, link, kShareBlobCap)->size();
  const std::optional<std::vector<uint8_t>> without = SealShare(snapshot, link, with - 1);
  ASSERT_TRUE(without);
  for (const ShareItem& item : OpenShare(*without, link).snapshot->items) {
    EXPECT_TRUE(item.favicon.empty());
  }
  EXPECT_FALSE(SealShare(snapshot, link, 64));
}

TEST(ShareSnapshot, ThePageEscapesWhatItShowsAndLinksOnlyHttp) {
  ShareSnapshot snapshot;
  snapshot.kind = BC_SHARE_TABS;
  snapshot.title = "<img src=x onerror=alert(1)>";
  snapshot.icon = "<script>alert(1)</script>";
  snapshot.items.push_back(
      Tab("</style><script>alert(1)</script>", "https://example.org/\"><script>alert(1)</script>"));
  snapshot.items.push_back(Tab("js", "javascript:alert(1)"));
  snapshot.items.push_back(Folder("<b onclick=alert(1)>f</b>", {Tab("ok", "https://a.example/")}));
  const std::string page = SharePage(snapshot, "<host>");

  EXPECT_NE(page.find("Content-Security-Policy\" content=\"default-src 'none'"), std::string::npos);
  EXPECT_EQ(page.find("<script"), std::string::npos);
  EXPECT_EQ(page.find("<img src=x"), std::string::npos);
  EXPECT_EQ(page.find("<b onclick"), std::string::npos);
  EXPECT_EQ(page.find("javascript:"), std::string::npos);
  EXPECT_EQ(page.find("<host>"), std::string::npos);
  EXPECT_NE(page.find("&lt;img src=x onerror=alert(1)&gt;"), std::string::npos);
  size_t at = 0;
  size_t links = 0;
  while ((at = page.find("href=\"", at)) != std::string::npos) {
    at += 6;
    ++links;
    EXPECT_TRUE(page.compare(at, 8, "https://") == 0 || page.compare(at, 7, "http://") == 0)
        << page.substr(at, 40);
  }
  EXPECT_EQ(links, 2u);

  for (bool gone : {true, false}) {
    EXPECT_EQ(ShareRefusedPage(gone, ShareRefusal::kKey).find("<script"), std::string::npos);
  }
}

}  // namespace
}  // namespace browser_core
