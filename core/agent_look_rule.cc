#include "browser_core/agent_look_rule.h"

namespace browser_core {

LookAnswer AnswerOfLook(bool shows, LookedDocument where) {
  if (!shows) {
    return {};
  }
  return {.withhold = true, .keep_from_cache = where == LookedDocument::kCached};
}

}  // namespace browser_core
