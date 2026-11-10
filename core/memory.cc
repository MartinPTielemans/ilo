#include "browser_core/memory.h"

#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/environment.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/supports_user_data.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/task/updateable_sequenced_task_runner.h"
#include "base/threading/sequence_bound.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/host.h"
#include "browser_core/memory_index.h"
#include "browser_core/memory_keep_out.h"
#include "browser_core/page_text.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_service_observer.h"
#include "components/history/core/browser/history_types.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/keyed_service/core/service_access_type.h"
#include "components/os_crypt/async/browser/os_crypt_async.h"
#include "components/os_crypt/async/common/encryptor.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/schema_org/common/metadata.mojom.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "mojo/public/cpp/bindings/callback_helpers.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "net/http/http_response_headers.h"
#include "services/service_manager/public/cpp/interface_provider.h"
#include "third_party/blink/public/mojom/document_metadata/document_metadata.mojom.h"
#include "ui/accessibility/ax_mode.h"
#include "ui/accessibility/ax_tree_update.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// The profile's: BC_MEMORY_UNASKED until the user answers the one-time ask. Not synced: Chrome
// syncs a pref only when it's registered as syncable.
constexpr char kMemoryPref[] = "browser_core.memory";
constexpr base::FilePath::CharType kFile[] = FILE_PATH_LITERAL("Memory");
// A spike's switch, so a check run measures the hot paths with the memory on: BROWSER_MEMORY=on
// turns it on in every profile without asking.
constexpr char kOnForChecks[] = "BROWSER_MEMORY";

// A page is kept once it has been loaded and still this long, as it's being read...
constexpr base::TimeDelta kMemoryIdle = base::Seconds(10);
// ...or this long after another tab is shown in its place, well past the switch's frames.
constexpr base::TimeDelta kMemoryBehind = base::Seconds(2);
// As PageText's, which reads the same text for agents.
constexpr size_t kMaxNodes = 20000;
constexpr base::TimeDelta kSnapshotTimeout = base::Seconds(5);
// ⌘T shows few rows of each kind.
constexpr size_t kShown = 3;

class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreMemory") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    registry->RegisterIntegerPref(kMemoryPref, BC_MEMORY_UNASKED);
  }
};

// Seals with the OS-held key Chrome's os_crypt keeps for the profile's passwords and cookies:
// the Keychain's on the Mac, DPAPI's on Windows, the Secret Service's or KWallet's on Linux.
class EncryptorSealer : public MemoryIndex::Sealer {
 public:
  explicit EncryptorSealer(scoped_refptr<os_crypt_async::Encryptor> encryptor)
      : encryptor_(std::move(encryptor)) {}

  std::optional<std::vector<uint8_t>> Seal(std::string_view plain) const override {
    return encryptor_->EncryptString(std::string(plain));
  }

  std::optional<std::string> Open(base::span<const uint8_t> sealed) const override {
    return encryptor_->DecryptData(sealed);
  }

 private:
  scoped_refptr<os_crypt_async::Encryptor> encryptor_;
};

std::string SiteOf(const GURL& url) {
  std::string site = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  return site.empty() ? std::string(url.host()) : site;
}

// A profile's memory, for as long as the profile, on a sequence of its own that may block. Its
// pages are kept at the lowest priority, which a query raises while it runs.
class Memory : public base::SupportsUserData::Data, public history::HistoryServiceObserver {
 public:
  static constexpr char kKey[] = "browser_core::Memory";

  static Memory* Of(Profile* profile) {
    return profile ? static_cast<Memory*>(profile->GetUserData(kKey)) : nullptr;
  }

  explicit Memory(Profile* profile)
      : profile_(profile),
        path_(profile->GetPath().Append(kFile)),
        runner_(base::ThreadPool::CreateUpdateableSequencedTaskRunner(
            {base::MayBlock(), base::TaskPriority::BEST_EFFORT,
             base::TaskShutdownBehavior::SKIP_ON_SHUTDOWN})) {
    if (history::HistoryService* history = HistoryServiceFactory::GetForProfile(
            profile, ServiceAccessType::EXPLICIT_ACCESS)) {
      observation_.Observe(history);
    }
    if (State() == BC_MEMORY_ON || State() == BC_MEMORY_PAUSED) {
      Open();
    }
  }

  int State() const { return profile_->GetPrefs()->GetInteger(kMemoryPref); }

  void SetState(int state) {
    if (state < BC_MEMORY_OFF || state > BC_MEMORY_PAUSED || state == State()) {
      return;
    }
    profile_->GetPrefs()->SetInteger(kMemoryPref, state);
    if (state == BC_MEMORY_OFF) {
      // Off keeps nothing: the file goes, the sites kept out with it.
      index_.Reset();
      kept_out_.clear();
      runner_->PostTask(FROM_HERE, base::BindOnce(
                                       [](base::FilePath path) {
                                         base::DeleteFile(path);
                                         base::DeleteFile(path.AddExtensionASCII("-journal"));
                                       },
                                       path_));
    } else if (index_.is_null()) {
      Open();
    }
  }

