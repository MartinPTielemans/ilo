// The tab a Space shows once a tab the user closed there has gone, when its page lets it go only
// after the window has left the Space (Window::CloseTab), apart so browser_core_unittests tests the
// rule alone (#3890). Tabs are the core's ids, bc_id's.

#ifndef BROWSER_CORE_CLOSING_WAY_BACK_H_
#define BROWSER_CORE_CLOSING_WAY_BACK_H_

#include <cstdint>
#include <map>
#include <optional>

namespace browser_core {

class ClosingWayBack {
 public:
  ClosingWayBack();
  ~ClosingWayBack();

  // The user closed |tab|, which goes once its page lets it.
  void Closing(uint64_t tab);
  // Whether the user closed |tab| and its page hasn't let it go yet.
  bool IsClosing(uint64_t tab) const;
  // The window left |tab|'s Space while |tab| closes. |next| is the tab its Space would show had
  // it closed then, 0 for none. Leaving forgets where the Space's tabs came from, so it's kept here
  // first.
  void Left(uint64_t tab, uint64_t next);
  // Its page kept |tab|, as Stay on its beforeunload does: it's a tab like any other again.
  void Stayed(uint64_t tab);
  // |tab| goes: the tab its Space shows instead, |next| unless the window left the Space while it
  // closed. Forgets |tab|.
  uint64_t Closed(uint64_t tab, uint64_t next);

 private:
  // Each tab closing, with the tab kept as the window left its Space once it has.
  std::map<uint64_t, std::optional<uint64_t>> closing_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_CLOSING_WAY_BACK_H_
