#ifndef BROWSER_CORE_AGENT_CLOSE_RULE_H_
#define BROWSER_CORE_AGENT_CLOSE_RULE_H_

#include <optional>

namespace browser_core {

// An agent's close of a tab whose page has a beforeunload handler: the core runs the handler as a
// user's close does, without its dialog, and answers the agent once with what came of it, whatever
// happens first (#3256). Apart from content so browser_core_unittests tests it alone.
class AgentCloseAsk {
 public:
  enum class Answer {
    // The tab is gone, by the close or by any other.
    kClosed,
    // Its page asked to stay, as one with unsaved changes does.
    kAsks,
    // It stays for another reason: it went to another page, or its page was unloaded.
    kStays,
  };

  // What the caller does next: close the tab without its handlers, whose going then answers, or
  // answer, after which nothing more.
  struct Step {
    bool close = false;
    std::optional<Answer> answer;
  };

  // The page's beforeunload answered: |proceed| false if it asked to stay. One that didn't is
  // closed by Chromium after its unload handlers, and answered as it goes.
  Step PageAnswered(bool proceed);
  // Another page committed in the tab before its page answered.
  Step Navigated();
  // Its renderer ended, so nothing on the page can ask: it closes, as a user's close of a crashed
  // tab does, once Closing() says so.
  Step RendererGone();
  // No answer in the time the core gives one: it closes, as Chromium closes a page whose handler
  // hangs, once Closing() says so.
  Step TimedOut();
  // The core is about to close it, after RendererGone() or TimedOut(): a tab that is by then the
  // user's, taken over or pinned, only the user closes, unless its page had agreed, as Chromium is
  // then closing it already.
  Step Closing(bool users);
  // The core asked for the close and the tab is still there, as when it was between windows.
  Step StillThere();
  // Its page went: with the tab, or, if |tab_stays|, unloaded while the tab stays.
  Step PageGone(bool tab_stays);

 private:
  Step Answering(Answer answer);

  bool agreed_ = false;
  bool answered_ = false;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_CLOSE_RULE_H_
