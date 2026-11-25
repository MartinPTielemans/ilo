#include "browser_core/secure_connections_rule.h"

#include <utility>

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

void Ask(Started& kept) {
  kept = {.asked = true};
}

void Keep(Started& kept, uint64_t tab, int64_t load) {
  if (kept.asked && load != 0) {
    kept.tab = tab;
    kept.load = load;
  }
}

int64_t TakeFor(Started& kept, uint64_t tab) {
  const Started had = std::exchange(kept, {});
  return had.tab == tab ? had.load : 0;
}

Asked TakenBy(Asked& kept, bool waits) {
  const Asked taken = std::exchange(kept, {});
  return waits ? taken : Asked();
}

bool AskedFor(const Asked& asked, int64_t taken) {
  return asked.load != 0 && asked.load == taken;
}

const char* NotOpened(bool opened) {
  return opened ? "The tab couldn't open." : "The tab couldn't go there.";
}

AtClose ToldAtClose(bool closed_as_not_secure, bool running, const Running& job) {
  if (!closed_as_not_secure || !running) {
    return AtClose::kTabEnded;
  }
  if (job.went) {
    return AtClose::kWent;
  }
  return job.waits && job.asked.load != 0 && job.asked.opened ? AtClose::kWentOn
                                                              : AtClose::kTabEnded;
}

}  // namespace browser_core::secure_connections_rule
