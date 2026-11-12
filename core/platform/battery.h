#ifndef BROWSER_CORE_PLATFORM_BATTERY_H_
#define BROWSER_CORE_PLATFORM_BATTERY_H_

#include <memory>

#include "base/power_monitor/battery_level_provider.h"
#include "base/power_monitor/sampling_event_source.h"

namespace browser_core::platform {

// The battery as the OS tells of its changes, for Chrome's battery sampler, which Battery Saver
// reads: a source that samples on the OS's own notices and the provider it samples. Both null
// where Chromium already waits for the OS's notices (the Mac's IOKit), and its own sampler stands.
// Chromium has none on Linux, and on Windows reads the battery each minute, which wakes an idle
// browser (#1771, #1772).
struct Battery {
  std::unique_ptr<base::SamplingEventSource> source;
  std::unique_ptr<base::BatteryLevelProvider> provider;
};
Battery OsBattery();

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_BATTERY_H_
