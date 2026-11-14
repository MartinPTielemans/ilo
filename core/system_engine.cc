#include "browser_core/system_engine.h"

#include <stdio.h>

#include <algorithm>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/barrier_callback.h"
#include "base/command_line.h"
#include "base/containers/fixed_flat_set.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/string_util.h"
#include "base/supports_user_data.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/uuid.h"
#include "base/values.h"
#include "browser_core/space.h"
#include "browser_core/system_stores.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/common/pref_names.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"
#include "components/prefs/scoped_user_pref_update.h"
#include "components/services/storage/public/mojom/local_storage_control.mojom.h"
#include "components/services/storage/public/mojom/storage_usage_info.mojom.h"
#include "content/public/browser/browser_thread.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/storage_partition.h"
#include "content/public/browser/web_contents.h"
#include "mojo/public/cpp/bindings/callback_helpers.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "net/cookies/canonical_cookie.h"
#include "net/cookies/cookie_access_result.h"
#include "net/cookies/cookie_options.h"
#include "services/network/public/mojom/cookie_manager.mojom.h"
#include "third_party/blink/public/common/storage_key/storage_key.h"
#include "third_party/blink/public/mojom/dom_storage/storage_area.mojom.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core::system_engine {

namespace {

// The sites a profile keeps for the system's engine. The id of its store, once it has one, is
// kSystemStorePref: the system's engine is only loaded for a profile with one.
constexpr char kSites[] = "browser_core.system_engine.sites";
// The sites whose protected-media note the user closed.
constexpr char kClosedNotes[] = "browser_core.system_engine.closed_notes";
// The listed services the user turned off, by their entries in kListed.
constexpr char kListedOff[] = "browser_core.system_engine.listed_off";

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreSystemEngine") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    registry->RegisterListPref(kSites);
    registry->RegisterListPref(kClosedNotes);
    registry->RegisterListPref(kListedOff);
    registry->RegisterStringPref(kSystemStorePref, std::string());
  }
};

// The streaming services whose main content is protected video or audio, which open in the system's
// engine from the first visit. Add an entry only for such a service, as its own host, or as its
// registrable domain where nothing else is at that domain: each matches its host and subdomains.
constexpr auto kListed = base::MakeFixedFlatSet<std::string_view>({
    "disneyplus.com",
    "hbomax.com",
    "hulu.com",
    "max.com",
    "netflix.com",
    "open.spotify.com",
    "paramountplus.com",
    "peacocktv.com",
    "primevideo.com",
    "tv.apple.com",
});

// For the app's check, which never loads a real service: a host of its own server, listed as the
// services are. Only a dev build of the app passes it (main.rs).
constexpr char kTestHostSwitch[] = "streaming-test-host";

