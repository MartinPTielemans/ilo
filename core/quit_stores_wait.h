#ifndef BROWSER_CORE_QUIT_STORES_WAIT_H_
#define BROWSER_CORE_QUIT_STORES_WAIT_H_

#include <optional>
#include <string>

#include "base/functional/callback.h"
#include "base/time/time.h"

namespace browser_core {

// How the system's engine answered a quit's ask to write its stores to the disk (#3685).
enum class StoresWritten {
  // The session made no store that keeps anything, so there was nothing to ask.
  kNone,
  kWritten,
  // The engine has no way to be asked.
  kNoWay,
};

// A quit waits this long at most for the system's engine to write its stores.
inline constexpr base::TimeDelta kStoresWriteLimit = base::Seconds(1);

// Asks |write| to write the stores, then runs |done| once with the line the quit logs: at once
// with none for kNone, else when |write| answers or kStoresWriteLimit passes, whichever is first.
// Apart from the engine so browser_core_unittests tests the rule alone.
void WaitForStores(base::OnceCallback<void(base::OnceCallback<void(StoresWritten)>)> write,
                   base::OnceCallback<void(std::optional<std::string> line)> done);

}  // namespace browser_core

#endif  // BROWSER_CORE_QUIT_STORES_WAIT_H_
