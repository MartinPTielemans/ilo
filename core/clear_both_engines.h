#ifndef BROWSER_CORE_CLEAR_BOTH_ENGINES_H_
#define BROWSER_CORE_CLEAR_BOTH_ENGINES_H_

#include <cstdint>

#include "base/functional/callback.h"
#include "base/memory/raw_ptr.h"
#include "base/time/time.h"
#include "content/public/browser/browsing_data_remover_delegate.h"

namespace browser_core {

// Chrome's delegate for a profile's BrowsingDataRemover, wrapped so a whole clear also reaches the
// system's engine. The remover calls its delegate for every removal, with the filter an observer
// never sees: the core's Clear Site Data and a site's Clear-Site-Data header go through it for one
// site, and act on the system's engine themselves or not at all.
class ClearBothEngines : public content::BrowsingDataRemoverDelegate {
 public:
  // Clears the system's engine's cookies, or everything it keeps when |everything|, that changed
  // since |begin|, then runs |done|.
  using ClearStore =
      base::RepeatingCallback<void(base::Time begin, bool everything, base::OnceClosure done)>;

  // |chrome| outlives this. |clear_choices| empties the profile's engine choices.
  ClearBothEngines(content::BrowsingDataRemoverDelegate* chrome,
                   ClearStore clear_store,
                   base::RepeatingClosure clear_choices);
  ClearBothEngines(const ClearBothEngines&) = delete;
  ClearBothEngines& operator=(const ClearBothEngines&) = delete;
  ~ClearBothEngines() override;

  content::BrowsingDataRemoverDelegate* chrome() const { return chrome_; }

  // content::BrowsingDataRemoverDelegate, every method forwarded. A method a later major adds
  // without a body fails to compile here; one with a body would go unforwarded, so a new major
  // reads that header against this list.
  EmbedderOriginTypeMatcher GetOriginTypeMatcher() override;
  bool MayRemoveDownloadHistory() override;
  void RemoveEmbedderData(const base::Time& delete_begin,
                          const base::Time& delete_end,
                          uint64_t remove_mask,
                          content::BrowsingDataFilterBuilder* filter_builder,
                          uint64_t origin_type_mask,
                          base::OnceCallback<void(uint64_t)> callback) override;
  void OnStartRemoving() override;
  void OnDoneRemoving() override;

 private:
  const raw_ptr<content::BrowsingDataRemoverDelegate> chrome_;
  const ClearStore clear_store_;
  const base::RepeatingClosure clear_choices_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_CLEAR_BOTH_ENGINES_H_
