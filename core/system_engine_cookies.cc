#include "browser_core/system_engine_cookies.h"

#include <algorithm>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"

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

bool CopiesSession(const Arrival& arrival) {
  return arrival.browser_initiated && !arrival.history && arrival.started_at_site &&
         !arrival.at_site;
}

StoreWait::StoreWait() = default;
StoreWait::~StoreWait() = default;

void StoreWait::Start(base::OnceCallback<void(bool answered)> go_on) {
  go_on_ = std::move(go_on);
  timer_.Start(FROM_HERE, kWait, base::BindOnce(&StoreWait::GoOn, base::Unretained(this), false));
}

void StoreWait::Answered() {
  GoOn(true);
}

void StoreWait::GoOn(bool answered) {
  if (!go_on_) {
    return;
  }
  timer_.Stop();
  // Last: the navigation going on can end it, and this with it.
  std::move(go_on_).Run(answered);
}

}  // namespace browser_core::system_engine
