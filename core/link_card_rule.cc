#include "browser_core/link_card_rule.h"

#include <algorithm>

#include "base/strings/string_util.h"
#include "net/base/net_errors.h"
#include "url/gurl.h"
#include "url/url_constants.h"

namespace browser_core {

bool LinkCardAsks(const GURL& url) {
  return url.is_valid() && url.SchemeIsHTTPOrHTTPS() && !url.host().empty();
}

bool LinkCardFollows(const GURL& to, int hop) {
  return hop <= kLinkCardMaxRedirects && LinkCardAsks(to);
}

bool LinkCardDirect(int net_error, bool found, bool direct) {
  return net_error == net::OK && found && direct;
}

bool LinkCardPage(std::string_view mime) {
  return base::EqualsCaseInsensitiveASCII(mime, "text/html") ||
         base::EqualsCaseInsensitiveASCII(mime, "application/xhtml+xml");
}

bool LinkCardPicture(std::string_view mime) {
  for (std::string_view kind : {"image/png", "image/jpeg", "image/gif", "image/webp"}) {
    if (base::EqualsCaseInsensitiveASCII(mime, kind)) {
      return true;
    }
  }
  return false;
}

size_t LinkCardKeep(std::string_view read) {
  constexpr std::string_view kEnd = "</head>";
  const std::string_view within = read.substr(0, std::min(read.size(), kLinkCardPageCap));
  const auto end = std::search(within.begin(), within.end(), kEnd.begin(), kEnd.end(),
                               [](char a, char b) { return base::ToLowerASCII(a) == b; });
  return end == within.end() ? within.size() : (end - within.begin()) + kEnd.size();
}

}  // namespace browser_core
