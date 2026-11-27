#include "browser_core/closed_windows.h"

#include <algorithm>
#include <utility>

#include "base/no_destructor.h"
#include "browser_core/host.h"
#include "browser_core/session.h"

namespace browser_core {

namespace {

// As long as the archive keeps a tab, and as many as Chrome's recently closed keeps.
constexpr base::TimeDelta kKeepFor = base::Days(30);
constexpr size_t kMaxClosedWindows = 25;

std::vector<ClosedWindow>& Windows() {
  static base::NoDestructor<std::vector<ClosedWindow>> windows;
  return *windows;
}

void Prune() {
  const base::Time oldest = base::Time::Now() - kKeepFor;
  std::erase_if(Windows(), [oldest](const ClosedWindow& window) { return window.closed < oldest; });
  if (Windows().size() > kMaxClosedWindows) {
    Windows().resize(kMaxClosedWindows);
  }
}

}  // namespace

const std::vector<ClosedWindow>& ClosedWindows() {
  return Windows();
}

void LoadClosedWindows(std::vector<ClosedWindow> windows) {
  Windows() = std::move(windows);
  for (ClosedWindow& window : Windows()) {
    window.id = NewId();
  }
  Prune();
}

void KeepClosedWindow(ClosedWindow window) {
  window.id = NewId();
  Windows().insert(Windows().begin(), std::move(window));
  Prune();
  SessionChanged();
}

std::optional<ClosedWindow> TakeClosedWindow(bc_id id) {
  auto it = std::ranges::find(Windows(), id, &ClosedWindow::id);
  if (it == Windows().end()) {
    return std::nullopt;
  }
  ClosedWindow window = std::move(*it);
  Windows().erase(it);
  SessionChanged();
  return window;
}

}  // namespace browser_core
