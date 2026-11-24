#include "browser_core/agent_job_voter.h"

#include <cstdint>
#include <memory>
#include <optional>
#include <utility>

#include "base/memory/raw_ptr.h"
#include "base/process/process.h"
#include "components/performance_manager/graph/frame_node_impl.h"
#include "components/performance_manager/graph/page_node_impl.h"
#include "components/performance_manager/graph/process_node_impl.h"
#include "components/performance_manager/test_support/graph_test_harness.h"
#include "components/performance_manager/test_support/mock_graphs.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using performance_manager::GraphTestHarness;
using performance_manager::MockSinglePageInSingleProcessGraph;
using performance_manager::execution_context_priority::PriorityVoter;
using performance_manager::execution_context_priority::PriorityVotingSystem;
using performance_manager::execution_context_priority::Vote;
using performance_manager::execution_context_priority::VotingChannel;
using Priority = base::Process::Priority;

// Stands for Chromium's own voters, as the one that votes the frames the user sees user-blocking.
class OtherVoter : public PriorityVoter {
 public:
  explicit OtherVoter(raw_ptr<VotingChannel>* channel) { *channel = &channel_; }

  void InitializeOnGraph(performance_manager::Graph* graph, VotingChannel channel) override {
    channel_ = std::move(channel);
  }
  void TearDownOnGraph(performance_manager::Graph* graph) override { channel_.Reset(); }

 private:
  VotingChannel channel_;
};

class AgentJobVoterTest : public GraphTestHarness {
 public:
  void SetUp() override {
    GraphTestHarness::SetUp();
    // Added before any page, as the browser's is.
    auto* voting = graph()->PassToGraph(std::make_unique<PriorityVotingSystem>());
    voting->AddPriorityVoter<AgentJobVoter>();
    voting->AddPriorityVoter<OtherVoter>(&other_);
    voter_ = AgentJobVoter::Get();
    ASSERT_TRUE(voter_);
  }

  void TearDown() override {
    voter_ = nullptr;
    other_ = nullptr;
    GraphTestHarness::TearDown();
    // The harness otherwise takes the graph down only in its destructor; the voter goes with it.
    TearDownAndDestroyGraph();
    EXPECT_FALSE(AgentJobVoter::Get());
  }

 protected:
  raw_ptr<AgentJobVoter> voter_ = nullptr;
  raw_ptr<VotingChannel> other_ = nullptr;
};

// A hidden page's process runs at the lowest priority until a job works in it, and goes back once
// the job answers.
TEST_F(AgentJobVoterTest, AJobLiftsItsPagesProcessUntilItEnds) {
  MockSinglePageInSingleProcessGraph tab(graph());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);

  voter_->JobStarted(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);
  EXPECT_EQ(tab.frame->GetPriorityAndReason().reason(), AgentJobVoter::kReason);

  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
}

// The next job in the same page lifts it again, and an end with no job running changes nothing.
TEST_F(AgentJobVoterTest, EachJobLiftsAndEndsOnce) {
  MockSinglePageInSingleProcessGraph tab(graph());

  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);

  voter_->JobStarted(tab.page.get());
  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
  voter_->JobEnded(tab.page.get());

  voter_->JobStarted(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);
  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
}

// A read of the page is one of the agent's calls, as an act is (#4227): it gives the page the vote
// as it starts and takes it at its answer. Under way beside an act in the same page, the page keeps
// the vote until the last of them answers, and never ranks with the page the user is looking at.
TEST_F(AgentJobVoterTest, AReadsCallGivesAndTakesBesideAnAct) {
  MockSinglePageInSingleProcessGraph tab(graph());

  voter_->JobStarted(tab.page.get());  // The read.
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);
  voter_->JobEnded(tab.page.get());  // Its answer.
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);

  voter_->JobStarted(tab.page.get());  // An act.
  voter_->JobStarted(tab.page.get());  // A read, while the act works.
  voter_->JobEnded(tab.page.get());    // The first answer.
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible)
      << "the first answer took the vote from the call still under way";
  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
}

