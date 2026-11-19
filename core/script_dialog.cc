#include "browser_core/script_dialog.h"

#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/weak_ptr.h"
#include "base/strings/utf_string_conversions.h"
#include "browser_core/page_dialog.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "components/javascript_dialogs/tab_modal_dialog_manager.h"
#include "components/javascript_dialogs/tab_modal_dialog_view.h"
#include "components/tabs/public/tab_interface.h"
#include "content/public/browser/web_contents.h"
#include "ui/base/l10n/l10n_util.h"
#include "ui/strings/grit/ui_strings.h"

namespace browser_core {

// One alert(), confirm() or prompt(), asked with the app's dialog, as Chrome's view would show it.
class ScriptDialog : public javascript_dialogs::TabModalDialogView {
 public:
  ScriptDialog(ScriptDialogs* owner,
               content::WebContents* contents,
               const std::u16string& title,
               content::JavaScriptDialogType type,
               const std::u16string& message,
               const std::u16string& default_text,
               content::JavaScriptDialogManager::DialogClosedCallback answered,
               base::OnceClosure force_closed)
      : owner_(owner),
        default_text_(default_text),
        answered_(std::move(answered)),
        force_closed_(std::move(force_closed)) {
    // As Chrome's view: nothing else modal, such as print preview, comes over it.
    tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(contents);
    if (tab && tab->CanShowModalUI()) {
      modal_ui_ = tab->ShowModalUI();
    }
    PageDialog::Words words;
    words.kind = type == content::JAVASCRIPT_DIALOG_TYPE_ALERT     ? BC_DIALOG_ALERT
                 : type == content::JAVASCRIPT_DIALOG_TYPE_CONFIRM ? BC_DIALOG_CONFIRM
                                                                   : BC_DIALOG_PROMPT;
    words.title = base::UTF16ToUTF8(title);
    words.message = base::UTF16ToUTF8(message);
    if (type == content::JAVASCRIPT_DIALOG_TYPE_PROMPT) {
      words.text = base::UTF16ToUTF8(default_text);
    }
    words.accept = l10n_util::GetStringUTF8(IDS_APP_OK);
    // An alert has only OK, as there's no choice to make.
    if (type != content::JAVASCRIPT_DIALOG_TYPE_ALERT) {
      words.cancel = l10n_util::GetStringUTF8(IDS_APP_CANCEL);
    }
    id_ = PageDialog::Ask(Tab::IdOf(contents), std::move(words),
                          base::BindOnce(&ScriptDialog::Answered, weak_factory_.GetWeakPtr()));
  }

  ScriptDialog(const ScriptDialog&) = delete;
  ScriptDialog& operator=(const ScriptDialog&) = delete;
  ~ScriptDialog() override { PageDialog::Withdraw(id_); }

  base::WeakPtr<ScriptDialog> GetWeakPtr() { return weak_factory_.GetWeakPtr(); }

  // javascript_dialogs::TabModalDialogView:
  void CloseDialogWithoutCallback() override {
    answered_.Reset();
    force_closed_.Reset();
    // Destroys this.
    owner_->Gone(this);
  }

  std::u16string GetUserInput() override {
    // Asked as DevTools answers for the user, who hasn't typed anything the core has seen.
    return default_text_;
  }

 private:
  void Answered(const PageDialog::Reply& reply) {
    // The user answered, so it wasn't forced closed.
    force_closed_.Reset();
    if (answered_) {
      std::move(answered_).Run(reply.accept, reply.accept ? reply.text : std::u16string());
    }
    // Destroys this.
    owner_->Gone(this);
  }

  const raw_ptr<ScriptDialogs> owner_;
  const std::u16string default_text_;
  content::JavaScriptDialogManager::DialogClosedCallback answered_;
  base::OnceClosure force_closed_;
  std::unique_ptr<tabs::ScopedTabModalUI> modal_ui_;
  bc_id id_ = 0;
  base::WeakPtrFactory<ScriptDialog> weak_factory_{this};
};

// static
void ScriptDialogs::Attach(content::WebContents* contents) {
  // Chrome's tab helpers may have given the tab theirs, with views, before its id was asked for;
  // if they come later, they keep this one.
  contents->RemoveUserData(javascript_dialogs::TabModalDialogManager::UserDataKey());
  javascript_dialogs::TabModalDialogManager::CreateForWebContents(
      contents, std::make_unique<ScriptDialogs>(contents));
}

// static
void ScriptDialogs::Cancel(content::WebContents* contents) {
  if (auto* manager = javascript_dialogs::TabModalDialogManager::FromWebContents(contents)) {
    // The tab's own, and the app-modal ones its page asks, as a leave-page question.
    manager->CancelDialogs(contents, /*reset_state=*/true);
  }
}

ScriptDialogs::ScriptDialogs(content::WebContents* contents) : contents_(contents) {}

ScriptDialogs::~ScriptDialogs() = default;

void ScriptDialogs::Gone(ScriptDialog* dialog) {
  if (dialog_.get() == dialog) {
    dialog_.reset();
  }
}

base::WeakPtr<javascript_dialogs::TabModalDialogView> ScriptDialogs::CreateNewDialog(
    content::WebContents* alerting_web_contents,
    const std::u16string& title,
    content::JavaScriptDialogType dialog_type,
    const std::u16string& message_text,
    const std::u16string& default_prompt_text,
    content::JavaScriptDialogManager::DialogClosedCallback dialog_callback,
    base::OnceClosure dialog_force_closed_callback) {
  // The tab's own, whichever of its frames asked, as the dialog is over the tab.
  dialog_ = std::make_unique<ScriptDialog>(this, contents_, title, dialog_type, message_text,
                                           default_prompt_text, std::move(dialog_callback),
                                           std::move(dialog_force_closed_callback));
  return dialog_->GetWeakPtr();
}

void ScriptDialogs::WillRunDialog() {}

void ScriptDialogs::DidCloseDialog() {}

void ScriptDialogs::SetTabNeedsAttention(bool attention) {
  // The alert waits for its tab to show, and the app shows it then.
}

bool ScriptDialogs::IsWebContentsForemost() {
  // As Chrome's: the tab the front window shows, or beside it in a split view.
  const std::vector<Window*> windows = Window::All();
  return !windows.empty() && Window::Showing(Tab::IdOf(contents_)) == windows.front();
}

bool ScriptDialogs::IsApp() {
  return false;
}

bool ScriptDialogs::CanShowModalUI() {
  tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(contents_);
  return tab && tab->CanShowModalUI();
}

}  // namespace browser_core
