// Which of a site's cookies cross between Chromium's store and the system's engine's, apart so
// browser_core_unittests tests the rule alone (#4356).
#ifndef BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_
#define BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_

#include <vector>

#include "base/time/time.h"
#include "net/cookies/canonical_cookie.h"

namespace browser_core::system_engine {

// Where a cookie was read from, which says when it was last set: Chromium's store keeps that as
// the cookie's last update, while the system's store keeps only when it was made, which
// SystemPage::GetCookies gives as the cookie's creation.
enum class Store { kChromium, kSystem };

base::Time SetAt(const net::CanonicalCookie& cookie, Store store);

// Of |from|, read from |from_store|, the cookies to write over |onto|, read from the other store:
// each but those |onto| holds an equivalent of, by name, domain and path, set at or after it. So a
// copy never takes back a session the site gave the user since, as an agent's page at the site
// signed out would otherwise do to the user's sign-in in the system's engine.
std::vector<net::CanonicalCookie> Newer(std::vector<net::CanonicalCookie> from,
                                        Store from_store,
                                        const std::vector<net::CanonicalCookie>& onto);

}  // namespace browser_core::system_engine

#endif  // BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_
