#ifndef BROWSER_CORE_AGENT_JOB_VOTER_H_
#define BROWSER_CORE_AGENT_JOB_VOTER_H_

#include <cstdint>
#include <map>
#include <optional>
#include <set>

#include "base/memory/weak_ptr.h"
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
//
// The vote steps aside while a page the user is looking at loads: on Windows the agent's renderer
// with the vote slowed the user's new tab during a long job (#3285). A load counts from the start
// of a navigation in a tab the user is looking at, its window's selected tab or one opened in
// front, to the page's first visually non-empty paint, a navigation that commits nothing, the end
// of the load or the tab's closing. A load in a tab the user isn't looking at, such as a page
// reloading itself in the background, doesn't count, so it can't keep taking the vote away. An
// agent's tabs never count while the agent has them. One the user has taken over is theirs until
// they hand it back, and its loads count as their other tabs' do, as does one moved out of the
// agent's Space. Tab says which loads count.
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

  // A job starts working in |page|, or stops: it answered, or the agent's session ended. A job is
  // any of the agent's calls under way in the page, an act or a read, and several may be at once:
  // the page has the vote from the first one's start to the last one's end.
  void JobStarted(const performance_manager::PageNode* page);
  void JobEnded(const performance_manager::PageNode* page);

  // A page the user is looking at starts loading, or stops counting: it painted, committed
  // nothing, stopped loading, went out of sight or closed. Each page counts once however often it
  // starts. While any counts, no job's page has the vote; when none does, each has it back.
  void UserLoadStarted(const performance_manager::PageNode* page);
  void UserLoadEnded(const performance_manager::PageNode* page);

  // PriorityVoter:
  void InitializeOnGraph(
      performance_manager::Graph* graph,
      performance_manager::execution_context_priority::VotingChannel voting_channel) override;
  void TearDownOnGraph(performance_manager::Graph* graph) override;

  // PageNodeObserver: a closing tab's page takes its vote with it, and its load ends.
  void OnBeforePageNodeRemoved(const performance_manager::PageNode* page) override;

 private:
  // Each job's page has the vote while |user_loads_| is empty.
  void GiveVote(const performance_manager::PageNode* page);
  void TakeVote(const performance_manager::PageNode* page);

  performance_manager::execution_context_priority::VotingChannel voting_channel_;
  // The jobs under way in each page that has one.
  std::map<const performance_manager::PageNode*, int> working_;
  std::set<const performance_manager::PageNode*> user_loads_;
};

// One of the user's tabs as the voter sees it: whether a page the user is looking at is loading in
// it, which Tab tells it from its page's events. It tells the voter when the load starts and stops
// counting. Apart from content so browser_core_unittests tests the rule alone.
class UserLoad {
 public:
  UserLoad();
  ~UserLoad();
  UserLoad(const UserLoad&) = delete;
  UserLoad& operator=(const UserLoad&) = delete;

  // |navigation| starts in the tab's main frame for another document. |page| is the tab's page,
  // |shown| whether the user is looking at it, and |agents| whether the agent has the tab now.
  void Started(int64_t navigation,
               base::WeakPtr<const performance_manager::PageNode> page,
               bool shown,
               bool agents);
  // The tab came to its page with a load already under way, as a tab the core makes once
  // Chrome's Navigate has started its first navigation does (#3285). It counts as Started's would,
  // with that navigation's id unknown: the first navigation to finish is taken to be it.
  void Joined(base::WeakPtr<const performance_manager::PageNode> page, bool shown, bool agents);
  // |navigation| finished, having |committed| a document or not. One that commits nothing, as a
  // download or a stopped navigation, ends the load; one the tab has since replaced doesn't.
  void Finished(int64_t navigation, bool committed);
  // The page's first visually non-empty paint, its load's end, or the tab's closing.
  void Painted();
  void Stopped();
  void Closed();
  // The tab is shown or hidden: its load counts only while the user is looking at it, and only
  // while |agents| says the agent doesn't have the tab, which a tab may learn only once it's made.
  void Shown(bool shown, bool agents);

  bool counts() const { return counted_; }

 private:
  void End();
  void Update();

  // Joined's navigation, whose id the tab never heard.
  static constexpr int64_t kInFlight = -1;

  int64_t navigation_ = 0;
  base::WeakPtr<const performance_manager::PageNode> page_;
  bool loading_ = false;
  bool shown_ = false;
  bool agents_ = false;
  // What the voter was last told.
  bool counted_ = false;
};

// Adds the voter to the browser's graph, once, as the browser starts.
void AddAgentJobVoter();

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_JOB_VOTER_H_
