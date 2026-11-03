// Whether a device's own first Space gives way when sync first brings the account's Spaces to the
// device (sync_spaces.cc), as Arc's does on a new device, rather than adding an empty Space to the
// account. Apart from the Space, so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SPACE_DEFAULT_H_
#define BROWSER_CORE_SPACE_DEFAULT_H_

#include <cstddef>

namespace browser_core {

// What the user did with the device's only Space before sync brought the account's.
struct FirstSpace {
  // Given a name or icon other than those it was made with (Space::touched()).
  bool named = false;
  // Given a colour or theme, as onboarding's first step gives it.
  bool coloured = false;
  size_t today_tabs = 0;
  // Pinned tabs and folders.
  size_t pinned_rows = 0;
  size_t favorites = 0;
};

// Whether |space| goes, its Today tabs moving to the account's first Space: while the user hasn't
// made it theirs. A colour is setup, not content, and its Today tabs move with it, so neither
// keeps it; a name, an icon, a pinned tab, a folder or a favorite does.
bool GivesWay(const FirstSpace& space);

}  // namespace browser_core

#endif  // BROWSER_CORE_SPACE_DEFAULT_H_
