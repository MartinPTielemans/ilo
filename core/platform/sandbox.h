#ifndef BROWSER_CORE_PLATFORM_SANDBOX_H_
#define BROWSER_CORE_PLATFORM_SANDBOX_H_

namespace browser_core::platform {

// Whether Chromium's sandbox confines the browser's other processes, for Report a Problem
// (bc_sandboxed). In the browser process, once Chromium has started.
bool Sandboxed();

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_SANDBOX_H_