// |url|'s entry in kListed, else empty: a lookup for each of its host's few labels.
std::string_view ListedEntry(const GURL& url) {
  static const base::NoDestructor<std::string> test_host(
      base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(kTestHostSwitch));
  std::string_view host = url.host();
  if (!test_host->empty() && host == *test_host) {
    return *test_host;
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

// A private session's store, which keeps nothing on disk and goes with the session's profile.
class PrivateStore : public base::SupportsUserData::Data {
 public:
  static constexpr char kKey[] = "browser_core_private_system_store";

  ~PrivateStore() override { platform::SystemPage::ReleaseStore(id); }

  const std::string id = base::Uuid::GenerateRandomV4().AsLowercaseString();
};

// The data folder the profiles are in.
base::FilePath DataFolder() {
  ProfileManager* manager = g_browser_process->profile_manager();
  return manager ? manager->user_data_dir() : base::FilePath();
}

// The data folder's Local State, which lists the stores its profiles were given.
PrefService& LocalState() {
  return *g_browser_process->local_state();
}

void DeleteStoreOf(const std::string& id) {
  if (id.empty()) {
    return;
  }
  platform::SystemPage::DeleteStore(id, base::DoNothing());
  std::vector<std::string> given = GivenStores(LocalState());
  if (std::erase(given, id)) {
    SetGivenStores(LocalState(), given);
  }
}

// Adds the stores the loaded profiles name to |named|. Read on the UI thread, where a profile takes
// a store: one loaded since the disk was read, or whose Preferences aren't written yet, names its
// store here.
void NameLoadedStores(ProfileManager& manager, std::set<std::string>& named) {
  for (Profile* profile : manager.GetLoadedProfiles()) {
    if (std::string id = profile->GetPrefs()->GetString(kSystemStorePref); !id.empty()) {
      named.insert(std::move(id));
    }
  }
}

// What the disk says about the stores an old version gave a data folder's profiles (#2701).
struct OldStoresOnDisk {
  // The stores the profiles on disk name.
  std::set<std::string> named;
  // Those of the paths the folder's profiles could have had that have a folder.
  std::set<base::FilePath> present;
};

OldStoresOnDisk ReadOldStores(const base::FilePath& folder, int profiles_created) {
  OldStoresOnDisk disk{StoresNamedOnDisk(folder), {}};
  for (const base::FilePath& path : OldProfilePaths(folder, profiles_created)) {
    if (base::DirectoryExists(path)) {
      disk.present.insert(path);
    }
  }
  return disk;
}

// Marks the data folder's old stores swept, written at once: a crash right after a first run must
// not leave a new folder to sweep a set-aside one's stores later.
void MarkOldStoresSwept() {
  LocalState().SetBoolean(kOldStoresSweptPref, true);
  LocalState().CommitPendingWrite();
}

void OldStoresDeleted(std::vector<bool> deleted) {
  // One that failed stays unmarked, and is tried again at the next launch, never sooner.
  if (std::ranges::all_of(deleted, [](bool each) { return each; })) {
    MarkOldStoresSwept();
    // check.sh reads this line.
    fprintf(stderr, "browser: old system stores swept\n");
  }
}

void DeleteOldStores(const std::vector<std::string>& listed,
                     const base::FilePath& folder,
                     int profiles_created,
                     OldStoresOnDisk disk) {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return;
  }
  NameLoadedStores(*manager, disk.named);
  for (std::string& id : GivenStores(LocalState())) {
    disk.named.insert(std::move(id));
  }
  const std::vector<std::string> gone =
      OldStoresUnnamed(listed, folder, profiles_created, disk.named, disk.present);
  if (gone.empty()) {
    OldStoresDeleted({});
    return;
  }
  const auto deleted = base::BarrierCallback<bool>(gone.size(), base::BindOnce(&OldStoresDeleted));
  for (const std::string& id : gone) {
    platform::SystemPage::DeleteStore(id, deleted);
  }
}

// Deletes, once per data folder, the stores a version before #2664 gave profiles that are gone,
// whose ids came from their paths (#2701). A new folder never had one, so it's only marked.
void SweepOldStores() {
  PrefService& local_state = LocalState();
  const bool created = local_state.GetInitializationStatus() ==
                       PrefService::INITIALIZATION_STATUS_CREATED_NEW_PREF_STORE;
  switch (OldStoresFor(created, local_state.GetBoolean(kOldStoresSweptPref))) {
    case OldStores::kNothing:
      return;
    case OldStores::kMark:
      MarkOldStoresSwept();
      return;
    case OldStores::kSweep:
      break;
  }
  const base::FilePath folder = DataFolder();
  const int profiles_created = local_state.GetInteger(prefs::kProfilesNumCreated);
  platform::SystemPage::ListStores(base::BindOnce(
      [](const base::FilePath& in, int count, std::vector<std::string> listed) {
        base::ThreadPool::PostTaskAndReplyWithResult(
            FROM_HERE, {base::MayBlock(), base::TaskPriority::BEST_EFFORT},
            base::BindOnce(&ReadOldStores, in, count),
            base::BindOnce(&DeleteOldStores, std::move(listed), in, count));
      },
      folder, profiles_created));
}

// Deletes the stores given to the data folder's profiles that no profile names any more, as when a
// profile's folder was removed by hand, and lists the ones named, such as a store an earlier
// version gave.
void Sweep(std::set<std::string> named) {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return;
  }
  NameLoadedStores(*manager, named);
  const std::vector<std::string> given = GivenStores(LocalState());
  for (const std::string& id : StoresUnnamed(given, named)) {
    platform::SystemPage::DeleteStore(id, base::DoNothing());
  }
  if (std::set<std::string>(given.begin(), given.end()) != named) {
    SetGivenStores(LocalState(), {named.begin(), named.end()});
  }
  SweepOldStores();
}

