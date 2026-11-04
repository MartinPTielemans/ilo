#include "browser_core/downloads.h"

#include <algorithm>
#include <functional>
#include <ios>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/supports_user_data.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/task/single_thread_task_runner.h"
#include "base/values.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/download/download_crx_util.h"
#include "chrome/browser/download/download_item_model.h"
#include "chrome/browser/download/download_prefs.h"
#include "chrome/browser/icon_loader.h"
#include "chrome/browser/icon_manager.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/common/pref_names.h"
#include "components/download/content/public/all_download_item_notifier.h"
#include "components/download/public/common/download_item.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/download_item_utils.h"
#include "content/public/browser/download_manager.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image.h"
#include "ui/gfx/image/image_skia.h"
#include "ui/gfx/image/image_skia_rep.h"

namespace browser_core {

namespace {

// Ours: Arc's limit isn't known. Chrome's history keeps downloads without end.
constexpr size_t kMaxListed = 500;

// The id the app knows a download by, kept on it.
struct DownloadId : base::SupportsUserData::Data {
  static constexpr char kKey[] = "browser_core::DownloadId";
  bc_id id = NewId();
};

bc_id IdOf(download::DownloadItem* item) {
  auto* data = static_cast<DownloadId*>(item->GetUserData(DownloadId::kKey));
  if (!data) {
    auto id = std::make_unique<DownloadId>();
    data = id.get();
    item->SetUserData(DownloadId::kKey, std::move(id));
  }
  return data->id;
}

// The profile a download is of, its private session's for a private one.
Profile* ProfileOf(download::DownloadItem* item) {
  return Profile::FromBrowserContext(content::DownloadItemUtils::GetBrowserContext(item));
}

int StateOf(const download::DownloadItem& item) {
  switch (item.GetState()) {
    case download::DownloadItem::IN_PROGRESS:
      return item.IsPaused() ? BC_DOWNLOAD_PAUSED : BC_DOWNLOAD_RUNNING;
    case download::DownloadItem::COMPLETE:
      return BC_DOWNLOAD_DONE;
    case download::DownloadItem::CANCELLED:
      return BC_DOWNLOAD_CANCELLED;
    default:
      return BC_DOWNLOAD_FAILED;
  }
}

class Downloads : public download::AllDownloadItemNotifier::Observer {
 public:
  // Adds |profile|'s downloads to the list, or its private session's, once.
  void Observe(Profile* profile) {
    content::DownloadManager* manager = profile->GetDownloadManager();
    if (std::ranges::any_of(notifiers_,
                            [manager](const auto& each) { return each->GetManager() == manager; })) {
      return;
    }
    // Made here, as it tells of a manager that has read the history at once.
    notifiers_.push_back(
        std::make_unique<download::AllDownloadItemNotifier>(manager, this));
    Changed();
  }

  // The ones Chrome's downloads page lists, of every profile the core uses and of their private
  // sessions while they run, newest first.
  std::vector<download::DownloadItem*> Listed() {
    std::vector<download::DownloadItem*> listed;
    for (const auto& notifier : notifiers_) {
      content::DownloadManager* manager = notifier->GetManager();
      if (!manager) {
        continue;
      }
      Profile* profile = Profile::FromBrowserContext(manager->GetBrowserContext());
      content::DownloadManager::DownloadVector all;
      manager->GetAllDownloads(&all);
      for (download::DownloadItem* item : all) {
        if (!download_crx_util::IsTrustedExtensionDownload(profile, *item) &&
            !item->IsTemporary() && !item->IsTransient() &&
            !item->GetFileNameToReportUser().empty() && !item->GetTargetFilePath().empty() &&
            !item->GetURL().is_empty() && DownloadItemModel(item).ShouldShowInUi()) {
          listed.push_back(item);
        }
      }
    }
    std::ranges::sort(listed, std::ranges::greater(), &download::DownloadItem::GetStartTime);
    if (listed.size() > kMaxListed) {
      listed.resize(kMaxListed);
    }
    return listed;
  }

