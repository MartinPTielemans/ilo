#ifndef BROWSER_CORE_AGENT_LOOK_RULE_H_
#define BROWSER_CORE_AGENT_LOOK_RULE_H_

#include <cstddef>

namespace browser_core {

// Where the document a look asked (AgentPage::Freeze) is when its answer comes, which may be after
// the user took the tab or took it elsewhere (#4098).
enum class LookedDocument {
  // Still the tab's page.
  kShown,
  // Kept in the back-forward cache, from where it would come back as it was.
  kCached,
  // Gone, as the tab went to another document.
  kLeft,
  // Gone with its renderer, the tab showing no live page.
  kRendererGone,
};

struct LookAnswer {
  // The tab's page is withheld (WithholdShownPage).
  bool withhold = false;
  // The document asked never comes back from the cache.
  bool keep_from_cache = false;
};

// What a look's answer does: a yes withholds the page the tab shows, wherever the document asked
// went, as an answer can't be told from the next document's, and keeps that document from the
// cache too. No answer clears a withhold. Apart from content so browser_core_unittests tests the
// rule alone.
LookAnswer AnswerOfLook(bool shows, LookedDocument where);

// What the call waiting on a look hears.
enum class LookEnd {
  // Nothing of the page is withheld, and the call goes on.
  kClear,
  // The page is withheld (PageWithheld).
  kWithheld,
  // The look didn't answer in time: the call does nothing and gets nothing, and the page isn't
  // withheld for it (BC_AGENT_FAILURE_UNCHECKED).
  kUnchecked,
};

// Why a look's call is answered before the look is.
enum class LookCut {
  // The reply limit, or a dialog the renderer waits on.
  kLate,
  // The agent's session with the tab ended, as the user took it over.
  kSessionEnded,
  // DevTools lets the page go, as the tab closes.
  kGone,
};

struct CutLook {
  // What the call hears.
  LookEnd call = LookEnd::kWithheld;
  // The look is kept for its own answer, which AnswerOfLook then decides on.
  bool kept = false;
};

// A cut look withholds nothing by itself. The renderer closes what it looked for as it answers, so
// while its session can still carry that answer the look is kept for it, and a late one is heard:
// a call cut for time hears it wasn't checked, and may be made again. A session's end answers
// its calls as a withheld page's, and a page let go has no answer left to hear.
CutLook CutOfLook(LookCut cut);

// The most looks a page keeps for their own answers. A session runs its commands in order, so a
// new look waits behind every kept one: a renderer that never answers would otherwise keep one more
// for each call an agent makes, until the tab closes or its renderer goes.
inline constexpr size_t kMostKeptLooks = 4;

// Whether a call on a page keeping |kept| looks looks too. One that doesn't is answered at once as
// a look cut for time is, and sends nothing to the page.
bool LooksAgain(size_t kept);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_LOOK_RULE_H_