// localStorage as Blink keeps it, which the browser can't include (cached_storage_area.cc in
// third_party/blink/renderer/modules/storage): a byte for Latin-1 (1) or UTF-16 (0), then the
// characters.
std::vector<uint8_t> StorageBytes(const std::u16string& text) {
  const bool latin1 = std::ranges::all_of(text, [](char16_t c) { return c <= 0xff; });
  std::vector<uint8_t> bytes = {static_cast<uint8_t>(latin1 ? 1 : 0)};
  for (char16_t c : text) {
    bytes.push_back(static_cast<uint8_t>(c & 0xff));
    if (!latin1) {
      bytes.push_back(static_cast<uint8_t>(c >> 8));
    }
  }
  return bytes;
}

std::optional<std::u16string> StorageText(const std::vector<uint8_t>& bytes) {
  if (bytes.empty()) {
    return std::u16string();
  }
  std::u16string text;
  if (bytes[0] == 1) {
    for (size_t i = 1; i < bytes.size(); ++i) {
      text.push_back(bytes[i]);
    }
    return text;
  }
  if (bytes[0] != 0 || bytes.size() % 2 != 1) {
    return std::nullopt;
  }
  for (size_t i = 1; i + 1 < bytes.size(); i += 2) {
    text.push_back(static_cast<char16_t>(bytes[i] | bytes[i + 1] << 8));
  }
  return text;
}

// Whether |origin| is a page's of |site|.
bool OriginOfSite(const url::Origin& origin, const std::string& site) {
  return (origin.scheme() == url::kHttpScheme || origin.scheme() == url::kHttpsScheme) &&
         platform::HostOfSite(origin.host(), site);
}

size_t ItemsIn(const platform::StorageByOrigin& storage) {
  size_t items = 0;
  for (const auto& [origin, each] : storage) {
    items += each.size();
  }
  return items;
}

// Reads a site's cookies and localStorage from a profile's Chromium store, then deletes itself.
// What the store doesn't answer, as when its service or its profile goes, comes back empty.
class ChromiumReader {
 public:
  using Done =
      base::OnceCallback<void(std::vector<net::CanonicalCookie>, platform::StorageByOrigin)>;

  // |page|'s origin is read whether or not the store has written it to disk yet.
  static void Run(Profile* profile, const std::string& site, const url::Origin& page, Done done) {
    new ChromiumReader(profile, site, page, std::move(done));
  }

 private:
  ChromiumReader(Profile* profile, const std::string& site, const url::Origin& page, Done done)
      : profile_(profile->GetWeakPtr()), site_(site), page_(page), done_(std::move(done)) {
    profile->GetDefaultStoragePartition()->GetCookieManagerForBrowserProcess()->GetAllCookies(
        mojo::WrapCallbackWithDefaultInvokeIfNotRun(
            base::BindOnce(&ChromiumReader::GotCookies, weak_factory_.GetWeakPtr()),
            net::CookieList()));
  }

  void GotCookies(const net::CookieList& all) {
    for (const net::CanonicalCookie& cookie : all) {
      // A cookie kept for the site inside another one's pages stays: WebKit keeps none that way.
      if (!cookie.IsPartitioned() && platform::HostOfSite(cookie.DomainWithoutDot(), site_)) {
        cookies_.push_back(cookie);
      }
    }
    storage::mojom::LocalStorageControl* control = Control();
    if (!control) {
      Finish();
      return;
    }
    control->GetUsage(mojo::WrapCallbackWithDefaultInvokeIfNotRun(
        base::BindOnce(&ChromiumReader::GotOrigins, weak_factory_.GetWeakPtr()),
        std::vector<storage::mojom::StorageUsageInfoPtr>()));
  }

