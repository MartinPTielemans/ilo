#include "browser_core/profiles.h"

#include <algorithm>
#include <map>
#include <memory>
#include <set>
#include <utility>
#include <vector>

#include "base/barrier_closure.h"
#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/functional/callback_helpers.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/auto_archive.h"
#include "browser_core/downloads.h"
#include "browser_core/extensions.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/search.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
#include "browser_core/system_engine.h"
#include "browser_core/window.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/content_settings/host_content_settings_map_factory.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/delete_profile_helper.h"
#include "chrome/browser/profiles/keep_alive/profile_keep_alive_types.h"
#include "chrome/browser/profiles/keep_alive/scoped_profile_keep_alive.h"
#include "chrome/browser/profiles/nuke_profile_directory_utils.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_attributes_entry.h"
#include "chrome/browser/profiles/profile_attributes_storage.h"
#include "chrome/browser/profiles/profile_destroyer.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/browser/profiles/profile_metrics.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "chrome/browser/ui/browser_window/public/browser_collection.h"
#include "chrome/browser/ui/browser_window/public/browser_collection_observer.h"
#include "chrome/browser/ui/browser_window/public/browser_window_interface.h"
#include "chrome/browser/ui/browser_window/public/desktop_browser_window_capabilities.h"
#include "chrome/browser/ui/browser_window/public/profile_browser_collection.h"
#include "chrome/browser/ui/tabs/tab_strip_model.h"
#include "chrome/browser/ui/tabs/tab_strip_model_observer.h"
#include "components/content_settings/core/browser/host_content_settings_map.h"
#include "components/content_settings/core/common/content_settings.h"
#include "components/content_settings/core/common/content_settings_types.h"
#include "content/public/browser/web_contents.h"
#include "ui/base/base_window.h"

