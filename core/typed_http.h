// Whether a load the user typed keeps its http:// (#2417), apart so browser_core_unittests tests
// the rule alone.

#ifndef BROWSER_CORE_TYPED_HTTP_H_
#define BROWSER_CORE_TYPED_HTTP_H_

#include <string_view>

class GURL;

namespace browser_core {

// Whether |url| is the http address written in |typed|, scheme and all. Chrome's address bar loads
// such an address as written, past HTTPS-Upgrades and HTTPS-First, but only for what was typed
// itself: an http:// that's only the start of a suggestion's address isn't that suggestion's
// scheme.
bool TypedHttp(std::string_view typed, const GURL& url);

}  // namespace browser_core

#endif  // BROWSER_CORE_TYPED_HTTP_H_
