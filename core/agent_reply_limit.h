#ifndef BROWSER_CORE_AGENT_REPLY_LIMIT_H_
#define BROWSER_CORE_AGENT_REPLY_LIMIT_H_

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>

#include "base/functional/callback.h"
#include "base/time/time.h"
#include "base/timer/timer.h"

namespace browser_core {

// How long an agent's job, or a look with none (AgentPage::Freeze), waits on its page's renderer,
// which a page's dialog or a hung page keeps from answering (#3536). The job ends with what the
// agent hears, and the next one runs; a look ends unheard. Apart from content so
// browser_core_unittests tests it alone.
class AgentReplyLimit {
 public:
  // As long as Chromium waits on a renderer before it calls the page hung on the desktop
  // (input::kHungRendererDelay, 15 s at 157), and as long as a job waits for its page to load.
  static constexpr base::TimeDelta kLimit = base::Seconds(15);

  // |why| as the agent hears it. |unstarted|, with a dialog the jobs queued behind this one would
  // wait on too, what each of them hears instead of running; without it they run as before.
  using Ends =
      base::RepeatingCallback<void(std::string why, std::optional<std::string> unstarted)>;
  explicit AgentReplyLimit(Ends ends);
  AgentReplyLimit(const AgentReplyLimit&) = delete;
  AgentReplyLimit& operator=(const AgentReplyLimit&) = delete;
  ~AgentReplyLimit();

  // A job started, or answered. The limit counts only between, or while a look is out, so an idle
  // page has no timer.
  void Started();
  void Ended();
  // How many looks are out, a read's or a request's with no job among them.
  void Looking(size_t looks);
  // The job sent the page a command, whose reply it's owed.
  void Sent();
  // A reply came, with |owed| still owed. The limit counts from the page's last word, and not
  // once nothing is owed, as when the job waits on a timer of its own.
  void Replied(size_t owed);
  // The page showed a dialog, |what| as the agent hears it ("an alert"). The renderer waits on its
  // answer, so the job running ends now, and the jobs queued behind it, which never started, and
  // the looks out.
  void DialogShown(std::string_view what);

  // Whether the limit is counting, which it is only while a job or a look waits on a reply.
  bool counting() const { return timer_.IsRunning(); }

 private:
  void Reached();
  void End(std::string why, std::optional<std::string> unstarted);
  bool waits() const { return working_ || looks_; }

  const Ends ends_;
  bool working_ = false;
  size_t looks_ = 0;
  base::OneShotTimer timer_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_REPLY_LIMIT_H_
