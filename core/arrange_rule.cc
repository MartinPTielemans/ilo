#include "browser_core/arrange_rule.h"

namespace browser_core::arrange_rule {

bool MayStart(std::string_view site, bool off_the_record, bool in_agent_space) {
  return !site.empty() && !off_the_record && !in_agent_space;
}

bool GiveToAgent(base::FunctionRef<void()> end_arrange, base::FunctionRef<bool()> move) {
  end_arrange();
  return move();
}

}  // namespace browser_core::arrange_rule
