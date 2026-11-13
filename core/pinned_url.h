// Whether a pinned tab or favorite is at its pinned URL (#2361), apart so browser_core_unittests
// tests the rule alone.

#ifndef BROWSER_CORE_PINNED_URL_H_
#define BROWSER_CORE_PINNED_URL_H_

class GURL;

namespace browser_core {

// Whether a page at |url|, whose entry's load started at |requested|, is at |pinned|: at it, or
// where loading it led, through the server's redirects, such as a sign-in hop, or after the page
// replaced its own address, both of which keep the entry's original request. So a pinned URL that
// always redirects is home where it lands, and is what ⌘↩ wouldn't load again. The page it lands
// on, reached by a link or typed, made its own entry, so it's away.
bool AtPinnedUrl(const GURL& pinned, const GURL& url, const GURL& requested);

}  // namespace browser_core

#endif  // BROWSER_CORE_PINNED_URL_H_
