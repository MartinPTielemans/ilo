#include "browser_core/page_text.h"

#include <algorithm>
#include <string_view>
#include <utility>

#include "base/containers/flat_map.h"
#include "base/functional/bind.h"
#include "base/strings/string_util.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/agent_page.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_mode.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core {

namespace {

// Enough for a long article, and few enough that a huge page's snapshot stays quick. The renderer
// stops at either, giving what it has.
constexpr size_t kMaxNodes = 20000;
constexpr base::TimeDelta kSnapshotTimeout = base::Seconds(5);
constexpr base::TimeDelta kLoadTimeout = base::Seconds(15);

// The URLs of the frames an agent may read: each one IsWebFrame admits, inside frames it admits, as
// agent_page.cc outlines them. A URL a frame it doesn't admit has too is left out, as the snapshot
// can't tell those frames apart.
std::set<std::string, std::less<>> ReadableUrls(content::WebContents& contents) {
  std::set<std::string, std::less<>> readable;
  std::set<std::string, std::less<>> refused;
  contents.GetPrimaryMainFrame()->ForEachRenderFrameHostWithAction(
      [&](content::RenderFrameHost* frame) {
        std::string url = frame->GetLastCommittedURL().spec();
        if (!IsWebFrame(frame)) {
          refused.insert(std::move(url));
          // Nor what's in it, whose own documents a refused one's subtree in the snapshot has.
          frame->ForEachRenderFrameHost([&refused](content::RenderFrameHost* inner) {
            refused.insert(inner->GetLastCommittedURL().spec());
          });
          return content::RenderFrameHost::FrameIterationAction::kSkipChildren;
        }
        readable.insert(std::move(url));
        return content::RenderFrameHost::FrameIterationAction::kContinue;
      });
  for (const std::string& url : refused) {
    readable.erase(url);
  }
  return readable;
}

void Report(bc_id tab, uint64_t request, std::optional<std::string> text) {
  if (Host().tab_text) {
    Host().tab_text(Host().context, tab, request, text ? text->c_str() : nullptr);
  }
}

}  // namespace

PageText::PageText(content::WebContents* contents)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PageText>(*contents),
      tab_(Tab::IdOf(contents)) {}

PageText::~PageText() = default;

void PageText::Read(uint64_t request) {
  // A tab without its page, or one opened behind and not yet shown, has nothing to read: it loads
  // once it's shown.
  if (!Tab::Loaded(tab_)) {
    Answer(request, std::nullopt);
    return;
  }
  waiting_.push_back(request);
  if (!GetWebContents().IsLoading()) {
    Snapshot();
  } else if (!give_up_.IsRunning()) {
    give_up_.Start(FROM_HERE, kLoadTimeout, this, &PageText::Snapshot);
  }
}

void PageText::Snapshot() {
  give_up_.Stop();
  for (uint64_t request : std::exchange(waiting_, {})) {
    asked_.insert(request);
    GetWebContents().RequestAXTreeSnapshot(
        base::BindOnce(&PageText::Snapshotted, weak_factory_.GetWeakPtr(), request),
        ui::kAXModeWebContentsOnly, kMaxNodes, kSnapshotTimeout,
        content::WebContents::AXTreeSnapshotPolicy::kAll);
  }
}

void PageText::Snapshotted(uint64_t request, ui::AXTreeUpdate& update) {
  asked_.erase(request);
  Answer(request, TextOf(update, ReadableUrls(GetWebContents())));
}

void PageText::Answer(uint64_t request, std::optional<std::string> text) {
  // Bound to the tab's id rather than this, which a closing tab deletes before the task runs.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Report, tab_, request, std::move(text)));
}

void PageText::DidStopLoading() {
  if (!waiting_.empty()) {
    Snapshot();
  }
}

void PageText::WebContentsDestroyed() {
  // The snapshots in the renderer never answer once the page is gone, so the app hears it here.
  give_up_.Stop();
  for (uint64_t request : std::exchange(waiting_, {})) {
    Answer(request, std::nullopt);
  }
  for (uint64_t request : std::exchange(asked_, {})) {
    Answer(request, std::nullopt);
  }
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PageText);

}  // namespace browser_core
