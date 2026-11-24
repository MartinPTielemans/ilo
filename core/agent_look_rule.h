#ifndef BROWSER_CORE_AGENT_LOOK_RULE_H_
#define BROWSER_CORE_AGENT_LOOK_RULE_H_

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

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_LOOK_RULE_H_
