#include "browser_core/platform/system_page.h"

#include <utility>

#include "base/functional/callback.h"

namespace browser_core::platform {

// No system engine off the Mac: Windows plays protected video with Chromium's own PlayReady
// (decision 1).

bool SystemPage::Available() {
  return false;
}

std::unique_ptr<SystemPage> SystemPage::Create(const SystemStore&, const GURL&, Delegate*) {
  return nullptr;
}

void SystemPage::DeleteStore(const std::string&) {}

void SystemPage::ReleaseStore(const std::string&) {}

void SystemPage::ClearSite(const SystemStore&,
                           const std::string&,
                           SiteData,
                           base::OnceClosure done) {
  std::move(done).Run();
}

void SystemPage::SetCookies(const SystemStore&,
                            const std::vector<net::CanonicalCookie>&,
                            base::OnceCallback<void(size_t)> done) {
  std::move(done).Run(0);
}

void SystemPage::GetCookies(const SystemStore&,
                            const std::string&,
                            base::OnceCallback<void(std::vector<net::CanonicalCookie>)> done) {
  std::move(done).Run({});
}

}  // namespace browser_core::platform
