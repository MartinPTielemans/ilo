#ifndef BROWSER_CORE_SCRIPT_DIALOG_H_
#define BROWSER_CORE_SCRIPT_DIALOG_H_

#include <memory>
#include <string>

#include "base/memory/raw_ptr.h"
#include "components/javascript_dialogs/tab_modal_dialog_manager_delegate.h"

namespace content {
class WebContents;
}

namespace browser_core {

class ScriptDialog;

// A tab's alert(), confirm() and prompt(), which the app asks with its dialog (PageDialog) in place
// of Chrome's view. Chrome's TabModalDialogManager still decides when a page may ask, and what its
// dialog becomes as the tab hides or goes to another page; this is the delegate it asks.
class ScriptDialogs : public javascript_dialogs::TabModalDialogManagerDelegate {
 public:
  // Has |contents|, a tab, ask with the app's dialog, in place of the manager Chrome's tab helpers
  // gave it.
  static void Attach(content::WebContents* contents);
  // Takes back |contents|' dialogs before its page is deleted out of its tab strip, as Chromium
  // does when a page closes with its delegate, which a tab detached for deletion no longer has.
  static void Cancel(content::WebContents* contents);

  explicit ScriptDialogs(content::WebContents* contents);
  ScriptDialogs(const ScriptDialogs&) = delete;
  ScriptDialogs& operator=(const ScriptDialogs&) = delete;
  ~ScriptDialogs() override;

  // From the dialog, once it's gone: answered, or taken back.
  void Gone(ScriptDialog* dialog);

  // javascript_dialogs::TabModalDialogManagerDelegate:
  base::WeakPtr<javascript_dialogs::TabModalDialogView> CreateNewDialog(
      content::WebContents* alerting_web_contents,
      const std::u16string& title,
      content::JavaScriptDialogType dialog_type,
      const std::u16string& message_text,
      const std::u16string& default_prompt_text,
      content::JavaScriptDialogManager::DialogClosedCallback dialog_callback,
      base::OnceClosure dialog_force_closed_callback) override;
  void WillRunDialog() override;
  void DidCloseDialog() override;
  void SetTabNeedsAttention(bool attention) override;
  bool IsWebContentsForemost() override;
  bool IsApp() override;
  bool CanShowModalUI() override;

 private:
  const raw_ptr<content::WebContents> contents_;
  // The dialog shown, at most one, as the manager closes one before it asks for the next.
  std::unique_ptr<ScriptDialog> dialog_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SCRIPT_DIALOG_H_