  void GotOrigins(std::vector<storage::mojom::StorageUsageInfoPtr> usage) {
    std::set<url::Origin> origins;
    if (OriginOfSite(page_, site_)) {
      origins.insert(page_);
    }
    for (const storage::mojom::StorageUsageInfoPtr& each : usage) {
      // Only the site's own pages': its storage inside other sites' pages is kept apart.
      if (each->storage_key.IsFirstPartyContext() &&
          OriginOfSite(each->storage_key.origin(), site_)) {
        origins.insert(each->storage_key.origin());
      }
    }
    storage::mojom::LocalStorageControl* control = Control();
    if (origins.empty() || !control) {
      Finish();
      return;
    }
    pending_ = origins.size();
    areas_.reserve(origins.size());
    for (const url::Origin& origin : origins) {
      mojo::Remote<blink::mojom::StorageArea>& area = areas_.emplace_back();
      control->BindStorageArea(blink::StorageKey::CreateFirstParty(origin),
                               area.BindNewPipeAndPassReceiver());
      area->GetAll(/*new_observer=*/mojo::NullRemote(),
                   mojo::WrapCallbackWithDefaultInvokeIfNotRun(
                       base::BindOnce(&ChromiumReader::GotItems, weak_factory_.GetWeakPtr(),
                                      origin),
                       std::vector<blink::mojom::KeyValuePtr>()));
    }
  }

  void GotItems(const url::Origin& origin, std::vector<blink::mojom::KeyValuePtr> data) {
    platform::StorageItems items;
    for (const blink::mojom::KeyValuePtr& each : data) {
      std::optional<std::u16string> key = StorageText(each->key);
      std::optional<std::u16string> value = StorageText(each->value);
      if (key && value) {
        items[*key] = *value;
      }
    }
    if (!items.empty()) {
      storage_[origin] = std::move(items);
    }
    if (--pending_ == 0) {
      Finish();
    }
  }

  // The profile's, unless it went: its store's callbacks run as the store goes, after it.
  storage::mojom::LocalStorageControl* Control() {
    return profile_ ? profile_->GetDefaultStoragePartition()->GetLocalStorageControl() : nullptr;
  }

  void Finish() {
    std::move(done_).Run(std::move(cookies_), std::move(storage_));
    delete this;
  }

  const base::WeakPtr<Profile> profile_;
  const std::string site_;
  const url::Origin page_;
  Done done_;
  std::vector<net::CanonicalCookie> cookies_;
  platform::StorageByOrigin storage_;
  std::vector<mojo::Remote<blink::mojom::StorageArea>> areas_;
  size_t pending_ = 0;
  // Last, so the callbacks the areas drop as they go find it gone.
  base::WeakPtrFactory<ChromiumReader> weak_factory_{this};
};

// Writes cookies and localStorage into a profile's Chromium store, then runs its callback with how
// many of each the store took, and deletes itself.
class ChromiumWriter {
 public:
  using Done = base::OnceCallback<void(size_t cookies, size_t items)>;

  static void Run(content::StoragePartition* partition,
                  const std::vector<net::CanonicalCookie>& cookies,
                  const platform::StorageByOrigin& storage,
                  Done done) {
    (new ChromiumWriter(std::move(done)))->Write(partition, cookies, storage);
  }

 private:
  explicit ChromiumWriter(Done done) : done_(std::move(done)) {}

