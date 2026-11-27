// What ⇧⌘T brings back (bc_window_reopen_closed_tab), apart so browser_core_unittests tests the
// rule alone (#4408).

#ifndef BROWSER_CORE_CLOSED_RULE_H_
#define BROWSER_CORE_CLOSED_RULE_H_

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "base/time/time.h"
#include "base/values.h"

namespace browser_core::closed_rule {

// A Today tab the archive keeps, or a regular window that closed while another stayed open.
struct Closed {
  uint64_t id = 0;
  bool window = false;
  base::Time at;
  // The directory of the profile it was in.
  std::string profile;
  // Whether the user closed it, not auto-archive's timer, which archives a tab but leaves it for
  // the Library alone: ⇧⌘T brings back only what the user closed (#4408).
  bool by_user = false;
};

// The archive's tabs and the closed windows, each newest first, as one list newest first, a tab
// before a window closed at the same time, as a window's tabs were archived before it closed.
std::vector<Closed> Merged(const std::vector<Closed>& tabs, const std::vector<Closed>& windows);

// The place in |newest_first| of what ⇧⌘T in a window showing a Space of |profile| brings back:
// the newest the user closed in that profile, or none.
std::optional<size_t> Next(const std::vector<Closed>& newest_first, const std::string& profile);

// Who closed it and in which profile, as the session file keeps them with each archived tab and
// closed window, so ⇧⌘T after a relaunch brings back what it would have before. One saved before
// they were kept reads as the timer's, which ⇧⌘T leaves alone.
void Save(const Closed& closed, base::DictValue& dict);
void Load(const base::DictValue& dict, Closed& closed);

}  // namespace browser_core::closed_rule

#endif  // BROWSER_CORE_CLOSED_RULE_H_
