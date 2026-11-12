#include "browser_core/ad_block.h"

#include <cstdio>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "base/base_paths.h"
#include "base/command_line.h"
#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/json/json_writer.h"
#include "base/location.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/path_service.h"
#include "base/scoped_observation.h"
#include "base/strings/string_split.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/values.h"
#include "browser_core/hooks.h"
#include "browser_core/session.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "components/policy/core/common/configuration_policy_provider.h"
#include "components/policy/core/common/policy_bundle.h"
#include "components/policy/core/common/policy_map.h"
#include "components/policy/core/common/policy_namespace.h"
#include "components/policy/core/common/policy_types.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/reload_type.h"
#include "content/public/browser/web_contents.h"
#include "extensions/browser/api/web_request/web_request_api_helpers.h"
#include "extensions/browser/disable_reason.h"
#include "extensions/browser/event_listener_map.h"
#include "extensions/browser/event_router.h"
#include "extensions/browser/extension_prefs.h"
#include "extensions/browser/extension_registrar.h"
#include "extensions/browser/extension_registry.h"
#include "extensions/browser/extension_registry_observer.h"
#include "extensions/browser/extension_util.h"
#include "extensions/browser/permissions/permissions_updater.h"
#include "extensions/browser/unpacked_installer.h"
#include "extensions/common/extension.h"
#include "extensions/common/permissions/permissions_data.h"
#include "extensions/common/url_pattern.h"
#include "extensions/common/url_pattern_set.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// uBlock Origin's id, from the key app/ublock.ini adds to its manifest.
constexpr char kId[] = "oghkpboejbohceocapipmnjifoklonij";

// Where it reads its filter lists' addresses from: "" for the assets.json it comes with, which
// names uBlock Origin's hosts and the lists' authors'. One of the browser's default service URLs,
// which a fork may point at its own.
constexpr char kFilterListsSource[] = "";

// How often, in hours, it updates its lists while the browser stays open: weekly, so an idle
// browser wakes for them once a week, as a launch updates the lists past their expiry 37 s in.
constexpr char kUpdatePeriodHours[] = "168";

// The patterns of the sites the user turned it off on, in its entry of the profile's extension
// prefs.
constexpr char kOffSitesPref[] = "browser_core_off_sites";

// Filters, one a line, for its "My filters" in place of the user's: check.sh's, which block and
// hide what its page loads.
constexpr char kFiltersSwitch[] = "ad-block-filters";

// An assets.json in place of kFilterListsSource: check.sh's, on its own server, naming a list it
// serves, so a run sees uBlock Origin fetch, apply and update a list without the network.
constexpr char kListsSwitch[] = "ad-block-lists";

constexpr int kSchemes = URLPattern::SCHEME_HTTP | URLPattern::SCHEME_HTTPS;

// What it listens to once it's ready, with its lists loaded and its request listener set: an event
// only ChromeOS's kiosk apps are sent, so the listener does nothing else (app/fetch-ublock.sh).
constexpr char kReadyEvent[] = "runtime.onRestartRequired";

// The longest a page waits for it, so one that never gets ready doesn't keep pages from loading.
constexpr base::TimeDelta kLongestHold = base::Seconds(10);

