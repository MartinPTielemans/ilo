#include "browser_core/history_list.h"

#include <algorithm>
#include <limits>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "base/barrier_callback.h"
#include "base/functional/bind.h"
#include "base/location.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/bind_post_task.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/task/sequenced_task_runner.h"
#include "base/time/time.h"
#include "browser_core/agent_space_rule.h"
#include "browser_core/host.h"
#include "browser_core/link_target.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "components/history/core/browser/history_database.h"
#include "components/history/core/browser/history_db_task.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/browser/url_row.h"
#include "components/history/core/browser/visit_database.h"
#include "components/keyed_service/core/service_access_type.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

base::CancelableTaskTracker& Tracker() {
  static base::NoDestructor<base::CancelableTaskTracker> tracker;
  return *tracker;
}

base::Time FromMicroseconds(int64_t microseconds) {
  return base::Time::UnixEpoch() + base::Microseconds(microseconds);
}

int64_t Microseconds(base::Time time) {
  return (time - base::Time::UnixEpoch()).InMicroseconds();
}

// The profile's history, while the user's history is kept and has loaded: none for a private
// profile, whose pages nothing keeps. BackendLoaded starts the load if it hasn't started.
history::HistoryService* LoadedHistory(Profile* profile) {
  if (!profile || profile->IsOffTheRecord()) {
    return nullptr;
  }
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile, ServiceAccessType::EXPLICIT_ACCESS);
  return history && history->BackendLoaded() ? history : nullptr;
}

void Listed(bc_id profile, uint64_t request, history::QueryResults results) {
  if (!Host().history_listed) {
    return;
  }
  // The strings the pages point into, which live until the call returns.
  std::vector<std::string> urls;
  std::vector<std::string> titles;
  urls.reserve(results.size());
  titles.reserve(results.size());
  for (const history::URLResult& result : results) {
    urls.push_back(result.url().spec());
    titles.push_back(base::UTF16ToUTF8(result.title()));
  }
  std::vector<bc_history_page> pages;
  pages.reserve(results.size());
  for (size_t i = 0; i < results.size(); ++i) {
    const history::URLResult& result = results[i];
    bc_history_page& page = pages.emplace_back();
    page.url = urls[i].c_str();
    page.title = titles[i].c_str();
    // The visit the row stands for, its page's last that day, as each page shows once a day.
    page.last_visit = Microseconds(result.visit_time());
    page.visits = result.visit_count();
    page.typed = result.typed_count();
  }
  Host().history_listed(Host().context, profile, request, pages.data(), pages.size(),
                        !results.reached_beginning());
}

// Runs on the history database's thread: the last visit to a page shown on |site_|, which it keeps
// to itself. Only the time goes back, and only to the pick.
// Reads one profile's history for the newest visit to any of |hosts|, by the history's index of
// addresses, so its cost doesn't grow with the history. Its answer comes once on the main thread,
// whether it runs, or is dropped unrun as its history shuts down.
class LastOnSite : public history::HistoryDBTask {
 public:
  LastOnSite(std::vector<std::string> hosts, base::OnceCallback<void(int64_t)> done)
      : hosts_(std::move(hosts)),
        answer_(base::BindPostTask(base::SequencedTaskRunner::GetCurrentDefault(),
                                   std::move(done))) {}

  bool RunOnDBThread(history::HistoryBackend*, history::HistoryDatabase* db) override {
    if (!db) {
      return true;
    }
    for (const std::string& host : hosts_) {
      base::Time visit;
      GURL url;
      if (db->GetLastVisitToHost(host, base::Time(), base::Time::Max(),
                                 history::VisitQuery404sPolicy::kInclude404s, &visit, &url) &&
          visit > last_) {
        last_ = visit;
      }
    }
    return true;
  }

  void DoneRunOnMainThread() override {
    answer_.Answer(last_.is_null() ? 0 : Microseconds(last_));
  }

 private:
  const std::vector<std::string> hosts_;
  // Dropped unanswered, as the task is when its history goes first, it answers 0 as it goes.
  agent_space_rule::PickAnswer answer_;
  base::Time last_;
};

void Picked(uint64_t request, std::vector<agent_space_rule::PickCandidate> candidates) {
  if (!Host().profile_picked) {
    return;
  }
  const bc_id profile = agent_space_rule::Picked(candidates);
  Host().profile_picked(Host().context, request, profile, profile ? BC_PICK_HISTORY : 0);
}

}  // namespace

void PickProfileForSite(const std::vector<std::pair<bc_id, Profile*>>& profiles,
                        const GURL& url,
                        uint64_t request) {
  const std::string site = SiteOf(url);
  std::vector<std::pair<size_t, history::HistoryService*>> read;
  for (size_t order = 0; order < profiles.size(); ++order) {
    // LoadedHistory is none for a private profile and for one whose history hasn't loaded.
    if (history::HistoryService* history = LoadedHistory(profiles[order].second)) {
      read.emplace_back(order, history);
    }
  }
  if (site.empty() || read.empty()) {
    // Answered after the call returns, as a pick that reads is.
    base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(&Picked, request, std::vector<agent_space_rule::PickCandidate>()));
    return;
  }
  const auto each = base::BarrierCallback<agent_space_rule::PickCandidate>(
      read.size(), base::BindOnce(&Picked, request));
  const std::vector<std::string> hosts = agent_space_rule::PickHosts(url.host(), site);
  for (const auto& [order, history] : read) {
    const bc_id profile = profiles[order].first;
    history->ScheduleDBTask(
        FROM_HERE,
        std::make_unique<LastOnSite>(
            hosts, base::BindOnce(
                       [](base::RepeatingCallback<void(agent_space_rule::PickCandidate)> each,
                          bc_id profile, size_t order, int64_t last) {
                         each.Run({.profile = profile,
                                   .order = order,
                                   .loaded = true,
                                   .last_visit = last});
                       },
                       each, profile, order)),
        &Tracker());
  }
}

bool ListHistory(Profile* profile,
                 bc_id id,
                 uint64_t request,
                 int64_t before,
                 const char* text,
                 size_t max) {
  history::HistoryService* history = LoadedHistory(profile);
  if (!history || !max) {
    return false;
  }
  history::QueryOptions options;
  // A null end is no end, so now: no visit is later.
  options.end_time = before > 0 ? FromMicroseconds(before) : base::Time();
  options.max_count =
      static_cast<int>(std::min<size_t>(max, std::numeric_limits<int>::max()));
  options.duplicate_policy = history::QueryOptions::REMOVE_DUPLICATES_PER_DAY;
  options.policy_for_404_visits = history::VisitQuery404sPolicy::kInclude404s;
  history->QueryHistory(base::UTF8ToUTF16(text ? text : ""), options,
                        base::BindOnce(&Listed, id, request), &Tracker());
  return true;
}

}  // namespace browser_core