namespace browser_core {

namespace {

// The profiles the core keeps loaded, by directory.
std::map<std::string, std::unique_ptr<ScopedProfileKeepAlive>>& Used() {
  static base::NoDestructor<std::map<std::string, std::unique_ptr<ScopedProfileKeepAlive>>> used;
  return *used;
}

// Each profile's id, by directory, and back.
std::map<std::string, bc_id>& Ids() {
  static base::NoDestructor<std::map<std::string, bc_id>> ids;
  return *ids;
}

base::FilePath PathOf(const std::string& dir) {
  ProfileManager* manager = g_browser_process->profile_manager();
  return manager ? manager->user_data_dir().AppendASCII(dir) : base::FilePath();
}

// Tells the app, after the call that changed them has returned, as the Spaces do.
void ReportChanged() {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce([] {
        if (Host().spaces_changed) {
          Host().spaces_changed(Host().context);
        }
      }));
}

// Keeps |profile| once it has loaded, if it did, then runs |loaded|.
void OnLoaded(base::OnceClosure loaded, Profile* profile) {
  if (profile) {
    UseProfile(profile);
  }
  std::move(loaded).Run();
}

// Tells the app once a private session ends, as Chrome destroys its off-the-record profile. Hears
// each of the session's Browsers go, too, as Chrome deletes it, so the core can end the session
// once the last has gone if Chrome didn't.
class SessionEnd : public ProfileObserver,
                   public BrowserCollectionObserver,
                   public TabStripModelObserver {
 public:
  explicit SessionEnd(Profile* profile) : profile_(profile) {
    observation_.Observe(profile);
    if (ProfileBrowserCollection* browsers = ProfileBrowserCollection::GetForProfile(profile)) {
      browsers_.Observe(browsers);
    }
  }

  // Whether a Browser of the session has closed and Chrome is still to delete it.
  bool BrowserClosing() { return TabStripModelObserver::IsObservingAny(this); }

  // ProfileObserver:
  void OnProfileWillBeDestroyed(Profile* profile) override;

  // BrowserCollectionObserver: Chrome deletes a Browser that has closed in a task it posts next,
  // and its tab strip goes with it.
  void OnBrowserClosed(BrowserWindowInterface* browser) override {
    browser->GetTabStripModel()->AddObserver(this);
  }

  // TabStripModelObserver:
  void OnTabStripModelDestroyed(TabStripModel* tab_strip_model) override;

 private:
  raw_ptr<Profile> profile_;
  base::ScopedObservation<Profile, ProfileObserver> observation_{this};
  base::ScopedObservation<ProfileBrowserCollection, BrowserCollectionObserver> browsers_{this};
};

// The private sessions running, by their off-the-record profile.
std::map<Profile*, std::unique_ptr<SessionEnd>>& Sessions() {
  static base::NoDestructor<std::map<Profile*, std::unique_ptr<SessionEnd>>> sessions;
  return *sessions;
}

// The off-the-record profiles of sessions that have ended, which Chrome destroys once their pages'
// processes have gone, up to a second later, and hands out again until then.
std::vector<base::WeakPtr<Profile>>& Ended() {
  static base::NoDestructor<std::vector<base::WeakPtr<Profile>>> ended;
  return *ended;
}

// Whether |off_the_record|'s session has ended, and Chrome is still to destroy it.
bool HasEnded(Profile* off_the_record) {
  return std::ranges::any_of(Ended(), [off_the_record](const base::WeakPtr<Profile>& each) {
    return each.get() == off_the_record;
  });
}

// The profiles whose private session has started and whose end the app hasn't heard.
std::set<bc_id>& Unheard() {
  static base::NoDestructor<std::set<bc_id>> unheard;
  return *unheard;
}

// Tells the app |profile|'s private session ended, once: Chrome destroying its off-the-record
// profile and the core finding it gone can come in either order. |how| says which, for the log.
void ReportEnded(bc_id profile, const char* how) {
  if (!Unheard().erase(profile)) {
    return;
  }
  // A quit ends them all, and the app has stopped hearing.
  if (browser_shutdown::IsTryingToQuit()) {
    LOG(ERROR) << "browser: private session over in the quit, as " << how;
    return;
  }
  LOG(ERROR) << "browser: private session over, as " << how;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id profile) {
                       if (Host().private_session_ended) {
                         Host().private_session_ended(Host().context, profile);
                       }
                     },
                     profile));
}

// Whether a private window of |profile|'s is open, or closing.
bool PrivateWindowOpen(Profile* profile) {
  return std::ranges::any_of(Window::All(), [dir = DirOf(profile)](const Window* window) {
    const Space* space = Space::FromId(window->space());
    return window->off_the_record() && space && space->profile() == dir;
  });
}

void SessionEnd::OnProfileWillBeDestroyed(Profile* profile) {
  const bc_id id = ProfileId(DirOf(profile));
  std::erase_if(Ended(), [](const base::WeakPtr<Profile>& each) { return !each; });
  Ended().push_back(profile->GetWeakPtr());
  // Deletes this.
  Sessions().erase(profile);
  ReportEnded(id, "Chrome destroyed its profile");
}

void SessionEnd::OnTabStripModelDestroyed(TabStripModel* tab_strip_model) {
  // Posted: the Browser is part way through its destructor, and once it's done Chrome ends the
  // session itself if that was the last. Gone, the profile was destroyed and the session ended.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Profile> off_the_record) {
                       if (off_the_record) {
                         EndPrivateSession(off_the_record->GetOriginalProfile());
                       }
                     },
                     profile_->GetWeakPtr()));
}

// Chrome numbers the directories it makes in order: "Default", then "Profile 1", "Profile 2".
int Order(const std::string& dir) {
  int number = 0;
  if (dir == "Default" || !base::StringToInt(dir.substr(dir.rfind(' ') + 1), &number)) {
    return dir == "Default" ? -1 : 0;
  }
  return number;
}

// What Chrome keeps about |profile|, such as its name, or null.
ProfileAttributesEntry* AttributesOf(bc_id profile) {
  ProfileManager* manager = g_browser_process->profile_manager();
  const std::string dir = ProfileDir(profile);
  return manager && !dir.empty()
             ? manager->GetProfileAttributesStorage().GetProfileAttributesWithPath(PathOf(dir))
             : nullptr;
}

}  // namespace

