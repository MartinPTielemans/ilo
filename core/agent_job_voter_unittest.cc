#include "browser_core/agent_job_voter.h"

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
  voter_->JobStarted(tab.page.get());
  voter_->JobEnded(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kBestEffort);

  voter_->JobStarted(tab.page.get());
  EXPECT_EQ(tab.process->GetPriority(), Priority::kUserVisible);
  voter_->JobEnded(tab.page.get());
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

// A tab closed mid-job takes its vote with its page. Otherwise the voting system's check that no
// vote outlives its page stops the test.
TEST_F(AgentJobVoterTest, ATabClosedMidJobTakesItsVote) {
  MockSinglePageInSingleProcessGraph tab(graph());
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

}  // namespace
}  // namespace browser_core
