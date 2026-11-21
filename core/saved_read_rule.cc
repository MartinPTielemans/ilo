#include "browser_core/saved_read_rule.h"

#include <algorithm>

#include "url/gurl.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

std::string SavedReadRefusal(const GURL& url,
                             bool off_the_record,
                             size_t max_bytes,
                             int64_t timeout_ms) {
  if (off_the_record) {
    return "A private window's session isn't read.";
  }
  if (!url.is_valid() || !url.SchemeIsHTTPOrHTTPS()) {
    return "Only an http or https address is read.";
  }
  if (url.has_username() || url.has_password()) {
    return "An address with a user name or password in it isn't read.";
  }
  if (url.has_ref()) {
    return "An address with a fragment (#) isn't read: the site never sees it.";
  }
  if (max_bytes == 0 || timeout_ms <= 0) {
    return "A read needs a size and a time to wait.";
  }
  return std::string();
}

size_t SavedReadBytes(size_t max_bytes) {
  return std::min(max_bytes, kSavedReadMaxBytes);
}

int64_t SavedReadMilliseconds(int64_t timeout_ms) {
  return std::min(timeout_ms, kSavedReadMaxMilliseconds);
}

bool SavedReadFollows(const GURL& asked, const GURL& to) {
  return to.is_valid() && to.SchemeIsHTTPOrHTTPS() &&
         url::Origin::Create(asked).IsSameOriginWith(url::Origin::Create(to));
}

}  // namespace browser_core