Profile* StartProfile() {
  // Chrome made the profile it starts with before the core's calls work. Its last used changes as
  // other profiles' Browsers become active, and this one doesn't.
  static Profile* start = nullptr;
  if (!start) {
    start = ProfileManager::GetLastUsedProfileIfLoaded();
  }
  return start;
}

Profile* ProfileFor(const std::string& dir) {
  ProfileManager* manager = g_browser_process->profile_manager();
  Profile* profile = manager && !dir.empty() ? manager->GetProfileByPath(PathOf(dir)) : nullptr;
  return profile ? profile : StartProfile();
}

std::string DirOf(Profile* profile) {
  return profile ? profile->GetBaseName().AsUTF8Unsafe() : std::string();
}

std::string DirOf(content::WebContents* contents) {
  return contents ? DirOf(Profile::FromBrowserContext(contents->GetBrowserContext()))
                  : std::string();
}

void LoadProfiles(const std::set<std::string>& dirs, base::OnceClosure done) {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    std::move(done).Run();
    return;
  }
  base::RepeatingClosure loaded = base::BarrierClosure(dirs.size(), std::move(done));
  for (const std::string& dir : dirs) {
    // Waits for one still being made. One deleted by hand comes back empty.
    manager->CreateProfileAsync(PathOf(dir), base::BindOnce(&OnLoaded, loaded));
  }
}

void UseProfile(Profile* profile) {
  std::unique_ptr<ScopedProfileKeepAlive>& kept = Used()[DirOf(profile)];
  if (kept) {
    return;
  }
  // Chrome unloads a profile once its last Browser closes, and loading it again would hold up the
  // next new tab, so the core keeps the ones its Spaces use.
  kept = std::make_unique<ScopedProfileKeepAlive>(profile, ProfileKeepAliveOrigin::kAppWindow);
  SetUpSearch(profile);
  GiveArchiveDefault(profile);
  ObserveDownloads(profile);
  ObserveExtensions(profile);
  ApplyAutoPictureInPicture(/*changed=*/false);
}

Profile* ProfileOrLoad(bc_id id) {
  const std::string dir = ProfileDir(id);
  // ProfileFor gives the first profile for one that isn't loaded.
  Profile* profile = dir.empty() ? nullptr : ProfileFor(dir);
  if (profile && DirOf(profile) == dir) {
    return profile;
  }
  // Once: a profile Chrome can't load is asked about each time Settings draws.
  static base::NoDestructor<std::set<std::string>> asked;
  if (!dir.empty() && asked->insert(dir).second) {
    LoadProfiles({dir}, base::BindOnce(&ReportChanged));
  }
  return nullptr;
}

void ReleaseProfiles() {
  Used().clear();
}

void ApplyAutoPictureInPicture(bool changed) {
  ProfileManager* manager = g_browser_process->profile_manager();
  const bool on = UserSettings().auto_picture_in_picture;
  if (!manager || (on && !changed)) {
    return;
  }
  // A private window's profile takes its profile's default.
  for (Profile* profile : manager->GetLoadedProfiles()) {
    HostContentSettingsMapFactory::GetForProfile(profile)->SetDefaultContentSetting(
        ContentSettingsType::AUTO_PICTURE_IN_PICTURE,
        on ? CONTENT_SETTING_DEFAULT : CONTENT_SETTING_BLOCK);
  }
}