  download::DownloadItem* Find(bc_id id) {
    for (download::DownloadItem* item : Listed()) {
      if (IdOf(item) == id) {
        return item;
      }
    }
    return nullptr;
  }

 private:
  // download::AllDownloadItemNotifier::Observer:
  void OnManagerInitialized(content::DownloadManager* manager) override { Changed(); }
  // A deleted profile's downloads leave the list.
  void OnManagerGoingDown(content::DownloadManager* manager) override { Changed(); }
  void OnDownloadCreated(content::DownloadManager* manager,
                         download::DownloadItem* item) override {
    // What Chrome decides a private session's download by, as a Save File dialog can come of it.
    if (Profile* profile = ProfileOf(item); profile && profile->IsOffTheRecord()) {
      DownloadPrefs* prefs = DownloadPrefs::FromDownloadManager(manager);
      LOG(ERROR) << "browser: private download created, folder "
                 << (prefs ? prefs->DownloadPath().AsUTF8Unsafe() : std::string("none"))
                 << ", asks " << std::boolalpha << (prefs && prefs->PromptForDownload())
                 << ", disposition " << static_cast<int>(item->GetTargetDisposition());
    }
    Changed();
  }
  void OnDownloadUpdated(content::DownloadManager* manager,
                         download::DownloadItem* item) override {
    Changed();
  }
  void OnDownloadRemoved(content::DownloadManager* manager,
                         download::DownloadItem* item) override {
    Changed();
  }

  // Tells the app once, after the call that changed them has returned. A running download changes
  // about twice a second.
  void Changed() {
    if (report_posted_) {
      return;
    }
    report_posted_ = true;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Downloads::Report, weak_factory_.GetWeakPtr()));
  }

  void Report() {
    report_posted_ = false;
    if (!Host().downloads_changed) {
      return;
    }
    const std::vector<download::DownloadItem*> items = Listed();
    // Each download's name, URL and path, kept until the call returns.
    std::vector<std::string> text;
    text.reserve(items.size() * 3);
    std::vector<bc_download> downloads;
    downloads.reserve(items.size());
    for (download::DownloadItem* item : items) {
      Profile* profile = ProfileOf(item);
      const std::string& name = text.emplace_back(item->GetFileNameToReportUser().AsUTF8Unsafe());
      const std::string& url = text.emplace_back(item->GetURL().spec());
      const std::string& path = text.emplace_back(item->GetTargetFilePath().AsUTF8Unsafe());
      downloads.push_back({.id = IdOf(item),
                           .name = name.c_str(),
                           .url = url.c_str(),
                           .path = path.c_str(),
                           .received = item->GetReceivedBytes(),
                           .total = item->GetTotalBytes(),
                           .state = StateOf(*item),
                           .open_when_done = item->GetOpenWhenComplete(),
                           .held = item->IsDangerous() || item->IsInsecure(),
                           .started = item->GetStartTime().InMillisecondsFSinceUnixEpoch(),
                           .profile = profile ? ProfileId(DirOf(profile->GetOriginalProfile())) : 0,
                           .off_the_record = profile && profile->IsOffTheRecord()});
    }
    Host().downloads_changed(Host().context, downloads.data(), downloads.size());
  }

  std::vector<std::unique_ptr<download::AllDownloadItemNotifier>> notifiers_;
  bool report_posted_ = false;
  base::WeakPtrFactory<Downloads> weak_factory_{this};
};

Downloads& TheDownloads() {
  static base::NoDestructor<Downloads> downloads;
  return *downloads;
}

download::DownloadItem* Find(bc_id id) {
  return TheDownloads().Find(id);
}

