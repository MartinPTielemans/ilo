#include "browser_core/page_in_field_rule.h"

namespace browser_core {

std::optional<bool> PageInField::Update(const Now& now) {
  const bool in_field = now.agents && now.editable && now.focused && now.key;
  if (in_field == said_) {
    return std::nullopt;
  }
  said_ = in_field;
  return in_field;
}

std::optional<bool> PageInField::Closed() {
  return Update(Now{});
}

}  // namespace browser_core
