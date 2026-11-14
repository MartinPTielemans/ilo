#include "browser_core/ad_block.h"

#include <algorithm>
#include <cstdio>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

#include "base/base_paths.h"
#include "base/command_line.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/json/json_reader.h"
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
#include "base/strings/string_util.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/values.h"
#include "browser_core/ad_block_off_sites.h"
#include "browser_core/extensions.h"
#include "browser_core/hooks.h"
#include "browser_core/native_ad_block.h"
#include "browser_core/native_ad_block_lists.h"
#include "browser_core/session.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/policy/chrome_browser_policy_connector.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/policy/core/common/configuration_policy_provider.h"
#include "components/policy/core/common/policy_bundle.h"
#include "components/policy/core/common/policy_map.h"
#include "components/policy/core/common/policy_namespace.h"
#include "components/policy/core/common/policy_service.h"
#include "components/policy/core/common/policy_types.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/sessions/content/session_tab_helper.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/reload_type.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "extensions/browser/api/web_request/web_request_api_helpers.h"
#include "extensions/browser/disable_reason.h"
#include "extensions/browser/event_listener_map.h"
#include "extensions/browser/event_router.h"
#include "extensions/browser/extension_action.h"
#include "extensions/browser/extension_action_manager.h"
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
#include "net/base/isolation_info.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"
#include "url/origin.h"
#include "url/url_constants.h"

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
// The browser's own ad blocker keeps the same times.
constexpr char kUpdatePeriodHours[] = "168";
constexpr base::TimeDelta kUpdatePeriod = base::Hours(168);
constexpr base::TimeDelta kFirstUpdate = base::Seconds(37);
// uBlock Origin's "emergency" update, for when a list is twice as old as it stays current.
constexpr base::TimeDelta kFirstUpdateOfStaleLists = base::Seconds(2);

// How long a list stays current where assets.json doesn't say, as uBlock Origin's default.
constexpr base::TimeDelta kUpdateAfter = base::Days(5);

// Where uBlock Origin's entry of the profile's extension prefs kept the patterns of the sites the
// user turned it off on, before the profile's own prefs did (ad_block_off_sites.h). Read once, to
// carry them over, and left for a release as the way back.
constexpr char kOldOffSitesPref[] = "browser_core_off_sites";

// Filters, one a line, for its "My filters" until the user sets theirs: check.sh's, which block
// and hide what its page loads.
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

// The groups of its filter lists, in the order Settings shows them, as its assets.json names them.
constexpr const char* kGroups[] = {"default",      "ads",        "privacy", "malware",
                                   "multipurpose", "annoyances", "regions"};

// Where |group| is in kGroups, or -1.
int GroupRank(const std::string& group) {
  for (size_t i = 0; i < std::size(kGroups); ++i) {
    if (group == kGroups[i]) {
      return static_cast<int>(i);
    }
  }
  return -1;
}

// A filter list as uBlock Origin comes with it: on unless its assets.json says it starts off, as
// the annoyance and regional lists do. uBlock Origin also turns on a new profile's regional list
// for its language, which this turns off again, as the user turns it on in Settings.
struct Listed {
  std::string key;
  std::string group;
  std::string title;
  bool on = false;
  // The copy the browser's own ad blocker reads (native_ad_block.h): the one it downloaded last,
  // else the one in the folder uBlock Origin comes in, if it comes with one, else none.
  base::FilePath file;
  // Where the browser's own downloads it from, and how long a copy stays current.
  std::vector<GURL> addresses;
  base::TimeDelta update_after;
};

// The lists, by group and title, once ReadLists has read them.
std::vector<Listed>& Lists() {
  static base::NoDestructor<std::vector<Listed>> lists;
  return *lists;
}