  // Whether a page of |site| is to be kept now.
  bool Keeps(const std::string& site) const {
    return State() == BC_MEMORY_ON && open_ && !kept_out_.contains(site);
  }

  void Keep(ui::AXTreeUpdate update, std::set<std::string, std::less<>> readable, GURL url,
            std::string site, std::string title) {
    if (!Keeps(site)) {
      return;
    }
    // The text is built off the UI thread too: a snapshot can be 20,000 nodes.
    index_.PostTaskWithThisObject(base::BindOnce(
        [](ui::AXTreeUpdate update, std::set<std::string, std::less<>> readable, GURL url,
           std::string site, std::string title, MemoryIndex* index) {
          std::string text = TextOf(update, readable);
          if (!text.empty()) {
            index->Add(url.spec(), site, title, text, base::Time::Now());
          }
        },
        std::move(update), std::move(readable), std::move(url), std::move(site),
        std::move(title)));
  }

  // The site said what it is: kept out, with why, until the user lets it in.
  void KeepOut(const std::string& site, KeptOut why, bool out) {
    if (index_.is_null()) {
      return;
    }
    if (out) {
      kept_out_.insert(site);
    } else {
      kept_out_.erase(site);
    }
    index_.AsyncCall(&MemoryIndex::KeepOut).WithArgs(site, why, out);
  }

  void Search(uint64_t request, const std::string& query) {
    if (index_.is_null() || !open_) {
      Found(request, {});
      return;
    }
    // Ahead of pages waiting to be kept, as someone is typing.
    runner_->UpdatePriority(base::TaskPriority::USER_BLOCKING);
    index_.AsyncCall(&MemoryIndex::Search)
        .WithArgs(query, kShown)
        .Then(base::BindOnce(&Memory::Found, weak_factory_.GetWeakPtr(), request));
  }

  void ForgetSite(const std::string& site) {
    if (!index_.is_null()) {
      index_.AsyncCall(&MemoryIndex::ForgetSite).WithArgs(site);
    }
  }

  void ForgetAll() {
    if (!index_.is_null()) {
      index_.AsyncCall(&MemoryIndex::ForgetAll);
    }
  }

  void ReadKeptOut(uint64_t request) {
    if (index_.is_null()) {
      Listed(request, {});
      return;
    }
    index_.AsyncCall(&MemoryIndex::SitesKeptOut)
        .Then(base::BindOnce(&Memory::Listed, weak_factory_.GetWeakPtr(), request));
  }

  // HistoryServiceObserver: clearing history clears the memory of the same pages, and history's
  // own expiry of old visits too, so the memory never holds a page history no longer does.
  void OnHistoryDeletions(history::HistoryService* history,
                          const history::DeletionInfo& info) override {
    if (index_.is_null()) {
      return;
    }
    if (info.IsAllHistory()) {
      ForgetAll();
      return;
    }
    std::vector<std::string> urls;
    for (const history::URLRow& row : info.deleted_rows()) {
      urls.push_back(row.url().spec());
    }
    if (info.restrict_urls()) {
      for (const GURL& url : *info.restrict_urls()) {
        urls.push_back(url.spec());
      }
    }
    base::Time begin;
    base::Time end;
    const history::DeletionTimeRange& range = info.time_range();
    if (range.IsValid() && !info.restrict_urls()) {
      begin = range.begin();
      end = range.end().is_null() ? base::Time::Max() : range.end();
    }
    index_.AsyncCall(&MemoryIndex::ForgetPages).WithArgs(std::move(urls), begin, end);
  }

  void HistoryServiceBeingDeleted(history::HistoryService* history) override {
    observation_.Reset();
  }

 private:
  void Open() {
    g_browser_process->os_crypt_async()->GetInstance(
        base::BindOnce(&Memory::GotEncryptor, weak_factory_.GetWeakPtr()));
  }

  void GotEncryptor(scoped_refptr<os_crypt_async::Encryptor> encryptor) {
    if (State() != BC_MEMORY_ON && State() != BC_MEMORY_PAUSED) {
      return;
    }
    index_.emplace(runner_, std::make_unique<EncryptorSealer>(std::move(encryptor)));
    index_.AsyncCall(&MemoryIndex::Open)
        .WithArgs(path_)
        .Then(base::BindOnce(&Memory::Opened, weak_factory_.GetWeakPtr()));
  }

