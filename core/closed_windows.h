#ifndef BROWSER_CORE_CLOSED_WINDOWS_H_
#define BROWSER_CORE_CLOSED_WINDOWS_H_

#include <optional>
#include <string>
#include <vector>

#include "browser_core/closed_rule.h"
#include "browser_core/public/browser_core.h"

namespace browser_core {

// The regular windows the user closed while another stayed open, which ⇧⌘T opens again
// (bc_window_reopen_closed_tab, #4408). Their tabs went to the window that stayed, in the Spaces
// every regular window shows, so one coming back shows its Space and its tab again. The last
// regular window isn't one: the session keeps it, as a quit does. Saved with the Spaces, and kept
// as long as the archive keeps a tab, the newest 25. ClosedWindow is in closed_rule.h.

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
