#include "browser_core/search_default.h"

#include <memory>
#include <string>

#include "base/command_line.h"
#include "base/strings/utf_string_conversions.h"
#include "base/test/task_environment.h"
#include "components/regional_capabilities/regional_capabilities_switches.h"
#include "components/search_engines/default_search_manager.h"
#include "components/search_engines/search_engines_pref_names.h"
#include "components/search_engines/search_engines_test_environment.h"
#include "components/search_engines/search_engines_test_util.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_data.h"
#include "components/search_engines/template_url_data_util.h"
#include "components/search_engines/template_url_prepopulate_data.h"
#include "components/search_engines/template_url_service.h"
#include "components/sync_preferences/testing_pref_service_syncable.h"
#include "components/variations/scoped_variations_ids_provider.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/search_engines_data/resources/definitions/prepopulated_engines.h"

namespace browser_core {
namespace {

// A profile's default search engine as the DefaultSearchManager in its TemplateURLService decides
// it, from the prefs and the policies a DefaultSearchProvider policy sets.
class SearchDefaultTest : public testing::Test {
 protected:
  void SetUp() override {
    base::CommandLine::ForCurrentProcess()->AppendSwitchASCII(
        switches::kSearchEngineChoiceCountry, "US");
    RegisterSearchDefaultPrefs(prefs()->registry());
    manager_ = std::make_unique<DefaultSearchManager>(
        prefs(), &environment_.search_engine_choice_service(),
        environment_.prepopulate_data_resolver(), DefaultSearchManager::ObserverCallback());
  }

  sync_preferences::TestingPrefServiceSyncable* prefs() { return &environment_.pref_service(); }
  DefaultSearchManager* manager() { return manager_.get(); }

  // The profile's engines, loaded.
  TemplateURLService* engines() {
    TemplateURLService* engines = environment_.template_url_service();
    engines->Load();
    return engines;
  }
  const TemplateURL* Google() { return engines()->GetTemplateURLForKeyword(u"google.com"); }

  // The default's keyword and where it comes from.
  std::string Default(DefaultSearchManager::Source* source) {
    const TemplateURLData* engine = manager_->GetDefaultSearchEngine(source);
    return engine ? base::UTF16ToUTF8(engine->keyword()) : std::string();
  }

  // DuckDuckGo as the core picked it before, and as the user picks it in Settings › Profiles.
  void PickDuckDuckGo() {
    manager_->SetUserSelectedDefaultSearchEngine(
        *TemplateURLDataFromPrepopulatedEngine(TemplateURLPrepopulateData::duckduckgo));
  }

  // A DefaultSearchProvider policy, mandatory or recommended.
  void SetPolicy(const std::string& keyword, bool mandatory) {
    std::unique_ptr<TemplateURLData> engine = GenerateDummyTemplateURLData(keyword);
    base::DictValue value = TemplateURLDataToDictionary(*engine);
    value.Set(DefaultSearchManager::kDisabledByPolicy, false);
    if (mandatory) {
      prefs()->SetManagedPref(DefaultSearchManager::kDefaultSearchProviderDataPrefName,
                              std::move(value));
    } else {
      prefs()->SetRecommendedPref(DefaultSearchManager::kDefaultSearchProviderDataPrefName,
                                  std::move(value));
    }
  }

 private:
  base::test::TaskEnvironment task_environment_;
  variations::test::ScopedVariationsIdsProvider variations_ids_provider_{
      variations::VariationsIdsProvider::Mode::kUseSignedInState};
  search_engines::SearchEnginesTestEnvironment environment_;
  std::unique_ptr<DefaultSearchManager> manager_;
};

TEST_F(SearchDefaultTest, DuckDuckGoIsTheDefaultNotAChoice) {
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "duckduckgo.com");
  EXPECT_EQ(source, DefaultSearchManager::FROM_FALLBACK);
  EXPECT_FALSE(prefs()->GetUserPrefValue(DefaultSearchManager::kDefaultSearchProviderDataPrefName));
}

TEST_F(SearchDefaultTest, RecommendedPolicyOutranksTheDefault) {
  SetPolicy("recommended", /*mandatory=*/false);
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "recommended");
  EXPECT_EQ(source, DefaultSearchManager::FROM_POLICY_RECOMMENDED);
}

TEST_F(SearchDefaultTest, UsersChoiceOutranksRecommendedPolicy) {
  manager()->SetUserSelectedDefaultSearchEngine(*GenerateDummyTemplateURLData("chosen"));
  SetPolicy("recommended", /*mandatory=*/false);
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "chosen");
  EXPECT_EQ(source, DefaultSearchManager::FROM_USER);
}

TEST_F(SearchDefaultTest, MandatoryPolicyOutranksUsersChoice) {
  manager()->SetUserSelectedDefaultSearchEngine(*GenerateDummyTemplateURLData("chosen"));
  SetPolicy("mandatory", /*mandatory=*/true);
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "mandatory");
  EXPECT_EQ(source, DefaultSearchManager::FROM_POLICY);
}

