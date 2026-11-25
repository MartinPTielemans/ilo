#ifndef BROWSER_CORE_PAGE_DIALOG_H_
#define BROWSER_CORE_PAGE_DIALOG_H_

#include <string>

#include "base/functional/callback.h"
#include "browser_core/public/browser_core.h"

namespace browser_core {

// A page's own dialog, such as its alert(), which the app asks the user in place of Chrome's view
// (tab_dialog), in Chrome's words. A tab asks one at a time, in the order Chrome asked. Whoever
// asked keeps it until the user answers or it takes it back.
class PageDialog {
 public:
  // What it says, as bc_page_dialog has it.
  struct Words {
    int kind = BC_DIALOG_ALERT;
    std::string title;
    std::string message;
    std::string text;
    std::string accept;
    std::string cancel;
    std::string check;
    std::string username;
    std::string password;
  };

  // The user's answer. The texts are what was typed, for whoever asked and no one else.
  struct Reply {
    bool accept = false;
    std::u16string text;
    std::u16string password;
    bool checked = false;
  };

  using Answered = base::OnceCallback<void(const Reply& reply)>;

  // Asks |words| over |tab| and returns the dialog's id. |answered| runs once the user answers,
  // from a task of its own, unless Withdraw comes first.
  static bc_id Ask(bc_id tab, Words words, Answered answered);
  // Takes the dialog back unanswered, if it's still asked.
  static void Withdraw(bc_id dialog);
  // See bc_tab_answer_dialog.
  static void Answer(bc_id tab, bc_id dialog, Reply reply);
  // Whether a dialog is asked over |tab|, the one shown or one waiting behind it.
  static bool Up(bc_id tab);

 private:
  // Tells the app the tab's first dialog, or that it has none, from a task of its own: Chrome asks
  // and takes back inside the app's calls, such as the one that answers the dialog before.
  static void Changed(bc_id tab);
  static void Report(bc_id tab);
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_DIALOG_H_