// Its settings, as uBlock Origin reads them from managed storage (its managed_storage.json).
policy::PolicyBundle Settings() {
  policy::PolicyMap map;
  const auto set = [&map](const char* name, base::Value value) {
    // Defaults, which the platform's policy for it outranks, so an admin's still wins.
    map.Set(name, policy::POLICY_LEVEL_MANDATORY, policy::POLICY_SCOPE_MACHINE,
            policy::POLICY_SOURCE_ENTERPRISE_DEFAULT, std::move(value), nullptr);
  };
  set("advancedSettings",
      base::Value(base::ListValue().Append(
          base::ListValue().Append("autoUpdatePeriod").Append(kUpdatePeriodHours))));
  const base::CommandLine& command_line = *base::CommandLine::ForCurrentProcess();
  const std::string lists = command_line.HasSwitch(kListsSwitch)
                                ? command_line.GetSwitchValueASCII(kListsSwitch)
                                : kFilterListsSource;
  // It ignores an empty address.
  set("adminSettings",
      base::Value(base::WriteJson(base::DictValue().Set("assetsBootstrapLocation", lists))
                      .value_or(std::string())));
  if (command_line.HasSwitch(kFiltersSwitch)) {
    base::ListValue filters;
    for (const std::string& filter :
         base::SplitString(command_line.GetSwitchValueASCII(kFiltersSwitch), "\n",
                           base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY)) {
      filters.Append(filter);
    }
    set("toOverwrite", base::Value(base::DictValue().Set("filters", std::move(filters))));
  }
  policy::PolicyBundle bundle;
  bundle.Get(policy::PolicyNamespace(policy::POLICY_DOMAIN_EXTENSIONS, kId)) = std::move(map);
  return bundle;
}

// Its settings for every profile, which never change while the browser runs.
class Policy : public policy::ConfigurationPolicyProvider {
 public:
  Policy() { UpdatePolicy(Settings()); }

  // policy::ConfigurationPolicyProvider:
  void RefreshPolicies(policy::PolicyFetchReason reason) override { UpdatePolicy(Settings()); }
};

policy::ConfigurationPolicyProvider* PolicyProvider() {
  static base::NoDestructor<Policy> policy;
  return policy.get();
}

// The pattern that turns it off on |url|'s site: its registrable domain with every subdomain, or
// the host alone where it has none, as an address or localhost.
std::string PatternFor(const GURL& url) {
  const std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  return domain.empty() ? "*://" + std::string(url.host()) + "/*" : "*://*." + domain + "/*";
}

// The patterns of the sites the user turned it off on, in the order they did.
std::vector<URLPattern> OffSitePatterns(Profile* profile) {
  std::vector<URLPattern> patterns;
  const base::ListValue* list =
      extensions::ExtensionPrefs::Get(profile)->ReadPrefAsList(kId, kOffSitesPref);
  if (!list) {
    return patterns;
  }
  for (const base::Value& site : *list) {
    URLPattern pattern(kSchemes);
    if (site.is_string() && pattern.Parse(site.GetString()) == URLPattern::ParseResult::kSuccess) {
      patterns.push_back(std::move(pattern));
    }
  }
  return patterns;
}

extensions::URLPatternSet OffSites(Profile* profile) {
  extensions::URLPatternSet sites;
  for (const URLPattern& pattern : OffSitePatterns(profile)) {
    sites.AddPattern(pattern);
  }
  return sites;
}

// Keeps it off the sites the user turned it off on: Chrome then hides their requests and pages
// from it, so it neither blocks nor hides anything there.
void ApplyOffSites(Profile* profile, const extensions::Extension& extension) {
  extensions::URLPatternSet sites = OffSites(profile);
  extensions::PermissionsUpdater updater(profile);
  if (sites.is_empty()) {
    if (!extension.permissions_data()->UsesDefaultPolicyHostRestrictions()) {
      updater.SetUsesDefaultHostRestrictions(&extension);
    }
    return;
  }
  // An extension's own restrictions replace the ones the platform's policy sets for all of them,
  // which still apply.
  const int context = extensions::util::GetBrowserContextId(profile);
  sites.AddPatterns(extensions::PermissionsData::GetDefaultPolicyBlockedHosts(context));
  updater.SetPolicyHostRestrictions(
      &extension, sites, extensions::PermissionsData::GetDefaultPolicyAllowedHosts(context));
}

// Keeps |sites|, URL patterns, as the sites it's off on in |profile|, and applies them.
void KeepOffSites(Profile* profile, base::ListValue sites) {
  extensions::ExtensionPrefs::Get(profile)->UpdateExtensionPref(kId, kOffSitesPref,
                                                                base::Value(std::move(sites)));
  // Turned off in Settings, it takes them up as it loads again.
  if (const extensions::Extension* extension =
          extensions::ExtensionRegistry::Get(profile)->enabled_extensions().GetByID(kId)) {
    ApplyOffSites(profile, *extension);
  }
  // What a page takes from its renderer's memory cache never reaches uBlock Origin, so a page
  // loaded again would run the ad script it loaded while blocking was off. Chrome empties those
  // caches as an extension adds a request listener, which turning blocking on in Settings does, but
  // not as a site's host restrictions change.
  extension_web_request_api_helpers::ClearCacheOnNavigation();
}

