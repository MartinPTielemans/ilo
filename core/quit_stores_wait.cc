#include "browser_core/quit_stores_wait.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/memory/ref_counted.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/task/single_thread_task_runner.h"

namespace browser_core {

namespace {

// The answer and the limit race for |done|; the first to come takes it.
class Wait : public base::RefCounted<Wait> {
 public:
  explicit Wait(base::OnceCallback<void(std::optional<std::string>)> done)
      : done_(std::move(done)) {}

  void Answered(StoresWritten written) {
    switch (written) {
      case StoresWritten::kNone:
        End(std::nullopt);
        return;
      case StoresWritten::kWritten:
        End(base::StrCat({"system stores written at ",
                          base::NumberToString((base::TimeTicks::Now() - start_).InMilliseconds()),
                          " ms"}));
        return;
      case StoresWritten::kNoWay:
        End("system stores not written, no way to write them");
        return;
    }
  }

  void Reached() {
    End(base::StrCat({"system stores not written, gave up at ",
                      base::NumberToString(kStoresWriteLimit.InMilliseconds()), " ms"}));
  }

  bool ended() const { return done_.is_null(); }

 private:
  friend class base::RefCounted<Wait>;
  ~Wait() = default;

  void End(std::optional<std::string> line) {
    if (done_) {
      std::move(done_).Run(std::move(line));
    }
  }

  const base::TimeTicks start_ = base::TimeTicks::Now();
  base::OnceCallback<void(std::optional<std::string>)> done_;
};

}  // namespace

void WaitForStores(base::OnceCallback<void(base::OnceCallback<void(StoresWritten)>)> write,
                   base::OnceCallback<void(std::optional<std::string> line)> done) {
  auto wait = base::MakeRefCounted<Wait>(std::move(done));
  std::move(write).Run(base::BindOnce(&Wait::Answered, wait));
  // An answer that came at once needs no limit, so a quit that had nothing to write posts nothing.
  if (!wait->ended()) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostDelayedTask(
        FROM_HERE, base::BindOnce(&Wait::Reached, wait), kStoresWriteLimit);
  }
}

}  // namespace browser_core