Profile* StartPrivateSession(Profile* profile) {
  // A session that has ended is gone at once, so the new window's starts without its cookies, and
  // Chrome doesn't destroy the profile later under the new window's Browser. Not while the window
  // that ended it is still closing, as it holds things of the profile until it's gone.
  if (Profile* ending = profile->GetPrimaryOTRProfile(/*create_if_needed=*/false);
      ending && !PrivateWindowOpen(profile) && HasEnded(ending)) {
    ProfileDestroyer::DestroyOTRProfileImmediately(ending);
  }
  Profile* off_the_record = profile->GetPrimaryOTRProfile(/*create_if_needed=*/true);
  if (off_the_record && !Sessions().contains(off_the_record)) {
    Sessions()[off_the_record] = std::make_unique<SessionEnd>(off_the_record);
    Unheard().insert(ProfileId(DirOf(profile)));
  }
  return off_the_record;
}

Profile* PrivateProfileOf(Profile* profile) {
  Profile* off_the_record = profile->GetPrimaryOTRProfile(/*create_if_needed=*/false);
  return off_the_record && !HasEnded(off_the_record) ? off_the_record : nullptr;
}

void EndPrivateSession(Profile* profile) {
  // Runs as each private window and each of the session's Browsers goes, and every way out says
  // which it took, so a session that never ends says why. With its off-the-record profile gone,
  // or going, the session has ended.
  Profile* off_the_record = profile->GetPrimaryOTRProfile(/*create_if_needed=*/false);
  if (!off_the_record) {
    LOG(ERROR) << "browser: private session to end has no off-the-record profile";
    ReportEnded(ProfileId(DirOf(profile)), "its off-the-record profile has gone");
    return;
  }
  auto session = Sessions().find(off_the_record);
  if (session == Sessions().end()) {
    LOG(ERROR) << "browser: private session to end has ended";
    ReportEnded(ProfileId(DirOf(profile)), "its off-the-record profile is going");
    return;
  }
  if (PrivateWindowOpen(profile)) {
    LOG(ERROR) << "browser: private session to end has a private window open";
    return;
  }
  if (browser_shutdown::IsTryingToQuit() || browser_shutdown::HasShutdownStarted()) {
    LOG(ERROR) << "browser: private session to end is left to the quit";
    return;
  }
  // Chrome ends it as it deletes the last, and if it doesn't, that one's going runs this again.
  if (session->second->BrowserClosing()) {
    LOG(ERROR) << "browser: private session to end waits for a closed Browser to go";
    return;
  }
  // A Browser something else made in the session keeps it. Closing it ends the session, as Chrome
  // ends it once the last one has gone, or its going runs this again. The log names it, for
  // check.sh. Listed first, since closing one changes the collection.
  std::vector<BrowserWindowInterface*> kept;
  if (ProfileBrowserCollection* browsers =
          ProfileBrowserCollection::GetForProfile(off_the_record)) {
    browsers->ForEach([&kept](BrowserWindowInterface* browser) {
      kept.push_back(browser);
      return true;
    });
  }
  if (!kept.empty()) {
    for (BrowserWindowInterface* browser : kept) {
      LOG(ERROR) << "browser: private session outlived its windows, kept by a Browser of type "
                 << static_cast<int>(browser->GetType()) << " with "
                 << browser->GetTabStripModel()->count() << " tabs, "
                 << (Window::ForBrowser(browser) ? "the core's" : "not the core's");
      if (!browser->capabilities()->IsAttemptingToCloseBrowser()) {
        browser->GetWindow()->Close();
      }
    }
    return;
  }
  // Its windows and Browsers have all gone and Chrome didn't end it.
  LOG(ERROR) << "browser: private session outlived its windows and Browsers";
  ProfileDestroyer::DestroyOTRProfileWhenAppropriate(off_the_record);
}

std::vector<bc_id> Profiles() {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return {};
  }
  std::vector<std::string> dirs;
  for (ProfileAttributesEntry* entry :
       manager->GetProfileAttributesStorage().GetAllProfilesAttributes()) {
    if (!entry->IsOmitted() && !entry->IsEphemeral() &&
        !IsProfileDirectoryMarkedForDeletion(entry->GetPath())) {
      dirs.push_back(entry->GetPath().BaseName().AsUTF8Unsafe());
    }
  }
  std::ranges::sort(dirs, {}, &Order);
  // The one the core started with first: it can't be deleted.
  std::ranges::stable_partition(dirs,
                                [](const std::string& dir) { return dir == DirOf(StartProfile()); });
  std::vector<bc_id> profiles;
  for (const std::string& dir : dirs) {
    profiles.push_back(ProfileId(dir));
  }
  return profiles;
}

