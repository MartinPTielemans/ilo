#include "browser_core/system_engine_choices.h"

#include <string>
#include <string_view>
#include <tuple>
#include <utility>
#include <vector>

#include "components/prefs/scoped_user_pref_update.h"
#include "components/prefs/testing_pref_service.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core::system_engine {
namespace {

// Every listed service by its entry and the host it plays at, as the user reaches it. A new entry
// needs its row here, so none ships untested: the checks never load a real service (#3593).
struct Service {
  std::string_view entry;
  std::string_view host;
};
constexpr Service kServices[] = {
    {"disneyplus.com", "www.disneyplus.com"},
    {"hbomax.com", "play.hbomax.com"},
    {"hulu.com", "www.hulu.com"},
    {"max.com", "play.max.com"},
    {"music.apple.com", "music.apple.com"},
    {"netflix.com", "www.netflix.com"},
    {"open.spotify.com", "open.spotify.com"},
    {"paramountplus.com", "www.paramountplus.com"},
    {"peacocktv.com", "www.peacocktv.com"},
    {"primevideo.com", "www.primevideo.com"},
    {"tv.apple.com", "tv.apple.com"},
};

GURL At(std::string_view host) {
  return GURL("https://" + std::string(host) + "/watch");
}

class SystemEngineChoicesTest : public testing::Test {
 protected:
  void SetUp() override { RegisterChoicePrefs(prefs_.registry()); }

  // The tab's off, as bc_tab_set_system_engine(tab, false) makes it: forgotten, then its note
  // closed.
  void TurnOff(const GURL& url) {
    choices_.Forget(url);
    choices_.CloseNote(url);
  }

  // What every service other than |entry| opens in, whether it's kept in Chromium, and whether
  // its note is closed.
  std::vector<std::tuple<bool, bool, bool>> Others(std::string_view entry) const {
    std::vector<std::tuple<bool, bool, bool>> others;
    for (const Service& each : kServices) {
      if (each.entry != entry) {
        const GURL url = At(each.host);
        others.emplace_back(choices_.Remembered(url), choices_.KeptInChromium(url),
                            choices_.NoteClosed(url));
      }
    }
    return others;
  }

  TestingPrefServiceSimple prefs_;
  Choices choices_{prefs_};
};

TEST_F(SystemEngineChoicesTest, TheTableIsTheShippedList) {
  std::vector<std::string_view> table;
  for (const Service& each : kServices) {
    table.push_back(each.entry);
  }
  EXPECT_EQ(ListedServices(), table);
}

TEST_F(SystemEngineChoicesTest, EachServiceOpensInTheSystemsEngineByItsHosts) {
  for (const Service& each : kServices) {
    SCOPED_TRACE(each.entry);
    for (const std::string& host :
         {std::string(each.host), std::string(each.entry), "www." + std::string(each.entry)}) {
      EXPECT_EQ(choices_.ListedEntry(At(host)), each.entry) << host;
      EXPECT_TRUE(choices_.Remembered(At(host))) << host;
      EXPECT_FALSE(choices_.KeptInChromium(At(host))) << host;
    }
  }
}

// A service's parent domain, its siblings and names that only look like it aren't listed.
TEST_F(SystemEngineChoicesTest, NothingAroundAServiceIsListed) {
  for (std::string_view host :
       {"apple.com", "www.apple.com", "support.apple.com", "spotify.com", "www.spotify.com",
        "notnetflix.com", "netflix.com.example", "music.apple.com.example", "applemusic.com"}) {
    EXPECT_TRUE(choices_.ListedEntry(At(host)).empty()) << host;
    EXPECT_FALSE(choices_.Remembered(At(host))) << host;
  }
  EXPECT_EQ(choices_.ListedEntry(GURL("http://music.apple.com/")), "music.apple.com");
  EXPECT_FALSE(choices_.Remembered(GURL("file:///music.apple.com")));
}

// Off keeps a service in Chromium and lists it in Settings by its entry, On from that list takes
// it back, the same for every service, and neither changes what any other service does: Apple
// Music's off leaves Apple TV, on the same site, as it was.
TEST_F(SystemEngineChoicesTest, OffAndOnAreTheSameForEachServiceAndItsAlone) {
  for (const Service& each : kServices) {
    SCOPED_TRACE(each.entry);
    choices_.Clear();
    const GURL url = At(each.host);
    const auto others = Others(each.entry);

    TurnOff(url);
    EXPECT_FALSE(choices_.Remembered(url));
    EXPECT_TRUE(choices_.KeptInChromium(url));
    // Its note asks no more, and the shown one goes, as the closed note's ✕ does.
    EXPECT_TRUE(choices_.NoteClosed(url));
    EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{std::string(each.entry)});
    EXPECT_TRUE(choices_.Sites().empty());
    EXPECT_EQ(Others(each.entry), others);

    // Settings' On, for the entry its list shows.
    ASSERT_TRUE(choices_.Offered(each.entry));
    choices_.Remember(At(each.entry));
    EXPECT_TRUE(choices_.Remembered(url));
    EXPECT_FALSE(choices_.KeptInChromium(url));
    EXPECT_FALSE(choices_.NoteClosed(url));
    EXPECT_TRUE(choices_.KeptSites().empty());
    EXPECT_FALSE(choices_.Offered(each.entry));
    EXPECT_EQ(Others(each.entry), others);
  }
}