  void Opened(bool open) {
    if (!open) {
      LOG(ERROR) << "browser: memory can't open its file";
      return;
    }
    index_.AsyncCall(&MemoryIndex::SitesKeptOut)
        .Then(base::BindOnce(&Memory::GotKeptOut, weak_factory_.GetWeakPtr()));
  }

  void GotKeptOut(std::vector<std::pair<std::string, KeptOut>> sites) {
    for (auto& [site, why] : sites) {
      kept_out_.insert(std::move(site));
    }
    open_ = true;
    // check.sh waits for this line.
    LOG(ERROR) << "browser: memory open";
  }

  void Found(uint64_t request, std::vector<MemoryMatch> matches) {
    runner_->UpdatePriority(base::TaskPriority::BEST_EFFORT);
    if (!Host().memory_found) {
      return;
    }
    std::vector<bc_memory_match> found;
    for (const MemoryMatch& match : matches) {
      found.push_back({match.url.c_str(), match.title.c_str(), match.snippet.c_str(),
                       (match.visited - base::Time::UnixEpoch()).InSeconds()});
    }
    Host().memory_found(Host().context, ProfileId(DirOf(profile_)), request, found.data(),
                        found.size());
  }

  void Listed(uint64_t request, std::vector<std::pair<std::string, KeptOut>> sites) {
    if (!Host().memory_kept_out) {
      return;
    }
    std::vector<const char*> names;
    std::vector<int> whys;
    for (const auto& [site, why] : sites) {
      names.push_back(site.c_str());
      whys.push_back(static_cast<int>(why));
    }
    Host().memory_kept_out(Host().context, ProfileId(DirOf(profile_)), request, names.data(),
                           whys.data(), names.size());
  }

  const raw_ptr<Profile> profile_;
  const base::FilePath path_;
  scoped_refptr<base::UpdateableSequencedTaskRunner> runner_;
  base::SequenceBound<MemoryIndex> index_;
  bool open_ = false;
  // The sites kept out, as the file has them, so a page is turned away on the UI thread without
  // waiting on the file.
  std::set<std::string, std::less<>> kept_out_;
  base::ScopedObservation<history::HistoryService, history::HistoryServiceObserver> observation_{
      this};
  base::WeakPtrFactory<Memory> weak_factory_{this};
};

Memory* MemoryOf(bc_id profile) {
  Profile* loaded = ProfileOrLoad(profile);
  return loaded && !loaded->IsOffTheRecord() ? Memory::Of(loaded) : nullptr;
}