// Whether it's ready to block in |context|, a profile or its private windows, each with a page of
// its own: that page has said so since it last started. Its listeners go with the page.
bool Ready(content::BrowserContext* context) {
  for (const auto& listener :
       extensions::EventRouter::Get(context)->listeners().GetEventListenersByName(kReadyEvent)) {
    if (listener->extension_id() == kId && listener->browser_context() == context) {
      return true;
    }
  }
  return false;
}

// Holds a page's request until it's ready in the page's profile. Chromium sends a request only to
// the listeners there are as it starts, and at launch its page starts with the first pages and is
// ready only seconds later, so they would load their ads (#744).
class Hold : public content::NavigationThrottle {
 public:
  using content::NavigationThrottle::NavigationThrottle;

  // Lets go of each held page whose profile it's ready in.
  static void ReleaseReady() {
    std::vector<base::WeakPtr<Hold>> holds;
    holds.swap(Held());
    for (const base::WeakPtr<Hold>& hold : holds) {
      if (!hold) {
        continue;
      }
      if (Ready(hold->Context())) {
        hold->Release();
      } else {
        Held().push_back(hold);
      }
    }
  }

  // content::NavigationThrottle:
  ThrottleCheckResult WillStartRequest() override {
    if (Ready(Context())) {
      return PROCEED;
    }
    held_ = base::TimeTicks::Now();
    Held().push_back(weak_factory_.GetWeakPtr());
    timer_.Start(FROM_HERE, kLongestHold, this, &Hold::Release);
    return DEFER;
  }
  const char* GetNameForLogging() override { return "browser_core::AdBlockHold"; }

 private:
  static std::vector<base::WeakPtr<Hold>>& Held() {
    static base::NoDestructor<std::vector<base::WeakPtr<Hold>>> held;
    return *held;
  }

  content::BrowserContext* Context() {
    return navigation_handle()->GetWebContents()->GetBrowserContext();
  }

  // This may delete this.
  void Release() {
    timer_.Stop();
    weak_factory_.InvalidateWeakPtrs();
    fprintf(stderr, "browser: ad blocking held a page %d ms, %s\n",
            static_cast<int>((base::TimeTicks::Now() - held_).InMilliseconds()),
            Ready(Context()) ? "ready" : "not ready");
    Resume();
  }

  base::TimeTicks held_;
  base::OneShotTimer timer_;
  base::WeakPtrFactory<Hold> weak_factory_{this};
};

// Loads it into |profile| from the framework's resources, as --load-extension would: synchronously,
// and kept for this launch only, so the next one loads the version the browser comes with then.
void Load(Profile* profile) {
  base::FilePath resources;
  if (!base::PathService::Get(base::DIR_ASSETS, &resources)) {
    return;
  }
  scoped_refptr<extensions::UnpackedInstaller> installer =
      extensions::UnpackedInstaller::Create(profile);
  // Chrome's dialog for a failure is Chrome's UI, and a build without it, as a spike's, just logs.
  installer->set_be_noisy_on_failure(false);
  // Its own instance in private windows ("incognito": "split"), even if the user took it away
  // there in Chrome's extensions page.
  installer->set_allow_incognito_access(true);
  std::string id;
  if (installer->LoadFromCommandLine(resources.Append(FILE_PATH_LITERAL("ublock")), &id,
                                     /*only_allow_apps=*/false) &&
      id != kId) {
    LOG(ERROR) << "uBlock Origin loaded as " << id << ", not " << kId
               << ": its manifest lacks app/ublock.ini's key";
  }
}

