#ifndef BROWSER_CORE_CLOSED_WINDOWS_H_
#define BROWSER_CORE_CLOSED_WINDOWS_H_

#include <optional>
#include <string>
#include <vector>

#include "base/time/time.h"
#include "base/uuid.h"
#include "browser_core/public/browser_core.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core {

// The regular windows the user closed while another stayed open, which ⇧⌘T opens again
// (bc_window_reopen_closed_tab, #4408). Their tabs went to the window that stayed, in the Spaces
// every regular window shows, so one coming back shows its Space and its tab again. The last
// regular window isn't one: the session keeps it, as a quit does. Saved with the Spaces, and kept
// as long as the archive keeps a tab, the newest 25.
struct ClosedWindow {
  // Its own id, for the app (window_reopen, bc_window_open_closed).
  bc_id id = 0;
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

// All of them, newest first.
const std::vector<ClosedWindow>& ClosedWindows();
// Puts back what the last launch saved, as the browser starts.
void LoadClosedWindows(std::vector<ClosedWindow> windows);
// Keeps |window|, closing now, under a new id.
void KeepClosedWindow(ClosedWindow window);
// Takes the closed window |id| out and returns it.
std::optional<ClosedWindow> TakeClosedWindow(bc_id id);

}  // namespace browser_core

#endif  // BROWSER_CORE_CLOSED_WINDOWS_H_
