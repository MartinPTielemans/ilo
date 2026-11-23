#ifndef BROWSER_CORE_PAGE_KEY_WAIT_H_
#define BROWSER_CORE_PAGE_KEY_WAIT_H_

#include <cstdint>
#include <optional>
#include <vector>

#include "base/time/time.h"

namespace browser_core {

// A browser shortcut the page gets first, as Chrome gives it every shortcut it doesn't reserve,
// waits on the page's answer at most kPageKeyBound. Past it the browser runs it as one the page
// didn't keep, and the page's late answer is dropped, so a busy page has a say in the browser's
// keys but never a hold on them (#3944). Apart from content so browser_core_unittests tests it
// alone.

// Ours, and interim. An idle page answered in 0.7 ms at the median, and in 88 ms at the slowest,
// on CI's Windows runner just after ⌘T; this is twice that and more. Pages still loading took
// 10 ms at the median and 77 ms at p95, and 2 in 100 took longer than this.
inline constexpr base::TimeDelta kPageKeyBound = base::Milliseconds(200);

// A key going down that went to a tab's page first.
struct PageKey {
  uint64_t tab = 0;
  // Chromium's time for the event, which names the press.
  base::TimeTicks time;
  int code = 0;
  int modifiers = 0;

  // The same key, though maybe not the same press.
  bool SameKey(const PageKey& other) const {
    return tab == other.tab && code == other.code && modifiers == other.modifiers;
  }
  // The same press, in whichever tab: Chromium hands a key back without its tab.
  bool SamePress(const PageKey& other) const {
    return time == other.time && code == other.code && modifiers == other.modifiers;
  }
  bool operator==(const PageKey& other) const { return SameKey(other) && time == other.time; }
};

// Where a key the app let pass goes. Only the page's answer comes to the browser's watch: DevTools
// docked beside the page, when it has the keyboard, and a tab dialog's keys (dialog_keys), which
// ask the browser themselves with no page behind them, answer where it doesn't hear.
enum class KeyGoesTo { kPage, kPastThePage };

class PageKeyWait {
 public:
  PageKeyWait();
  PageKeyWait(const PageKeyWait&) = delete;
  PageKeyWait& operator=(const PageKeyWait&) = delete;
  ~PageKeyWait();

  // |key| went where |to| says at |now|. True if it waits on its page's answer, as only a key to
  // the page does: one past it is answered by its hand-back alone, or kept there.
  bool Sent(const PageKey& key, base::TimeTicks now, KeyGoesTo to = KeyGoesTo::kPage);

  // The page answered |key|, which it |kept| or not. True if it answered in time, so the browser
  // goes on as it always has: nothing for a key it kept, else the key runs as one it didn't keep.
  // False if the bound or a later press already ran it: its answer is dropped.
  bool Answered(const PageKey& key, bool kept);

  // Chromium hands |key| back as one the page didn't keep, by whichever route: the page's own, or
  // one the watch doesn't hear, as DevTools' or a tab dialog's. Whether the browser runs it: once,
  // so not for a key the bound or a later press already ran. A press still waiting is settled by
  // it, as it is the page's answer. Matched by the press alone, as Chromium doesn't say the tab.
  bool RunsUnkept(const PageKey& key);

  // The app ran |key|'s command before its page, as a second press within a second does. A press
  // of the same key still waiting is settled by it, and its late answer dropped.
  void TakenBefore(const PageKey& key);

  // The keys whose bound is past at |now|, in the order sent. They stop waiting, and their late
  // answers are dropped; the caller runs each as a key its page didn't keep.
  std::vector<PageKey> Due(base::TimeTicks now);

  // When the next key's bound ends, if one waits. The caller's timer runs only then.
  std::optional<base::TimeTicks> NextDue() const;

  // |tab|'s page is gone, or the tab. Its keys will never be answered, and none runs.
  void TabGone(uint64_t tab);

  // |key| waits on its page's answer.
  bool Waits(const PageKey& key) const;
  // A key of |tab|'s waits on its answer, or ran without it and its late answer is still to come.
  // The caller hears its page's answers only while one does.
  bool HasKeysOf(uint64_t tab) const;

  bool waiting() const { return !waiting_.empty(); }

 private:
  struct Waiting {
    PageKey key;
    base::TimeTicks due;
  };
  std::vector<Waiting> waiting_;
  // Keys run without the page's answer, whose answer is dropped when it comes.
  std::vector<PageKey> late_;
  // Keys whose late answer said the page didn't keep them, whose hand-back is dropped.
  std::vector<PageKey> unkept_late_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_KEY_WAIT_H_
