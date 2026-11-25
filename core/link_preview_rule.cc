#include "browser_core/link_preview_rule.h"

#include <utility>

namespace browser_core {

void LinkPreviewAsk::Keep(std::string_view url) {
  asked_ = url;
}

bool LinkPreviewAsk::Take(std::string_view url) {
  const std::string asked = std::exchange(asked_, std::string());
  return !url.empty() && asked == url;
}

}  // namespace browser_core
