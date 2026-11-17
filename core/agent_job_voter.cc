#include "browser_core/agent_job_voter.h"

#include <utility>

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
  // User-visible and not user-blocking: a hidden agent page doesn't rank with the page the user is
  // looking at.
  if (working_.insert(page).second) {
    voting_channel_.SetVote(page, Vote(base::Process::Priority::kUserVisible, kReason));
  }
}

void AgentJobVoter::JobEnded(const PageNode* page) {
  if (working_.erase(page)) {
    voting_channel_.SetVote(page, std::nullopt);
  }
}

void AgentJobVoter::InitializeOnGraph(Graph* graph, VotingChannel voting_channel) {
  voting_channel_ = std::move(voting_channel);
  graph->AddPageNodeObserver(this);
  g_voter = this;
}

void AgentJobVoter::TearDownOnGraph(Graph* graph) {
  g_voter = nullptr;
  for (const PageNode* page : std::exchange(working_, {})) {
    voting_channel_.SetVote(page, std::nullopt);
  }
  graph->RemovePageNodeObserver(this);
  voting_channel_.Reset();
}

void AgentJobVoter::OnBeforePageNodeRemoved(const PageNode* page) {
  JobEnded(page);
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