// Watches one tab's pages, keeping each once: never on the tab's switch, as kMemoryBehind waits
// past it, and never while it loads.
class MemoryTab : public content::WebContentsObserver,
                  public content::WebContentsUserData<MemoryTab> {
 public:
  MemoryTab(const MemoryTab&) = delete;
  MemoryTab& operator=(const MemoryTab&) = delete;
  ~MemoryTab() override = default;

 private:
  friend class content::WebContentsUserData<MemoryTab>;

  explicit MemoryTab(content::WebContents* contents)
      : content::WebContentsObserver(contents),
        content::WebContentsUserData<MemoryTab>(*contents) {}

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override {
    if (!navigation->IsInPrimaryMainFrame() || !navigation->HasCommitted()) {
      return;
    }
    // A page that changes its URL itself, as a web app's views do, is a page of its own; it
    // keeps the response it came in.
    if (!navigation->IsSameDocument()) {
      const net::HttpResponseHeaders* headers = navigation->GetResponseHeaders();
      // HTTP's "keep no copy": banks send it with signed-in pages.
      no_store_ = navigation->IsErrorPage() ||
                  (headers && headers->HasHeaderValue("cache-control", "no-store"));
    }
    kept_ = false;
    timer_.Stop();
  }

  void DidStopLoading() override { Soon(kMemoryIdle); }

  void OnVisibilityChanged(content::Visibility visibility) override {
    if (visibility == content::Visibility::HIDDEN && !web_contents()->IsLoading()) {
      Soon(kMemoryBehind);
    }
  }

  void Soon(base::TimeDelta delay) {
    if (kept_ || no_store_) {
      return;
    }
    // One timer a page, which fires once: no wakeups while idle after that.
    timer_.Start(FROM_HERE, delay, this, &MemoryTab::Keep);
  }

  void Keep() {
    content::WebContents* contents = web_contents();
    Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
    if (kept_ || no_store_ || profile->IsOffTheRecord() || contents->IsLoading()) {
      return;
    }
    GURL url = contents->GetLastCommittedURL();
    if (!url.SchemeIsHTTPOrHTTPS()) {
      return;
    }
    // An agent's pages are its own reading, not the user's.
    Space* space = Space::Holding(Tab::IdOf(contents));
    if (space && space->agent()) {
      return;
    }
    Memory* memory = Memory::Of(profile);
    std::string site = SiteOf(url);
    if (!memory || !memory->Keeps(site)) {
      return;
    }
    kept_ = true;
    if (std::optional<KeptOut> why = MemorySiteSays(site, {})) {
      memory->KeepOut(site, *why, true);
      return;
    }
    // Blink reads the page's JSON-LD for the browser, without running the page's script.
    metadata_.reset();
    contents->GetPrimaryMainFrame()->GetRemoteInterfaces()->GetInterface(
        metadata_.BindNewPipeAndPassReceiver());
    metadata_->GetEntities(mojo::WrapCallbackWithDefaultInvokeIfNotRun(
        base::BindOnce(&MemoryTab::GotEntities, weak_factory_.GetWeakPtr(), url, site),
        nullptr));
  }

  static void TypesOf(const schema_org::mojom::Entity& entity, int depth,
                      std::vector<std::string>& types) {
    types.push_back(entity.type);
    if (depth > 4) {
      return;
    }
    for (const auto& property : entity.properties) {
      if (property->values && property->values->is_entity_values()) {
        for (const auto& inner : property->values->get_entity_values()) {
          TypesOf(*inner, depth + 1, types);
        }
      }
    }
  }

  void GotEntities(GURL url, std::string site, blink::mojom::WebPagePtr page) {
    metadata_.reset();
    content::WebContents* contents = web_contents();
    if (contents->GetLastCommittedURL() != url) {
      return;
    }
    Memory* memory =
        Memory::Of(Profile::FromBrowserContext(contents->GetBrowserContext()));
    if (!memory) {
      return;
    }
    std::vector<std::string> types;
    if (page) {
      for (const auto& entity : page->entities) {
        TypesOf(*entity, 0, types);
      }
    }
    if (std::optional<KeptOut> why = MemorySiteSays(site, types)) {
      memory->KeepOut(site, *why, true);
      return;
    }
    contents->RequestAXTreeSnapshot(
        base::BindOnce(&MemoryTab::Snapshotted, weak_factory_.GetWeakPtr(), url, site),
        ui::kAXModeWebContentsOnly, kMaxNodes, kSnapshotTimeout,
        content::WebContents::AXTreeSnapshotPolicy::kAll);
  }

  void Snapshotted(GURL url, std::string site, ui::AXTreeUpdate& update) {
    content::WebContents* contents = web_contents();
    if (contents->GetLastCommittedURL() != url) {
      return;
    }
    if (Memory* memory =
            Memory::Of(Profile::FromBrowserContext(contents->GetBrowserContext()))) {
      memory->Keep(std::move(update), ReadableUrls(*contents), std::move(url), std::move(site),
                   base::UTF16ToUTF8(contents->GetTitle()));
    }
  }

  bool kept_ = false;
  bool no_store_ = false;
  base::OneShotTimer timer_;
  mojo::Remote<blink::mojom::DocumentMetadata> metadata_;
  base::WeakPtrFactory<MemoryTab> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(MemoryTab);

}  // namespace

void RegisterMemoryPrefs() {
  PrefsFactory::GetInstance();
}

void SetUpMemory(Profile* profile) {
  if (!profile || profile->IsOffTheRecord() || profile->GetUserData(Memory::kKey)) {
    return;
  }
  std::unique_ptr<base::Environment> environment = base::Environment::Create();
  if (environment->GetVar(kOnForChecks) == "on") {
    profile->GetPrefs()->SetInteger(kMemoryPref, BC_MEMORY_ON);
  }
  profile->SetUserData(Memory::kKey, std::make_unique<Memory>(profile));
}

void WatchForMemory(content::WebContents* contents) {
  MemoryTab::CreateForWebContents(contents);
}

int MemoryState(bc_id profile) {
  Memory* memory = MemoryOf(profile);
  return memory ? memory->State() : BC_MEMORY_UNASKED;
}

void SetMemoryState(bc_id profile, int state) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->SetState(state);
  }
}

void SearchMemory(bc_id profile, uint64_t request, const std::string& query) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->Search(request, query);
  } else if (Host().memory_found) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id profile, uint64_t request) {
                         if (Host().memory_found) {
                           Host().memory_found(Host().context, profile, request, nullptr, 0);
                         }
                       },
                       profile, request));
  }
}

void ForgetMemorySite(bc_id profile, const std::string& site) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->ForgetSite(SiteOf(GURL("https://" + site)));
  }
}

void ForgetMemory(bc_id profile) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->ForgetAll();
  }
}

void KeepOutOfMemory(bc_id profile, const std::string& site, bool out) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->KeepOut(SiteOf(GURL("https://" + site)), KeptOut::kUser, out);
  }
}

void ReadMemoryKeptOut(bc_id profile, uint64_t request) {
  if (Memory* memory = MemoryOf(profile)) {
    memory->ReadKeptOut(request);
  }
}

}  // namespace browser_core