// Apple Music's off is its own: the rest of apple.com, Apple TV and the site's other pages, keeps
// its note and opens as before, and Settings offers Apple Music alone.
TEST_F(SystemEngineChoicesTest, AppleMusicsOffLeavesTheRestOfAppleCom) {
  TurnOff(At("music.apple.com"));
  EXPECT_TRUE(choices_.Remembered(At("tv.apple.com")));
  EXPECT_FALSE(choices_.KeptInChromium(At("tv.apple.com")));
  EXPECT_FALSE(choices_.NoteClosed(At("tv.apple.com")));
  EXPECT_FALSE(choices_.NoteClosed(At("www.apple.com")));
  EXPECT_FALSE(choices_.KeptInChromium(At("www.apple.com")));
  EXPECT_FALSE(choices_.Offered("apple.com"));
  EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{"music.apple.com"});
}

// A note the user closed at a service's site, apart from its off, shows beside the service in
// Settings, the way back from it, and On for it leaves the service off.
TEST_F(SystemEngineChoicesTest, AClosedNoteAtAServicesSiteShowsBesideIt) {
  choices_.CloseNote(At("www.apple.com"));
  TurnOff(At("music.apple.com"));
  EXPECT_EQ(choices_.KeptSites(), (std::vector<std::string>{"music.apple.com", "apple.com"}));
  ASSERT_TRUE(choices_.Offered("apple.com"));
  choices_.Remember(At("apple.com"));
  EXPECT_FALSE(choices_.NoteClosed(At("www.apple.com")));
  EXPECT_FALSE(choices_.Remembered(At("music.apple.com")));
  EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{"music.apple.com"});
}

// An off from before a service's off counted alone closed its site's note too: where that site is
// the entry, Settings shows it once, and On takes back both.
TEST_F(SystemEngineChoicesTest, AnEarlierOffShowsOnce) {
  ScopedListPrefUpdate(&prefs_, kListedOffPref)->Append("netflix.com");
  ScopedListPrefUpdate(&prefs_, kClosedNotesPref)->Append("netflix.com");
  EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{"netflix.com"});
  choices_.Remember(At("netflix.com"));
  EXPECT_TRUE(choices_.Remembered(At("www.netflix.com")));
  EXPECT_FALSE(choices_.NoteClosed(At("www.netflix.com")));
  EXPECT_TRUE(choices_.KeptSites().empty());
}

// A site the user kept for the system's engine that holds a service doesn't override the
// service's off, and the off leaves the site kept: each is its own choice.
TEST_F(SystemEngineChoicesTest, AKeptSiteAndAServicesOffAreApart) {
  choices_.Remember(At("www.apple.com"));
  TurnOff(At("music.apple.com"));
  EXPECT_FALSE(choices_.Remembered(At("music.apple.com")));
  EXPECT_TRUE(choices_.Remembered(At("www.apple.com")));
  EXPECT_EQ(choices_.Sites(), std::vector<std::string>{"apple.com"});
  EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{"music.apple.com"});
}

// Clearing a service's site data forgets its off, and it opens in the system's engine again.
TEST_F(SystemEngineChoicesTest, ForgettingChoicesTakesBackAnOff) {
  for (const Service& each : kServices) {
    SCOPED_TRACE(each.entry);
    const GURL url = At(each.host);
    TurnOff(url);
    choices_.ForgetChoices(url);
    EXPECT_TRUE(choices_.Remembered(url));
    EXPECT_FALSE(choices_.KeptInChromium(url));
  }
  EXPECT_TRUE(choices_.KeptSites().empty());
}

// An unlisted site the user kept goes by its site, as it did before the list.
TEST_F(SystemEngineChoicesTest, AnUnlistedSiteIsKeptBySite) {
  const GURL url("https://video.example.com/watch");
  choices_.Remember(url);
  EXPECT_TRUE(choices_.Remembered(GURL("https://www.example.com/")));
  EXPECT_EQ(choices_.Sites(), std::vector<std::string>{"example.com"});
  TurnOff(url);
  EXPECT_FALSE(choices_.Remembered(url));
  EXPECT_TRUE(choices_.KeptInChromium(url));
  EXPECT_EQ(choices_.KeptSites(), std::vector<std::string>{"example.com"});
}

TEST_F(SystemEngineChoicesTest, TheChecksHostIsListedAsAService) {
  Choices test(prefs_, "127.0.0.1");
  EXPECT_EQ(test.ListedEntry(GURL("https://127.0.0.1:8443/")), "127.0.0.1");
  EXPECT_TRUE(choices_.ListedEntry(GURL("https://127.0.0.1:8443/")).empty());
}

}  // namespace
}  // namespace browser_core::system_engine
