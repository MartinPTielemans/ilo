#ifndef BROWSER_CORE_MAIN_DELEGATE_H_
#define BROWSER_CORE_MAIN_DELEGATE_H_

#include <optional>

#include "browser_core/public/browser_core.h"
#include "build/build_config.h"
// Compiled into chrome_dll, which links this directory in.
#include "chrome/app/chrome_main_delegate.h"  // nogncheck

namespace browser_core {

// Runs Chromium with the app's callbacks and returns its exit code.
int Run(int argc, const char** argv, const bc_host& host);

// Asks Chrome to quit. Once its Browsers have closed, the core lets go of the browser process, which
// stays up with no windows until then.
void Quit();

// Quits as Quit does, then starts the browser again (bc_restart).
void Restart();

// Chrome's main delegate in every process of ours. chrome_main.cc makes this one instead
// (engine/patches/ours/chromium-core-main.patch).
class MainDelegate : public ChromeMainDelegate {
 public:
  explicit MainDelegate(const StartupTimestamps& timestamps);
  ~MainDelegate() override;

 protected:
  std::optional<int> BasicStartupComplete() override;
  void PreSandboxStartup() override;
#if BUILDFLAG(IS_LINUX)
  void ZygoteForked() override;
#endif
  std::optional<int> PostEarlyInitialization(InvokedIn invoked_in) override;
  content::ContentClient* CreateContentClient() override;
  content::ContentBrowserClient* CreateContentBrowserClient() override;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_MAIN_DELEGATE_H_