// The first of |entry|'s addresses that's a file in |folder|, uBlock Origin's, where it has one:
// assets.json names its copy beside the lists' authors' addresses. Off the UI thread.
base::FilePath LocalCopy(const base::FilePath& folder, const base::DictValue& entry) {
  std::vector<std::string> addresses;
  if (const std::string* one = entry.FindString("contentURL")) {
    addresses.push_back(*one);
  } else if (const base::ListValue* several = entry.FindList("contentURL")) {
    for (const base::Value& address : *several) {
      if (address.is_string()) {
        addresses.push_back(address.GetString());
      }
    }
  }
  for (const std::string& address : addresses) {
    if (address.find("://") != std::string::npos) {
      continue;
    }
    const base::FilePath file = folder.AppendASCII(address);
    if (base::PathExists(file)) {
      return file;
    }
  }
  return base::FilePath();
}

// |entry|'s web addresses, in the order uBlock Origin updates a list from: its mirrors, then the
// list's authors'.
std::vector<GURL> Addresses(const base::DictValue& entry) {
  std::vector<GURL> addresses;
  for (const char* field : {"cdnURLs", "contentURL"}) {
    std::vector<std::string> strings;
    if (const std::string* one = entry.FindString(field)) {
      strings.push_back(*one);
    } else if (const base::ListValue* several = entry.FindList(field)) {
      for (const base::Value& address : *several) {
        if (address.is_string()) {
          strings.push_back(address.GetString());
        }
      }
    }
    for (const std::string& address : strings) {
      if (const GURL url(address); url.SchemeIs(url::kHttpsScheme)) {
        addresses.push_back(url);
      }
    }
  }
  return addresses;
}

// Reads the lists from the assets.json at |path|, the one uBlock Origin comes with. Off the UI
// thread. Lists its source adds later, as it updates assets.json, aren't Settings' until a browser
// update brings them.
std::vector<Listed> ReadLists(const base::FilePath& path) {
  std::vector<Listed> lists;
  std::string json;
  if (!base::ReadFileToString(path, &json)) {
    LOG(ERROR) << "uBlock Origin's lists couldn't be read from " << path;
    return lists;
  }
  std::optional<base::DictValue> assets = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
  if (!assets) {
    return lists;
  }
  for (const auto [key, value] : *assets) {
    const base::DictValue* entry = value.GetIfDict();
    const std::string* content = entry ? entry->FindString("content") : nullptr;
    const std::string* group = entry ? entry->FindString("group") : nullptr;
    const std::string* title = entry ? entry->FindString("title") : nullptr;
    if (content && *content == "filters" && group && GroupRank(*group) >= 0 && title) {
      base::FilePath file = DownloadedAdBlockList(key);
      if (!base::PathExists(file)) {
        file = LocalCopy(path.DirName().DirName(), *entry);
      }
      const std::optional<double> days = entry->FindDouble("updateAfter");
      lists.push_back({key, *group, *title, !entry->FindBool("off").value_or(false),
                       std::move(file), Addresses(*entry),
                       days && *days > 0 ? base::Days(*days) : kUpdateAfter});
    }
  }
  std::ranges::sort(lists, {}, [](const Listed& list) {
    return std::tuple(GroupRank(list.group), base::ToLowerASCII(list.title));
  });
  return lists;
}

// What an administrator's policy for it overwrites at each launch, its toOverwrite, if any: the
// core's own policy sets none.
const base::DictValue* AdminOverwrite() {
  if (!g_browser_process || !g_browser_process->browser_policy_connector()) {
    return nullptr;
  }
  const base::Value* overwrite =
      g_browser_process->browser_policy_connector()
          ->GetPolicyService()
          ->GetPolicies(policy::PolicyNamespace(policy::POLICY_DOMAIN_EXTENSIONS, kId))
          .GetValue("toOverwrite", base::Value::Type::DICT);
  return overwrite ? overwrite->GetIfDict() : nullptr;
}

// The lists an administrator's policy chose, if it did, which uBlock Origin takes in place of the
// user's.
const base::ListValue* AdminLists() {
  const base::DictValue* overwrite = AdminOverwrite();
  const base::ListValue* lists = overwrite ? overwrite->FindList("filterLists") : nullptr;
  return lists && !lists->empty() ? lists : nullptr;
}

