#include "browser_core/agent_download_hold.h"

#include <memory>
#include <utility>

#include "base/files/file_path.h"
#include "base/supports_user_data.h"
#include "components/download/public/common/download_item.h"

namespace browser_core {

namespace {

// Kept on a download a page in an agent's Space started, until it ends.
struct AgentDownload : base::SupportsUserData::Data {
  static constexpr char kKey[] = "browser_core::AgentDownload";
  std::string agent;
  uint64_t tab = 0;
  // The user said yes, so its completion goes on.
  bool allowed = false;
  // Paused while it asks, once its target was resolved.
  bool paused = false;
  // Chrome's question about its completion, asked again once the user says yes.
  base::OnceClosure again;
};

AgentDownload* Of(download::DownloadItem* item) {
  return static_cast<AgentDownload*>(item->GetUserData(AgentDownload::kKey));
}

}  // namespace

void MarkAgentDownload(download::DownloadItem* item, std::string agent, uint64_t tab) {
  auto held = std::make_unique<AgentDownload>();
  held->agent = std::move(agent);
  held->tab = tab;
  item->SetUserData(AgentDownload::kKey, std::move(held));
}

bool AgentDownloadOf(download::DownloadItem* item, std::string* agent, uint64_t* tab) {
  AgentDownload* held = Of(item);
  if (!held) {
    return false;
  }
  *agent = held->agent;
  *tab = held->tab;
  return true;
}

void ForgetAgentDownload(download::DownloadItem* item) {
  item->RemoveUserData(AgentDownload::kKey);
}

bool ResolvedToPause(download::DownloadItem* item) {
  AgentDownload* held = Of(item);
  if (!held || held->allowed || held->paused ||
      item->GetState() != download::DownloadItem::IN_PROGRESS || item->GetFullPath().empty()) {
    return false;
  }
  held->paused = true;
  return true;
}

void PauseHeld(download::DownloadItem* item) {
  AgentDownload* held = Of(item);
  if (held && !held->allowed && item->GetState() == download::DownloadItem::IN_PROGRESS) {
    item->Pause();
  }
}

bool HoldCompletion(download::DownloadItem* item, base::OnceClosure again) {
  AgentDownload* held = Of(item);
  if (!held || held->allowed) {
    return false;
  }
  held->again = std::move(again);
  return true;
}

void AllowAgentDownload(download::DownloadItem* item) {
  AgentDownload* held = Of(item);
  if (!held) {
    return;
  }
  held->allowed = true;
  // Out first, as a completion it lets go on ends the download, and its data with it.
  base::OnceClosure again = std::move(held->again);
  item->Resume(/*user_resume=*/true);
  if (again && item->GetState() == download::DownloadItem::IN_PROGRESS) {
    std::move(again).Run();
  }
}

void RefuseAgentDownload(download::DownloadItem* item) {
  if (!Of(item)) {
    return;
  }
  item->Cancel(/*user_cancel=*/true);
  item->Remove();
}

}  // namespace browser_core
