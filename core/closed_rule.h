// What ⇧⌘T brings back (bc_window_reopen_closed_tab), apart so browser_core_unittests tests the
// rule alone (#4408).

#ifndef BROWSER_CORE_CLOSED_RULE_H_
#define BROWSER_CORE_CLOSED_RULE_H_

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "base/time/time.h"
#include "base/uuid.h"
#include "base/values.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core {

// A regular window the user closed while another stayed open, which ⇧⌘T opens again
// (closed_windows.h). Here so the unittests read and write it as the session file does.
struct ClosedWindow {
  // Its own id, for the app (window_reopen, bc_window_open_closed): a bc_id, new each launch.
  uint64_t id = 0;
  base::Time closed;
  // Its frame out of full screen and maximized, and whether it was either, as the session keeps a
  // window's (bc_saved_window_frames). Never maximized on the Mac.
  gfx::Rect bounds;
  bool maximized = false;
  bool fullscreen = false;
  double sidebar_width = 0;
  // The node of the Space it showed, and of the pinned or Today tab it showed there, if any.
  base::Uuid space;
  base::Uuid tab;
  // The directory of that Space's profile, whose windows' ⇧⌘T bring it back.
  std::string profile;
};

}  // namespace browser_core

namespace browser_core::closed_rule {

// How many of the tabs the user closed keep their history, as Chrome's recently closed keeps it.
inline constexpr size_t kWithHistory = 25;

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

// The ids of those in |newest_first| that keep their history, back and forward: the newest
// kWithHistory the user closed, in any profile. The rest come back at their page (#4408).
std::vector<uint64_t> WithHistory(const std::vector<Closed>& newest_first);

// |window| as the session file keeps it, and back, with no id: none outlasts a launch. A record
// without its time or Space reads as none.
base::DictValue SaveWindow(const ClosedWindow& window);
std::optional<ClosedWindow> LoadWindow(const base::DictValue& dict);

}  // namespace browser_core::closed_rule

#endif  // BROWSER_CORE_CLOSED_RULE_H_
