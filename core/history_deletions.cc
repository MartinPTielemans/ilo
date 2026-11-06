#include "browser_core/history_deletions.h"

#include <cstdint>
#include <limits>
#include <memory>
#include <string>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "base/scoped_observation.h"
#include "base/supports_user_data.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_service_observer.h"
#include "components/history/core/browser/history_types.h"
#include "components/keyed_service/core/service_access_type.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Seconds since 1970, as the app keeps the times it learned.
int64_t Seconds(base::Time time) {
  return (time - base::Time::UnixEpoch()).InSeconds();
}

// Lives as long as the profile, and stops hearing its history as the history service goes, which
// is before the profile.
class Deletions : public base::SupportsUserData::Data, public history::HistoryServiceObserver {
 public:
  static constexpr char kKey[] = "browser_core::Deletions";

  Deletions(bc_id profile, history::HistoryService* history) : profile_(profile) {
    observation_.Observe(history);
  }

  // HistoryServiceObserver:
  void OnHistoryDeletions(history::HistoryService* history,
                          const history::DeletionInfo& info) override {
    // History running out of old visits isn't the user deleting them.
    if (!Host().history_deleted || info.is_from_expiration()) {
      return;
    }
    int64_t begin = 0;
    int64_t end = 0;
    std::vector<std::string> urls;
    if (!info.IsAllHistory()) {
      const history::DeletionTimeRange& range = info.time_range();
      if (range.IsValid()) {
        begin = range.begin().is_null() ? 0 : Seconds(range.begin());
        end = range.end().is_null() || range.end().is_max() ? std::numeric_limits<int64_t>::max()
                                                            : Seconds(range.end());
      }
      // deleted_rows is undefined for all of it.
      for (const history::URLRow& row : info.deleted_rows()) {
        urls.push_back(row.url().spec());
      }
      if (info.restrict_urls()) {
        for (const GURL& url : *info.restrict_urls()) {
          urls.push_back(url.spec());
        }
      }
    }
    std::vector<const char*> specs;
    for (const std::string& url : urls) {
      specs.push_back(url.c_str());
    }
    Host().history_deleted(Host().context, profile_, info.IsAllHistory(), begin, end, specs.data(),
                           specs.size());
  }

  void HistoryServiceBeingDeleted(history::HistoryService* history) override {
    observation_.Reset();
  }

 private:
  const bc_id profile_;
  base::ScopedObservation<history::HistoryService, history::HistoryServiceObserver> observation_{
      this};
};

}  // namespace

void ObserveHistoryDeletions(Profile* profile) {
  if (!profile || profile->IsOffTheRecord() || profile->GetUserData(Deletions::kKey)) {
    return;
  }
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile, ServiceAccessType::EXPLICIT_ACCESS);
  if (!history) {
    return;
  }
  profile->SetUserData(Deletions::kKey,
                       std::make_unique<Deletions>(ProfileId(DirOf(profile)), history));
}

}  // namespace browser_core
