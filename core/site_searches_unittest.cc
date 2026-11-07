#include "browser_core/site_searches.h"

#include <memory>
#include <string>

#include "base/command_line.h"
#include "base/strings/utf_string_conversions.h"
#include "base/test/task_environment.h"
#include "components/regional_capabilities/regional_capabilities_switches.h"
#include "components/search_engines/search_engines_test_environment.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_data.h"
#include "components/search_engines/template_url_service.h"
#include "components/sync_preferences/testing_pref_service_syncable.h"
#include "components/variations/scoped_variations_ids_provider.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Active = TemplateURLData::ActiveStatus;

// A profile's engines, loaded, and its prefs.
class SiteSearchesTest : public testing::Test {
 protected:
  void SetUp() override {
    base::CommandLine::ForCurrentProcess()->AppendSwitchASCII(
        switches::kSearchEngineChoiceCountry, "US");
    RegisterSiteSearchesPrefs(environment_.pref_service().registry());
    engines()->Load();
    ASSERT_TRUE(engines()->loaded());
  }

  TemplateURLService* engines() { return environment_.template_url_service(); }
  void Seed() { SeedSiteSearches(&environment_.pref_service(), engines()); }

  TemplateURL* Engine(const std::string& keyword) {
    return engines()->GetTemplateURLForKeyword(base::UTF8ToUTF16(keyword));
  }

  // An engine the profile had before, as Chrome makes one from a site's search form when it's
  // |active| unspecified, or as the user added or turned one off.
  void Add(const std::string& keyword, Active active) {
    TemplateURLData data;
    data.SetShortName(base::UTF8ToUTF16(keyword));
    data.SetKeyword(base::UTF8ToUTF16(keyword));
    data.SetURL("https://" + keyword + "/own?q={searchTerms}");
    data.safe_for_autoreplace = active == Active::kUnspecified;
    data.is_active = active;
    engines()->Add(std::make_unique<TemplateURL>(data));
  }

  // The keyword of the site search |text| starts, or "".
  std::string Started(const std::string& text) {
    const TemplateURL* engine = SiteStarted(engines(), base::UTF8ToUTF16(text));
    return engine ? base::UTF16ToUTF8(engine->keyword()) : std::string();
  }

 private:
  base::test::TaskEnvironment task_environment_;
  variations::test::ScopedVariationsIdsProvider variations_ids_provider_{
      variations::VariationsIdsProvider::Mode::kUseSignedInState};
  search_engines::SearchEnginesTestEnvironment environment_;
};

TEST_F(SiteSearchesTest, NewProfileHasArcsSitesActive) {
  Seed();
  for (const char* keyword :
       {"drive.google.com", "mail.google.com", "maps.google.com", "calendar.google.com",
        "docs.google.com", "dropbox.com", "amazon.com", "github.com", "figma.com", "x.com",
        "twitter.com", "youtube.com", "wikipedia.org", "reddit.com", "soundcloud.com", "asana.com",
        "linkedin.com", "music.apple.com", "spotify.com"}) {
    const TemplateURL* engine = Engine(keyword);
    ASSERT_TRUE(engine) << keyword;
    EXPECT_TRUE(engine->CanBeUsedForKeywordMatching()) << keyword;
  }
  EXPECT_EQ(Engine("twitter.com")->short_name(), u"X");
  EXPECT_EQ(Engine("youtube.com")->url(),
            "https://www.youtube.com/results?search_query={searchTerms}");
}

// Once a profile: one the user deletes stays deleted.
TEST_F(SiteSearchesTest, DeletedSiteStaysDeleted) {
  Seed();
  engines()->Remove(Engine("youtube.com"));
  Seed();
  EXPECT_FALSE(Engine("youtube.com"));
}

TEST_F(SiteSearchesTest, ProfilesOwnEnginesStay) {
  Add("github.com", Active::kTrue);
  Add("youtube.com", Active::kUnspecified);
  Add("reddit.com", Active::kFalse);
  Seed();
  // The user's keeps its page.
  EXPECT_EQ(Engine("github.com")->url(), "https://github.com/own?q={searchTerms}");
  // The one Chrome made from the site's own form is turned on, as Arc's are.
  EXPECT_EQ(Engine("youtube.com")->is_active(), Active::kTrue);
  EXPECT_EQ(Engine("youtube.com")->url(), "https://youtube.com/own?q={searchTerms}");
  // One the user turned off stays off.
  EXPECT_EQ(Engine("reddit.com")->is_active(), Active::kFalse);
}

TEST_F(SiteSearchesTest, WordHintsTheSiteItStarts) {
  Seed();
  EXPECT_EQ(Started("yout"), "youtube.com");
  EXPECT_EQ(Started("YouT"), "youtube.com");
  EXPECT_EQ(Started("youtube.com"), "youtube.com");
  EXPECT_EQ(Started("wiki"), "wikipedia.org");
  // By its host: Gmail's is mail.google.com.
  EXPECT_EQ(Started("gm"), "");
  EXPECT_EQ(Started("mai"), "mail.google.com");
  // One letter hints none.
  EXPECT_EQ(Started("y"), "");
  EXPECT_EQ(Started("x."), "x.com");
}

// The engines Chrome makes as sites are visited stay inactive and never hint, so "twit" hints X
// though the field would complete twitch.tv; one the user turns on does, the shortest first.
TEST_F(SiteSearchesTest, InactiveEnginesNeverHint) {
  Seed();
  Add("twitch.tv", Active::kUnspecified);
  EXPECT_EQ(Started("twit"), "twitter.com");
  engines()->SetIsActiveTemplateURL(Engine("twitch.tv"), true);
  EXPECT_EQ(Started("twit"), "twitch.tv");
}

}  // namespace
}  // namespace browser_core
