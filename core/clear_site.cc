#include "browser_core/clear_site.h"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

#include "base/barrier_closure.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/scoped_observation.h"
#include "base/time/time.h"
#include "browser_core/platform/system_page.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "chrome/browser/profiles/profile.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/browsing_data_filter_builder.h"
#include "content/public/browser/browsing_data_remover.h"
#include "content/public/browser/web_contents.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Loads the tab's page again, in whichever engine shows it, if the tab's still open.
void Reload(bc_id tab, bool bypass_cache) {
  if (content::WebContents* contents = Tab::FromId(tab)) {
    Tab::FromWebContents(contents)->Reload(bypass_cache);
  }
}

// Runs |done| once the remover's task is done, then goes.
class WhenCleared : public content::BrowsingDataRemover::Observer {
 public:
  WhenCleared(content::BrowsingDataRemover* remover, base::OnceClosure done)
      : done_(std::move(done)) {
    // The remover tells only the observers it knows when their task is done.
    observation_.Observe(remover);
  }

  void OnBrowsingDataRemoverDone(uint64_t failed_data_types) override {
    std::move(done_).Run();
    delete this;
  }

 private:
  base::OnceClosure done_;
  base::ScopedObservation<content::BrowsingDataRemover, content::BrowsingDataRemover::Observer>
      observation_{this};
};

}  // namespace

void ClearSiteAndReload(bc_id tab, platform::SiteData what) {
  // A tab without its page has nothing to reload, and loads afresh once shown.
  const Tab* clearing = Tab::Get(tab);
  if (!clearing || !clearing->profile()) {
    return;
  }
  const GURL url = clearing->Url();
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  // A site's cookies can be any of its hosts', so the filter takes the site's registrable domain,
  // as the Clear-Site-Data header does. An IP address or a bare host name is a site of its own.
  std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  if (domain.empty()) {
    domain = url.host();
  }
  auto filter = content::BrowsingDataFilterBuilder::Create(
      content::BrowsingDataFilterBuilder::Mode::kDelete);
  filter->AddRegisterableDomain(domain);
  Profile* profile = clearing->profile();
  content::BrowsingDataRemover* remover = profile->GetBrowsingDataRemover();
  using Remover = content::BrowsingDataRemover;
  uint64_t types = 0;
  switch (what) {
    case platform::SiteData::kCookies:
      types = Remover::DATA_TYPE_COOKIES;
      break;
    case platform::SiteData::kCache:
      types = Remover::DATA_TYPE_CACHE;
      break;
    case platform::SiteData::kAll:
      types = Remover::DATA_TYPE_COOKIES | Remover::DATA_TYPE_DOM_STORAGE |
              Remover::DATA_TYPE_CACHE;
      break;
  }
  // The system's engine keeps the site's data apart, once the profile has used it, and the page
  // loads again once both have let it go. A reload after the cache's clearing also fetches again
  // what the page loads from other sites.
  const bool system_store = system_engine::StoreUsed(profile);
  const base::RepeatingClosure done = base::BarrierClosure(
      system_store ? 2 : 1, base::BindOnce(&Reload, tab, what != platform::SiteData::kCookies));
  remover->RemoveWithFilterAndReply(base::Time(), base::Time::Max(), types,
                                    Remover::ORIGIN_TYPE_UNPROTECTED_WEB, std::move(filter),
                                    new WhenCleared(remover, done));
  if (system_store) {
    platform::SystemPage::ClearSite(system_engine::StoreOf(profile), domain, what, done);
  }
}

}  // namespace browser_core