// The filters an administrator's policy set, if it did, even none, in place of the user's.
const base::ListValue* AdminFilters() {
  const base::DictValue* overwrite = AdminOverwrite();
  return overwrite ? overwrite->FindList("filters") : nullptr;
}

// Whether |list| is on as the user left it.
bool UserOn(const Listed& list) {
  const std::map<std::string, bool>& chosen = UserSettings().ad_block_lists;
  const auto it = chosen.find(list.key);
  return it == chosen.end() ? list.on : it->second;
}

// The user's own filters: as they set them, else check.sh's, else nullopt, which leaves uBlock
// Origin's as they are, as they were before Settings had them.
std::optional<std::string> UserFilters() {
  if (UserSettings().ad_block_filters) {
    return UserSettings().ad_block_filters;
  }
  const base::CommandLine& command_line = *base::CommandLine::ForCurrentProcess();
  if (!command_line.HasSwitch(kFiltersSwitch)) {
    return std::nullopt;
  }
  return base::JoinString(base::SplitString(command_line.GetSwitchValueASCII(kFiltersSwitch), "\n",
                                            base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY),
                          "\n");
}

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
  // Settings › Ad blocking's, which the bridge app/fetch-ublock.sh adds to its background page
  // applies as it starts and whenever they change: the lists to turn on and off, My filters among
  // them, and the user's own filters. It leaves what an administrator's toOverwrite sets.
  base::DictValue ilo;
  if (!Lists().empty()) {
    base::ListValue on;
    base::ListValue off;
    on.Append("user-filters");
    for (const Listed& list : Lists()) {
      (UserOn(list) ? on : off).Append(list.key);
    }
    ilo.Set("on", std::move(on));
    ilo.Set("off", std::move(off));
  }
  if (std::optional<std::string> filters = UserFilters()) {
    ilo.Set("filters", *filters);
  }
  set("ilo", base::Value(std::move(ilo)));
  policy::PolicyBundle bundle;
  bundle.Get(policy::PolicyNamespace(policy::POLICY_DOMAIN_EXTENSIONS, kId)) = std::move(map);
  return bundle;
}

// Its settings for every profile, which change as the user's do.
class Policy : public policy::ConfigurationPolicyProvider {
 public:
  Policy() { UpdatePolicy(Settings()); }

  // Gives it the settings as they are now. The policy service tells it only if they changed.
  void Update() { UpdatePolicy(Settings()); }

  // policy::ConfigurationPolicyProvider:
  void RefreshPolicies(policy::PolicyFetchReason reason) override { Update(); }
};

Policy* ThePolicy() {
  static base::NoDestructor<Policy> policy;
  return policy.get();
}

policy::ConfigurationPolicyProvider* PolicyProvider() {
  return ThePolicy();
}

// Gives the ad blocker the lists and filters as they are now: uBlock Origin as its policy, which
// the policy service tells it of only if they changed, or else the browser's own, once the lists
// are read.
void Apply() {
  ThePolicy()->Update();
  base::FilePath resources;
  if (!NativeAdBlock() || Lists().empty() ||
      !base::PathService::Get(base::DIR_ASSETS, &resources)) {
    return;
  }
  std::vector<base::FilePath> files;
  for (const AdBlockList& list : AdBlockLists()) {
    const auto listed = std::ranges::find(Lists(), list.key, &Listed::key);
    if (list.on && listed != Lists().end() && !listed->file.empty()) {
      files.push_back(listed->file);
    }
  }
  UseNativeAdBlock(resources.Append(FILE_PATH_LITERAL("ublock")), files, AdBlockFilters());
}

// The lists turned on that can be downloaded.
std::vector<NativeAdBlockList> ListsToKeepCurrent() {
  std::vector<NativeAdBlockList> lists;
  for (const AdBlockList& list : AdBlockLists()) {
    const auto listed = std::ranges::find(Lists(), list.key, &Listed::key);
    if (list.on && listed != Lists().end() && !listed->addresses.empty()) {
      lists.push_back({listed->key, listed->addresses, listed->update_after});
    }
  }
  return lists;
}

