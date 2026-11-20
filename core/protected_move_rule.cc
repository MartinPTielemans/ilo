#include "browser_core/protected_move_rule.h"

namespace browser_core {

ProtectedAnswer AnswerProtectedAsk(const ProtectedAsk& ask) {
  if (!ask.allowed || ask.preview || ask.kept_in_chromium || !ask.own_site) {
    return ProtectedAnswer::kNote;
  }
  const bool users_press = !ask.users_input.is_null() && ask.users_input <= ask.now &&
                           ask.now - ask.users_input <= kPressLifespan &&
                           (ask.agents_input.is_null() || ask.users_input > ask.agents_input);
  if (!users_press) {
    return ProtectedAnswer::kNote;
  }
  if (ask.capturing) {
    return ProtectedAnswer::kAskCapturing;
  }
  if (ask.typed || ask.asks_before_leaving) {
    return ProtectedAnswer::kAskTyped;
  }
  return ProtectedAnswer::kMove;
}

}  // namespace browser_core
