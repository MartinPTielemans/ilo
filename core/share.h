// Shared links (#680, public/browser_core.h, bc_share_*): the snapshot of a folder, a Space or
// chosen tabs sealed and sent to the account's server, the account's links listed and unshared,
// and a link the user opens recognised by its answer, shown in a page of the core's own, and added.
// share_snapshot.h holds the format, its caps and the page. Nothing of a share is logged.

#ifndef BROWSER_CORE_SHARE_H_
#define BROWSER_CORE_SHARE_H_

#include <cstddef>
#include <cstdint>
#include <string>

#include "browser_core/public/browser_core.h"

namespace content {
class NavigationThrottleRegistry;
}

namespace browser_core {

// The C API's calls, as public/browser_core.h says.
bool ShareCount(int kind,
                const bc_id* items,
                size_t count,
                size_t* tabs,
                size_t* folders,
                size_t* left_out);
uint64_t ShareMake(int kind, const bc_id* items, size_t count);
void ShareList();
void Unshare(const std::string& id);
bc_id TabAnswerShare(bc_id tab, bc_id space, bool add);

// Adds the throttle that looks for a share's answer to a main-frame navigation the user made in a
// tab of theirs, as bc_tab_answer_share says.
void AddShareThrottle(content::NavigationThrottleRegistry& registry);

}  // namespace browser_core

#endif  // BROWSER_CORE_SHARE_H_
