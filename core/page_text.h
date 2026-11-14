#ifndef BROWSER_CORE_PAGE_TEXT_H_
#define BROWSER_CORE_PAGE_TEXT_H_

#include <cstdint>
#include <optional>
#include <set>
#include <string>
#include <vector>

#include "base/functional/callback_forward.h"
#include "base/memory/weak_ptr.h"
#include "base/timer/timer.h"
#include "browser_core/ax_text.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"

namespace ui {
struct AXTreeUpdate;
}  // namespace ui

namespace browser_core {

// The text of a tab's page as its accessibility tree has it, across the frames an agent may read
// (IsWebFrame): what a screen reader reads. Chromium builds the tree in the renderer for this one
// snapshot, without turning accessibility on for the tab and without running the page's
// JavaScript. It leaves out every field's value, typed or autofilled, a password's above all. See
// bc_tab_read_text.
class PageText : public content::WebContentsObserver,
                 public content::WebContentsUserData<PageText> {
 public:
  PageText(const PageText&) = delete;
  PageText& operator=(const PageText&) = delete;
  ~PageText() override;

  // See bc_tab_read_text.
  void Read(uint64_t request);

 private:
  friend class content::WebContentsUserData<PageText>;

  explicit PageText(content::WebContents* contents);

  // Snapshots the page for each request waiting.
  void Snapshot();
  void Snapshotted(uint64_t request, ui::AXTreeUpdate& update);
  // Answers |request| from a task of its own, so the app never hears inside its own call.
  void Answer(uint64_t request, std::optional<std::string> text);

  // content::WebContentsObserver:
  void DidStopLoading() override;
  void WebContentsDestroyed() override;

  const bc_id tab_;
  // The requests waiting for the page to finish loading, and those the renderer is answering.
  std::vector<uint64_t> waiting_;
  std::set<uint64_t> asked_;
  // A page that never stops loading, as one that streams, is read as it is after a while.
  base::OneShotTimer give_up_;
  base::WeakPtrFactory<PageText> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

// The URLs of the frames an agent may read, which TextOf takes, as the memory reads the same text.
std::set<std::string, std::less<>> ReadableUrls(content::WebContents& contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_TEXT_H_
