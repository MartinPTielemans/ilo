#include "browser_core/tile.h"

#include <cstdio>

#include "base/no_destructor.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/tab.h"
#include "browser_core/tile_rule.h"
#include "browser_core/window.h"
#include "content/public/browser/web_contents.h"

namespace browser_core::tiles {

namespace {

TileRule& Rule() {
  static base::NoDestructor<TileRule> rule;
  return *rule;
}

}  // namespace

bool Show(bc_id tab, void* view, const gfx::Rect& frame, double corner_radius) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return false;
  }
  const TileRule::Shown shown = Rule().Show(tab, view, IsAgentTab(contents),
                                            Window::Showing(tab), platform::CanTile());
  // Not for each frame the app moves it by.
  if (shown != TileRule::Shown::kFramed) {
    fprintf(stderr, "browser: tile %llu %s\n", static_cast<unsigned long long>(tab),
            TileRule::Name(shown));
  }
  switch (shown) {
    case TileRule::Shown::kNew:
    case TileRule::Shown::kFramed:
    case TileRule::Shown::kMoved:
      platform::ShowTile(contents, view, frame, corner_radius);
      return true;
    default:
      return false;
  }
}

void End(bc_id tab) {
  if (!Rule().End(tab)) {
    return;
  }
  fprintf(stderr, "browser: tile %llu ended\n", static_cast<unsigned long long>(tab));
  if (content::WebContents* contents = Tab::FromId(tab)) {
    platform::EndTile(contents);
  }
}

bool Holds(bc_id tab) {
  return Rule().Holds(tab);
}

}  // namespace browser_core::tiles