// A page the user is looking at in the same process keeps it user-blocking, during the job and
// after it.
TEST_F(AgentJobVoterTest, NeverLowersAProcessTheUserSees) {
  MockSinglePageInSingleProcessGraph tab(graph());
  auto users_page = CreateNode<performance_manager::PageNodeImpl>();
  auto users_frame = CreateFrameNodeAutoId(tab.process.get(), users_page.get());
  other_->SetVote(users_frame.get(), Vote(Priority::kUserBlocking, "visible"));
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserBlocking);

  voter_->JobStarted(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserBlocking);
  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserBlocking);

  other_->SetVote(users_frame.get(), std::nullopt);
}

// A navigation that swaps the page's process mid-job: the new process has the lift from its first
// frame, before the navigation commits, and the old one loses it once its frame goes.
TEST_F(AgentJobVoterTest, AProcessSwapMovesTheLift) {
  MockSinglePageInSingleProcessGraph tab(graph());
  voter_->JobStarted(tab.page.get());

  auto next_process = CreateRendererProcessNode();
  auto next_frame = CreateSpeculativeFrameNodeAutoId(next_process.get(), tab.page.get());
  EXPECT_EQ(next_process->GetPriority(), Priority::kUserVisible);
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);

  tab.frame.reset();
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
  EXPECT_EQ(next_process->GetPriority(), Priority::kUserVisible);

  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(next_process->GetPriority(), Priority::kBestEffort);
}

// A renderer that crashes mid-job takes its frames with it; the page's next renderer is lifted
// while the job still runs, and the crashed one never is again.
TEST_F(AgentJobVoterTest, ACrashedRenderersReplacementIsLifted) {
  MockSinglePageInSingleProcessGraph tab(graph());
  voter_->JobStarted(tab.page.get());

  tab.frame.reset();
  tab.process.reset();

  auto next_process = CreateRendererProcessNode();
  auto next_frame = CreateFrameNodeAutoId(next_process.get(), tab.page.get());
  EXPECT_EQ(next_process->GetPriority(), Priority::kUserVisible);

  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(next_process->GetPriority(), Priority::kBestEffort);
}

// A tab closed mid-job takes its vote with its page, however many jobs were under way in it.
// Otherwise the voting system's check that no vote outlives its page stops the test.
TEST_F(AgentJobVoterTest, ATabClosedMidJobTakesItsVote) {
  MockSinglePageInSingleProcessGraph tab(graph());
  voter_->JobStarted(tab.page.get());
  voter_->JobStarted(tab.page.get());

  tab.frame.reset();
  tab.page.reset();
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);

  // The next tab in that process is lifted by its own job only.
  auto next_page = CreateNode<performance_manager::PageNodeImpl>();
  auto next_frame = CreateFrameNodeAutoId(tab.process.get(), next_page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);
  voter_->JobStarted(next_page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);
  voter_->JobEnded(next_page.get());
}

base::WeakPtr<const performance_manager::PageNode> PageOf(
    const performance_manager::PageNodeImpl* page) {
  return static_cast<const performance_manager::PageNode*>(page)->GetWeakPtr();
}

// The user's page that loads, in a renderer of its own, beside the agent's job's.
class UserLoadTest : public AgentJobVoterTest {
 public:
  void SetUp() override {
    AgentJobVoterTest::SetUp();
    job_ = std::make_unique<MockSinglePageInSingleProcessGraph>(graph());
    users_ = std::make_unique<MockSinglePageInSingleProcessGraph>(graph());
    voter_->JobStarted(job_->page.get());
  }

  void TearDown() override {
    if (job_->page.get()) {
      voter_->JobEnded(job_->page.get());
    }
    users_.reset();
    job_.reset();
    AgentJobVoterTest::TearDown();
  }

 protected:
  bool JobLifted() const { return job_->process->GetPriority() == Priority::kUserVisible; }

  // The user's selected tab starts loading |navigation|.
  void StartShown(UserLoad& load, int64_t navigation = 1) {
    load.Started(navigation, PageOf(users_->page.get()), /*shown=*/true, /*agents=*/false);
  }

  std::unique_ptr<MockSinglePageInSingleProcessGraph> job_;
  std::unique_ptr<MockSinglePageInSingleProcessGraph> users_;
};

