// The Space's colours a Boost reads with var() (docs/ux-spec.md › Boosts), apart so
// browser_core_unittests tests the sheet alone.

#ifndef BROWSER_CORE_BOOST_COLORS_H_
#define BROWSER_CORE_BOOST_COLORS_H_

#include <string>
#include <string_view>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// The sheet a Boost's page takes: |css|, after |colors| as ilo's custom properties at the root
// when there are any and |css| names one of them, so the user's CSS reads them with var() and can
// set its own. The page's own scripts can read a property on the root too, so a Boost that uses
// none tells its site nothing of the Space or of ilo. Only the colours' values go in, as #rrggbb,
// so nothing else a caller gives reaches the page.
std::string BoostSheet(const bc_boost_colors* colors, std::string_view css);

// A Space changed, or a window showed another: each Boosted tab whose Space's colours aren't the
// ones it took takes them again. In boosts.cc; posted after the change.
void BoostSpacesChanged();
// A Space went, and its colours with it. In boosts.cc.
void BoostSpaceGone(bc_id space);

}  // namespace browser_core

#endif  // BROWSER_CORE_BOOST_COLORS_H_
