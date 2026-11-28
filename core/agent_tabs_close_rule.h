// When a ⌘T thread's own tabs close after it goes quiet (bc_agent_tabs_close_after), and which,
// apart so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_AGENT_TABS_CLOSE_RULE_H_
#define BROWSER_CORE_AGENT_TABS_CLOSE_RULE_H_

#include <optional>

#include "base/time/time.h"

namespace browser_core::agent_tabs_close_rule {

// Settings › Agents' pop-up, in seconds: Never, 5, 10, 15, 30 and 60 minutes, and 2 hours.
inline constexpr double kChoices[] = {0, 300, 600, 900, 1800, 3600, 7200};
inline constexpr double kDefault = 900;

// The choice nearest |seconds|, the default for one that isn't a number.
double Choice(double seconds);
// How long a thread waits for the |choice|, or none for Never.
std::optional<base::TimeDelta> After(double choice);

// One thread's last activity, and whether a run of it is under way.
class Clock {
 public:
  // An agent call in one of its tabs, or a tab opened for it.
  void Acted(base::Time now);
  // A run started or ended. Ending one is activity: the count starts there.
  void SetWorking(bool working, base::Time now);
  bool working() const { return working_; }
  // When its tabs close: never while a run is under way, for Never, or before any activity.
  std::optional<base::Time> DueAt(std::optional<base::TimeDelta> after) const;
  bool Due(base::Time now, std::optional<base::TimeDelta> after) const;

 private:
  std::optional<base::Time> last_;
  bool working_ = false;
};

// What the rule asks of a tab in the thread's own Space.
struct Tab {
  // The user handed it the thread (bc_tab_give_to_agent): the user's, never closed.
  bool handed = false;
  // The user holds it, at a handoff or after taking it over.
  bool held = false;
  bool pinned = false;
  // A window shows or pictures it: it closes once none does (ClosesNow).
  bool shown = false;
};

// Whether |tab| is the thread's own copy, which closes once the thread is due.
bool Own(const Tab& tab);
// Whether it closes now that the thread is due: its own, and no window shows it.
bool ClosesNow(const Tab& tab);

}  // namespace browser_core::agent_tabs_close_rule

#endif  // BROWSER_CORE_AGENT_TABS_CLOSE_RULE_H_
