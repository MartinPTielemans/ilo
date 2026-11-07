#include "browser_core/window_frames.h"

#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

const gfx::Rect kPlaced(60, 80, 1000, 700);
const gfx::Rect kMoved(120, 90, 1000, 700);
const gfx::Rect kMaximized(0, 0, 1280, 800);

// Mutter and KWin say the window is maximized before they give it the frame, so the newest frame
// kept is the one it had.
TEST(WindowFramesTest, AMaximizeSaidFirstUnmaximizesToTheNewestFrame) {
  std::vector<gfx::Rect> frames;
  KeepNormalFrame(frames, kPlaced);
  KeepNormalFrame(frames, kMoved);
  EXPECT_EQ(FrameBeforeMaximized(frames, kMaximized), kMoved);
}

// openbox gives the window the maximized frame first, while it's still normal (#1454).
TEST(WindowFramesTest, AMaximizedFrameGivenFirstIsPassedOver) {
  std::vector<gfx::Rect> frames;
  KeepNormalFrame(frames, kPlaced);
  KeepNormalFrame(frames, kMaximized);
  KeepNormalFrame(frames, kMaximized);
  EXPECT_EQ(FrameBeforeMaximized(frames, kMaximized), kPlaced);
}

// A window that was never anything but maximized has nowhere smaller to go.
TEST(WindowFramesTest, WithNoOtherFrameItStaysMaximized) {
  std::vector<gfx::Rect> frames;
  EXPECT_EQ(FrameBeforeMaximized(frames, kMaximized), kMaximized);
  KeepNormalFrame(frames, kMaximized);
  EXPECT_EQ(FrameBeforeMaximized(frames, kMaximized), kMaximized);
}

// Only the last few are kept, a repeat and an empty frame not at all.
TEST(WindowFramesTest, TheLastFewDifferentFramesAreKept) {
  std::vector<gfx::Rect> frames;
  KeepNormalFrame(frames, gfx::Rect());
  for (int x = 0; x < 10; ++x) {
    KeepNormalFrame(frames, gfx::Rect(x, 0, 800, 600));
    KeepNormalFrame(frames, gfx::Rect(x, 0, 800, 600));
  }
  ASSERT_EQ(frames.size(), 4u);
  EXPECT_EQ(frames.front(), gfx::Rect(6, 0, 800, 600));
  EXPECT_EQ(frames.back(), gfx::Rect(9, 0, 800, 600));
}

}  // namespace
}  // namespace browser_core
