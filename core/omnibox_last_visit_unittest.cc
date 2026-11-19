// The engine's chromium-omnibox-match-last-visit.patch gives a match made from a history row that
// row's last visit, which the address bar's rows say as "Visited yesterday" (#2337). Nothing else
// in Chromium reads the field, so these fail if a rebase onto the next Chromium loses it.

#include <memory>
#include <utility>

#include "base/files/scoped_temp_dir.h"
#include "base/functional/callback.h"
#include "base/memory/scoped_refptr.h"
#include "base/run_loop.h"
#include "base/test/task_environment.h"
#include "base/time/time.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/test/history_service_test_util.h"
#include "components/omnibox/browser/autocomplete_input.h"
#include "components/omnibox/browser/autocomplete_match.h"
#include "components/omnibox/browser/autocomplete_provider.h"
#include "components/omnibox/browser/autocomplete_provider_listener.h"
#include "components/omnibox/browser/fake_autocomplete_provider_client.h"
#include "components/omnibox/browser/history_quick_provider.h"
#include "components/omnibox/browser/history_url_provider.h"
#include "components/omnibox/browser/in_memory_url_index.h"
#include "components/omnibox/browser/test_scheme_classifier.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/metrics_proto/omnibox_event.pb.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

constexpr char kVisited[] = "http://example.com/visited";

class OmniboxLastVisitTest : public testing::Test, public AutocompleteProviderListener {
 protected:
  void SetUp() override {
    ASSERT_TRUE(dir_.CreateUniqueTempDir());
    client_ = std::make_unique<FakeAutocompleteProviderClient>();
    client_->set_history_service(history::CreateHistoryService(dir_.GetPath(), true));
    ASSERT_NE(client_->GetHistoryService(), nullptr);
    // No bookmark model, which the providers do without: an empty one titles its folders from
    // the ResourceBundle, which this suite doesn't load, so it would crash the test as it starts.
    // As Chromium's own HistoryURLProvider tests add their pages.
    client_->GetHistoryService()->AddPageWithDetails(GURL(kVisited), u"Visited page",
                                                     /*visit_count=*/3, /*typed_count=*/1,
                                                     visited_, /*hidden=*/false,
                                                     history::SOURCE_BROWSED);
    history::BlockUntilHistoryProcessesPendingRequests(client_->GetHistoryService());
  }

  void TearDown() override {
    client_.reset();
    task_environment_.RunUntilIdle();
  }

  // AutocompleteProviderListener:
  void OnProviderUpdate(bool updated_matches, const AutocompleteProvider* provider) override {
    if (provider->done() && quit_) {
      std::move(quit_).Run();
    }
  }

  // The match |provider| gives for the visited page, after it's done.
  const AutocompleteMatch* MatchFrom(AutocompleteProvider* provider, const std::u16string& text) {
    AutocompleteInput input(text, metrics::OmniboxEventProto::OTHER, TestSchemeClassifier());
    provider->Start(input, false);
    if (!provider->done()) {
      base::RunLoop loop{base::RunLoop::Type::kNestableTasksAllowed};
      quit_ = loop.QuitWhenIdleClosure();
      loop.Run();
    }
    for (const AutocompleteMatch& match : provider->matches()) {
      if (match.destination_url == GURL(kVisited)) {
        return &match;
      }
    }
    return nullptr;
  }

  void ExpectVisit(const AutocompleteMatch& match) {
    ASSERT_FALSE(match.last_visit.is_null()) << "the engine patch no longer sets last_visit";
    EXPECT_LT((match.last_visit - visited_).magnitude(), base::Seconds(1));
  }

  base::test::TaskEnvironment task_environment_;
  base::ScopedTempDir dir_;
  std::unique_ptr<FakeAutocompleteProviderClient> client_;
  const base::Time visited_ = base::Time::Now() - base::Days(1);
  base::OnceClosure quit_;
};

TEST_F(OmniboxLastVisitTest, HistoryUrlMatchHasItsLastVisit) {
  // Alone, as the address bar's HistoryURL gives way to HistoryQuick otherwise.
  HistoryQuickProvider::set_disabled(true);
  auto provider = base::MakeRefCounted<HistoryURLProvider>(client_.get(), this);
  const AutocompleteMatch* match = MatchFrom(provider.get(), u"example.com/vis");
  HistoryQuickProvider::set_disabled(false);
  ASSERT_NE(match, nullptr);
  ExpectVisit(*match);
}

TEST_F(OmniboxLastVisitTest, HistoryQuickMatchHasItsLastVisit) {
  client_->set_in_memory_url_index(std::make_unique<InMemoryURLIndex>(
      /*bookmark_model=*/nullptr, client_->GetHistoryService(), nullptr, dir_.GetPath(),
      SchemeSet()));
  client_->GetInMemoryURLIndex()->Init();
  history::BlockUntilHistoryProcessesPendingRequests(client_->GetHistoryService());
  ASSERT_TRUE(client_->GetInMemoryURLIndex()->restored());
  auto provider = base::MakeRefCounted<HistoryQuickProvider>(client_.get());
  const AutocompleteMatch* match = MatchFrom(provider.get(), u"visited page");
  ASSERT_NE(match, nullptr);
  ExpectVisit(*match);
}

// AutocompleteMatch's copy and move are written out field by field, so each needs the field.
TEST(OmniboxLastVisit, CopiesAndMovesKeepIt) {
  AutocompleteMatch match;
  match.last_visit = base::Time::Now();
  AutocompleteMatch copied(match);
  EXPECT_EQ(copied.last_visit, match.last_visit);
  AutocompleteMatch assigned;
  assigned = match;
  EXPECT_EQ(assigned.last_visit, match.last_visit);
  AutocompleteMatch moved(std::move(copied));
  EXPECT_EQ(moved.last_visit, match.last_visit);
}

}  // namespace
}  // namespace browser_core
