#include "browser_core/window_frames.h"

namespace browser_core {

namespace {

// Enough for the moves a window manager makes on its way to the maximized frame.
constexpr size_t kKept = 4;

}  // namespace

void KeepNormalFrame(std::vector<gfx::Rect>& frames, const gfx::Rect& frame) {
  if (frame.IsEmpty() || (!frames.empty() && frames.back() == frame)) {
    return;
  }
  frames.push_back(frame);
  if (frames.size() > kKept) {
    frames.erase(frames.begin(), frames.end() - kKept);
  }
}

gfx::Rect FrameBeforeMaximized(const std::vector<gfx::Rect>& frames, const gfx::Rect& maximized) {
  for (auto frame = frames.rbegin(); frame != frames.rend(); ++frame) {
    if (*frame != maximized) {
      return *frame;
    }
  }
  return maximized;
}

}  // namespace browser_core
