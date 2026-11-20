#ifndef BROWSER_CORE_PROTECTED_MOVE_RULE_H_
#define BROWSER_CORE_PROTECTED_MOVE_RULE_H_

#include <cstdint>
#include <optional>

#include "base/time/time.h"

namespace browser_core {

// What a tab does when a page Chromium shows was refused the protected media it asked for, which
// only the system's engine plays (system_engine.h, #3593). Apart from content so
// browser_core_unittests tests it alone.

// How long after the user's press a page's ask counts as theirs: Chromium's transient activation,
// which a press gives a page for as long.
inline constexpr base::TimeDelta kPressLifespan = base::Seconds(5);

struct ProtectedAsk {
  // When the ask came, and the last input that reached the tab's page from the user, and from an
  // agent, which comes through DevTools: null if none has.
  base::TimeTicks now;
  base::TimeTicks users_input;
  base::TimeTicks agents_input;
  // The tab may show its page in the system's engine: one of the user's, with the engine on.
  bool allowed = false;
  // A link preview's tab, whose card draws Chromium's page alone.
  bool preview = false;
  // The profile keeps the site in Chromium: the user turned it off or closed its note.
  bool kept_in_chromium = false;
  // The page asking, or a frame of the page's own site. Another site's frame, as an ad's, never
  // moves the page it's in.
  bool own_site = false;
  // What the move, a reload in another engine, would lose: text the user put in the page, a page
  // that asks before it's left, or a capture of the camera, microphone or screen.
  bool typed = false;
  bool asks_before_leaving = false;
  bool capturing = false;
};

enum class ProtectedAnswer {
  // The protected-media note, as for a page that asks as it loads.
  kNote,
  // The tab moves to the system's engine, and the note says so.
  kMove,
  // It stays, and the note asks first: it would lose what was typed, or the capture.
  kAskTyped,
  kAskCapturing,
};

// The last of the user's presses and of an agent's input that reached a tab's page, each with the
// document it reached, known by its page's UKM source id, which a main-frame document change gives
// anew. A press is its document's alone, as Chromium's activation is: the page a tab goes to next
// never counts one made on the page before, as ⌘L's on the page an address is typed over (#3593).
class DocumentPresses {
 public:
  // The user's press, a button, finger or key going down, at |at| on |document|'s page. A key's
  // |key| is its event's time stamp, which tells it when the page leaves it (TakenBack).
  void Users(int64_t document, base::TimeTicks at, std::optional<base::TimeTicks> key);
  void Agents(int64_t document, base::TimeTicks at);
  // The page left the key stamped |key| and the browser ran it, as ⌘L: pressed for the browser, so
  // it gives the page nothing, and the user's press before it stands. A key the browser keeps
  // before the page never reaches it, so never comes here.
  void TakenBack(base::TimeTicks key);
  // The ask's inputs for a page showing |document|: none made on another.
  void Fill(int64_t document, ProtectedAsk& ask) const;

 private:
  struct Press {
    int64_t document = 0;
    base::TimeTicks at;
  };
  Press users_;
  Press agents_;
  // While the user's last press is a key the page may still leave: its stamp and the press before.
  std::optional<base::TimeTicks> key_;
  Press before_key_;
};

// Moves only on the user's own press: their input inside kPressLifespan of the ask, and newer than
// any agent's, so an agent's press followed by a late ask never moves a tab the user took over.
ProtectedAnswer AnswerProtectedAsk(const ProtectedAsk& ask);

}  // namespace browser_core

#endif  // BROWSER_CORE_PROTECTED_MOVE_RULE_H_
