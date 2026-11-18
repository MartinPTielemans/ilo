#include "browser_core/agent_page_rule.h"

#include "url/gurl.h"

namespace browser_core {
namespace {

bool IsBoard(const GURL& url) {
  return url.SchemeIs("chrome") && url.host() == "board";
}

bool OffTheWeb(const GURL& url) {
  return !url.is_empty() && !url.IsAboutBlank() && !url.SchemeIsHTTPOrHTTPS();
}

}  // namespace

const char* AgentPageRefusal(const GURL& page, const GURL& visible, const GURL& pinned) {
  if (IsBoard(page) || IsBoard(visible) || IsBoard(pinned)) {
    return kNotAgentsTab;
  }
  // As the address field shows it, so a load away from such a page waits for the page it leads
  // to. A tab yet to load is at the page it's pinned to.
  const GURL& shown = !visible.is_empty() ? visible : !page.is_empty() ? page : pinned;
  if (OffTheWeb(shown)) {
    return kNotWebPage;
  }
  return nullptr;
}

}  // namespace browser_core
