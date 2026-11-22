#include "browser_core/profile_reads.h"

#include <map>
#include <memory>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/saved_read.h"
#include "browser_core/saved_read_rule.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "content/public/browser/storage_partition.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Tells the app how |read| ended. The log line, for the checks, names no address and no body;
// outside a check the app quiets error lines (launch.rs's quiet_log_level).
void Report(uint64_t read, const SavedReadAnswer& answer) {
  // Past here the app may have let go of what it waited with.
  if (browser_shutdown::HasShutdownStarted()) {
    return;
  }
  if (!answer.redirect.empty()) {
    LOG(ERROR) << "browser: saved read " << read << " stopped at a redirect to another origin";
  } else if (!answer.error.empty()) {
    LOG(ERROR) << "browser: saved read " << read << " ended: " << answer.error;
  } else {
    LOG(ERROR) << "browser: saved read " << read << " answered " << answer.status
               << (answer.cut ? " cut" : "");
  }
  if (!Host().profile_read_done) {
    return;
  }
  const bc_read_answer out = {
      .status = answer.status,
      .url = answer.url.c_str(),
      .redirect = answer.redirect.empty() ? nullptr : answer.redirect.c_str(),
      .type = answer.type.c_str(),
      .body = answer.body.data(),
      .size = answer.body.size(),
      .cut = answer.cut,
      .error = answer.error.empty() ? nullptr : answer.error.c_str(),
  };
  Host().profile_read_done(Host().context, read, &out);
}

void Refuse(uint64_t read, std::string why) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](uint64_t read, std::string why) { Report(read, {.error = why}); }, read,
                     std::move(why)));
}

// One profile's reads, on its storage partition's loader, as live_folder.cc's: no tab, no
// renderer. They end as the profile goes, before its partition does.
class ProfileReads : public ProfileObserver {
 public:
  explicit ProfileReads(Profile* profile)
      : reads_(profile->GetDefaultStoragePartition()->GetURLLoaderFactoryForBrowserProcess()) {
    observation_.Observe(profile);
  }

  SavedReads& reads() { return reads_; }

  // ProfileObserver:
  void OnProfileWillBeDestroyed(Profile* profile) override;

 private:
  SavedReads reads_;
  base::ScopedObservation<Profile, ProfileObserver> observation_{this};
};

std::map<Profile*, std::unique_ptr<ProfileReads>>& All() {
  static base::NoDestructor<std::map<Profile*, std::unique_ptr<ProfileReads>>> all;
  return *all;
}

void ProfileReads::OnProfileWillBeDestroyed(Profile* profile) {
  // A profile deleted while the app runs answers each read; one going with the quit drops them.
  if (!browser_shutdown::HasShutdownStarted()) {
    reads_.EndAll("The profile closed before it answered.");
  }
  All().erase(profile);
}

void ReadOn(Profile* profile, uint64_t read, const GURL& url, size_t max_bytes,
            int64_t timeout_ms) {
  const std::string refused =
      SavedReadRefusal(url, profile->IsOffTheRecord(), max_bytes, timeout_ms);
  if (!refused.empty()) {
    Refuse(read, refused);
    return;
  }
  std::unique_ptr<ProfileReads>& reads = All()[profile];
  if (!reads) {
    reads = std::make_unique<ProfileReads>(profile);
  }
  reads->reads().Read(url, max_bytes, timeout_ms, base::BindOnce(&Report, read));
}

// ProfileFor stands the start profile in for one that isn't loaded.
Profile* Loaded(const std::string& dir) {
  Profile* profile = ProfileFor(dir);
  return profile && DirOf(profile) == dir ? profile : nullptr;
}

}  // namespace

void ReadOnProfile(bc_id profile, uint64_t read, const GURL& url, size_t max_bytes,
                   int64_t timeout_ms) {
  const std::string refused =
      SavedReadRefusal(url, /*off_the_record=*/false, max_bytes, timeout_ms);
  if (!refused.empty()) {
    Refuse(read, refused);
    return;
  }
  const std::string dir = ProfileDir(profile);
  if (dir.empty()) {
    Refuse(read, "There's no such profile.");
    return;
  }
  if (Profile* loaded = Loaded(dir)) {
    ReadOn(loaded, read, url, max_bytes, timeout_ms);
    return;
  }
  LoadProfiles({dir}, base::BindOnce(
                          [](const std::string& dir, uint64_t read, const GURL& url,
                             size_t max_bytes, int64_t timeout_ms) {
                            Profile* loaded = Loaded(dir);
                            if (!loaded) {
                              Refuse(read, "The profile couldn't be loaded.");
                              return;
                            }
                            ReadOn(loaded, read, url, max_bytes, timeout_ms);
                          },
                          dir, read, url, max_bytes, timeout_ms));
}

}  // namespace browser_core