// Follows it in a profile: Chrome sets its permissions afresh each time it loads, a profile made
// with ad blocking off everywhere has to turn it off, it has to be loaded again when it reloads
// itself, and the pages held for it go once it's ready, there or in the profile's private windows,
// which share the profile's event router.
class Follower : public extensions::ExtensionRegistryObserver,
                 public extensions::EventRouter::Observer {
 public:
  explicit Follower(Profile* profile)
      : profile_(profile), event_router_(extensions::EventRouter::Get(profile)) {
    observation_.Observe(extensions::ExtensionRegistry::Get(profile));
    event_router_->RegisterObserver(this, kReadyEvent);
  }
  ~Follower() override { event_router_->UnregisterObserver(this); }

  // extensions::EventRouter::Observer:
  void OnListenerAdded(const extensions::EventListenerInfo& details) override {
    if (details.extension_id == kId) {
      // Not inside the router's notification, as a page let go may finish at once.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&Hold::ReleaseReady));
    }
  }

  // extensions::ExtensionRegistryObserver:
  void OnExtensionLoaded(content::BrowserContext* browser_context,
                         const extensions::Extension* extension) override {
    if (extension->id() != kId) {
      return;
    }
    ApplyOffSites(profile_, *extension);
    if (!UserSettings().ad_blocking) {
      // Not inside the registry's notification.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&ApplyAdBlocking));
    }
  }
  void OnExtensionInstalled(content::BrowserContext* browser_context,
                            const extensions::Extension* extension,
                            bool is_update) override {
    if (extension->id() != kId) {
      return;
    }
    if (extension->location() != extensions::mojom::ManifestLocation::kCommandLine) {
      // It reloads itself when the user restores or resets its settings (app/fetch-ublock.sh takes
      // out the reload as it first starts). Chrome loads it back as one from its extensions page,
      // which its policy turns off while developer mode is, so it would block nothing until the
      // next launch. Not inside the registry's notification.
      reloaded_ = true;
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&Follower::LoadAgain, weak_factory_.GetWeakPtr()));
    } else if (std::exchange(reloaded_, false)) {
      // Loaded again, it keeps the reason Chrome turned it off for, which the user's setting
      // clears.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&ApplyAdBlocking));
    }
  }
  void OnShutdown(extensions::ExtensionRegistry* registry) override;

 private:
  void LoadAgain() { Load(profile_); }

  const raw_ptr<Profile> profile_;
  const raw_ptr<extensions::EventRouter> event_router_;
  // Whether it reloaded itself and hasn't been loaded again since.
  bool reloaded_ = false;
  base::ScopedObservation<extensions::ExtensionRegistry, extensions::ExtensionRegistryObserver>
      observation_{this};
  base::WeakPtrFactory<Follower> weak_factory_{this};
};

std::map<Profile*, std::unique_ptr<Follower>>& Followers() {
  static base::NoDestructor<std::map<Profile*, std::unique_ptr<Follower>>> followers;
  return *followers;
}

// The profile is going. This deletes this.
void Follower::OnShutdown(extensions::ExtensionRegistry* registry) {
  Followers().erase(profile_);
}

// Loads it into |profile| once its saved extensions have loaded, and follows it there.
void LoadInto(Profile* profile) {
  if (profile->IsOffTheRecord()) {
    return;
  }
  Followers()[profile] = std::make_unique<Follower>(profile);
  Load(profile);
}

}  // namespace

void SetUpAdBlock() {
  g_browser_core_load_bundled_extensions = &LoadInto;
  g_browser_core_policy_provider = &PolicyProvider;
}

bool IsAdBlocker(const std::string& id) {
  return id == kId;
}

