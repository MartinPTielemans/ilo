// The removals the Library's History undoes on ⌘Z (#2337), on a history service alone. Removing a
// day's row deletes its visits at once, so no list, search or suggestion shows them after, nothing
// keeps them but the removal, and an undo puts them back as they were.

#include "browser_core/history_removal.h"

#include <memory>
#include <optional>
#include <set>
#include <utility>
#include <vector>

#include "base/files/scoped_temp_dir.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/memory/scoped_refptr.h"
#include "base/run_loop.h"
#include "base/scoped_observation.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/test/bind.h"
#include "base/test/task_environment.h"
#include "base/time/time.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_service_observer.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/test/history_service_test_util.h"
#include "components/omnibox/browser/autocomplete_input.h"
#include "components/omnibox/browser/autocomplete_match.h"
#include "components/omnibox/browser/autocomplete_provider_listener.h"
#include "components/omnibox/browser/fake_autocomplete_provider_client.h"
#include "components/omnibox/browser/history_quick_provider.h"
#include "components/omnibox/browser/in_memory_url_index.h"
#include "components/omnibox/browser/test_scheme_classifier.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/metrics_proto/omnibox_event.pb.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

// Visited the day before yesterday and twice yesterday.
constexpr char kPage[] = "http://example.com/removal-page";
// Visited only yesterday.
constexpr char kOnce[] = "http://example.com/once";

