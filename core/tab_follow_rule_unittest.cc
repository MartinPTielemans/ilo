// What bc_tab_follow and bc_agent_navigate load (#3576): http(s) alone, as a link, so an agent's
// visit or a card's row never counts as an address the user typed.

#include "browser_core/tab_follow_rule.h"

#include <memory>
#include <utility>

#include "base/files/scoped_temp_dir.h"
#include "base/run_loop.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/test/bind.h"
#include "base/test/task_environment.h"
#include "base/time/time.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/test/history_service_test_util.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core::tab_follow {
namespace {

TEST(TabFollowRuleTest, LoadsHttpAndHttpsAlone) {
  EXPECT_TRUE(MayLoad(GURL("https://github.com/owner/repo/pull/12")));
  EXPECT_TRUE(MayLoad(GURL("http://example.com/")));

  EXPECT_FALSE(MayLoad(GURL("javascript:alert(1)")));
  EXPECT_FALSE(MayLoad(GURL("file:///etc/hosts")));
  EXPECT_FALSE(MayLoad(GURL("data:text/html,<p>hi</p>")));
  EXPECT_FALSE(MayLoad(GURL("chrome://settings")));
  EXPECT_FALSE(MayLoad(GURL("about:blank")));
  EXPECT_FALSE(MayLoad(GURL("ftp://example.com/file")));
  EXPECT_FALSE(MayLoad(GURL("blob:https://example.com/0f1e2d3c")));
  EXPECT_FALSE(MayLoad(GURL("not a url")));
  EXPECT_FALSE(MayLoad(GURL()));
}

class TabFollowHistoryTest : public testing::Test {
 protected:
  void SetUp() override {
    ASSERT_TRUE(dir_.CreateUniqueTempDir());
    history_ = history::CreateHistoryService(dir_.GetPath(), true);
    ASSERT_NE(history_, nullptr);
  }

  void TearDown() override {
    history_.reset();
    task_environment_.RunUntilIdle();
  }

  void Visit(const GURL& url, ui::PageTransition transition) {
    history_->AddPage(url, base::Time::Now(), history::ContextID(), /*nav_entry_id=*/0,
                      /*referrer=*/GURL(), history::RedirectList(), transition,
                      history::SOURCE_BROWSED, history::VisitResponseCodeCategory::kNot404,
                      /*did_replace_entry=*/false);
    history::BlockUntilHistoryProcessesPendingRequests(history_.get());
  }

  history::URLRow Row(const GURL& url) {
    history::QueryURLAndVisitsResult visits;
    base::RunLoop loop;
    history_->QueryURLAndVisits(
        url, history::VisitQuery404sPolicy::kInclude404s,
        base::BindLambdaForTesting([&](history::QueryURLAndVisitsResult result) {
          visits = std::move(result);
          loop.Quit();
        }),
        &tracker_);
    loop.Run();
    EXPECT_TRUE(visits.success);
    return visits.row;
  }

  base::test::TaskEnvironment task_environment_;
  base::ScopedTempDir dir_;
  std::unique_ptr<history::HistoryService> history_;
  base::CancelableTaskTracker tracker_;
};

// bc_agent_navigate loaded as typed before, so each page an agent visited climbed the address
// bar's typed matches as if the user had written it out.
TEST_F(TabFollowHistoryTest, AVisitIsNotTyped) {
  const GURL followed("https://example.com/followed");
  Visit(followed, kTransition);
  Visit(followed, kTransition);
  EXPECT_EQ(Row(followed).visit_count(), 2);
  EXPECT_EQ(Row(followed).typed_count(), 0);

  // What it was: the typed count rises with each visit.
  const GURL typed("https://example.com/typed");
  Visit(typed, ui::PAGE_TRANSITION_TYPED);
  EXPECT_EQ(Row(typed).typed_count(), 1);
}

}  // namespace
}  // namespace browser_core::tab_follow
