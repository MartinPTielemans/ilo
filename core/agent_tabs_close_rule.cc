#include "browser_core/agent_tabs_close_rule.h"

#include <cmath>

namespace browser_core::agent_tabs_close_rule {

double Choice(double seconds) {
  if (!std::isfinite(seconds)) {
    return kDefault;
  }
  double nearest = kChoices[0];
  for (double choice : kChoices) {
    if (std::abs(choice - seconds) < std::abs(nearest - seconds)) {
      nearest = choice;
    }
  }
  return nearest;
}

std::optional<base::TimeDelta> After(double choice) {
  if (choice <= 0) {
    return std::nullopt;
  }
  return base::Seconds(choice);
}

void Clock::Acted(base::Time now) {
  last_ = now;
}

void Clock::SetWorking(bool working, base::Time now) {
  if (working_ && !working) {
    last_ = now;
  }
  // A thread the app names at launch, idle, counts from then.
  if (!working && !last_) {
    last_ = now;
  }
  working_ = working;
}

std::optional<base::Time> Clock::DueAt(std::optional<base::TimeDelta> after) const {
  if (working_ || !after || !last_) {
    return std::nullopt;
  }
  return *last_ + *after;
}

bool Clock::Due(base::Time now, std::optional<base::TimeDelta> after) const {
  const std::optional<base::Time> at = DueAt(after);
  return at && *at <= now;
}

bool Own(const Tab& tab) {
  return !tab.handed && !tab.held && !tab.pinned;
}

bool ClosesNow(const Tab& tab) {
  return Own(tab) && !tab.shown;
}

}  // namespace browser_core::agent_tabs_close_rule
