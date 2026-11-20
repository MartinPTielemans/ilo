#include "browser_core/clear_both_engines.h"

#include <cstdint>
#include <memory>
#include <optional>
#include <utility>

#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/test/bind.h"
#include "base/time/time.h"
#include "chrome/browser/browsing_data/chrome_browsing_data_remover_constants.h"
#include "content/public/browser/browsing_data_filter_builder.h"
#include "content/public/browser/browsing_data_remover.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"
#include "url/origin.h"

namespace browser_core {
namespace {

using Filter = content::BrowsingDataFilterBuilder;
using Remover = content::BrowsingDataRemover;

// Chrome's delegate as the wrapper sees it: what reached it, and its removal's callback, held
// until the test lets it finish.
class ChromesDelegate : public content::BrowsingDataRemoverDelegate {
 public:
  EmbedderOriginTypeMatcher GetOriginTypeMatcher() override {
    ++matchers;
    return EmbedderOriginTypeMatcher();
  }
  bool MayRemoveDownloadHistory() override { return may_remove_downloads; }
  void RemoveEmbedderData(const base::Time& delete_begin,
                          const base::Time& delete_end,
                          uint64_t remove_mask,
                          content::BrowsingDataFilterBuilder* filter_builder,
                          uint64_t origin_type_mask,
                          base::OnceCallback<void(uint64_t)> callback) override {
    ++removals;
    begin = delete_begin;
    end = delete_end;
    mask = remove_mask;
    origin_types = origin_type_mask;
    finish = std::move(callback);
  }
  void OnStartRemoving() override { ++starts; }
  void OnDoneRemoving() override { ++dones; }

  int matchers = 0;
  bool may_remove_downloads = false;
  int removals = 0;
  base::Time begin;
  base::Time end;
  uint64_t mask = 0;
  uint64_t origin_types = 0;
  base::OnceCallback<void(uint64_t)> finish;
  int starts = 0;
  int dones = 0;
};

class ClearBothEnginesTest : public testing::Test {
 protected:
  // What reached the system's engine's store, if anything did.
  struct StoreClear {
    base::Time begin;
    bool everything = false;
  };

  ClearBothEnginesTest()
      : clear_(&chrome_,
               base::BindLambdaForTesting(
                   [this](base::Time begin, bool everything, base::OnceClosure done) {
                     store_ = StoreClear{begin, everything};
                     store_done_ = std::move(done);
                   }),
               base::BindLambdaForTesting([this] { ++choices_cleared_; })) {}

  // Removes |mask| through the wrapper, as the remover does, and says whether the remover heard
  // it done, with the types that failed.
  void Remove(base::Time begin, uint64_t mask, Filter& filter) {
    clear_.RemoveEmbedderData(begin, base::Time::Max(), mask, &filter,
                              Remover::ORIGIN_TYPE_UNPROTECTED_WEB,
                              base::BindLambdaForTesting([this](uint64_t failed) {
                                ++removals_done_;
                                failed_ = failed;
                              }));
  }

  // Lets Chrome's part finish, with |failed| types.
  void ChromeFinishes(uint64_t failed = 0) {
    ASSERT_TRUE(chrome_.finish);
    std::move(chrome_.finish).Run(failed);
  }

  ChromesDelegate chrome_;
  ClearBothEngines clear_;
  std::optional<StoreClear> store_;
  base::OnceClosure store_done_;
  int choices_cleared_ = 0;
  int removals_done_ = 0;
  uint64_t failed_ = 0;
};

constexpr uint64_t kSiteData = chrome_browsing_data_remover::DATA_TYPE_SITE_DATA;
constexpr uint64_t kSiteSettings = chrome_browsing_data_remover::DATA_TYPE_CONTENT_SETTINGS;

std::unique_ptr<Filter> Everything() {
  return Filter::Create(Filter::Mode::kPreserve);
}

TEST_F(ClearBothEnginesTest, EveryMethodReachesChromesDelegate) {
  clear_.GetOriginTypeMatcher();
  EXPECT_EQ(chrome_.matchers, 1);
  chrome_.may_remove_downloads = true;
  EXPECT_TRUE(clear_.MayRemoveDownloadHistory());
  chrome_.may_remove_downloads = false;
  EXPECT_FALSE(clear_.MayRemoveDownloadHistory());
  clear_.OnStartRemoving();
  clear_.OnDoneRemoving();
  EXPECT_EQ(chrome_.starts, 1);
  EXPECT_EQ(chrome_.dones, 1);

  const base::Time begin = base::Time::Now() - base::Hours(1);
  auto filter = Everything();
  Remove(begin, kSiteData, *filter);
  EXPECT_EQ(chrome_.removals, 1);
  EXPECT_EQ(chrome_.begin, begin);
  EXPECT_EQ(chrome_.end, base::Time::Max());
  EXPECT_EQ(chrome_.mask, kSiteData);
  EXPECT_EQ(chrome_.origin_types, Remover::ORIGIN_TYPE_UNPROTECTED_WEB);
}

// The core's Clear Site Data, which clears the system's engine itself.
TEST_F(ClearBothEnginesTest, TheCoresClearSiteDataLeavesTheSystemEngineAlone) {
  auto filter = Filter::Create(Filter::Mode::kDelete);
  filter->AddRegisterableDomain("example.com");
  Remove(base::Time(),
         Remover::DATA_TYPE_COOKIES | Remover::DATA_TYPE_DOM_STORAGE | Remover::DATA_TYPE_CACHE,
         *filter);
  ChromeFinishes();
  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 0);
  EXPECT_EQ(removals_done_, 1);
}

// A site's Clear-Site-Data header: its cookies by registrable domain, its storage by origin.
TEST_F(ClearBothEnginesTest, AClearSiteDataHeaderLeavesTheSystemEngineAlone) {
  auto cookies = Filter::Create(Filter::Mode::kDelete);
  cookies->AddRegisterableDomain("example.com");
  Remove(base::Time(), Remover::DATA_TYPE_COOKIES, *cookies);
  ChromeFinishes();

  auto storage = Filter::Create(Filter::Mode::kDelete);
  storage->AddOrigin(url::Origin::Create(GURL("https://example.com")));
  Remove(base::Time(), Remover::DATA_TYPE_DOM_STORAGE | Remover::DATA_TYPE_CACHE, *storage);
  ChromeFinishes();

  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 0);
  EXPECT_EQ(removals_done_, 2);
}

