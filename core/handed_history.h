// Which entries of its history a tab the user handed an agent (handed.h) may be taken to. The
// history the tab had when it was handed is the user's: the thread may be on the entry it was
// handed on and on those made since, and nothing takes the tab to another, whoever asks: the
// agent's back and forward, the app's, the page's history.back(), forward() and go(), or its
// navigation.back(), forward() and traverseTo(). Chromium takes them all through one function,
// which asks here first and says whether a page asked (chromium-core-history-traversal.patch).

#ifndef BROWSER_CORE_HANDED_HISTORY_H_
#define BROWSER_CORE_HANDED_HISTORY_H_

#include <set>
#include <vector>

namespace browser_core::handed_history {

class Bound {
 public:
  // The tab was handed, the user handed it back after holding it, or its page was made again: its
  // history is |entries|, each by the id Chromium gave it (NavigationEntry::GetUniqueID), and it's
  // on |current|. Every entry but that one is the user's from here. An entry made later has an id
  // none of these had, so it's the thread's, whichever it replaced or cut off.
  void Set(const std::vector<int>& entries, int current);

  // Whether the tab may be taken to |entry|. While the user holds the tab (|held|: they took it
  // over, or the agent handed it off), they drive it, and the history is theirs: the browser's own
  // back, forward and go to an entry pass. A page's own traversal (|from_page|) is never a step of
  // theirs: an agent can put the tab in the user's hands at will, and a page it steered to could
  // then go to an entry of the user's by itself, for the thread to read once it's handed back.
  bool MayGoTo(int entry, bool held, bool from_page) const;

 private:
  std::set<int> users_;
};

}  // namespace browser_core::handed_history

#endif  // BROWSER_CORE_HANDED_HISTORY_H_
