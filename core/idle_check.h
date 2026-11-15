// How a check for idle tabs (idle_unload.h) runs, apart so browser_core_unittests tests its order
// alone: one at a time, taking the tabs idle when it starts rather than when it's asked for, and
// looking at one tab a task.

#ifndef BROWSER_CORE_IDLE_CHECK_H_
#define BROWSER_CORE_IDLE_CHECK_H_

#include <cstdint>
#include <optional>
#include <vector>

#include "base/functional/callback.h"
#include "base/memory/raw_ref.h"
#include "base/time/time.h"

namespace browser_core {

class IdleCheck {
 public:
  // What a check asks of the core. Tabs are their bc_ids.
  class Host {
   public:
    virtual ~Host() = default;
    // Runs |task| once the work at hand is done.
    virtual void Post(base::OnceClosure task) = 0;
    // Whether tabs may be unloaded at all: the user's setting, and the app not quitting.
    virtual bool On() = 0;
    virtual std::vector<uint64_t> Tabs() = 0;
    // When |tab| was last seen, or none once it's closed.
    virtual std::optional<base::Time> LastSeen(uint64_t tab) = 0;
    // Lets |tab|, still idle, go of its page if nothing the user would lose holds it there.
    virtual void Unload(uint64_t tab) = 0;
    // A check has looked at every tab idle since the last ask.
    virtual void Checked() = 0;
  };

  // Unloads the tabs unseen for |after|.
  IdleCheck(Host& host, base::TimeDelta after);
  IdleCheck(const IdleCheck&) = delete;
  IdleCheck& operator=(const IdleCheck&) = delete;
  ~IdleCheck();

  // Checks soon. While one is still to start, that one will do; while one is under way, another
  // follows it, for the tabs it took before they were idle.
  void Ask();

 private:
  enum class State { kIdle, kPosted, kRunning };

  void Start();
  void Next();

  const raw_ref<Host> host_;
  const base::TimeDelta after_;
  State state_ = State::kIdle;
  bool again_ = false;
  // The tabs idle when the check started that it's still to look at.
  std::vector<uint64_t> unchecked_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_IDLE_CHECK_H_
