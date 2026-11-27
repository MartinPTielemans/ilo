// Which of a site's cookies cross between Chromium's store and the system's engine's, when, and how
// long a page waits for them, apart so browser_core_unittests tests them alone (#4356).
#ifndef BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_
#define BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_

#include <vector>

#include "base/functional/callback.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
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

// An agent's page's navigation to a site the profile keeps for the system's engine, as the throttle
// finds it.
struct Arrival {
  // Started by the browser, as the agent's open and navigate and the user's give are, not by a
  // page's link, form or script, which the app's fence judges only once the page is there (#3724).
  bool browser_initiated = false;
  // Back or forward in its history, or a restore, which the fence judges only once there too.
  bool history = false;
  // Its first address is at the site it arrives at, not at another site that redirected it there.
  bool started_at_site = false;
  // The tab's page was at the site already, which had the session when it came.
  bool at_site = false;
};

// Whether |arrival| takes the user's session from the system's store: only an arrival the app let
// through before it loads, so no session goes to a page at a site the user put off limits to
// agents, and once a visit.
bool CopiesSession(const Arrival& arrival);

// The wait of a navigation deferred for a copy between the stores, which a store that never
// answers can't make endless: it goes on once, when the copy ends or after kWait, whichever is
// first. Its timer runs only while the navigation waits.
class StoreWait {
 public:
  static constexpr base::TimeDelta kWait = base::Seconds(3);

  StoreWait();
  StoreWait(const StoreWait&) = delete;
  StoreWait& operator=(const StoreWait&) = delete;
  ~StoreWait();

  // |go_on| runs once, with whether the store answered in time.
  void Start(base::OnceCallback<void(bool answered)> go_on);
  // The copy ended. Nothing once the wait is over.
  void Answered();

 private:
  void GoOn(bool answered);

  base::OneShotTimer timer_;
  base::OnceCallback<void(bool)> go_on_;
};

}  // namespace browser_core::system_engine

#endif  // BROWSER_CORE_SYSTEM_ENGINE_COOKIES_H_
