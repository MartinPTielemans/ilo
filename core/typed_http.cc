#include "browser_core/typed_http.h"

#include "base/strings/string_util.h"
#include "url/gurl.h"
#include "url/url_constants.h"

namespace browser_core {

bool TypedHttp(std::string_view typed, const GURL& url) {
  // As Chrome's bar: the input had http's scheme, and the match is what was typed.
  const GURL written(base::TrimWhitespaceASCII(typed, base::TRIM_ALL));
  return written.is_valid() && written.SchemeIs(url::kHttpScheme) && written == url;
}

}  // namespace browser_core
