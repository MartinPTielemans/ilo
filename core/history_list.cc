#include "browser_core/history_list.h"

#include <algorithm>
#include <limits>
#include <optional>
#include <set>
#include <string>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/location.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/browser/url_row.h"
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

}  // namespace

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

bool RemoveHistory(Profile* profile, const char* url, int64_t begin, int64_t end) {
  history::HistoryService* history = LoadedHistory(profile);
  GURL page(url ? url : "");
  if (!history || !page.is_valid() || begin >= end) {
    return false;
  }
  // The page's visits in the span alone, as Chrome's history page removes a day's row; its
  // observers hear the deletion, and with them history_deleted. No synced history to tell: this
  // build has no Google account web history.
  history->ExpireHistoryBetween(std::set<GURL>{page}, std::nullopt, FromMicroseconds(begin),
                                FromMicroseconds(end), /*user_initiated=*/true,
                                base::DoNothing(), &Tracker());
  return true;
}

}  // namespace browser_core