bc_id ProfileId(const std::string& dir) {
  auto [it, made] = Ids().try_emplace(dir, 0);
  if (made) {
    it->second = NewId();
  }
  return it->second;
}

std::string ProfileDir(bc_id profile) {
  for (const auto& [dir, id] : Ids()) {
    if (id == profile) {
      return dir;
    }
  }
  return std::string();
}

std::string ProfileName(bc_id profile) {
  ProfileAttributesEntry* entry = AttributesOf(profile);
  if (!entry) {
    return std::string();
  }
  // Chrome's name for a profile no one named, "Your Chromium", names the browser, not the profile.
  return entry->IsUsingDefaultName() ? "Default" : base::UTF16ToUTF8(entry->GetLocalProfileName());
}

bool RenameProfile(bc_id profile, const std::string& name) {
  ProfileAttributesEntry* entry = AttributesOf(profile);
  if (!entry || name.empty()) {
    return false;
  }
  entry->SetLocalProfileName(base::UTF8ToUTF16(name), /*is_default_name=*/false);
  ReportChanged();
  return true;
}

bc_id CreateProfile(const std::string& name, base::OnceCallback<void(Profile*)> made) {
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return 0;
  }
  const base::FilePath path = manager->GenerateNextProfileDirectoryPath();
  manager->CreateProfileAsync(
      path, base::BindOnce(
                [](std::u16string name, base::OnceCallback<void(Profile*)> made,
                   Profile* profile) {
                  if (profile) {
                    if (ProfileAttributesEntry* entry =
                            g_browser_process->profile_manager()
                                ->GetProfileAttributesStorage()
                                .GetProfileAttributesWithPath(profile->GetPath())) {
                      entry->SetLocalProfileName(name, /*is_default_name=*/false);
                    }
                    UseProfile(profile);
                  }
                  ReportChanged();
                  std::move(made).Run(profile);
                },
                base::UTF8ToUTF16(name), std::move(made)));
  return ProfileId(path.BaseName().AsUTF8Unsafe());
}

// Chrome deletes the profile at |path| once it's unloaded.
static void ScheduleDeletion(const base::FilePath& path) {
  if (ProfileManager* manager = g_browser_process->profile_manager()) {
    manager->GetDeleteProfileHelper().MaybeScheduleProfileForDeletion(
        path, base::DoNothing(), ProfileMetrics::DELETE_PROFILE_SETTINGS);
  }
}

bool DeleteProfile(bc_id profile) {
  ProfileManager* manager = g_browser_process->profile_manager();
  const std::string dir = ProfileDir(profile);
  if (!manager || dir.empty() || dir == DirOf(StartProfile()) || Profiles().size() < 2) {
    return false;
  }
  for (Space* space : Space::All()) {
    if (space->profile() == dir) {
      return false;
    }
  }
  if (!Favorites::Of(dir).empty()) {
    return false;
  }
  // Nor while a private window of it is open.
  if (Profile* loaded = ProfileFor(dir);
      DirOf(loaded) == dir && loaded->HasPrimaryOTRProfile()) {
    return false;
  }
  // Let go first: Chrome deletes it once it's unloaded, and a window may have built on it.
  if (Profile* loaded = ProfileFor(dir); DirOf(loaded) == dir) {
    Window::LetProfileGo(loaded);
  }
  Used().erase(dir);
  // The system's engine keeps the profile's cookies apart, and no tab of it is open. Whether it
  // has a store is read from the profile before Chrome deletes it.
  system_engine::DeleteStore(PathOf(dir), base::BindOnce(&ScheduleDeletion, PathOf(dir)));
  Ids().erase(dir);
  ReportChanged();
  return true;
}

}  // namespace browser_core
