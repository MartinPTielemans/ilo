#include "browser_core/platform/sandbox.h"

#include "base/command_line.h"
#include "sandbox/policy/switches.h"

namespace browser_core::platform {

// The Mac's and Windows's sandboxes start with every process unless the browser is told not to.
bool Sandboxed() {
  return !base::CommandLine::ForCurrentProcess()->HasSwitch(sandbox::policy::switches::kNoSandbox);
}

}  // namespace browser_core::platform
