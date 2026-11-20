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

void DocumentPresses::Users(int64_t document,
                            base::TimeTicks at,
                            std::optional<base::TimeTicks> key) {
  before_key_ = users_;
  key_ = key;
  users_ = {document, at};
}

void DocumentPresses::Agents(int64_t document, base::TimeTicks at) {
  agents_ = {document, at};
}

void DocumentPresses::TakenBack(base::TimeTicks key) {
  // A later press of the user's is theirs, whatever came of the key before it.
  if (key_ == key) {
    users_ = before_key_;
    key_.reset();
  }
}

void DocumentPresses::Fill(int64_t document, ProtectedAsk& ask) const {
  ask.users_input = users_.document == document ? users_.at : base::TimeTicks();
  ask.agents_input = agents_.document == document ? agents_.at : base::TimeTicks();
}

}  // namespace browser_core
