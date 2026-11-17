#ifndef BROWSER_CORE_AGENT_JOB_VOTER_H_
#define BROWSER_CORE_AGENT_JOB_VOTER_H_

#include <optional>
#include <set>

#include "components/performance_manager/public/execution_context_priority/execution_context_priority.h"
#include "components/performance_manager/public/execution_context_priority/priority_voting_system.h"
#include "components/performance_manager/public/graph/page_node.h"

namespace browser_core {

// An agent acts in a hidden tab, and the OS runs a hidden tab's renderer as a background process,
// which on Windows lost the CPU about 28 ms in every 48 while an agent typed (#3285). While an
// agent's job works in a page, this votes the page user-visible in Chromium's priority voting. Its
// performance manager gives the vote to every frame of the page, in whatever process holds each,
// and gives each process the highest vote of its frames, so a process the user's own page shares
// is never lowered, and a navigation that moves the page to another process takes the vote along.
// Apart from content so browser_core_unittests tests it on a graph alone.
class AgentJobVoter : public performance_manager::execution_context_priority::PriorityVoter,
                      public performance_manager::PageNodeObserver {
 public:
  static const char kReason[];

  AgentJobVoter();
  ~AgentJobVoter() override;
  AgentJobVoter(const AgentJobVoter&) = delete;
  AgentJobVoter& operator=(const AgentJobVoter&) = delete;

  // The voter on the browser's graph, or null before it's added and once the graph is gone.
  static AgentJobVoter* Get();

  // A job starts working in |page|, or stops: it answered, or the agent's session ended.
  void JobStarted(const performance_manager::PageNode* page);
  void JobEnded(const performance_manager::PageNode* page);

  // PriorityVoter:
  void InitializeOnGraph(
      performance_manager::Graph* graph,
      performance_manager::execution_context_priority::VotingChannel voting_channel) override;
  void TearDownOnGraph(performance_manager::Graph* graph) override;

  // PageNodeObserver: a closing tab's page takes its vote with it.
  void OnBeforePageNodeRemoved(const performance_manager::PageNode* page) override;

 private:
  performance_manager::execution_context_priority::VotingChannel voting_channel_;
  std::set<const performance_manager::PageNode*> working_;
};

// Adds the voter to the browser's graph, once, as the browser starts.
void AddAgentJobVoter();

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_JOB_VOTER_H_