  void Write(content::StoragePartition* partition,
             const std::vector<net::CanonicalCookie>& cookies,
             const platform::StorageByOrigin& storage) {
    for (const net::CanonicalCookie& cookie : cookies) {
      ++pending_;
      // As the cookie's own site would set it, HttpOnly and SameSite included.
      const GURL source((cookie.SecureAttribute() ? "https://" : "http://") +
                        cookie.DomainWithoutDot() + cookie.Path());
      partition->GetCookieManagerForBrowserProcess()->SetCanonicalCookie(
          cookie, source, net::CookieOptions::MakeAllInclusive(),
          mojo::WrapCallbackWithDropHandler(
              base::BindOnce(&ChromiumWriter::CookieSet, weak_factory_.GetWeakPtr()),
              base::BindOnce(&ChromiumWriter::Wrote, weak_factory_.GetWeakPtr(),
                             /*cookie=*/true, /*took=*/false)));
    }
    storage::mojom::LocalStorageControl* control = partition->GetLocalStorageControl();
    for (const auto& [origin, items] : storage) {
      if (!control) {
        break;
      }
      mojo::Remote<blink::mojom::StorageArea>& area = areas_.emplace_back();
      control->BindStorageArea(blink::StorageKey::CreateFirstParty(origin),
                               area.BindNewPipeAndPassReceiver());
      for (const auto& [key, value] : items) {
        ++pending_;
        // No old value: the storage service reads it, for the pages its observers tell.
        area->Put(StorageBytes(key), StorageBytes(value), /*client_old_value=*/std::nullopt,
                  /*source=*/nullptr,
                  mojo::WrapCallbackWithDefaultInvokeIfNotRun(
                      base::BindOnce(&ChromiumWriter::Wrote, weak_factory_.GetWeakPtr(),
                                     /*cookie=*/false),
                      false));
      }
    }
    if (pending_ == 0) {
      Finish();
    }
  }

  void CookieSet(net::CookieAccessResult result) {
    Wrote(/*cookie=*/true, result.status.IsInclude());
  }

  void Wrote(bool cookie, bool took) {
    if (took) {
      ++(cookie ? cookies_ : items_);
    }
    if (--pending_ == 0) {
      Finish();
    }
  }

  void Finish() {
    std::move(done_).Run(cookies_, items_);
    delete this;
  }

  Done done_;
  std::vector<mojo::Remote<blink::mojom::StorageArea>> areas_;
  size_t pending_ = 0;
  size_t cookies_ = 0;
  size_t items_ = 0;
  base::WeakPtrFactory<ChromiumWriter> weak_factory_{this};
};

// Has |tabs| show their pages in Chromium, those still at |site| in the system's engine, once
// their session is there.
void HandBack(base::WeakPtr<Profile> profile,
              const std::string& site,
              const std::vector<bc_id>& tabs) {
  for (bc_id id : tabs) {
    content::WebContents* contents = Tab::FromId(id);
    Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
    platform::SystemPage* page = tab ? tab->system_page() : nullptr;
    // The site turned on again in the meantime keeps it.
    if (page && profile && SiteOf(page->url()) == site && !Remembered(profile.get(), page->url())) {
      tab->UseChromium(page->url());
    }
  }
}

class Throttle : public content::NavigationThrottle {
 public:
  using content::NavigationThrottle::NavigationThrottle;

  ThrottleCheckResult WillStartRequest() override { return Check(); }
  ThrottleCheckResult WillRedirectRequest() override { return Check(); }
  const char* GetNameForLogging() override { return "BrowserCoreSystemEngineThrottle"; }

