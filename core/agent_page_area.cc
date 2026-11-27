#include "browser_core/agent_page_area.h"

#include <algorithm>

namespace browser_core {

gfx::Size AgentPageArea(const std::vector<gfx::Size>& front_to_back, const gfx::Size& last) {
  const auto front =
      std::ranges::find_if(front_to_back, [](const gfx::Size& area) { return !area.IsEmpty(); });
  return front != front_to_back.end() ? *front : last;
}

}  // namespace browser_core
