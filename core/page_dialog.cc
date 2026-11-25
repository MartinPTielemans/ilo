#include "browser_core/page_dialog.h"

#include <algorithm>
#include <map>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/agent_page.h"
#include "browser_core/host.h"

namespace browser_core {

namespace {

struct Asked {
  bc_id id;
  bc_id tab;
  PageDialog::Words words;
  PageDialog::Answered answered;
};

// The dialogs, in the order Chrome asked them.
std::vector<Asked>& Dialogs() {
  static base::NoDestructor<std::vector<Asked>> dialogs;
  return *dialogs;
}

// The dialog each tab last told the app it asks.
std::map<bc_id, bc_id>& Told() {
  static base::NoDestructor<std::map<bc_id, bc_id>> told;
  return *told;
}

}  // namespace

// static
bc_id PageDialog::Ask(bc_id tab, Words words, Answered answered) {
  const bc_id id = NewId();
  const int kind = words.kind;
  Dialogs().push_back(Asked{id, tab, std::move(words), std::move(answered)});
  Changed(tab);
  // After the report is posted, so the app hears of the dialog before the job's answer.
  AgentPage::DialogShown(tab, kind);
  return id;
}

// static
void PageDialog::Withdraw(bc_id dialog) {
  std::vector<Asked>& dialogs = Dialogs();
  auto found = std::ranges::find(dialogs, dialog, &Asked::id);
  if (found == dialogs.end()) {
    return;
  }
  const bc_id tab = found->tab;
  dialogs.erase(found);
  Changed(tab);
}

// static
void PageDialog::Answer(bc_id tab, bc_id dialog, Reply reply) {
  std::vector<Asked>& dialogs = Dialogs();
  auto found = std::ranges::find(dialogs, dialog, &Asked::id);
  if (found == dialogs.end() || found->tab != tab) {
    return;
  }
  Answered answered = std::move(found->answered);
  dialogs.erase(found);
  Changed(tab);
  // Posted: the page goes on inside the call, and can ask again or close its tab.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(std::move(answered), std::move(reply)));
}

// static
bool PageDialog::Up(bc_id tab) {
  return std::ranges::contains(Dialogs(), tab, &Asked::tab);
}

// static
void PageDialog::Changed(bc_id tab) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&PageDialog::Report, tab));
}

// static
void PageDialog::Report(bc_id tab) {
  const auto first = std::ranges::find(Dialogs(), tab, &Asked::tab);
  const Asked* asked = first == Dialogs().end() ? nullptr : &*first;
  const auto told = Told().find(tab);
  if ((told == Told().end() ? 0 : told->second) == (asked ? asked->id : 0) ||
      !Host().tab_dialog) {
    return;
  }
  if (!asked) {
    Told().erase(told);
    Host().tab_dialog(Host().context, tab, nullptr);
    return;
  }
  Told()[tab] = asked->id;
  const Words& words = asked->words;
  const bc_page_dialog dialog = {
      .id = asked->id,
      .kind = words.kind,
      .title = words.title.c_str(),
      .message = words.message.c_str(),
      .text = words.text.c_str(),
      .accept = words.accept.c_str(),
      .cancel = words.cancel.c_str(),
      .check = words.check.c_str(),
      .username = words.username.c_str(),
      .password = words.password.c_str(),
  };
  Host().tab_dialog(Host().context, tab, &dialog);
}

}  // namespace browser_core
