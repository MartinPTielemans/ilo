#include "browser_core/system_engine_cookies.h"

#include <algorithm>
#include <utility>

namespace browser_core::system_engine {

base::Time SetAt(const net::CanonicalCookie& cookie, Store store) {
  // A cookie Chromium's store took before it kept last updates has only its creation.
  if (store == Store::kChromium && !cookie.LastUpdateDate().is_null()) {
    return cookie.LastUpdateDate();
  }
  return cookie.CreationDate();
}

std::vector<net::CanonicalCookie> Newer(std::vector<net::CanonicalCookie> from,
                                        Store from_store,
                                        const std::vector<net::CanonicalCookie>& onto) {
  const Store onto_store = from_store == Store::kChromium ? Store::kSystem : Store::kChromium;
  std::erase_if(from, [&](const net::CanonicalCookie& cookie) {
    const base::Time set = SetAt(cookie, from_store);
    return std::ranges::any_of(onto, [&](const net::CanonicalCookie& held) {
      return held.IsEquivalent(cookie) && SetAt(held, onto_store) >= set;
    });
  });
  return from;
}

}  // namespace browser_core::system_engine
