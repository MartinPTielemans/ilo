#include "browser_core/agent_job_voter.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/process/process.h"
#include "components/performance_manager/public/graph/graph.h"
#include "components/performance_manager/public/performance_manager.h"

namespace browser_core {

namespace {

using performance_manager::Graph;
using performance_manager::PageNode;
using performance_manager::execution_context_priority::PriorityVotingSystem;
using performance_manager::execution_context_priority::Vote;
using performance_manager::execution_context_priority::VotingChannel;

AgentJobVoter* g_voter = nullptr;

}  // namespace

// static
const char AgentJobVoter::kReason[] = "An agent's job works in the page.";

AgentJobVoter::AgentJobVoter() = default;

AgentJobVoter::~AgentJobVoter() = default;

// static
AgentJobVoter* AgentJobVoter::Get() {
  return g_voter;
}

void AgentJobVoter::JobStarted(const PageNode* page) {
  if (++working_[page] == 1 && user_loads_.empty()) {
    GiveVote(page);
  }
}

void AgentJobVoter::JobEnded(const PageNode* page) {
  auto job = working_.find(page);
  if (job == working_.end() || --job->second > 0) {
    return;
  }
  working_.erase(job);
  if (user_loads_.empty()) {
    TakeVote(page);
  }
}

// static
base::ScopedClosureRunner AgentJobVoter::Lift(base::WeakPtr<PageNode> page) {
  AgentJobVoter* voter = Get();
  if (!page || !voter) {
    return {};
  }
  voter->JobStarted(page.get());
  return base::ScopedClosureRunner(base::BindOnce(
      [](base::WeakPtr<PageNode> page) {
        if (AgentJobVoter* voter = Get(); voter && page) {
          voter->JobEnded(page.get());
        }
      },
      std::move(page)));
}

void AgentJobVoter::UserLoadStarted(const PageNode* page) {
  if (!user_loads_.insert(page).second || user_loads_.size() > 1) {
    return;
  }
  for (const auto& [job, count] : working_) {
    TakeVote(job);
  }
}

void AgentJobVoter::UserLoadEnded(const PageNode* page) {
  if (!user_loads_.erase(page) || !user_loads_.empty()) {
    return;
  }
  for (const auto& [job, count] : working_) {
    GiveVote(job);
  }
}

void AgentJobVoter::GiveVote(const PageNode* page) {
  // User-visible and not user-blocking: a hidden agent page doesn't rank with the page the user is
  // looking at.
  voting_channel_.SetVote(page, Vote(base::Process::Priority::kUserVisible, kReason));
}

void AgentJobVoter::TakeVote(const PageNode* page) {
  voting_channel_.SetVote(page, std::nullopt);
}

void AgentJobVoter::InitializeOnGraph(Graph* graph, VotingChannel voting_channel) {
  voting_channel_ = std::move(voting_channel);
  graph->AddPageNodeObserver(this);
  g_voter = this;
}

void AgentJobVoter::TearDownOnGraph(Graph* graph) {
  g_voter = nullptr;
  // While a load of the user's counts, the jobs' pages have no vote to take.
  if (std::exchange(user_loads_, {}).empty()) {
    for (const auto& [page, count] : working_) {
      TakeVote(page);
    }
  }
  working_.clear();
  graph->RemovePageNodeObserver(this);
  voting_channel_.Reset();
}

void AgentJobVoter::OnBeforePageNodeRemoved(const PageNode* page) {
  // Every job under way in it ends with it; its jobs' own ends, later, find no page.
  if (working_.erase(page) && user_loads_.empty()) {
    TakeVote(page);
  }
  UserLoadEnded(page);
}

UserLoad::UserLoad() = default;

UserLoad::~UserLoad() {
  Closed();
}

void UserLoad::Started(int64_t navigation,
                       base::WeakPtr<const PageNode> page,
                       bool shown,
                       bool agents) {
  // A navigation that replaces one in flight goes on counting as the same load. The tab's page
  // having changed between them ends the first page's.
  if (counted_ && page.get() != page_.get()) {
    End();
  }
  navigation_ = navigation;
  page_ = std::move(page);
  loading_ = true;
  shown_ = shown;
  agents_ = agents;
  Update();
}

void UserLoad::Joined(base::WeakPtr<const PageNode> page, bool shown, bool agents) {
  Started(kInFlight, std::move(page), shown, agents);
}

void UserLoad::Finished(int64_t navigation, bool committed) {
  if (navigation_ == kInFlight) {
    navigation_ = navigation;
  }
  if (navigation == navigation_ && !committed) {
    End();
  }
}

void UserLoad::Painted() {
  End();
}

void UserLoad::Stopped() {
  End();
}

void UserLoad::Closed() {
  End();
}

void UserLoad::Shown(bool shown, bool agents) {
  shown_ = shown;
  agents_ = agents;
  Update();
}

void UserLoad::End() {
  loading_ = false;
  Update();
}

void UserLoad::Update() {
  const bool counts = loading_ && shown_ && !agents_ && page_;
  if (counts == counted_) {
    return;
  }
  counted_ = counts;
  // A page already gone has left the voter's count with its node.
  AgentJobVoter* voter = AgentJobVoter::Get();
  if (!voter || !page_) {
    return;
  }
  if (counts) {
    voter->UserLoadStarted(page_.get());
  } else {
    voter->UserLoadEnded(page_.get());
  }
}

void AddAgentJobVoter() {
  Graph* graph = performance_manager::PerformanceManager::GetGraph();
  PriorityVotingSystem* voting =
      graph ? graph->GetRegisteredObjectAs<PriorityVotingSystem>() : nullptr;
  if (voting && !g_voter) {
    voting->AddPriorityVoter<AgentJobVoter>();
  }
}

}  // namespace browser_core