// A clear for given origins, as a site's own data is cleared from Chrome's settings, and one that
// keeps some sites, as clearing on exit with exceptions does.
TEST_F(ClearBothEnginesTest, AClearOfSomeSitesLeavesTheSystemEngineAlone) {
  auto some = Filter::Create(Filter::Mode::kDelete);
  some->AddOrigin(url::Origin::Create(GURL("https://example.com")));
  some->AddOrigin(url::Origin::Create(GURL("https://example.org")));
  Remove(base::Time(), kSiteData | kSiteSettings, *some);
  ChromeFinishes();

  auto all_but = Filter::Create(Filter::Mode::kPreserve);
  all_but->AddRegisterableDomain("example.com");
  Remove(base::Time(), kSiteData | kSiteSettings, *all_but);
  ChromeFinishes();

  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 0);
  EXPECT_EQ(removals_done_, 2);
}

TEST_F(ClearBothEnginesTest, SiteDataForAllTimeClearsTheStoreAndTheChoices) {
  auto filter = Everything();
  Remove(base::Time(), kSiteData, *filter);
  ASSERT_TRUE(store_);
  EXPECT_TRUE(store_->everything);
  EXPECT_TRUE(store_->begin.is_null());
  EXPECT_EQ(choices_cleared_, 1);
}

TEST_F(ClearBothEnginesTest, CookiesAloneClearOnlyTheStoresCookies) {
  auto filter = Everything();
  Remove(base::Time(), Remover::DATA_TYPE_COOKIES, *filter);
  ASSERT_TRUE(store_);
  EXPECT_FALSE(store_->everything);
  EXPECT_EQ(choices_cleared_, 1);
}

TEST_F(ClearBothEnginesTest, ARangeClearsTheStoreSinceItsStartAndKeepsTheChoices) {
  const base::Time begin = base::Time::Now() - base::Hours(1);
  auto filter = Everything();
  Remove(begin, kSiteData | kSiteSettings, *filter);
  ASSERT_TRUE(store_);
  EXPECT_EQ(store_->begin, begin);
  EXPECT_EQ(choices_cleared_, 0);
}

// "Older than 30 days" starts at no time and ends 30 days ago, which WebKit has no clear for.
TEST_F(ClearBothEnginesTest, ARangeThatEndsBeforeNowLeavesTheSystemEngineAlone) {
  auto filter = Everything();
  clear_.RemoveEmbedderData(base::Time(), base::Time::Now() - base::Days(30),
                            kSiteData | kSiteSettings, filter.get(),
                            Remover::ORIGIN_TYPE_UNPROTECTED_WEB,
                            base::BindLambdaForTesting([this](uint64_t) { ++removals_done_; }));
  ChromeFinishes();
  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 0);
  EXPECT_EQ(removals_done_, 1);
}

TEST_F(ClearBothEnginesTest, SiteSettingsForAllTimeEmptyOnlyTheChoices) {
  auto filter = Everything();
  Remove(base::Time(), kSiteSettings, *filter);
  ChromeFinishes();
  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 1);
  EXPECT_EQ(removals_done_, 1);
}

TEST_F(ClearBothEnginesTest, TheOtherTypesLeaveTheSystemEngineAlone) {
  auto filter = Everything();
  Remove(base::Time(),
         Remover::DATA_TYPE_CACHE | Remover::DATA_TYPE_DOWNLOADS |
             chrome_browsing_data_remover::DATA_TYPE_HISTORY |
             chrome_browsing_data_remover::DATA_TYPE_FORM_DATA |
             chrome_browsing_data_remover::DATA_TYPE_PASSWORDS,
         *filter);
  ChromeFinishes();
  EXPECT_FALSE(store_);
  EXPECT_EQ(choices_cleared_, 0);
  EXPECT_EQ(removals_done_, 1);
}

TEST_F(ClearBothEnginesTest, TheRemovalEndsOnceBothEnginesHaveCleared) {
  auto filter = Everything();
  Remove(base::Time(), kSiteData, *filter);
  ChromeFinishes(Remover::DATA_TYPE_DOM_STORAGE);
  EXPECT_EQ(removals_done_, 0);
  ASSERT_TRUE(store_done_);
  std::move(store_done_).Run();
  EXPECT_EQ(removals_done_, 1);
  EXPECT_EQ(failed_, Remover::DATA_TYPE_DOM_STORAGE);
}

TEST_F(ClearBothEnginesTest, TheRemovalEndsWhicheverEngineClearsLast) {
  auto filter = Everything();
  Remove(base::Time(), kSiteData, *filter);
  ASSERT_TRUE(store_done_);
  std::move(store_done_).Run();
  EXPECT_EQ(removals_done_, 0);
  ChromeFinishes();
  EXPECT_EQ(removals_done_, 1);
  EXPECT_EQ(failed_, 0u);
}

}  // namespace
}  // namespace browser_core
