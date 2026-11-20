#include "browser_core/tab_follow_rule.h"

namespace browser_core::tab_follow {

bool MayLoad(const GURL& url) {
  return url.is_valid() && url.SchemeIsHTTPOrHTTPS();
}

}  // namespace browser_core::tab_follow
