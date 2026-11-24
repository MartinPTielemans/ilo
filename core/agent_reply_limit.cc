#include "browser_core/agent_reply_limit.h"

#include <optional>
#include <utility>

#include "base/functional/bind.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"

namespace browser_core {

AgentReplyLimit::AgentReplyLimit(Ends ends) : ends_(std::move(ends)) {}

AgentReplyLimit::~AgentReplyLimit() = default;

void AgentReplyLimit::Started() {
  working_ = true;
  // A look out keeps counting from the page's last word.
  if (!looks_) {
    timer_.Stop();
  }
}

void AgentReplyLimit::Ended() {
  working_ = false;
  if (!looks_) {
    timer_.Stop();
  }
}

void AgentReplyLimit::Looking(size_t looks) {
  looks_ = looks;
  if (!waits()) {
    timer_.Stop();
  }
}

void AgentReplyLimit::Sent() {
  if (!waits()) {
    return;
  }
  // Unretained: the timer is this object's, and stops with it.
  timer_.Start(FROM_HERE, kLimit,
               base::BindOnce(&AgentReplyLimit::Reached, base::Unretained(this)));
}

void AgentReplyLimit::Reached() {
  End(base::StrCat({"The tab's page didn't answer for ", base::NumberToString(kLimit.InSeconds()),
                    " s, so the action stopped part way. The page may be busy or stuck; "
                    "close_tab closes the tab."}),
      /*unstarted=*/std::nullopt);
}

void AgentReplyLimit::Replied(size_t owed) {
  if (!waits()) {
    return;
  }
  if (owed) {
    Sent();
  } else {
    timer_.Stop();
  }
}

void AgentReplyLimit::DialogShown(std::string_view what) {
  if (!waits()) {
    return;
  }
  static constexpr std::string_view kStuck =
      " Nothing more can be done on the tab while it's up, and no tool answers it. close_tab "
      "closes the tab, and the dialog with it.";
  // The action running may have taken effect, and those queued never started: the agent redoes
  // them by what each was told.
  End(base::StrCat({"The tab's page showed ", what, " while the action ran.", kStuck}),
      base::StrCat({"The tab's page showed ", what,
                    " before this action started, so it wasn't done.", kStuck}));
}

void AgentReplyLimit::End(std::string why, std::optional<std::string> unstarted) {
  // Once: the job is over whether or not its page has said Ended(), and the looks out with it.
  looks_ = 0;
  Ended();
  ends_.Run(std::move(why), std::move(unstarted));
}

}  // namespace browser_core
