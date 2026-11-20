#include "browser_core/clear_both_engines.h"

#include <utility>
#include <vector>

#include "base/barrier_callback.h"
#include "base/functional/bind.h"
#include "chrome/browser/browsing_data/chrome_browsing_data_remover_constants.h"
#include "content/public/browser/browsing_data_filter_builder.h"
#include "content/public/browser/browsing_data_remover.h"

namespace browser_core {

ClearBothEngines::ClearBothEngines(content::BrowsingDataRemoverDelegate* chrome,
                                   ClearStore clear_store,
                                   base::RepeatingClosure clear_choices)
    : chrome_(chrome),
      clear_store_(std::move(clear_store)),
      clear_choices_(std::move(clear_choices)) {}

ClearBothEngines::~ClearBothEngines() = default;

content::BrowsingDataRemoverDelegate::EmbedderOriginTypeMatcher
ClearBothEngines::GetOriginTypeMatcher() {
  return chrome_->GetOriginTypeMatcher();
}

bool ClearBothEngines::MayRemoveDownloadHistory() {
  return chrome_->MayRemoveDownloadHistory();
}

void ClearBothEngines::RemoveEmbedderData(const base::Time& delete_begin,
                                          const base::Time& delete_end,
                                          uint64_t remove_mask,
                                          content::BrowsingDataFilterBuilder* filter_builder,
                                          uint64_t origin_type_mask,
                                          base::OnceCallback<void(uint64_t)> callback) {
  using Remover = content::BrowsingDataRemover;
  // WebKit clears from a start to now. Every range of Clear Browsing Data's but "Older than 30
  // days" ends now, and that one leaves the system's engine alone.
  const bool whole = filter_builder->MatchesAllOriginsAndDomains() && delete_end.is_max();
  const bool cookies = remove_mask & Remover::DATA_TYPE_COOKIES;
  // Chrome's "Cookies and other site data" sends cookies with storage, which clears everything the
  // system's engine keeps. Storage alone isn't one of Clear Browsing Data's choices.
  const bool store = whole && cookies;
  const bool everything = remove_mask & Remover::DATA_TYPE_DOM_STORAGE;
  // The choices keep no times, so only a clear for all time empties them.
  const bool choices =
      whole && delete_begin.is_null() &&
      (cookies || (remove_mask & chrome_browsing_data_remover::DATA_TYPE_CONTENT_SETTINGS));
  if (choices) {
    clear_choices_.Run();
  }
  if (!store) {
    chrome_->RemoveEmbedderData(delete_begin, delete_end, remove_mask, filter_builder,
                                origin_type_mask, std::move(callback));
    return;
  }
  // The remover counts the removal done once Chrome's part and the system's engine's both are,
  // with the types Chrome's part failed.
  const auto both = base::BarrierCallback<uint64_t>(
      2, base::BindOnce(
             [](base::OnceCallback<void(uint64_t)> callback, std::vector<uint64_t> failed) {
               uint64_t types = 0;
               for (uint64_t failed_types : failed) {
                 types |= failed_types;
               }
               std::move(callback).Run(types);
             },
             std::move(callback)));
  chrome_->RemoveEmbedderData(delete_begin, delete_end, remove_mask, filter_builder,
                              origin_type_mask, both);
  clear_store_.Run(delete_begin, everything, base::BindOnce(both, uint64_t{0}));
}

void ClearBothEngines::OnStartRemoving() {
  chrome_->OnStartRemoving();
}

void ClearBothEngines::OnDoneRemoving() {
  chrome_->OnDoneRemoving();
}

}  // namespace browser_core
