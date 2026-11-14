#include "browser_core/history_removal.h"

#include <algorithm>
#include <optional>
#include <set>

#include "components/history/core/browser/history_backend.h"
#include "components/history/core/browser/history_database.h"
#include "components/history/core/browser/keyword_search_term.h"

namespace browser_core {

RemovedPage::RemovedPage() = default;
RemovedPage::RemovedPage(const RemovedPage&) = default;
RemovedPage::RemovedPage(RemovedPage&&) = default;
RemovedPage& RemovedPage::operator=(const RemovedPage&) = default;
RemovedPage& RemovedPage::operator=(RemovedPage&&) = default;
RemovedPage::~RemovedPage() = default;

RemovedHistory::RemovedHistory() = default;
RemovedHistory::~RemovedHistory() = default;

KeepHistory::KeepHistory(scoped_refptr<RemovedHistory> removed,
                         std::vector<GURL> pages,
                         std::vector<std::pair<history::KeywordID, std::u16string>> searches,
                         base::Time begin,
                         base::Time end,
                         bool take_out,
                         base::OnceClosure done)
    : removed_(std::move(removed)),
      pages_(std::move(pages)),
      searches_(std::move(searches)),
      begin_(begin),
      end_(end),
      take_out_(take_out),
      done_(std::move(done)) {}

KeepHistory::~KeepHistory() = default;

bool KeepHistory::RunOnDBThread(history::HistoryBackend* backend, history::HistoryDatabase* db) {
  if (!db) {
    return true;
  }
  Read(db);
  if (!take_out_) {
    return true;
  }
  if (!end_.is_null()) {
    // As Chrome's history page removes a day's row. An empty set would be every page's.
    if (!pages_.empty()) {
      backend->ExpireHistoryBetween(std::set<GURL>(pages_.begin(), pages_.end()), std::nullopt,
                                    begin_, end_, /*user_initiated=*/true);
    }
    return true;
  }
  if (!pages_.empty()) {
    backend->DeleteURLs(pages_);
  }
  for (const auto& [keyword, term] : searches_) {
    backend->DeleteMatchingURLsForKeyword(keyword, term);
  }
  return true;
}

void KeepHistory::DoneRunOnMainThread() {
  std::move(done_).Run();
}

void KeepHistory::Read(history::HistoryDatabase* db) {
  std::vector<history::URLID> ids;
  for (const GURL& url : pages_) {
    history::URLRow row;
    if (history::URLID id = db->GetRowForURL(url, &row)) {
      ids.push_back(id);
    }
  }
  for (const auto& [keyword, term] : searches_) {
    std::vector<history::KeywordSearchTermRow> rows;
    db->GetKeywordSearchTermRows(term, &rows);
    for (const history::KeywordSearchTermRow& row : rows) {
      if (row.keyword_id == keyword) {
        ids.push_back(row.url_id);
      }
    }
  }
  std::ranges::sort(ids);
  ids.erase(std::ranges::unique(ids).begin(), ids.end());
  for (history::URLID id : ids) {
    RemovedPage page;
    if (!db->GetURLRow(id, &page.row)) {
      continue;
    }
    db->GetVisitsForURL(id, &page.visits);
    if (!end_.is_null()) {
      std::erase_if(page.visits, [&](const history::VisitRow& visit) {
        return visit.visit_time < begin_ || visit.visit_time >= end_;
      });
      // Nothing of it is taken out, so there's nothing to put back.
      if (page.visits.empty()) {
        continue;
      }
    }
    std::ranges::sort(page.visits, {}, &history::VisitRow::visit_time);
    db->GetVisitsSource(page.visits, &page.sources);
    if (history::KeywordSearchTermRow term; db->GetKeywordSearchTermRow(id, &term)) {
      page.keyword = term.keyword_id;
      page.term = term.term;
    }
    removed_->pages.push_back(std::move(page));
  }
}

PutBackHistory::PutBackHistory(scoped_refptr<RemovedHistory> removed, base::OnceClosure done)
    : removed_(std::move(removed)), done_(std::move(done)) {}

PutBackHistory::~PutBackHistory() = default;

bool PutBackHistory::RunOnDBThread(history::HistoryBackend* backend,
                                   history::HistoryDatabase* db) {
  for (const RemovedPage& page : removed_->pages) {
    const GURL& url = page.row.url();
    // A page with no visits left, as one only bookmarked, comes back as a row with one.
    if (page.visits.empty()) {
      backend->AddPagesWithDetails({page.row}, history::SOURCE_BROWSED);
    }
    // A visit older than the page's last leaves its last visit as it is, so a day put back under
    // a later one keeps the later one.
    for (const history::VisitRow& visit : page.visits) {
      const auto source = page.sources.find(visit.visit_id);
      backend->AddPage(history::HistoryAddPageArgs(
          url, visit.visit_time, history::ContextID(), /*nav_entry_id=*/0,
          /*local_navigation_id=*/std::nullopt, /*referrer=*/GURL(), history::RedirectList(),
          visit.transition, page.row.hidden(),
          source == page.sources.end() ? history::SOURCE_BROWSED : source->second,
          history::VisitResponseCodeCategory::kNot404, /*did_replace_entry=*/false,
          visit.consider_for_ntp_most_visited));
    }
    backend->SetPageTitle(url, page.row.title());
    if (page.keyword) {
      backend->SetKeywordSearchTermsForURL(url, page.keyword, page.term);
    }
  }
  return true;
}

void PutBackHistory::DoneRunOnMainThread() {
  std::move(done_).Run();
}

}  // namespace browser_core
