#include "browser_core/pinned_url.h"

#include "url/gurl.h"

namespace browser_core {

bool AtPinnedUrl(const GURL& pinned, const GURL& url, const GURL& requested) {
  return pinned.is_valid() && (url == pinned || requested == pinned);
}

}  // namespace browser_core
