#ifndef BROWSER_CORE_NATIVE_AD_BLOCK_LISTS_H_
#define BROWSER_CORE_NATIVE_AD_BLOCK_LISTS_H_

#include <string>
#include <vector>

#include "base/files/file_path.h"
#include "base/functional/callback_forward.h"
#include "base/time/time.h"
#include "url/gurl.h"

namespace browser_core {

// The browser's own ad blocker's filter lists, kept current as uBlock Origin keeps its own: the
// copies it comes with go stale, and a list's authors add the hosts of new trackers every day.

// A filter list to keep current, as uBlock Origin's assets.json names it.
struct NativeAdBlockList {
  std::string key;
  // Where to download it from, in the order to try: uBlock Origin's mirrors, then its authors'.
  std::vector<GURL> addresses;
  // How long a downloaded copy stays current: assets.json's updateAfter.
  base::TimeDelta update_after;
};

// Where the downloaded copy of the list |key| is kept, in the browser's folder, as the lists are
// the browser's and not a profile's. Any thread.
base::FilePath DownloadedAdBlockList(const std::string& key);

// Whether any of |lists| has no downloaded copy, or one twice as old as it stays current, when
// uBlock Origin updates its lists at once. Off the UI thread.
bool AnyNativeAdBlockListLongOutOfDate(const std::vector<NativeAdBlockList>& lists);

// Downloads, one after another, those of |lists| whose copy is older than its update_after, or
// that has none, trying each address until one answers with a list, and then calls |done| on the
// UI thread with the keys of the lists it downloaded. A list none of its addresses gives keeps the
// copy it has. On the UI thread.
void UpdateNativeAdBlockLists(std::vector<NativeAdBlockList> lists,
                              base::OnceCallback<void(std::vector<std::string>)> done);

}  // namespace browser_core

#endif  // BROWSER_CORE_NATIVE_AD_BLOCK_LISTS_H_
