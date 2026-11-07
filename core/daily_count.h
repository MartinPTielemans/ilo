#ifndef BROWSER_CORE_DAILY_COUNT_H_
#define BROWSER_CORE_DAILY_COUNT_H_

#include <string_view>

#include "base/functional/callback.h"

namespace browser_core::daily_count {

// The daily usage count (bc_send_daily_count): one anonymous count a day, posted to the endpoint
// the app names with --daily-count-url, which only a build naming a server passes. The app keeps
// the switch and the last count's date and decides when; the core only builds and posts the count,
// so nothing but its fields can reach the endpoint, and never anything to another address.

// Posts the count, the OS, its major version and the chip added, and gives `sent` whether the
// endpoint took it, always after returning. One at a time.
void Send(std::string_view version,
          std::string_view channel,
          bool first,
          bool weekly,
          bool monthly,
          base::OnceCallback<void(bool)> sent);

}  // namespace browser_core::daily_count

#endif  // BROWSER_CORE_DAILY_COUNT_H_