// The page the user is looking at loads: the job's page has no vote until its first visually
// non-empty paint, then has it back.
TEST_F(UserLoadTest, ALoadTheUserSeesTakesTheVoteUntilItPaints) {
  UserLoad load;
  EXPECT_TRUE(JobLifted());

  StartShown(load);
  EXPECT_TRUE(load.counts());
  EXPECT_EQ(job_->process->GetPriority(), Priority::kBestEffort);

  load.Painted();
  EXPECT_FALSE(load.counts());
  EXPECT_TRUE(JobLifted());
}

// A load that never paints ends as it stops loading.
TEST_F(UserLoadTest, TheLoadEndsAsItStops) {
  UserLoad load;
  StartShown(load);
  load.Stopped();
  EXPECT_TRUE(JobLifted());
}

// A navigation that commits nothing, as a download or one the user stopped, ends the load. One
// that commits goes on counting until the page paints.
TEST_F(UserLoadTest, ANavigationThatCommitsNothingEndsTheLoad) {
  UserLoad load;
  StartShown(load, 1);
  load.Finished(1, /*committed=*/true);
  EXPECT_FALSE(JobLifted());
  load.Painted();

  StartShown(load, 2);
  load.Finished(2, /*committed=*/false);
  EXPECT_TRUE(JobLifted());
}

// A navigation the user replaced with another finishes without committing; the new one still
// counts.
TEST_F(UserLoadTest, AReplacedNavigationDoesntEndTheNextOnesLoad) {
  UserLoad load;
  StartShown(load, 1);
  StartShown(load, 2);
  load.Finished(1, /*committed=*/false);
  EXPECT_TRUE(load.counts());
  EXPECT_FALSE(JobLifted());

  load.Finished(2, /*committed=*/false);
  EXPECT_TRUE(JobLifted());
}

// A tab closed mid-load ends its load, and so does its page going from the graph first.
TEST_F(UserLoadTest, ATabClosedMidLoadEndsIt) {
  {
    UserLoad load;
    StartShown(load);
    load.Closed();
    EXPECT_TRUE(JobLifted());
  }
  {
    UserLoad load;
    StartShown(load);
  }
  EXPECT_TRUE(JobLifted());

  UserLoad load;
  StartShown(load);
  EXPECT_FALSE(JobLifted());
  users_->frame.reset();
  users_->page.reset();
  EXPECT_TRUE(JobLifted());
  load.Painted();
  EXPECT_TRUE(JobLifted());
}

// A background tab of the user's, such as a page reloading itself, never takes the vote.
TEST_F(UserLoadTest, ABackgroundTabsLoadDoesntCount) {
  UserLoad load;
  for (int navigation = 1; navigation <= 3; ++navigation) {
    load.Started(navigation, PageOf(users_->page.get()), /*shown=*/false, /*agents=*/false);
    EXPECT_FALSE(load.counts());
    EXPECT_TRUE(JobLifted());
    load.Painted();
  }
}

// A tab opened in front may start its navigation before it's shown: the load counts from then
// until it paints. A tab the user leaves mid-load stops counting.
TEST_F(UserLoadTest, ALoadCountsWhileItsTabIsShown) {
  UserLoad load;
  load.Started(1, PageOf(users_->page.get()), /*shown=*/false, /*agents=*/false);
  EXPECT_TRUE(JobLifted());

  load.Shown(true, /*agents=*/false);
  EXPECT_FALSE(JobLifted());

  load.Shown(false, /*agents=*/false);
  EXPECT_TRUE(JobLifted());

  load.Shown(true, /*agents=*/false);
  load.Painted();
  EXPECT_TRUE(JobLifted());

  // Shown again once its load is over, it doesn't count.
  load.Shown(false, /*agents=*/false);
  load.Shown(true, /*agents=*/false);
  EXPECT_TRUE(JobLifted());
}

// A tab the core makes once its first navigation has started, as a tab typed in the command bar
// is, joins that load: it counts once the tab is shown, until it paints (#3285).
TEST_F(UserLoadTest, ATabMadeMidLoadCountsThatLoadOnceShown) {
  UserLoad load;
  load.Joined(PageOf(users_->page.get()), /*shown=*/false, /*agents=*/false);
  EXPECT_FALSE(load.counts());
  EXPECT_TRUE(JobLifted());

  load.Shown(true, /*agents=*/false);
  EXPECT_TRUE(load.counts());
  EXPECT_FALSE(JobLifted());

  // The joined navigation commits: the load goes on until the page paints.
  load.Finished(7, /*committed=*/true);
  EXPECT_FALSE(JobLifted());
  load.Painted();
  EXPECT_TRUE(JobLifted());
}

