#include "browser_core/agent_reply_limit.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"

namespace browser_core {

AgentReplyLimit::AgentReplyLimit(Ends ends) : ends_(std::move(ends)) {}

AgentReplyLimit::~AgentReplyLimit() = default;

void AgentReplyLimit::Started() {
  working_ = true;
  timer_.Stop();
}

void AgentReplyLimit::Ended() {
  working_ = false;
  timer_.Stop();
}

void AgentReplyLimit::Sent() {
  if (!working_) {
    return;
  }
  // Unretained: the timer is this object's, and stops with it.
  timer_.Start(FROM_HERE, kLimit,
               base::BindOnce(&AgentReplyLimit::Reached, base::Unretained(this)));
}

void AgentReplyLimit::Reached() {
  End(base::StrCat({"The tab's page didn't answer for ", base::NumberToString(kLimit.InSeconds()),
                    " s, so the action stopped part way. The page may be busy or stuck; "
                    "close_tab closes the tab."}));
}

void AgentReplyLimit::Replied(size_t owed) {
  if (!working_) {
    return;
  }
  if (owed) {
    Sent();
  } else {
    timer_.Stop();
  }
}

void AgentReplyLimit::DialogShown(std::string_view what) {
  if (!working_) {
    return;
  }
  End(base::StrCat({"The tab's page showed ", what,
                    " while the action ran. Nothing more can be done on the tab while it's up, "
                    "and no tool answers it. close_tab closes the tab, and the dialog with it."}));
}

void AgentReplyLimit::End(std::string why) {
  // Once: the job is over whether or not its page has said Ended().
  Ended();
  ends_.Run(std::move(why));
}

}  // namespace browser_core
