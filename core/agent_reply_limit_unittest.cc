#include "browser_core/agent_reply_limit.h"

#include <optional>
#include <string>
#include <vector>

#include "base/functional/bind.h"
#include "base/test/task_environment.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr base::TimeDelta kTick = base::Milliseconds(1);

class AgentReplyLimitTest : public testing::Test {
 protected:
  base::test::TaskEnvironment clock_{base::test::TaskEnvironment::TimeSource::MOCK_TIME};
  std::vector<std::string> ended_;
  // What the jobs queued behind each ended one hear, with a dialog, which ends them too.
  std::vector<std::optional<std::string>> unstarted_;
  AgentReplyLimit limit_{base::BindRepeating(
      [](AgentReplyLimitTest* test, std::string why, std::optional<std::string> unstarted) {
        test->ended_.push_back(std::move(why));
        test->unstarted_.push_back(std::move(unstarted));
      },
      base::Unretained(this))};
};

// A click whose page shows an alert: the click's reply never comes, as the renderer waits on the
// alert, and the job ends as it shows, not at the limit. The click may have taken effect, and the
// jobs queued behind it never started, which each is told.
TEST_F(AgentReplyLimitTest, AJobInFlightWhenTheDialogShowsEndsWithTheDialogsAnswer) {
  limit_.Started();
  limit_.Sent();
  limit_.DialogShown("an alert");
  ASSERT_EQ(ended_.size(), 1u);
  EXPECT_EQ(ended_[0],
            "The tab's page showed an alert while the action ran. Nothing more can be done on the "
            "tab while it's up, and no tool answers it. close_tab closes the tab, and the dialog "
            "with it.");
  EXPECT_EQ(unstarted_[0],
            "The tab's page showed an alert before this action started, so it wasn't done. Nothing "
            "more can be done on the tab while it's up, and no tool answers it. close_tab closes "
            "the tab, and the dialog with it.");
  EXPECT_FALSE(limit_.counting());
  // Answered once: the limit doesn't end it again.
  clock_.FastForwardBy(AgentReplyLimit::kLimit * 2);
  EXPECT_EQ(ended_.size(), 1u);
}

// A job waiting for its page to load has sent nothing yet, and a dialog then ends it too.
TEST_F(AgentReplyLimitTest, ADialogEndsAJobThatHasSentNothing) {
  limit_.Started();
  EXPECT_FALSE(limit_.counting());
  limit_.DialogShown("a dialog asking whether to leave it");
  ASSERT_EQ(ended_.size(), 1u);
  EXPECT_TRUE(ended_[0].starts_with(
      "The tab's page showed a dialog asking whether to leave it while the action ran."));
  ASSERT_TRUE(unstarted_[0]);
  EXPECT_TRUE(unstarted_[0]->starts_with(
      "The tab's page showed a dialog asking whether to leave it before this action started, so "
      "it wasn't done."));
}

TEST_F(AgentReplyLimitTest, AReplyThatNeverComesEndsAtTheLimit) {
  limit_.Started();
  limit_.Sent();
  EXPECT_TRUE(limit_.counting());
  clock_.FastForwardBy(AgentReplyLimit::kLimit - kTick);
  EXPECT_TRUE(ended_.empty());
  clock_.FastForwardBy(kTick);
  ASSERT_EQ(ended_.size(), 1u);
  EXPECT_EQ(ended_[0],
            "The tab's page didn't answer for 15 s, so the action stopped part way. The page may "
            "be busy or stuck; close_tab closes the tab.");
  // A hung page may answer the next job, so only this one ends.
  EXPECT_EQ(unstarted_[0], std::nullopt);
  EXPECT_FALSE(limit_.counting());
}

