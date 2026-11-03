#include "browser_core/auto_archive.h"

#include <algorithm>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/command_line.h"
#include "base/functional/bind.h"
#include "base/functional/function_ref.h"
#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/wall_clock_timer.h"
#include "browser_core/profiles.h"
#include "browser_core/host.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

// For the app's check, which can't wait 12 hours. It wins over the user's settings.
constexpr char kArchiveAfterSwitch[] = "archive-tabs-after";
// As kArchiveAfterSwitch, for Little ilo windows, which wait 6 hours.
constexpr char kLittleArchiveAfterSwitch[] = "archive-little-after";

// The profile's own time, in seconds, which it takes from Settings' default the first time the
// core uses it once the Spaces are read.
constexpr char kArchiveAfter[] = "browser_core.archive_after";

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreAutoArchive") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    registry->RegisterDoublePref(kArchiveAfter, 0);
  }
};

// Whether the Spaces, and with them the default, have been read.
bool g_default_read = false;

// The seconds the switch |name| gives, if it gives a positive number.
std::optional<base::TimeDelta> Given(const char* name) {
  int seconds = 0;
  const std::string value = base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(name);
  if (base::StringToInt(value, &seconds) && seconds > 0) {
    return base::Seconds(seconds);
  }
  return std::nullopt;
}

// |space|'s time: its profile's own, or for one not loaded, the profile its tabs open in.
base::TimeDelta ArchiveAfter(const Space& space) {
  static const std::optional<base::TimeDelta> given = Given(kArchiveAfterSwitch);
  return given.value_or(ProfileArchiveAfter(ProfileFor(space.profile())));
}

// How long a Little ilo window that isn't active stays open (bc_little_archive_after).
base::TimeDelta LittleArchiveAfter() {
  static const std::optional<base::TimeDelta> given = Given(kLittleArchiveAfterSwitch);
  return given.value_or(UserSettings().little_archive_after);
}

// Each Little ilo window that isn't the active window, nor closing, with when it's due.
void ForEachUnusedLittle(base::FunctionRef<void(Window*, base::Time)> each) {
  const base::TimeDelta after = LittleArchiveAfter();
  for (Window* window : Window::All()) {
    if (window->little() && !window->active() && !window->closing()) {
      each(window, window->inactive_since() + after);
    }
  }
}

base::WallClockTimer& Timer() {
  static base::NoDestructor<base::WallClockTimer> timer;
  return *timer;
}

bool g_schedule_posted = false;

// Each Today tab no window shows, with its page or without.
void ForEachUnseen(base::FunctionRef<void(bc_id, Tab*, base::TimeDelta)> each) {
  for (Space* space : Space::All()) {
    const base::TimeDelta after = ArchiveAfter(*space);
    // A copy, as closing a tab takes it out.
    for (bc_id id : std::vector<bc_id>(space->today())) {
      Tab* tab = Tab::Get(id);
      if (tab && !Window::Showing(id)) {
        each(id, tab, after);
      }
    }
  }
}

void Schedule();

// Whether auto-archive has taken tabs since the launch, for the note on them.
bool g_archived = false;

void NoteChanged() {
  if (Host().archive_note_changed) {
    Host().archive_note_changed(Host().context);
  }
}

void ArchiveDue() {
  if (browser_shutdown::IsTryingToQuit()) {
    return;
  }
  const base::Time now = base::Time::Now();
  std::vector<bc_id> due;
  ForEachUnseen([&](bc_id id, Tab* tab, base::TimeDelta after) {
    if (tab->last_seen() + after > now) {
      return;
    }
    // Seen now, so one playing sound, or one its page keeps open, waits another interval.
    tab->set_last_seen(now);
    if (!tab->web_contents() || !tab->web_contents()->IsCurrentlyAudible()) {
      due.push_back(id);
    }
  });
  for (bc_id id : due) {
    Window::CloseTab(id);
  }
  // A Little ilo window closes into the archive with its tab (KeepClosed), as Arc's Little Arc
  // windows do, but for one whose page plays sound, which waits another interval.
  std::vector<bc_id> little;
  ForEachUnusedLittle([&](Window* window, base::Time at) {
    if (at > now) {
      return;
    }
    content::WebContents* page = Tab::FromId(window->selected());
    if (page && page->IsCurrentlyAudible()) {
      window->set_inactive_since(now);
    } else {
      little.push_back(window->id());
    }
  });
  for (bc_id id : little) {
    if (Window* window = Window::FromId(id)) {
      window->Close();
    }
  }
  if (!due.empty() && !g_archived) {
    g_archived = true;
    if (!UserSettings().archive_noted) {
      NoteChanged();
    }
  }
  Schedule();
}

void Schedule() {
  g_schedule_posted = false;
  std::optional<base::Time> next;
  ForEachUnseen([&](bc_id, Tab* tab, base::TimeDelta after) {
    const base::Time due = tab->last_seen() + after;
    next = next ? std::min(*next, due) : due;
  });
  ForEachUnusedLittle([&](Window*, base::Time due) { next = next ? std::min(*next, due) : due; });
  if (next) {
    Timer().Start(FROM_HERE, *next, base::BindOnce(&ArchiveDue));
  } else {
    Timer().Stop();
  }
}

}  // namespace

void RegisterArchivePrefs() {
  PrefsFactory::GetInstance();
}

void GiveArchiveDefault(Profile* profile) {
  PrefService* prefs = profile->GetOriginalProfile()->GetPrefs();
  if (g_default_read && !prefs->HasPrefPath(kArchiveAfter)) {
    prefs->SetDouble(kArchiveAfter, UserSettings().archive_after.InSecondsF());
  }
}

void ArchiveDefaultRead() {
  g_default_read = true;
  // The profile the browser started in loaded before the Spaces were read.
  for (Profile* profile : g_browser_process->profile_manager()->GetLoadedProfiles()) {
    GiveArchiveDefault(profile);
  }
}

base::TimeDelta ProfileArchiveAfter(Profile* profile) {
  const double seconds =
      profile ? profile->GetOriginalProfile()->GetPrefs()->GetDouble(kArchiveAfter) : 0;
  return seconds > 0 ? base::Seconds(seconds) : UserSettings().archive_after;
}

bool SetProfileArchiveAfter(Profile* profile, base::TimeDelta after) {
  if (!profile || !after.is_positive()) {
    return false;
  }
  profile->GetOriginalProfile()->GetPrefs()->SetDouble(kArchiveAfter, after.InSecondsF());
  ScheduleAutoArchive();
  return true;
}

bool ArchiveNoteShown() {
  return g_archived && !UserSettings().archive_noted;
}

void DismissArchiveNote() {
  if (!ArchiveNoteShown()) {
    return;
  }
  UserSettings().archive_noted = true;
  SessionChanged();
  NoteChanged();
}

void ScheduleAutoArchive() {
  if (g_schedule_posted) {
    return;
  }
  g_schedule_posted = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, base::BindOnce(&Schedule));
}

}  // namespace browser_core
