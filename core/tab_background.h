#ifndef BROWSER_CORE_TAB_BACKGROUND_H_
#define BROWSER_CORE_TAB_BACKGROUND_H_

#include <cstdint>
#include <optional>

#include "base/time/time.h"

namespace browser_core {

// The background colour of a tab's page, as the engine reports it, kept for the UI, which draws
// the open tab's line in it: what's kept, and when the UI hears of a change. Apart from content so
// browser_core_unittests tests the rule alone.
class TabBackground {
 public:
  // How long a page's background must have stayed as it is before the UI hears of a change that
  // came while it was changing. A page that animates its background reports a colour per frame it
  // draws; the UI gets the first and, this long after the last, the one it settled on, so the
  // sidebar is drawn twice for a fade and not once per frame. Longer than any gap between an
  // animation's frames, and short enough that the line follows a page's change of theme with it.
  static constexpr base::TimeDelta kSettle = base::Milliseconds(250);

  // What the tab does next.
  struct Step {
    // Tell the UI the colour kept.
    bool tell = false;
    // Call Settled once this long has passed. Asked for only while no wait is under way.
    std::optional<base::TimeDelta> wait;
  };

  // The page's background is |argb| now, or it reports none. |shown|: a window shows the tab.
  Step Changed(std::optional<uint32_t> argb, bool shown, base::TimeTicks now);
  // Whether the pane shows something of the core's own in the page's place: the card of a page
  // that died, or a page of the system's engine. For that long the colour kept is none, a change
  // as any other, and the page's own is held for when the pane shows the page again.
  Step Covered(bool covered, bool shown, base::TimeTicks now);
  // The wait a step asked for is over. One for a tab no window shows any more ends there, told
  // nothing: the next window to show it reads color().
  Step Settled(bool shown, base::TimeTicks now);
  // The tab went to another page: a change still waiting is told now, as the new page's colour,
  // if it's the same, never comes as a change. Then no wait is under way.
  Step LeftPage(bool shown);

  // The colour kept, 0xRRGGBB, or none: nothing reported, or a colour that isn't opaque.
  std::optional<uint32_t> color() const { return color_; }
  // Whether a wait is under way.
  bool waiting() const { return waiting_; }

 private:
  // Keeps the page's colour, or none while it's covered, and says what a change asks for.
  Step Keep(bool shown, base::TimeTicks now);

  // The page's own colour, as it last reported it.
  std::optional<uint32_t> page_;
  bool covered_ = false;
  std::optional<uint32_t> color_;
  // When the colour last changed.
  base::TimeTicks changed_at_;
  bool waiting_ = false;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_TAB_BACKGROUND_H_
