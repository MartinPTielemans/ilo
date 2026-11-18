#include "browser_core/focus_leaving.h"

#include <cstdio>
#include <utility>

#include "base/memory/raw_ptr.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/focus_leaving_rule.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_view_delegate.h"

namespace browser_core {
namespace {

// Chrome's delegate, which it passes every call to but TakeFocus from a tab's page.
class FocusLeavingDelegate : public content::WebContentsViewDelegate {
 public:
  FocusLeavingDelegate(content::WebContents* contents,
                       std::unique_ptr<content::WebContentsViewDelegate> chrome)
      : contents_(contents), chrome_(std::move(chrome)) {}

  bool TakeFocus(bool reverse) override {
    if (!Tab::FromWebContents(contents_)) {
      return chrome_->TakeFocus(reverse);
    }
    FocusLeftPage(contents_, reverse);
    // Answered, so neither Chrome's delegate nor, on the Mac, the window's key view loop moves it.
    return true;
  }

  gfx::NativeWindow GetNativeWindow() override { return chrome_->GetNativeWindow(); }
  content::WebDragDestDelegate* GetDragDestDelegate() override {
    return chrome_->GetDragDestDelegate();
  }
  void ShowContextMenu(content::RenderFrameHost& render_frame_host,
                       const content::ContextMenuParams& params) override {
    chrome_->ShowContextMenu(render_frame_host, params);
  }
  void DismissContextMenu() override { chrome_->DismissContextMenu(); }
  void ExecuteCommandForTesting(int command_id, int event_flags) override {
    chrome_->ExecuteCommandForTesting(command_id, event_flags);
  }
  bool IsContextMenuShowingForTesting() override {
    return chrome_->IsContextMenuShowingForTesting();
  }
  void StoreFocus() override { chrome_->StoreFocus(); }
  bool RestoreFocus() override { return chrome_->RestoreFocus(); }
  void ResetStoredFocus() override { chrome_->ResetStoredFocus(); }
  bool Focus() override { return chrome_->Focus(); }
  // The Mac's is an Objective-C object, autoreleased, which this passes on untouched.
  void* GetDelegateForHost(content::RenderWidgetHost* render_widget_host, bool is_popup) override {
    return chrome_->GetDelegateForHost(render_widget_host, is_popup);
  }
  void OnPerformingDrop(const content::DropData& drop_data,
                        DropCompletionCallback callback) override {
    chrome_->OnPerformingDrop(drop_data, std::move(callback));
  }
  void WebContentsDragEnded() override { chrome_->WebContentsDragEnded(); }

 private:
  // The page view owns this delegate, and the page owns its view.
  const raw_ptr<content::WebContents> contents_;
  const std::unique_ptr<content::WebContentsViewDelegate> chrome_;
};

}  // namespace

bool FocusLeftPage(content::WebContents* contents, bool reverse) {
  const Tab* tab = Tab::FromWebContents(contents);
  if (!tab) {
    return false;
  }
  const bc_id id = tab->id();
  const focus_leaving_rule::Kept kept =
      focus_leaving_rule::Leave(IsAgentTab(contents), Window::Showing(id) != nullptr, [&] {
        return Host().tab_focus_leaving && Host().tab_focus_leaving(Host().context, id, reverse);
      });
  fprintf(stderr, "browser: tab %llu focus left its page at the %s, kept by the %s\n",
          static_cast<unsigned long long>(id), reverse ? "start" : "end",
          focus_leaving_rule::Name(kept));
  return kept == focus_leaving_rule::Kept::kByApp;
}

std::unique_ptr<content::WebContentsViewDelegate> AnswerFocusLeaving(
    content::WebContents* contents,
    std::unique_ptr<content::WebContentsViewDelegate> chrome) {
  if (!chrome) {
    return chrome;
  }
  return std::make_unique<FocusLeavingDelegate>(contents, std::move(chrome));
}

}  // namespace browser_core