// Downloads the lists turned on that are past their expiry, or that the browser has no copy of,
// for the browser's own ad blocker, and gives it those it downloaded. One update at a time.
void UpdateLists() {
  static bool updating = false;
  if (!NativeAdBlock() || !UserSettings().ad_blocking || updating) {
    return;
  }
  updating = true;
  UpdateNativeAdBlockLists(ListsToKeepCurrent(), base::BindOnce([](std::vector<std::string> keys) {
                             updating = false;
                             for (const std::string& key : keys) {
                               const auto listed = std::ranges::find(Lists(), key, &Listed::key);
                               if (listed != Lists().end()) {
                                 listed->file = DownloadedAdBlockList(key);
                               }
                             }
                             if (!keys.empty()) {
                               Apply();
                             }
                           }));
}

// Updates the lists 37 s after launch, or 2 s if one is long out of date, as on a first launch
// with the lists the browser came with, and weekly after, as uBlock Origin does its own.
void KeepListsCurrent() {
  static base::NoDestructor<base::RepeatingTimer> weekly;
  weekly->Start(FROM_HERE, kUpdatePeriod, base::BindRepeating(&UpdateLists));
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_VISIBLE},
      base::BindOnce(&AnyNativeAdBlockListLongOutOfDate, ListsToKeepCurrent()),
      base::BindOnce([](bool long_out_of_date) {
        static base::NoDestructor<base::OneShotTimer> first;
        first->Start(FROM_HERE, long_out_of_date ? kFirstUpdateOfStaleLists : kFirstUpdate,
                     base::BindOnce(&UpdateLists));
      }));
}

// Reads the lists once, as the first profile loads, and gives them to it.
void ReadListsOnce() {
  static bool read = false;
  base::FilePath resources;
  if (std::exchange(read, true) || !base::PathService::Get(base::DIR_ASSETS, &resources)) {
    return;
  }
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_VISIBLE},
      base::BindOnce(&ReadLists, resources.Append(FILE_PATH_LITERAL("ublock"))
                                     .Append(FILE_PATH_LITERAL("assets"))
                                     .Append(FILE_PATH_LITERAL("assets.json"))),
      base::BindOnce([](std::vector<Listed> lists) {
        Lists() = std::move(lists);
        Apply();
        if (NativeAdBlock()) {
          KeepListsCurrent();
        }
      }));
}

// The pattern that turns it off on |url|'s site: its registrable domain with every subdomain, or
// the host alone where it has none, as an address or localhost.
std::string PatternFor(const GURL& url) {
  const std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  return domain.empty() ? "*://" + std::string(url.host()) + "/*" : "*://*." + domain + "/*";
}

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreAdBlock") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    RegisterAdBlockOffSitesPrefs(registry);
  }
};

// The prefs of |profile|, which keep the sites the user turned it off on: uBlock Origin's, carried
// over as they're first read.
PrefService* OffSitePrefs(Profile* profile) {
  PrefService* prefs = profile->GetOriginalProfile()->GetPrefs();
  if (!prefs->GetBoolean(kAdBlockOffSitesCarriedPref)) {
    const extensions::ExtensionPrefs* old =
        extensions::ExtensionPrefs::Get(profile->GetOriginalProfile());
    CarryOverAdBlockOffSites(prefs, old ? old->ReadPrefAsList(kId, kOldOffSitesPref) : nullptr);
  }
  return prefs;
}

// The URL patterns of the sites the user turned it off on, in the order they did.
const base::ListValue& OffSiteList(Profile* profile) {
  return OffSitePrefs(profile)->GetList(kAdBlockOffSitesPref);
}