 private:
  ThrottleCheckResult Check() {
    content::NavigationHandle* navigation = navigation_handle();
    content::WebContents* contents = navigation->GetWebContents();
    if (!contents || !navigation->IsInPrimaryMainFrame()) {
      return PROCEED;
    }
    if (Tab* tab = Tab::FromWebContents(contents)) {
      return Decide(tab);
    }
    // A tab Chrome makes starts its first load before the core has the tab or its window has
    // placed it in a Space, as ⌘T's does: one going to a site kept for the system's engine waits
    // a turn for them. Any other goes on at once, so new tabs cost nothing more.
    if (!Kept()) {
      return PROCEED;
    }
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Throttle::Placed, weak_factory_.GetWeakPtr()));
    return DEFER;
  }

  // Whether the navigation goes to a site the profile keeps for the system's engine, and could
  // load there.
  bool Kept() {
    content::NavigationHandle* navigation = navigation_handle();
    const GURL& url = navigation->GetURL();
    // A form's answer stays: the system's engine would load it without what was sent.
    return !navigation->IsPost() && url.SchemeIsHTTPOrHTTPS() &&
           Remembered(Profile::FromBrowserContext(navigation->GetWebContents()->GetBrowserContext()),
                      url);
  }

  // The deferred load, once its tab is made and placed. A page of no window's goes on in
  // Chromium.
  void Placed() {
    content::WebContents* contents = navigation_handle()->GetWebContents();
    Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
    const ThrottleCheckResult result = tab ? Decide(tab) : PROCEED;
    if (result.action() == PROCEED) {
      Resume();
    } else {
      // Last: the navigation, and this with it, can go here.
      CancelDeferredNavigation(result);
    }
  }

  ThrottleCheckResult Decide(Tab* tab) {
    // Its page is the system's, and Chromium's waits, as when Chrome reloads a discarded tab.
    if (tab->system_page()) {
      return CANCEL_AND_IGNORE;
    }
    // It loads the page it waits for once shown, as a tab unloaded from a system page does, not the
    // page Chromium kept, which it reloads then, and which may be at a site kept for the system's
    // engine.
    if (tab->waiting()) {
      return CANCEL_AND_IGNORE;
    }
    content::WebContents* contents = navigation_handle()->GetWebContents();
    if (!Kept() || !Allowed(Tab::IdOf(contents))) {
      return PROCEED;
    }
    // Posted: the navigation is part way through.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<content::WebContents> contents, const GURL& url) {
                         if (Tab* tab = contents ? Tab::FromWebContents(contents.get()) : nullptr) {
                           tab->UseSystemEngine(url);
                         }
                       },
                       contents->GetWeakPtr(), navigation_handle()->GetURL()));
    return CANCEL_AND_IGNORE;
  }

  base::WeakPtrFactory<Throttle> weak_factory_{this};
};

}  // namespace

void RegisterProfilePrefs() {
  PrefsFactory::GetInstance();
}

bool Available() {
  return platform::SystemPage::Available();
}

bool Allowed(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  // Placed or about to be: a tab Chrome just made is in no Space yet.
  const Space* space = contents ? Window::SpaceOf(contents) : Space::Holding(tab);
  return Available() && !(space && space->agent());
}

std::string SiteOf(const GURL& url) {
  const std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  // An IP address or a bare host name is a site of its own, as the core clears sites.
  return domain.empty() ? std::string(url.host()) : domain;
}

bool Remembered(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS() || !Available()) {
    return false;
  }
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    const base::ListValue& off = profile->GetPrefs()->GetList(kListedOff);
    if (!std::ranges::contains(off, base::Value(entry))) {
      return true;
    }
  }
  const base::ListValue& sites = profile->GetPrefs()->GetList(kSites);
  return !sites.empty() && std::ranges::contains(sites, base::Value(SiteOf(url)));
}

void Remember(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  // A listed service is never kept: turning it on takes back the user's off.
  if (const std::string_view entry = ListedEntry(url); !entry.empty()) {
    ScopedListPrefUpdate(profile->GetPrefs(), kListedOff)->EraseValue(base::Value(entry));
    return;
  }
  if (!Remembered(profile, url)) {
    ScopedListPrefUpdate(profile->GetPrefs(), kSites)->Append(SiteOf(url));
  }
}

void Forget(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS()) {
    return;
  }
  ScopedListPrefUpdate(profile->GetPrefs(), kSites)->EraseValue(base::Value(SiteOf(url)));
  const std::string_view entry = ListedEntry(url);
  if (!entry.empty() &&
      !std::ranges::contains(profile->GetPrefs()->GetList(kListedOff), base::Value(entry))) {
    ScopedListPrefUpdate(profile->GetPrefs(), kListedOff)->Append(std::string(entry));
  }
}