// The joined navigation, whose id the tab never heard, ends the load if it commits nothing. A
// navigation the user starts after it is told by its id as before.
TEST_F(UserLoadTest, AJoinedLoadEndsOnItsFirstFinishWithoutACommit) {
  UserLoad load;
  load.Joined(PageOf(users_->page.get()), /*shown=*/true, /*agents=*/false);
  EXPECT_FALSE(JobLifted());
  load.Finished(9, /*committed=*/false);
  EXPECT_FALSE(load.counts());
  EXPECT_TRUE(JobLifted());

  load.Joined(PageOf(users_->page.get()), /*shown=*/true, /*agents=*/false);
  StartShown(load, 12);
  load.Finished(11, /*committed=*/false);
  EXPECT_FALSE(JobLifted());
  load.Finished(12, /*committed=*/false);
  EXPECT_TRUE(JobLifted());

  load.Joined(PageOf(users_->page.get()), /*shown=*/true, /*agents=*/false);
  load.Stopped();
  EXPECT_TRUE(JobLifted());
}

// An agent's tab is made before its Space holds it, so it can't tell it's the agent's as it joins
// its first load; shown, it can, and its load never counts.
TEST_F(UserLoadTest, AnAgentsTabMadeMidLoadNeverCounts) {
  UserLoad load;
  load.Joined(PageOf(users_->page.get()), /*shown=*/false, /*agents=*/false);
  load.Shown(true, /*agents=*/true);
  EXPECT_FALSE(load.counts());
  EXPECT_TRUE(JobLifted());
  load.Painted();
  EXPECT_TRUE(JobLifted());
}

// A tab the agent has never counts, even shown. Once the user has taken it over it's theirs, and
// its next load counts.
TEST_F(UserLoadTest, AnAgentsTabCountsOnlyOnceTheUserHasIt) {
  UserLoad load;
  load.Started(1, PageOf(users_->page.get()), /*shown=*/true, /*agents=*/true);
  EXPECT_FALSE(load.counts());
  EXPECT_TRUE(JobLifted());
  load.Painted();

  load.Started(2, PageOf(users_->page.get()), /*shown=*/true, /*agents=*/false);
  EXPECT_FALSE(JobLifted());
  load.Painted();
  EXPECT_TRUE(JobLifted());
}

// Two of the user's loads: the vote comes back once both end.
TEST_F(UserLoadTest, TheVoteComesBackWhenEveryLoadHasEnded) {
  MockSinglePageInSingleProcessGraph other(graph());
  UserLoad first;
  UserLoad second;
  StartShown(first);
  second.Started(1, PageOf(other.page.get()), /*shown=*/true, /*agents=*/false);

  first.Painted();
  EXPECT_FALSE(JobLifted());
  second.Painted();
  EXPECT_TRUE(JobLifted());
}

// A job that starts during the user's load is lifted once the load ends, and one that ends during
// it isn't lifted again.
TEST_F(UserLoadTest, AJobStartingOrEndingMidLoadFollowsTheCount) {
  MockSinglePageInSingleProcessGraph next(graph());
  UserLoad load;
  StartShown(load);

  voter_->JobStarted(next.page.get());
  EXPECT_EQ(next.process->GetPriority(), Priority::kBestEffort);
  voter_->JobEnded(job_->page.get());

  load.Painted();
  EXPECT_EQ(next.process->GetPriority(), Priority::kUserVisible);
  EXPECT_EQ(job_->process->GetPriority(), Priority::kBestEffort);

  voter_->JobEnded(next.page.get());
}

// The job's tab closes during the user's load: its page took its vote along, and the load's end
// gives none back to it, which the voting system's check that no vote outlives its page would stop.
TEST_F(UserLoadTest, AJobsTabClosedMidLoadGetsNoVoteBack) {
  UserLoad load;
  StartShown(load);
  job_->frame.reset();
  job_->page.reset();
  load.Painted();
  EXPECT_EQ(job_->process->GetPriority(), Priority::kBestEffort);
}

}  // namespace
}  // namespace browser_core