void AddAdBlockHold(content::NavigationThrottleRegistry& registry) {
  content::NavigationHandle& navigation = registry.GetNavigationHandle();
  const GURL& url = navigation.GetURL();
  content::BrowserContext* context = navigation.GetWebContents()->GetBrowserContext();
  Profile* profile = Profile::FromBrowserContext(context);
  // Only where it filters and will get ready: http(s), in a profile it's loaded and on in, and not
  // on a site the user turned it off on, whose requests it never sees.
  if (!url.SchemeIsHTTPOrHTTPS() || !Followers().contains(profile->GetOriginalProfile()) ||
      !extensions::ExtensionRegistry::Get(profile)->enabled_extensions().Contains(kId) ||
      (profile->IsOffTheRecord() && !extensions::util::IsIncognitoEnabled(kId, profile)) ||
      Ready(context) || OffSites(profile->GetOriginalProfile()).MatchesURL(url)) {
    return;
  }
  registry.AddThrottle(std::make_unique<Hold>(registry));
}

void ApplyAdBlocking() {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return;
  }
  const bool on = UserSettings().ad_blocking;
  for (Profile* profile : manager->GetLoadedProfiles()) {
    extensions::ExtensionRegistrar* registrar = extensions::ExtensionRegistrar::Get(profile);
    extensions::ExtensionRegistry* registry = extensions::ExtensionRegistry::Get(profile);
    if (!registrar || !registry || !registry->GetInstalledExtension(kId)) {
      continue;
    }
    const bool enabled = registry->enabled_extensions().Contains(kId);
    if (on && !enabled) {
      registrar->EnableExtension(kId);
    } else if (!on && enabled) {
      // As the user's switch in Chrome's extensions page, which the next launch's load clears, so
      // the setting decides again.
      registrar->DisableExtension(kId, {extensions::disable_reason::DISABLE_USER_ACTION});
    }
  }
}

std::optional<bool> AdBlockingOnSite(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const GURL& url = contents->GetLastCommittedURL();
  // A private window's sites aren't kept, and its windows follow the profile's.
  if (profile->IsOffTheRecord() || !url.SchemeIsHTTPOrHTTPS() ||
      !extensions::ExtensionRegistry::Get(profile)->enabled_extensions().Contains(kId)) {
    return std::nullopt;
  }
  return !OffSites(profile).MatchesURL(url);
}

void SetAdBlockingOnSite(content::WebContents* contents, bool on) {
  const std::optional<bool> now = AdBlockingOnSite(contents);
  if (!now || *now == on) {
    return;
  }
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const GURL url = contents->GetLastCommittedURL();
  extensions::ExtensionPrefs* prefs = extensions::ExtensionPrefs::Get(profile);
  base::ListValue sites;
  if (const base::ListValue* list = prefs->ReadPrefAsList(kId, kOffSitesPref)) {
    for (const base::Value& site : *list) {
      URLPattern pattern(kSchemes);
      // Turning it back on lets go of every pattern that covers the page.
      if (on && site.is_string() &&
          pattern.Parse(site.GetString()) == URLPattern::ParseResult::kSuccess &&
          pattern.MatchesURL(url)) {
        continue;
      }
      sites.Append(site.Clone());
    }
  }
  if (!on) {
    sites.Append(PatternFor(url));
  }
  KeepOffSites(profile, std::move(sites));
  contents->GetController().Reload(content::ReloadType::NORMAL, /*check_for_repost=*/true);
}

std::vector<std::string> AdBlockingOffSites(Profile* profile) {
  std::vector<std::string> sites;
  for (const URLPattern& pattern : OffSitePatterns(profile)) {
    sites.push_back(pattern.host());
  }
  return sites;
}

void TurnOnAdBlockingOnSite(Profile* profile, const std::string& site) {
  const base::ListValue* list =
      extensions::ExtensionPrefs::Get(profile)->ReadPrefAsList(kId, kOffSitesPref);
  if (!list) {
    return;
  }
  base::ListValue sites;
  bool found = false;
  for (const base::Value& kept : *list) {
    URLPattern pattern(kSchemes);
    if (kept.is_string() && pattern.Parse(kept.GetString()) == URLPattern::ParseResult::kSuccess &&
        pattern.host() == site) {
      found = true;
      continue;
    }
    sites.Append(kept.Clone());
  }
  if (found) {
    KeepOffSites(profile, std::move(sites));
  }
}

}  // namespace browser_core