bool NoteClosed(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS()) {
    return false;
  }
  const base::ListValue& sites = profile->GetPrefs()->GetList(kClosedNotes);
  return !sites.empty() && std::ranges::contains(sites, base::Value(SiteOf(url)));
}

void CloseNote(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS() || NoteClosed(profile, url)) {
    return;
  }
  ScopedListPrefUpdate(profile->GetPrefs(), kClosedNotes)->Append(SiteOf(url));
}

platform::SystemStore StoreOf(Profile* profile) {
  if (profile->IsOffTheRecord()) {
    if (!profile->GetUserData(PrivateStore::kKey)) {
      profile->SetUserData(PrivateStore::kKey, std::make_unique<PrivateStore>());
    }
    return {static_cast<PrivateStore*>(profile->GetUserData(PrivateStore::kKey))->id, false};
  }
  PrefService* prefs = profile->GetPrefs();
  std::string id = prefs->GetString(kSystemStorePref);
  if (id.empty()) {
    // Random, and kept in the profile, so a profile made later at its path, once its folder was
    // removed by hand, has a store of its own (#2657). Kept and listed before the store is made,
    // so no store goes unnamed.
    id = base::Uuid::GenerateRandomV4().AsLowercaseString();
    prefs->SetString(kSystemStorePref, id);
    std::vector<std::string> given = GivenStores(LocalState());
    given.push_back(id);
    SetGivenStores(LocalState(), given);
  }
  return {id, true};
}

bool StoreUsed(Profile* profile) {
  return profile->IsOffTheRecord() ? profile->GetUserData(PrivateStore::kKey) != nullptr
                                   : !profile->GetPrefs()->GetString(kSystemStorePref).empty();
}

void CopyToSystemEngine(Profile* profile,
                        const GURL& url,
                        base::OnceCallback<void(platform::StorageByOrigin)> done) {
  const std::string site = SiteOf(url);
  ChromiumReader::Run(
      profile, site, url::Origin::Create(url),
      base::BindOnce(
          [](base::WeakPtr<Profile> profile, const platform::SystemStore& store,
             const std::string& site, base::OnceCallback<void(platform::StorageByOrigin)> done,
             std::vector<net::CanonicalCookie> cookies, platform::StorageByOrigin storage) {
            // Gone with its profile, which a private one's store was let go with.
            if (!profile) {
              return;
            }
            platform::SystemPage::SetCookies(
                store, cookies,
                base::BindOnce(
                    [](const std::string& site,
                       base::OnceCallback<void(platform::StorageByOrigin)> done,
                       platform::StorageByOrigin storage, size_t copied) {
                      // check.sh reads this line. Counts only: never a name or a value.
                      fprintf(stderr,
                              "browser: copied %zu cookies and %zu storage items for %s into "
                              "the system's engine\n",
                              copied, ItemsIn(storage), site.c_str());
                      std::move(done).Run(std::move(storage));
                    },
                    site, std::move(done), std::move(storage)));
          },
          profile->GetWeakPtr(), StoreOf(profile), site, std::move(done)));
}