class HistoryRemovalTest : public testing::Test,
                           public AutocompleteProviderListener,
                           public history::HistoryServiceObserver {
 protected:
  void SetUp() override {
    ASSERT_TRUE(dir_.CreateUniqueTempDir());
    client_ = std::make_unique<FakeAutocompleteProviderClient>();
    client_->set_history_service(history::CreateHistoryService(dir_.GetPath(), true));
    ASSERT_NE(history(), nullptr);
    // The page's visit the day before yesterday is typed, so its typed count says whether an undo
    // put back only what it took.
    Visit(GURL(kPage), before_, ui::PAGE_TRANSITION_TYPED);
    for (base::Time time : yesterday_) {
      Visit(GURL(kPage), time, ui::PAGE_TRANSITION_LINK);
    }
    Visit(GURL(kOnce), yesterday_[0], ui::PAGE_TRANSITION_LINK);
    history()->SetPageTitle(GURL(kPage), u"Removal page");
    history()->SetPageTitle(GURL(kOnce), u"Once page");
    history::BlockUntilHistoryProcessesPendingRequests(history());
    observation_.Observe(history());
  }

  void TearDown() override {
    observation_.Reset();
    client_.reset();
    task_environment_.RunUntilIdle();
  }

  history::HistoryService* history() { return client_->GetHistoryService(); }

  void Visit(const GURL& url, base::Time time, ui::PageTransition transition) {
    history()->AddPage(url, time, history::ContextID(), /*nav_entry_id=*/0, /*referrer=*/GURL(),
                       history::RedirectList(), transition, history::SOURCE_BROWSED,
                       history::VisitResponseCodeCategory::kNot404,
                       /*did_replace_entry=*/false);
  }

  // As the Library's History removes |url|'s row for yesterday, and returns once it's done.
  scoped_refptr<RemovedHistory> RemoveYesterday(const GURL& url) {
    auto removed = base::MakeRefCounted<RemovedHistory>();
    base::RunLoop loop;
    history()->ScheduleDBTask(
        FROM_HERE,
        std::make_unique<KeepHistory>(removed, std::vector<GURL>{url},
                                      std::vector<std::pair<history::KeywordID, std::u16string>>(),
                                      day_, day_ + base::Days(1), /*take_out=*/true,
                                      loop.QuitClosure()),
        &tracker_);
    loop.Run();
    return removed;
  }

  void PutBack(scoped_refptr<RemovedHistory> removed) {
    base::RunLoop loop;
    history()->ScheduleDBTask(
        FROM_HERE, std::make_unique<PutBackHistory>(std::move(removed), loop.QuitClosure()),
        &tracker_);
    loop.Run();
  }

  // The rows the Library's History lists for |text|: each page once a day, newest first.
  history::QueryResults List(const std::u16string& text) {
    history::QueryOptions options;
    options.duplicate_policy = history::QueryOptions::REMOVE_DUPLICATES_PER_DAY;
    history::QueryResults listed;
    base::RunLoop loop;
    history()->QueryHistory(text, options,
                            base::BindLambdaForTesting([&](history::QueryResults results) {
                              listed.Swap(&results);
                              loop.Quit();
                            }),
                            &tracker_);
    loop.Run();
    return listed;
  }

  // Whether |results| has a row for |url| on yesterday.
  bool ListsYesterday(const history::QueryResults& results, const char* url) {
    for (const history::URLResult& result : results) {
      if (result.url() == GURL(url) && result.visit_time() >= day_ &&
          result.visit_time() < day_ + base::Days(1)) {
        return true;
      }
    }
    return false;
  }

  history::QueryURLAndVisitsResult Visits(const GURL& url) {
    history::QueryURLAndVisitsResult visits;
    base::RunLoop loop;
    history()->QueryURLAndVisits(url, history::VisitQuery404sPolicy::kInclude404s,
                                 base::BindLambdaForTesting(
                                     [&](history::QueryURLAndVisitsResult result) {
                                       visits = std::move(result);
                                       loop.Quit();
                                     }),
                                 &tracker_);
    loop.Run();
    return visits;
  }

  // The command bar's history match for |text| going to |url|, as its HistoryQuick source finds
  // it, or none.
  std::optional<AutocompleteMatch> BarMatch(const std::u16string& text, const char* url) {
    auto provider = base::MakeRefCounted<HistoryQuickProvider>(client_.get());
    AutocompleteInput input(text, metrics::OmniboxEventProto::OTHER, TestSchemeClassifier());
    provider->Start(input, false);
    if (!provider->done()) {
      base::RunLoop loop{base::RunLoop::Type::kNestableTasksAllowed};
      quit_ = loop.QuitWhenIdleClosure();
      loop.Run();
    }
    for (const AutocompleteMatch& match : provider->matches()) {
      if (match.destination_url == GURL(url)) {
        return match;
      }
    }
    return std::nullopt;
  }

  void IndexForTheBar() {
    client_->set_in_memory_url_index(std::make_unique<InMemoryURLIndex>(
        /*bookmark_model=*/nullptr, history(), nullptr, dir_.GetPath(), SchemeSet()));
    client_->GetInMemoryURLIndex()->Init();
    history::BlockUntilHistoryProcessesPendingRequests(history());
    ASSERT_TRUE(client_->GetInMemoryURLIndex()->restored());
  }

  // AutocompleteProviderListener:
  void OnProviderUpdate(bool updated_matches, const AutocompleteProvider* provider) override {
    if (provider->done() && quit_) {
      std::move(quit_).Run();
    }
  }

  // history::HistoryServiceObserver:
  void OnHistoryDeletions(history::HistoryService* history,
                          const history::DeletionInfo& info) override {
    deletions_.push_back(info);
  }
  void OnURLVisited(history::HistoryService* history,
                    const history::VisitedURLInfo& visited) override {
    visited_.push_back(visited.url_row.url());
  }

  base::test::TaskEnvironment task_environment_;
  base::ScopedTempDir dir_;
  std::unique_ptr<FakeAutocompleteProviderClient> client_;
  base::CancelableTaskTracker tracker_;
  // Yesterday, as the Library's History heads a day, and visits in it and the day before.
  const base::Time day_ = base::Time::Now().LocalMidnight() - base::Days(1);
  const base::Time yesterday_[2] = {day_ + base::Hours(9), day_ + base::Hours(15)};
  const base::Time before_ = day_ - base::Days(1) + base::Hours(10);
  std::vector<history::DeletionInfo> deletions_;
  std::vector<GURL> visited_;
  base::OnceClosure quit_;
  base::ScopedObservation<history::HistoryService, history::HistoryServiceObserver> observation_{
      this};
};

// The orchestrator's first condition: the row is gone everywhere at once. The list, a search and
// the command bar show nothing of yesterday's visits once the removal's task has run, before
// anything else is asked.
TEST_F(HistoryRemovalTest, ADaysRowLeavesTheListASearchAndTheBarAtOnce) {
  IndexForTheBar();
  ASSERT_TRUE(ListsYesterday(List(u""), kPage));
  ASSERT_TRUE(BarMatch(u"once page", kOnce).has_value());

  RemoveYesterday(GURL(kPage));
  RemoveYesterday(GURL(kOnce));

  const history::QueryResults all = List(u"");
  EXPECT_FALSE(ListsYesterday(all, kPage));
  EXPECT_FALSE(ListsYesterday(all, kOnce));
  // The day before's row stays: only the day's visits went.
  bool before = false;
  for (const history::URLResult& result : all) {
    before |= result.url() == GURL(kPage) && result.visit_time() == before_;
  }
  EXPECT_TRUE(before) << "the removal took the page's other days too";
  EXPECT_FALSE(ListsYesterday(List(u"removal page"), kPage));
  EXPECT_FALSE(ListsYesterday(List(u"once"), kOnce));
  // A page visited only that day isn't suggested; one visited before says that day's visit.
  EXPECT_FALSE(BarMatch(u"once page", kOnce).has_value());
  std::optional<AutocompleteMatch> page = BarMatch(u"removal page", kPage);
  ASSERT_TRUE(page.has_value());
  EXPECT_EQ(page->last_visit, before_);
}

