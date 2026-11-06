#ifndef BROWSER_CORE_AGENT_DOWNLOAD_HOLD_H_
#define BROWSER_CORE_AGENT_DOWNLOAD_HOLD_H_

#include <cstdint>
#include <string>

#include "base/functional/callback.h"

namespace download {
class DownloadItem;
}

namespace browser_core {

// A download a page in an agent's Space started, which waits on the user's answer before its file
// is put in place (#1446). Pausing it as it's created doesn't hold a small one, which comes whole
// in the same moment, so its completion is held too, where Chrome holds a download's for its own
// checks (engine/patches/ours/chromium-core-download-completion-hold.patch). Apart from the rest of
// the downloads so browser_core_unittests tests it alone.

// Marks |item| as the agent |agent|'s, from its tab |tab|, until it ends.
void MarkAgentDownload(download::DownloadItem* item, std::string agent, uint64_t tab);
// The agent |item| is held for, and its tab, or false if it isn't an agent's.
bool AgentDownloadOf(download::DownloadItem* item, std::string* agent, uint64_t* tab);
// Forgets that |item| is an agent's, as it ends.
void ForgetAgentDownload(download::DownloadItem* item);

// Pauses |item|, an agent's the user hasn't answered, so it takes nothing more while it asks, once
// Chrome has put it where it goes. Chrome allows no pause before: a download must enter
// IN_PROGRESS unpaused, and its file has a path only from that moment. Once, so the pause's own
// update doesn't ask again.
void PauseOnceResolved(download::DownloadItem* item);

// Chrome's question before it puts |item|'s file in place: true holds it, keeping |again|, which
// asks once more, while an agent's download waits on the user. Any other goes on.
bool HoldCompletion(download::DownloadItem* item, base::OnceClosure again);

// The user's yes: |item| goes on, and its held completion with it.
void AllowAgentDownload(download::DownloadItem* item);
// The user's no: |item| is cancelled, which deletes what it had written, partial or whole, and
// taken off the list, so it leaves nothing behind.
void RefuseAgentDownload(download::DownloadItem* item);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_DOWNLOAD_HOLD_H_
