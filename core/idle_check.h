// How a check for idle tabs (idle_unload.h) runs, apart so browser_core_unittests tests its order
// alone: one at a time, taking the tabs idle when it starts rather than when it's asked for, and
// looking at one tab a task. And which tabs may let go of their pages, and which one does when
// memory is short, apart so it tests those rules alone.

#ifndef BROWSER_CORE_IDLE_CHECK_H_
#define BROWSER_CORE_IDLE_CHECK_H_

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

#include "base/functional/callback.h"
#include "base/functional/function_ref.h"
#include "base/memory/raw_ref.h"
#include "base/time/time.h"

namespace browser_core {

// What the core knows of a tab as it asks whether the tab may let go of its page.
struct UnloadFacts {
  // Its WebContents is gone, as the tab closes.
  bool closing = false;
  // A window shows it, or has a picture of it while another shows it.
  bool shown = false;
  // A private window's, whose session ends with it anyway, or one with no profile.
  bool private_window = false;
  // In an agent's Space, where an agent may be working in it out of sight.
  bool agents = false;
  // The system's engine's page, which may be playing or hold what the user typed, and which
  // Chrome's checks can't see into.
  bool system_page = false;
  // It has a page: not one opened behind and not yet shown, nor one already let go.
  bool has_page = true;
  bool devtools_open = false;
  // Its page asks before it's left, and would never get to.
  bool asks_before_leaving = false;
  // In a call, even one that takes no camera, microphone or screen and makes no sound, such as
  // one listening alone, which Chrome's checks would let go (#686).
  bool in_call = false;
  // Chrome's Memory Saver checks keep it, or couldn't look: sound now or lately, capturing,
  // picture-in-picture, typed form text, a device connected, an extension keeping it.
  bool chrome_keeps = false;
};

// Whether a tab may let go of its page now without the user losing anything. One with no page has
// none to lose.
bool MayUnload(const UnloadFacts& tab);

// The tab that lets go of its page each time the system says memory is short: of |tabs|, each with
// when it was last seen, the one unseen longest whose page |facts| says may go, at any age. Only a
// tab with a page counts, so each notification frees one, and once only the kept ones are left,
// none. |facts| is asked of the tabs oldest first, until one may go.
std::optional<uint64_t> PickUnderPressure(
    std::vector<std::pair<uint64_t, base::Time>> tabs,
    base::FunctionRef<UnloadFacts(uint64_t)> facts);

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