void ReportIcon(bc_id id, gfx::Image icon) {
  if (!Host().download_icon) {
    return;
  }
  std::optional<std::vector<uint8_t>> png;
  if (!icon.IsEmpty()) {
    png = gfx::PNGCodec::EncodeBGRASkBitmap(icon.AsImageSkia().GetRepresentation(2).GetBitmap(),
                                            /*discard_transparency=*/false);
  }
  Host().download_icon(Host().context, id, png ? png->data() : nullptr, png ? png->size() : 0);
}

// Builds no service: Chrome registers a profile's prefs through its factories, after its own.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreDownloads") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  // ungoogled-chromium asks where each download goes by default; Arc doesn't. A profile whose user
  // turned asking on in chrome://settings keeps it, as that's saved and a default isn't.
  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    registry->SetDefaultPrefValue(prefs::kPromptForDownload, base::Value(false));
  }
};

}  // namespace

void RegisterDownloadPrefs() {
  PrefsFactory::GetInstance();
}

void ObserveDownloads(Profile* profile) {
  // A private session saw none of its profile's download settings, so it asked where to save,
  // ungoogled-chromium's default, and offered the system's Downloads folder, where a regular
  // window saves to the profile's folder at once. It takes them from its profile as it starts.
  // Neither is a pref an off-the-record session writes through, so they stay in its memory and
  // go with it, and a profile whose user turned asking on still asks.
  if (profile->IsOffTheRecord()) {
    PrefService* original = profile->GetOriginalProfile()->GetPrefs();
    PrefService* session = profile->GetPrefs();
    session->SetBoolean(prefs::kPromptForDownload, original->GetBoolean(prefs::kPromptForDownload));
    session->SetFilePath(prefs::kDownloadDefaultDirectory,
                         original->GetFilePath(prefs::kDownloadDefaultDirectory));
    // check.sh waits for this line.
    LOG(ERROR) << "browser: private downloads observed, asking " << std::boolalpha
               << session->GetBoolean(prefs::kPromptForDownload);
  }
  TheDownloads().Observe(profile);
}

void ActOnDownload(bc_id id, int action) {
  download::DownloadItem* item = Find(id);
  if (!item) {
    return;
  }
  switch (action) {
    case BC_DOWNLOAD_OPEN:
      // A download still running opens once it's done, or no longer.
      item->OpenDownload();
      break;
    case BC_DOWNLOAD_SHOW:
      item->ShowDownloadInShell();
      break;
    case BC_DOWNLOAD_PAUSE:
      item->Pause();
      break;
    case BC_DOWNLOAD_RESUME:
      item->Resume(/*user_resume=*/true);
      break;
    case BC_DOWNLOAD_CANCEL:
      item->Cancel(/*user_cancel=*/true);
      break;
    case BC_DOWNLOAD_REMOVE:
      item->Remove();
      break;
    case BC_DOWNLOAD_KEEP:
      if (item->IsInsecure()) {
        item->ValidateInsecureDownload();
      } else if (item->IsDangerous()) {
        item->ValidateDangerousDownload();
      }
      break;
  }
}

void ClearDownloads(bc_id profile, bool off_the_record) {
  for (download::DownloadItem* item : TheDownloads().Listed()) {
    Profile* of = ProfileOf(item);
    const bool session = of && of->IsOffTheRecord();
    if (session != off_the_record ||
        (off_the_record && ProfileId(DirOf(of->GetOriginalProfile())) != profile)) {
      continue;
    }
    if (item->GetState() != download::DownloadItem::IN_PROGRESS) {
      item->Remove();
    }
  }
}

void FindDownloadIcon(bc_id id) {
  download::DownloadItem* item = Find(id);
  IconManager* icons = g_browser_process->icon_manager();
  if (!item || !icons) {
    return;
  }
  static base::NoDestructor<base::CancelableTaskTracker> tracker;
  // 16 points at 2×, as favicons are.
  icons->LoadIcon(item->GetTargetFilePath(), IconLoader::SMALL, 2,
                  base::BindOnce(&ReportIcon, id), tracker.get());
}

}  // namespace browser_core