// A long job whose page keeps answering, as a tree read after each frame, is never cut off: the
// limit counts from the page's last reply.
TEST_F(AgentReplyLimitTest, EachReplyCountsAgainFromNow) {
  limit_.Started();
  limit_.Sent();
  limit_.Sent();
  clock_.FastForwardBy(AgentReplyLimit::kLimit - kTick);
  limit_.Replied(/*owed=*/1);
  clock_.FastForwardBy(AgentReplyLimit::kLimit - kTick);
  EXPECT_TRUE(ended_.empty());
  clock_.FastForwardBy(kTick);
  EXPECT_EQ(ended_.size(), 1u);
}

// The timer is there only while a job waits on a reply: not on an idle page, not for a picture
// taken with no job, not while the job waits on its own timer with nothing owed, and not after.
TEST_F(AgentReplyLimitTest, ItCountsOnlyWhileAJobWaitsOnAReply) {
  limit_.Sent();
  EXPECT_FALSE(limit_.counting());
  limit_.Replied(/*owed=*/1);
  EXPECT_FALSE(limit_.counting());

  limit_.Started();
  EXPECT_FALSE(limit_.counting());
  limit_.Sent();
  EXPECT_TRUE(limit_.counting());
  limit_.Replied(/*owed=*/0);
  EXPECT_FALSE(limit_.counting());
  clock_.FastForwardBy(AgentReplyLimit::kLimit * 2);
  EXPECT_TRUE(ended_.empty());

  limit_.Sent();
  EXPECT_TRUE(limit_.counting());
  limit_.Ended();
  EXPECT_FALSE(limit_.counting());
  limit_.Sent();
  EXPECT_FALSE(limit_.counting());
  clock_.FastForwardBy(AgentReplyLimit::kLimit * 2);
  EXPECT_TRUE(ended_.empty());
}

// The user's own page, or an agent's with no job running, shows its dialog and nothing ends.
TEST_F(AgentReplyLimitTest, ADialogWithNoJobEndsNothing) {
  limit_.DialogShown("an alert");
  limit_.Started();
  limit_.Ended();
  limit_.DialogShown("an alert");
  EXPECT_TRUE(ended_.empty());
}

// A read or a request looks with no job running: a renderer that never answers the look ends it at
// the same limit, so the page is withheld and the call answered.
TEST_F(AgentReplyLimitTest, ALookWithNoJobEndsAtTheLimit) {
  limit_.Looking(1);
  limit_.Sent();
  EXPECT_TRUE(limit_.counting());
  clock_.FastForwardBy(AgentReplyLimit::kLimit - kTick);
  EXPECT_TRUE(ended_.empty());
  clock_.FastForwardBy(kTick);
  EXPECT_EQ(ended_.size(), 1u);
  // The look went with it, so what's sent after doesn't count.
  limit_.Sent();
  EXPECT_FALSE(limit_.counting());
}

TEST_F(AgentReplyLimitTest, ALookAnsweredStopsTheLimit) {
  limit_.Looking(2);
  limit_.Sent();
  limit_.Looking(1);
  EXPECT_TRUE(limit_.counting());
  limit_.Looking(0);
  EXPECT_FALSE(limit_.counting());
  clock_.FastForwardBy(AgentReplyLimit::kLimit * 2);
  EXPECT_TRUE(ended_.empty());
}

// A job that ends, or one that starts, while a look with no job is out leaves it counting.
TEST_F(AgentReplyLimitTest, ALookOutlivesTheJobsAroundIt) {
  limit_.Started();
  limit_.Sent();
  limit_.Looking(1);
  limit_.Sent();
  limit_.Ended();
  EXPECT_TRUE(limit_.counting());
  limit_.Started();
  EXPECT_TRUE(limit_.counting());
  limit_.Ended();
  clock_.FastForwardBy(AgentReplyLimit::kLimit);
  EXPECT_EQ(ended_.size(), 1u);
}

// A dialog keeps the renderer from answering a look as it does a job's command.
TEST_F(AgentReplyLimitTest, ADialogEndsALookWithNoJob) {
  limit_.Looking(1);
  limit_.Sent();
  limit_.DialogShown("an alert");
  EXPECT_EQ(ended_.size(), 1u);
  EXPECT_FALSE(limit_.counting());
}

}  // namespace
}  // namespace browser_core
