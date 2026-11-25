#include "browser_core/secure_connections_rule.h"

namespace browser_core::secure_connections_rule {

bool IsOn(Mode mode) {
  return mode != Mode::kOff;
}

Mode After(Mode now, bool on) {
  if (!on) {
    return Mode::kOff;
  }
  return IsOn(now) ? now : Mode::kBalanced;
}

bool NoWarning(const agent_space_rule::Tab& tab) {
  return tab.agents && !tab.held;
}

PutBack PutBackTo(bool has_a_page_behind) {
  return has_a_page_behind ? PutBack::kBack : PutBack::kClosed;
}

}  // namespace browser_core::secure_connections_rule
