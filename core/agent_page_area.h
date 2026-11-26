// The size an agent's page is made at, apart so browser_core_unittests tests the rule alone.
#ifndef BROWSER_CORE_AGENT_PAGE_AREA_H_
#define BROWSER_CORE_AGENT_PAGE_AREA_H_

#include <vector>

#include "ui/gfx/geometry/size.h"

namespace browser_core {

// An agent's page is the user's size from the start (#4355), so a watch never lays it out again
// mid-task: the page area of the frontmost window that has one, given front to back, else
// |last|, the one a window had last, as with every window closed. Empty while no window has
// laid out a page, which leaves a page the size it was made at.
gfx::Size AgentPageArea(const std::vector<gfx::Size>& front_to_back, const gfx::Size& last);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_PAGE_AREA_H_