// The second: the visits are really deleted no later than quit and no later than the tenth removal
// after it. They're deleted in the removal's own task, so they're gone from the database a history
// service opened again after a quit finds, with no other removal, undo or quit in between.
TEST_F(HistoryRemovalTest, TheVisitsAreDeletedInTheRemovalsOwnTask) {
  RemoveYesterday(GURL(kPage));
  history::QueryURLAndVisitsResult now = Visits(GURL(kPage));
  ASSERT_TRUE(now.success);
  ASSERT_EQ(now.visits.size(), 1u);
  EXPECT_EQ(now.visits[0].visit_time, before_);

  // Quit, as the browser does, and open the profile's history again.
  observation_.Reset();
  base::RunLoop closed;
  history()->SetOnBackendDestroyTask(closed.QuitClosure());
  client_->set_history_service(nullptr);
  closed.Run();
  client_->set_history_service(history::CreateHistoryService(dir_.GetPath(), true));
  history::QueryURLAndVisitsResult reopened = Visits(GURL(kPage));
  ASSERT_TRUE(reopened.success);
  ASSERT_EQ(reopened.visits.size(), 1u);
  EXPECT_EQ(reopened.visits[0].visit_time, before_);
  EXPECT_FALSE(Visits(GURL(kOnce)).visits.empty()) << "the removal took another page";
}

// The third: sync never carries a removed visit in between. What syncs history hears the deletion
// with the removal, as the user's and not as old visits expiring, and between it and the undo
// nothing has a visit to carry: the database has none and none is added. The undo then puts the day
// back as it was, its visits at their times, the page's counts as before.
TEST_F(HistoryRemovalTest, NothingHasARemovedVisitUntilTheUndo) {
  const history::QueryURLAndVisitsResult was = Visits(GURL(kPage));
  ASSERT_TRUE(was.success);
  visited_.clear();

  scoped_refptr<RemovedHistory> removed = RemoveYesterday(GURL(kPage));
  ASSERT_EQ(deletions_.size(), 1u);
  const history::DeletionInfo& deletion = deletions_[0];
  EXPECT_FALSE(deletion.is_from_expiration());
  ASSERT_TRUE(deletion.time_range().IsValid());
  EXPECT_EQ(deletion.time_range().begin(), day_);
  EXPECT_EQ(deletion.time_range().end(), day_ + base::Days(1));
  ASSERT_TRUE(deletion.restrict_urls().has_value());
  EXPECT_EQ(*deletion.restrict_urls(), std::set<GURL>{GURL(kPage)});
  for (const history::VisitRow& visit : Visits(GURL(kPage)).visits) {
    EXPECT_FALSE(visit.visit_time >= day_ && visit.visit_time < day_ + base::Days(1));
  }
  EXPECT_TRUE(visited_.empty());

  PutBack(removed);
  const history::QueryURLAndVisitsResult back = Visits(GURL(kPage));
  ASSERT_TRUE(back.success);
  ASSERT_EQ(back.visits.size(), was.visits.size());
  for (size_t i = 0; i < was.visits.size(); ++i) {
    EXPECT_EQ(back.visits[i].visit_time, was.visits[i].visit_time);
    EXPECT_TRUE(ui::PageTransitionCoreTypeIs(
        back.visits[i].transition, ui::PageTransitionStripQualifier(was.visits[i].transition)));
  }
  EXPECT_EQ(back.row.visit_count(), was.row.visit_count());
  EXPECT_EQ(back.row.typed_count(), was.row.typed_count());
  EXPECT_EQ(back.row.last_visit(), was.row.last_visit());
  EXPECT_EQ(back.row.title(), was.row.title());
}

}  // namespace
}  // namespace browser_core