void ReturnToChromium(Profile* profile, const GURL& url) {
  const std::string site = SiteOf(url);
  std::vector<bc_id> tabs;
  std::vector<platform::SystemPage*> pages;
  for (content::WebContents* each : Tab::Pages()) {
    platform::SystemPage* page = Tab::FromWebContents(each)->system_page();
    if (each->GetBrowserContext() == profile && page && SiteOf(page->url()) == site) {
      tabs.push_back(Tab::IdOf(each));
      pages.push_back(page);
    }
  }
  using Read = std::pair<url::Origin, platform::StorageItems>;
  // The pages' localStorage, then the store's cookies, then both into Chromium's store, and then
  // the pages there.
  auto write = base::BindOnce(
      [](base::WeakPtr<Profile> profile, const std::string& site, std::vector<bc_id> tabs,
         std::vector<Read> reads, std::vector<net::CanonicalCookie> cookies) {
        if (!profile) {
          return;
        }
        platform::StorageByOrigin storage;
        for (auto& [origin, items] : reads) {
          if (OriginOfSite(origin, site)) {
            storage[origin].merge(items);
          }
        }
        ChromiumWriter::Run(
            profile->GetDefaultStoragePartition(), cookies, storage,
            base::BindOnce(
                [](base::WeakPtr<Profile> profile, const std::string& site,
                   const std::vector<bc_id>& tabs, size_t cookies, size_t items) {
                  // check.sh reads this line. Counts only: never a name or a value.
                  fprintf(stderr,
                          "browser: copied %zu cookies and %zu storage items for %s back to "
                          "Chromium\n",
                          cookies, items, site.c_str());
                  HandBack(profile, site, tabs);
                },
                profile, site, std::move(tabs)));
      },
      profile->GetWeakPtr(), site, std::move(tabs));
  auto read_cookies = base::BindOnce(
      [](base::WeakPtr<Profile> profile, const std::string& site,
         base::OnceCallback<void(std::vector<Read>, std::vector<net::CanonicalCookie>)> write,
         std::vector<Read> reads) {
        if (!profile) {
          return;
        }
        // A store never made has nothing, and the system's engine stays unloaded.
        if (!StoreUsed(profile.get())) {
          std::move(write).Run(std::move(reads), {});
          return;
        }
        platform::SystemPage::GetCookies(StoreOf(profile.get()), site,
                                         base::BindOnce(std::move(write), std::move(reads)));
      },
      profile->GetWeakPtr(), site, std::move(write));
  // Each page answers, or a moment passes.
  base::RepeatingCallback<void(Read)> read =
      base::BarrierCallback<Read>(pages.size(), std::move(read_cookies));
  for (platform::SystemPage* page : pages) {
    page->GetStorage(base::BindOnce(
        [](base::RepeatingCallback<void(Read)> read, url::Origin origin,
           platform::StorageItems items) { read.Run({std::move(origin), std::move(items)}); },
        read));
  }
}

void DeleteStore(const base::FilePath& path, base::OnceClosure then) {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (Profile* loaded = manager ? manager->GetProfileByPath(path) : nullptr) {
    DeleteStoreOf(loaded->GetPrefs()->GetString(kSystemStorePref));
    std::move(then).Run();
    return;
  }
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_VISIBLE},
      base::BindOnce(&StoreNamedOnDisk, path),
      base::BindOnce(
          [](base::OnceClosure then, const std::string& id) {
            DeleteStoreOf(id);
            std::move(then).Run();
          },
          std::move(then)));
}

void RegisterLocalStatePrefs(PrefRegistrySimple* registry) {
  RegisterGivenStores(registry);
  registry->RegisterBooleanPref(kOldStoresSweptPref, false);
}

void SweepStores() {
  if (!Available()) {
    return;
  }
  // Best effort, behind the work launch queued, and off the UI thread for the disk, so a launch
  // never waits for it.
  content::GetUIThreadTaskRunner({base::TaskPriority::BEST_EFFORT})
      ->PostTask(FROM_HERE, base::BindOnce([] {
                   const base::FilePath folder = DataFolder();
                   if (folder.empty()) {
                     return;
                   }
                   base::ThreadPool::PostTaskAndReplyWithResult(
                       FROM_HERE, {base::MayBlock(), base::TaskPriority::BEST_EFFORT},
                       base::BindOnce(&StoresNamedOnDisk, folder), base::BindOnce(&Sweep));
                 }));
}

void CreateThrottlesForNavigation(content::NavigationThrottleRegistry& registry) {
  if (Available() && registry.GetNavigationHandle().IsInPrimaryMainFrame()) {
    registry.AddThrottle(std::make_unique<Throttle>(registry));
  }
}

}  // namespace browser_core::system_engine