// The patterns of the sites the user turned it off on, in the order they did.
std::vector<URLPattern> OffSitePatterns(Profile* profile) {
  std::vector<URLPattern> patterns;
  for (const base::Value& site : OffSiteList(profile)) {
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
  OffSitePrefs(profile)->SetList(kAdBlockOffSitesPref, std::move(sites));
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
  // uBlock Origin, unless the browser's own blocks, which reads the lists uBlock Origin comes with.
  // A safe start holds it by not loading it, so no page waits for it.
  const bool uses_ublock = !NativeAdBlock();
  const bool held = uses_ublock && SafeStart();
  if (uses_ublock && !held) {
    Followers()[profile] = std::make_unique<Follower>(profile);
    Load(profile);
  }
  ReadListsOnce();
  // Here, as the profile's saved extensions have loaded and none has run yet.
  HoldExtensionsForSafeStart(profile, held ? 1 : 0);
}

}  // namespace

void RegisterAdBlockPrefs() {
  PrefsFactory::GetInstance();
}

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
  // The browser's own has nothing to hold: a request waits for it itself, for the moment after
  // launch it takes to load. It gives a web page what it hides and runs where it blocks, on the
  // site of the page the frame is in, as ProxyForAdBlock's requests.
  if (NativeAdBlock()) {
    if (!UserSettings().ad_blocking || !url.SchemeIsHTTPOrHTTPS()) {
      return;
    }
    content::RenderFrameHost* parent = navigation.GetParentFrameOrOuterDocument();
    // A tab's page asks at each address it's redirected to, as a site turned off may be one.
    if (!parent) {
      AddNativeAdBlockPage(registry);
      return;
    }
    const GURL page = parent->GetOutermostMainFrame()->GetLastCommittedURL();
    Profile* profile =
        Profile::FromBrowserContext(navigation.GetWebContents()->GetBrowserContext());
    if (page.SchemeIsHTTPOrHTTPS() && !OffSites(profile->GetOriginalProfile()).MatchesURL(page)) {
      AddNativeAdBlockPage(registry);
    }
    return;
  }
  content::BrowserContext* context = navigation.GetWebContents()->GetBrowserContext();
  Profile* profile = Profile::FromBrowserContext(context);
  // Only where it filters and will get ready: http(s), in a profile it's loaded and on in, and not
  // on a site the user turned it off on, whose requests it never sees. Off the record, its page
  // starts only in the profile's private windows, the original's primary off-the-record profile,
  // never in one made apart, as the sync sign-in window's (#2619).
  if (!url.SchemeIsHTTPOrHTTPS() || !Followers().contains(profile->GetOriginalProfile()) ||
      !extensions::ExtensionRegistry::Get(profile)->enabled_extensions().Contains(kId) ||
      (profile->IsOffTheRecord() && (!profile->IsPrimaryOTRProfile() ||
                                     !extensions::util::IsIncognitoEnabled(kId, profile))) ||
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
  // The settings read, its lists and filters are the user's.
  Apply();
  const bool on = UserSettings().ad_blocking;
  if (NativeAdBlock()) {
    SetNativeAdBlockOn(on);
    return;
  }
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

void ProxyForAdBlock(content::BrowserContext* context,
                     content::RenderFrameHost* frame,
                     content::ContentBrowserClient::URLLoaderFactoryType type,
                     const url::Origin& request_initiator,
                     const net::IsolationInfo& isolation_info,
                     ukm::SourceIdObj ukm_source_id,
                     network::URLLoaderFactoryBuilder& factory_builder) {
  using Type = content::ContentBrowserClient::URLLoaderFactoryType;
  if (!NativeAdBlock() || !UserSettings().ad_blocking || type == Type::kDownload ||
      type == Type::kDevTools) {
    return;
  }
  // The page the requests are for, whose site decides: a frame's navigation is its page's, and a
  // page's own goes ahead (native_ad_block.cc).
  GURL page;
  if (type == Type::kNavigation) {
    if (!frame || !frame->GetParentOrOuterDocument()) {
      return;
    }
    page = frame->GetOutermostMainFrame()->GetLastCommittedURL();
  } else if (isolation_info.top_frame_origin()) {
    page = isolation_info.top_frame_origin()->GetURL();
  } else {
    page = request_initiator.GetURL();
  }
  // Only a web page's, as uBlock Origin sees: not the browser's own pages' nor an extension's.
  if (!page.SchemeIsHTTPOrHTTPS() ||
      OffSites(Profile::FromBrowserContext(context)->GetOriginalProfile()).MatchesURL(page)) {
    return;
  }
  ProxyForNativeAdBlock(factory_builder, frame, ukm_source_id.ToInt64());
}

bool AdBlockingOffAt(content::BrowserContext* context, const GURL& url) {
  return OffSites(Profile::FromBrowserContext(context)->GetOriginalProfile()).MatchesURL(url);
}

std::optional<bool> AdBlockingOnSite(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const GURL& url = contents->GetLastCommittedURL();
  // A private window's sites aren't kept, and its windows follow the profile's.
  const bool on =
      NativeAdBlock()
          ? UserSettings().ad_blocking
          : extensions::ExtensionRegistry::Get(profile)->enabled_extensions().Contains(kId);
  if (profile->IsOffTheRecord() || !url.SchemeIsHTTPOrHTTPS() || !on) {
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
  base::ListValue sites;
  for (const base::Value& site : OffSiteList(profile)) {
    URLPattern pattern(kSchemes);
    // Turning it back on lets go of every pattern that covers the page.
    if (on && site.is_string() &&
        pattern.Parse(site.GetString()) == URLPattern::ParseResult::kSuccess &&
        pattern.MatchesURL(url)) {
      continue;
    }
    sites.Append(site.Clone());
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
  base::ListValue sites;
  bool found = false;
  for (const base::Value& kept : OffSiteList(profile)) {
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

std::vector<AdBlockList> AdBlockLists() {
  const base::ListValue* admin = AdminLists();
  std::vector<AdBlockList> lists;
  for (const Listed& list : Lists()) {
    lists.push_back({.key = list.key,
                     .group = list.group,
                     .title = list.title,
                     .on = admin ? admin->contains(list.key) : UserOn(list)});
  }
  return lists;
}

bool AdBlockListsLocked() {
  return AdminLists() != nullptr;
}

void SetAdBlockList(const std::string& key, bool on) {
  const auto list = std::ranges::find(Lists(), key, &Listed::key);
  if (list == Lists().end() || AdminLists()) {
    return;
  }
  // Only what differs from how it comes, so a list a new version turns on by default follows it.
  if (on == list->on) {
    UserSettings().ad_block_lists.erase(key);
  } else {
    UserSettings().ad_block_lists[key] = on;
  }
  SessionChanged();
  Apply();
  // One the browser has no copy of blocks once it's downloaded.
  if (on) {
    UpdateLists();
  }
}

std::string AdBlockFilters() {
  if (const base::ListValue* admin = AdminFilters()) {
    std::vector<std::string> lines;
    for (const base::Value& line : *admin) {
      if (line.is_string()) {
        lines.push_back(line.GetString());
      }
    }
    return base::JoinString(lines, "\n");
  }
  return UserFilters().value_or(std::string());
}

bool AdBlockFiltersLocked() {
  return AdminFilters() != nullptr;
}

void SetAdBlockFilters(const std::string& filters) {
  if (AdminFilters()) {
    return;
  }
  UserSettings().ad_block_filters = std::string(base::TrimWhitespaceASCII(filters, base::TRIM_ALL));
  SessionChanged();
  Apply();
}

std::string AdBlockedOn(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const extensions::Extension* extension =
      extensions::ExtensionRegistry::Get(profile)->enabled_extensions().GetByID(kId);
  extensions::ExtensionAction* action =
      extension ? extensions::ExtensionActionManager::Get(profile)->GetExtensionAction(*extension)
                : nullptr;
  if (!action) {
    return std::string();
  }
  // Its badge is "!" while it hasn't seen the page's requests, and empty at none.
  std::string count =
      action->GetDisplayBadgeText(sessions::SessionTabHelper::IdForTab(contents).id());
  return std::ranges::any_of(count, base::IsAsciiDigit<char>) ? count : std::string();
}

}  // namespace browser_core
