#include "browser_core/page_key_wait.h"

#include <algorithm>
#include <cstddef>

namespace browser_core {

namespace {

// A page that never answers, as one hung, leaves its keys in |late_| until its tab or page goes.
// Past this many, the oldest go: a hand-back that old is long past being pressed for.
constexpr size_t kLateKept = 16;

void Bound(std::vector<PageKey>& list) {
  if (list.size() > kLateKept) {
    list.erase(list.begin(), list.end() - kLateKept);
  }
}

// Removes |key| from |list|, saying whether it was there.
bool Take(std::vector<PageKey>& list, const PageKey& key) {
  auto at = std::find(list.begin(), list.end(), key);
  if (at == list.end()) {
    return false;
  }
  list.erase(at);
  return true;
}

}  // namespace

PageKeyWait::PageKeyWait() = default;
PageKeyWait::~PageKeyWait() = default;

bool PageKeyWait::Sent(const PageKey& key, base::TimeTicks now, KeyGoesTo to) {
  if (to != KeyGoesTo::kPage) {
    return false;
  }
  waiting_.push_back({key, now + kPageKeyBound});
  return true;
}

bool PageKeyWait::Answered(const PageKey& key, bool kept) {
  auto at = std::find_if(waiting_.begin(), waiting_.end(),
                         [&](const Waiting& each) { return each.key == key; });
  if (at != waiting_.end()) {
    waiting_.erase(at);
    return true;
  }
  if (!Take(late_, key)) {
    // Never waited on, as a key sent before the browser watched its page: as it always was.
    return true;
  }
  if (!kept) {
    unkept_late_.push_back(key);
    Bound(unkept_late_);
  }
  return false;
}

bool PageKeyWait::RunsUnkept(const PageKey& key) {
  const auto same = [&](const PageKey& each) { return each.SamePress(key); };
  // Ran already: its late answer was heard first, or this is it.
  if (std::erase_if(unkept_late_, same) || std::erase_if(late_, same)) {
    return false;
  }
  // In time: this is its answer, which the watch may not hear.
  std::erase_if(waiting_, [&](const Waiting& each) { return each.key.SamePress(key); });
  return true;
}

void PageKeyWait::TakenBefore(const PageKey& key) {
  std::erase_if(waiting_, [&](const Waiting& each) {
    if (!each.key.SameKey(key)) {
      return false;
    }
    late_.push_back(each.key);
    return true;
  });
  Bound(late_);
}

std::vector<PageKey> PageKeyWait::Due(base::TimeTicks now) {
  std::vector<PageKey> due;
  std::erase_if(waiting_, [&](const Waiting& each) {
    if (each.due > now) {
      return false;
    }
    due.push_back(each.key);
    late_.push_back(each.key);
    return true;
  });
  Bound(late_);
  return due;
}

std::optional<base::TimeTicks> PageKeyWait::NextDue() const {
  if (waiting_.empty()) {
    return std::nullopt;
  }
  return std::min_element(waiting_.begin(), waiting_.end(),
                          [](const Waiting& a, const Waiting& b) { return a.due < b.due; })
      ->due;
}

bool PageKeyWait::Waits(const PageKey& key) const {
  return std::any_of(waiting_.begin(), waiting_.end(),
                     [&](const Waiting& each) { return each.key == key; });
}

bool PageKeyWait::HasKeysOf(uint64_t tab) const {
  return std::any_of(waiting_.begin(), waiting_.end(),
                     [&](const Waiting& each) { return each.key.tab == tab; }) ||
         std::any_of(late_.begin(), late_.end(),
                     [&](const PageKey& each) { return each.tab == tab; });
}

void PageKeyWait::TabGone(uint64_t tab) {
  std::erase_if(waiting_, [&](const Waiting& each) { return each.key.tab == tab; });
  std::erase_if(late_, [&](const PageKey& each) { return each.tab == tab; });
  std::erase_if(unkept_late_, [&](const PageKey& each) { return each.tab == tab; });
}

}  // namespace browser_core
