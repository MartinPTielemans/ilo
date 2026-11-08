#include "browser_core/platform/battery.h"

namespace browser_core::platform {

// The Mac's sampler already reads the battery on IOKit's power-source notices.
Battery OsBattery() {
  return {};
}

}  // namespace browser_core::platform
