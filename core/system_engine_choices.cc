#include "browser_core/system_engine_choices.h"

#include <algorithm>
#include <set>
#include <utility>

#include "base/containers/fixed_flat_set.h"
#include "base/values.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"
#include "components/prefs/scoped_user_pref_update.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"

namespace browser_core::system_engine {

namespace {

// Add an entry only for a service whose main content is protected video or audio, as its own
// host, or as its registrable domain where nothing else is at that domain.
constexpr auto kListed = base::MakeFixedFlatSet<std::string_view>({
    "disneyplus.com",
    "hbomax.com",
    "hulu.com",
    "max.com",
    "music.apple.com",
    "netflix.com",
    "open.spotify.com",
    "paramountplus.com",
    "peacocktv.com",
    "primevideo.com",
    "tv.apple.com",
});

}  // namespace

void RegisterChoicePrefs(PrefRegistrySimple* registry) {
  registry->RegisterListPref(kSitesPref);
  registry->RegisterListPref(kClosedNotesPref);
  registry->RegisterListPref(kListedOffPref);
}

std::vector<std::string_view> ListedServices() {
  return {kListed.begin(), kListed.end()};
}

std::string SiteOf(const GURL& url) {
  const std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  // An IP address or a bare host name is a site of its own, as the core clears sites.
  return domain.empty() ? std::string(url.host()) : domain;
}

Choices::Choices(PrefService& prefs, std::string_view test_host)
    : prefs_(prefs), test_host_(test_host) {}

// A lookup for each of the host's few labels.
std::string_view Choices::ListedEntry(const GURL& url) const {
  std::string_view host = url.host();
  if (!test_host_.empty() && host == test_host_) {
    return test_host_;
  }
  while (!host.empty()) {
    if (auto entry = kListed.find(host); entry != kListed.end()) {
      return *entry;
    }
    const size_t dot = host.find('.');
    host = dot == std::string_view::npos ? std::string_view() : host.substr(dot + 1);
  }
  return std::string_view();
}

bool Choices::Remembered(const GURL& url) const {
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return false;
  }
  // A listed service goes by its entry alone, not by a site the user kept that holds it.
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    return !Holds(kListedOffPref, entry);
  }
  return Holds(kSitesPref, SiteOf(url));
}

void Choices::Remember(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  // Kept for the system's engine, the site no longer counts as kept in Chromium.
  if (NoteClosed(url)) {
    ScopedListPrefUpdate(&*prefs_, kClosedNotesPref)->EraseValue(base::Value(SiteOf(url)));
  }
  // A listed service is never kept: turning it on takes back the user's off.
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    ScopedListPrefUpdate(&*prefs_, kListedOffPref)->EraseValue(base::Value(entry));
    return;
  }
  if (!Holds(kSitesPref, SiteOf(url))) {
    ScopedListPrefUpdate(&*prefs_, kSitesPref)->Append(SiteOf(url));
  }
}

void Choices::Forget(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  // A listed service's off is its entry's alone, and leaves a site the user kept that holds it.
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    if (!Holds(kListedOffPref, entry)) {
      ScopedListPrefUpdate(&*prefs_, kListedOffPref)->Append(std::string(entry));
    }
    return;
  }
  ScopedListPrefUpdate(&*prefs_, kSitesPref)->EraseValue(base::Value(SiteOf(url)));
}

// A listed service the user turned off counts by its entry alone, so its off closes no note for
// the rest of its site, where another listed service can be: Apple Music's leaves Apple TV's.
bool Choices::NoteClosed(const GURL& url) const {
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return false;
  }
  const std::string_view entry = ListedEntry(url);
  return (!entry.empty() && Holds(kListedOffPref, entry)) || Holds(kClosedNotesPref, SiteOf(url));
}

void Choices::CloseNote(const GURL& url) {
  if (url.SchemeIsHTTPOrHTTPS() && !NoteClosed(url)) {
    ScopedListPrefUpdate(&*prefs_, kClosedNotesPref)->Append(SiteOf(url));
  }
}

bool Choices::KeptInChromium(const GURL& url) const {
  return NoteClosed(url);
}

std::vector<std::string> Choices::Sites() const {
  return Strings(kSitesPref);
}

std::vector<std::string> Choices::KeptSites() const {
  std::vector<std::string> kept = Strings(kListedOffPref);
  // A service's off from before it closed no note also closed its site's, which is the same
  // choice where the site is its entry. Any other closed note shows, the way back from it.
  const std::set<std::string> entries(kept.begin(), kept.end());
  for (std::string& site : Strings(kClosedNotesPref)) {
    if (!entries.contains(site)) {
      kept.push_back(std::move(site));
    }
  }
  return kept;
}

bool Choices::Offered(std::string_view site) const {
  return Holds(kSitesPref, site) || Holds(kListedOffPref, site) || Holds(kClosedNotesPref, site);
}

void Choices::ForgetChoices(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  const std::string site = SiteOf(url);
  ScopedListPrefUpdate(&*prefs_, kSitesPref)->EraseValue(base::Value(site));
  ScopedListPrefUpdate(&*prefs_, kClosedNotesPref)->EraseValue(base::Value(site));
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    ScopedListPrefUpdate(&*prefs_, kListedOffPref)->EraseValue(base::Value(entry));
  }
}

void Choices::Clear() {
  for (const char* pref : {kSitesPref, kClosedNotesPref, kListedOffPref}) {
    prefs_->ClearPref(pref);
  }
}

bool Choices::Holds(const char* pref, std::string_view value) const {
  const base::ListValue& list = prefs_->GetList(pref);
  return !list.empty() && std::ranges::contains(list, base::Value(value));
}

std::vector<std::string> Choices::Strings(const char* pref) const {
  std::vector<std::string> all;
  for (const base::Value& each : prefs_->GetList(pref)) {
    if (each.is_string()) {
      all.push_back(each.GetString());
    }
  }
  return all;
}

}  // namespace browser_core::system_engine
