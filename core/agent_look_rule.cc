#include "browser_core/agent_look_rule.h"

namespace browser_core {

LookAnswer AnswerOfLook(bool shows, LookedDocument where) {
  if (!shows) {
    return LookAnswer::kNothing;
  }
  switch (where) {
    case LookedDocument::kShown:
    case LookedDocument::kRendererGone:
      return LookAnswer::kWithhold;
    case LookedDocument::kCached:
      return LookAnswer::kKeepFromCache;
    case LookedDocument::kLeft:
      return LookAnswer::kNothing;
  }
  return LookAnswer::kWithhold;
}

}  // namespace browser_core