// A profile the core picked DuckDuckGo for before it was the default.
TEST_F(SearchDefaultTest, OldPickIsForgottenForRecommendedPolicy) {
  PickDuckDuckGo();
  SetPolicy("recommended", /*mandatory=*/false);
  DefaultSearchManager::Source source;
  ASSERT_EQ(Default(&source), "duckduckgo.com");
  ASSERT_EQ(source, DefaultSearchManager::FROM_USER);

  ForgetPickedDefault(prefs());
  EXPECT_EQ(Default(&source), "recommended");
  EXPECT_EQ(source, DefaultSearchManager::FROM_POLICY_RECOMMENDED);
  // Without telling the user their engine was reset.
  EXPECT_FALSE(prefs()->GetBoolean(prefs::kUnacknowledgedDefaultSearchEngineResetOccurred));

  prefs()->RemoveRecommendedPref(DefaultSearchManager::kDefaultSearchProviderDataPrefName);
  EXPECT_EQ(Default(&source), "duckduckgo.com");
  EXPECT_EQ(source, DefaultSearchManager::FROM_FALLBACK);
}

TEST_F(SearchDefaultTest, OtherChoiceIsKept) {
  manager()->SetUserSelectedDefaultSearchEngine(*GenerateDummyTemplateURLData("chosen"));
  ForgetPickedDefault(prefs());
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "chosen");
  EXPECT_EQ(source, DefaultSearchManager::FROM_USER);
}

// The user picks DuckDuckGo after the core forgot its own pick.
TEST_F(SearchDefaultTest, LaterChoiceOfDuckDuckGoIsKept) {
  ForgetPickedDefault(prefs());
  PickDuckDuckGo();
  ForgetPickedDefault(prefs());
  SetPolicy("recommended", /*mandatory=*/false);
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "duckduckgo.com");
  EXPECT_EQ(source, DefaultSearchManager::FROM_USER);
}

TEST_F(SearchDefaultTest, GoogleSearchesWithNothingElse) {
  const TemplateURLData google = GoogleChoice();
  EXPECT_EQ(google.url(), "https://www.google.com/search?q={searchTerms}");
  EXPECT_TRUE(google.suggestions_url.empty());
  EXPECT_TRUE(google.favicon_url.is_empty());
  EXPECT_TRUE(google.image_url.empty());
  EXPECT_TRUE(google.new_tab_url.empty());
  EXPECT_TRUE(google.alternate_urls.empty());
  EXPECT_TRUE(google.search_intent_params.empty());
  EXPECT_EQ(google.prepopulate_id, 0);
}

TEST_F(SearchDefaultTest, GoogleIsNeverTheDefault) {
  AddGoogleChoice(prefs(), engines());
  ASSERT_TRUE(Google());
  EXPECT_TRUE(IsGoogleChoice(*Google()));
  EXPECT_NE(engines()->GetDefaultSearchProvider(), Google());
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "duckduckgo.com");
  EXPECT_EQ(source, DefaultSearchManager::FROM_FALLBACK);
  EXPECT_FALSE(prefs()->GetUserPrefValue(DefaultSearchManager::kDefaultSearchProviderDataPrefName));
}

TEST_F(SearchDefaultTest, RecommendedPolicyOutranksTheDefaultWithGoogle) {
  AddGoogleChoice(prefs(), engines());
  SetPolicy("recommended", /*mandatory=*/false);
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "recommended");
  EXPECT_EQ(source, DefaultSearchManager::FROM_POLICY_RECOMMENDED);
}

TEST_F(SearchDefaultTest, ChoiceOfGoogleIsKept) {
  manager()->SetUserSelectedDefaultSearchEngine(GoogleChoice());
  ForgetPickedDefault(prefs());
  DefaultSearchManager::Source source;
  EXPECT_EQ(Default(&source), "google.com");
  EXPECT_EQ(source, DefaultSearchManager::FROM_USER);
}

TEST_F(SearchDefaultTest, RemovedGoogleStaysRemoved) {
  AddGoogleChoice(prefs(), engines());
  ASSERT_TRUE(Google());
  engines()->Remove(Google());
  AddGoogleChoice(prefs(), engines());
  EXPECT_FALSE(Google());
}

TEST_F(SearchDefaultTest, UsersOwnGoogleIsKept) {
  TemplateURLData own;
  own.SetShortName(u"Mine");
  own.SetKeyword(u"google.com");
  own.SetURL("https://www.google.com/own?q={searchTerms}");
  own.safe_for_autoreplace = false;
  engines()->Add(std::make_unique<TemplateURL>(own));
  AddGoogleChoice(prefs(), engines());
  ASSERT_TRUE(Google());
  EXPECT_EQ(Google()->short_name(), u"Mine");
  EXPECT_FALSE(IsGoogleChoice(*Google()));
}

}  // namespace
}  // namespace browser_core
