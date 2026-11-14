#ifndef BROWSER_CORE_LINK_TARGET_H_
#define BROWSER_CORE_LINK_TARGET_H_

#include <cstddef>
#include <string>

class GURL;

namespace browser_core {

// A link's address as the app shows it, and which of it is the site.
struct LinkTarget {
  // UTF-8. https:// is left out, since it's the default, and a name written in another script shows
  // in it only if that can't pass for another site's (Chrome's rule for the address bar).
  std::string text;
  // The bytes of |text| that are the host's registrable domain, such as "example.co.uk" in
  // "login.example.co.uk": a look-alike differs there. The whole host if it has no registry, such
  // as an IP address or "localhost". Empty (both 0) for an address with no host, such as mailto:.
  size_t domain_begin = 0;
  size_t domain_end = 0;
};

// |url| as the app shows a link the pointer is on.
LinkTarget FormatLinkTarget(const GURL& url);

// The site of the page at |url|, ASCII: the registrable domain the plate emphasizes, or the host if
// it has none. Empty if it isn't http(s).
std::string SiteOf(const GURL& url);

}  // namespace browser_core

#endif  // BROWSER_CORE_LINK_TARGET_H_
