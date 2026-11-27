// This computer's view of a regular window, which the session file keeps with the window and sync
// never carries (#4415), apart so browser_core_unittests tests its reading alone.

#ifndef BROWSER_CORE_WINDOW_VIEW_H_
#define BROWSER_CORE_WINDOW_VIEW_H_

#include <utility>
#include <vector>

#include "base/values.h"

namespace browser_core {

// A tab's place in the session: a Space's place and the tab's index there, counting its pinned
// tabs then its Today tabs as SavedSpace::shown does, or -1 and the favorite's among the session's.
using TabPlace = std::pair<int, int>;

struct WindowView {
  // Whether its docked sidebar was hidden.
  bool sidebar_hidden = false;
  // Whether it stayed on top of other apps' windows.
  bool on_top = false;
  // Its tabs, most recently shown first, as the app's ⌃Tab walks them.
  std::vector<TabPlace> recent;
  // How far down each Space's list it was scrolled, by the Space's place, in points, past 0.
  std::vector<std::pair<int, double>> scroll;

  bool operator==(const WindowView&) const = default;
};

// Writes |view| into the window's |dict|, leaving out what's as a new window has it.
void WriteWindowView(const WindowView& view, base::DictValue& dict);

// The view the window's |dict| holds, each place moved to where its tab is among those read:
// |favorites| and each Space's of |kept| give, by a place as saved, the tab's index among those
// read, or -1 for one that wasn't. A place that names nothing read is left out, and so is a scroll
// of a Space past |last_space|. A window saved before views were kept reads as a new one.
WindowView ReadWindowView(const base::DictValue& dict,
                          const std::vector<int>& favorites,
                          const std::vector<std::vector<int>>& kept,
                          int last_space);

}  // namespace browser_core

#endif  // BROWSER_CORE_WINDOW_VIEW_H_
