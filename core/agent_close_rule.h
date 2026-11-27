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

// An agent's archive of a tab the user handed it (bc_agent_archive_tab), from what the core knows
// at the call. Its page is never asked: one that agreed would be closed by Chromium in the agent's
// Space, where nothing is archived and the tab never goes back, and one that asked would need its
// dialog. So nothing shows and nothing moves but the tab, at once (#4278).
struct AgentArchive {
  // The tab is in |agent|'s Space, and the user handed it there (handed::Of).
  bool handed = false;
  // The user holds it, taken over or at a handoff.
  bool held = false;
  // It goes back pinned or onto a favorite's tile, which only the user closes.
  bool pinned = false;
  // A frame of its page has a beforeunload handler, so it may hold unsaved changes
  // (Tab::AsksBeforeLeaving).
  bool asks = false;
  // The agent said to lose what it holds (and its user's access allows it).
  bool discard = false;

  enum class Does {
    // Not its to archive: it stays as it is.
    kRefused,
    // Stays handed: only the user closes a pinned tab or a favorite.
    kPinned,
    // Stays handed, its page untouched: it may hold unsaved changes.
    kAsks,
    // Goes back to its place and closes there without its handlers, into the archive.
    kArchives,
  };
  Does What() const;

  // What an archive the rule let go ahead came to, from whether the tab went back to the place it
  // left and whether it's gone since.
  enum class Came {
    // It couldn't go back, as when the Space it left is gone: still handed, for the app to give
    // back.
    kStaysHanded,
    // It went back but stays open, as between windows: the user's again, and nothing archived.
    kReturned,
    // It's gone, into the archive.
    kArchived,
  };
  static Came After(bool returned, bool gone);
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_CLOSE_RULE_H_
