#include "browser_core/idle_check.h"

#include <utility>

#include "base/functional/bind.h"

namespace browser_core {

IdleCheck::IdleCheck(Host& host, base::TimeDelta after) : host_(host), after_(after) {}

IdleCheck::~IdleCheck() = default;

void IdleCheck::Ask() {
  if (state_ == State::kIdle) {
    state_ = State::kPosted;
    // The core's IdleCheck lives as long as the app, and a test's runs its tasks before it ends.
    host_->Post(base::BindOnce(&IdleCheck::Start, base::Unretained(this)));
  } else if (state_ == State::kRunning) {
    again_ = true;
  }
}

// The tabs are taken as the check starts, as a switch may come back to them, or check.sh age them,
// between the ask and the start, and a best-effort task can wait seconds to start.
void IdleCheck::Start() {
  state_ = State::kRunning;
  again_ = false;
  unchecked_.clear();
  if (host_->On()) {
    const base::Time now = base::Time::Now();
    for (uint64_t tab : host_->Tabs()) {
      std::optional<base::Time> seen = host_->LastSeen(tab);
      if (seen && now - *seen >= after_) {
        unchecked_.push_back(tab);
      }
    }
  }
  Next();
}

// One tab a task, so the frames and keys of the switch that asked run between them: a page's
// teardown is 5-6 ms of the main thread, and a user back after hours has every tab idle at once,
// which in one task held a switch 164 ms (#2387).
void IdleCheck::Next() {
  if (!unchecked_.empty() && host_->On()) {
    const uint64_t tab = unchecked_.back();
    unchecked_.pop_back();
    // Unless it has been seen since the check started.
    std::optional<base::Time> seen = host_->LastSeen(tab);
    if (seen && base::Time::Now() - *seen >= after_) {
      host_->Unload(tab);
    }
    host_->Post(base::BindOnce(&IdleCheck::Next, base::Unretained(this)));
    return;
  }
  unchecked_.clear();
  state_ = State::kIdle;
  if (again_) {
    again_ = false;
    Ask();
    return;
  }
  host_->Checked();
}

}  // namespace browser_core
