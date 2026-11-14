#ifndef BROWSER_CORE_HISTORY_REMOVAL_H_
#define BROWSER_CORE_HISTORY_REMOVAL_H_

#include <string>
#include <utility>
#include <vector>

#include "base/functional/callback.h"
#include "base/memory/ref_counted.h"
#include "base/memory/scoped_refptr.h"
#include "base/time/time.h"
#include "components/history/core/browser/history_db_task.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/browser/keyword_id.h"
#include "components/history/core/browser/url_row.h"
#include "url/gurl.h"

namespace history {
class HistoryBackend;
class HistoryDatabase;
}  // namespace history

namespace browser_core {

// The history tasks of a window's removals (suggest.h), apart so browser_core_unittests tests them
// on a history service alone.

// A page a removal took out of history, as it was: its row, its visits oldest first and where each
// came from, and the term it searched for, if it was a search's results.
struct RemovedPage {
  RemovedPage();
  RemovedPage(const RemovedPage&);
  RemovedPage(RemovedPage&&);
  RemovedPage& operator=(const RemovedPage&);
  RemovedPage& operator=(RemovedPage&&);
  ~RemovedPage();

  history::URLRow row;
  history::VisitVector visits;
  history::VisitSourceMap sources;
  // Chromium 157 makes this an IdType, whose default is the invalid ID, 0.
  history::KeywordID keyword{};
  std::u16string term;
};

// What a removal took out of a profile's history, as it was. Filled and read only on the history
// database's thread, in the order the tasks were posted there, so the main thread holds it without
// reading it.
struct RemovedHistory : base::RefCountedThreadSafe<RemovedHistory> {
  RemovedHistory();

  std::vector<RemovedPage> pages;

 private:
  friend class base::RefCountedThreadSafe<RemovedHistory>;
  ~RemovedHistory();
};

// Reads what a removal takes out: |pages|, and the pages of each search for a term with an engine's
// keyword, as Chrome's DeleteMatchingURLsForKeyword finds them. Each page's visits from |begin| to
// |end|, as the Library's History removes a day's row, or all of them while |end| is null. With
// |take_out|, takes them out then, in the same task, so nothing between could keep or show what it
// read. Then |done|. Another task queued ahead of it on the history sequence runs it later than
// what was posted after it, so what reads before a removal does the removal or has it done.
class KeepHistory : public history::HistoryDBTask {
 public:
  KeepHistory(scoped_refptr<RemovedHistory> removed,
              std::vector<GURL> pages,
              std::vector<std::pair<history::KeywordID, std::u16string>> searches,
              base::Time begin,
              base::Time end,
              bool take_out,
              base::OnceClosure done);
  ~KeepHistory() override;

  // history::HistoryDBTask:
  bool RunOnDBThread(history::HistoryBackend* backend, history::HistoryDatabase* db) override;
  void DoneRunOnMainThread() override;

 private:
  void Read(history::HistoryDatabase* db);

  const scoped_refptr<RemovedHistory> removed_;
  const std::vector<GURL> pages_;
  const std::vector<std::pair<history::KeywordID, std::u16string>> searches_;
  const base::Time begin_;
  const base::Time end_;
  const bool take_out_;
  base::OnceClosure done_;
};

// Puts the pages a removal took out back: each visit again, at its time with its transition, so
// the history service counts the page's visits and typed visits as it did the first time, then its
// title and the term it searched for. Then |done|.
class PutBackHistory : public history::HistoryDBTask {
 public:
  PutBackHistory(scoped_refptr<RemovedHistory> removed, base::OnceClosure done);
  ~PutBackHistory() override;

  // history::HistoryDBTask:
  bool RunOnDBThread(history::HistoryBackend* backend, history::HistoryDatabase* db) override;
  void DoneRunOnMainThread() override;

 private:
  const scoped_refptr<RemovedHistory> removed_;
  base::OnceClosure done_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_HISTORY_REMOVAL_H_
