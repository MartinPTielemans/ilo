// browser_core_unittests' own main doesn't start Mojo, as most suites don't need it. Each suite
// that answers through a loader factory starts it here, once a process, since the test launcher
// runs several suites in one.

#ifndef BROWSER_CORE_MOJO_ONCE_FOR_TESTING_H_
#define BROWSER_CORE_MOJO_ONCE_FOR_TESTING_H_

#include "mojo/core/embedder/embedder.h"

namespace browser_core {

inline void StartMojoOnceForTesting() {
  static const bool started = (mojo::core::Init(), true);
  (void)started;
}

}  // namespace browser_core

#endif  // BROWSER_CORE_MOJO_ONCE_FOR_TESTING_H_
