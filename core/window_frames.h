// The frame a window the window manager maximized un-maximizes to (Window::RestoredFrame), apart so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_WINDOW_FRAMES_H_
#define BROWSER_CORE_WINDOW_FRAMES_H_

#include <vector>

#include "ui/gfx/geometry/rect.h"

namespace browser_core {

// Keeps |frame|, one a window had out of maximized and full screen, as the newest of |frames|: the
// last few that differ, newest last, for FrameBeforeMaximized.
void KeepNormalFrame(std::vector<gfx::Rect>& frames, const gfx::Rect& frame);

// The frame a window the window manager maximized on X11 un-maximizes to, which Chromium knows only
// when it asked for the maximize itself: the newest of |frames| (KeepNormalFrame) that isn't
// |maximized|, the frame it has maximized. A window manager such as openbox gives the window that
// frame before it says the window is maximized, so the newest may be it. |maximized| if none is
// another.
gfx::Rect FrameBeforeMaximized(const std::vector<gfx::Rect>& frames, const gfx::Rect& maximized);

}  // namespace browser_core

#endif  // BROWSER_CORE_WINDOW_FRAMES_H_
