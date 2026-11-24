#include "browser_core/agent_look_rule.h"

namespace browser_core {

LookAnswer AnswerOfLook(bool shows, LookedDocument where) {
  if (!shows) {
    return {};
  }
  return {.withhold = true, .keep_from_cache = where == LookedDocument::kCached};
}

CutLook CutOfLook(LookCut cut) {
  if (cut == LookCut::kLate) {
    return {.call = LookEnd::kUnchecked, .kept = true};
  }
  return {.call = LookEnd::kWithheld, .kept = cut == LookCut::kSessionEnded};
}

bool LooksAgain(size_t kept) {
  return kept < kMostKeptLooks;
}

}  // namespace browser_core
