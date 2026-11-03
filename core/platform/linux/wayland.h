// The app's UI on Chromium's Wayland connection: a subsurface of the browser window's surface,
// under the pages, on the display Chromium reads.

#ifndef BROWSER_CORE_PLATFORM_LINUX_WAYLAND_H_
#define BROWSER_CORE_PLATFORM_LINUX_WAYLAND_H_

#include <optional>
#include <string>

#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/size.h"
#include "ui/views/widget/widget.h"

namespace views {
class WidgetDelegate;
}  // namespace views

namespace browser_core::platform {

// Whether Chromium runs on Wayland, rather than X11.
bool OnWayland();

// Chromium's wl_display, from now on calling |dispatched| after each time Chromium dispatches it
// (bc_wayland_display). Null off Wayland.
void* JoinWaylandDisplay(void (*dispatched)());

// |widget|'s wl_surface, and its content's size in DIPs. Null off Wayland.
void* WaylandSurfaceOf(views::Widget* widget, gfx::Size* size);

// Makes the browser window frameless, under Wayland and X11, as the app draws the window's buttons,
// and under Wayland see-through, for the app's UI under it to show and take the clicks the pages
// don't.
void SetUpWindow(views::Widget::InitParams& params, views::WidgetDelegate& delegate);

// The width in DIPs of the band inside |widget|'s edges that resizes it, which Chromium's hit test
// takes (SetUpWindow) and the app's UI lets through to the window (window_views.cc). None while
// the window is maximized or full screen.
int ResizeEdgeOf(views::Widget* widget);

// Has the window manager or compositor move |widget|'s window with the pointer, its button held
// down after a press in the app's UI, as Chromium does for a press in its own title bar
// (bc_window_start_move). With |press|, where the press was in |widget| in DIPs, X11's window
// manager starts from there rather than from where the pointer is now, so the window keeps the
// press under the pointer (mini_player.cc). In window_views.cc.
void StartMove(views::Widget* widget, std::optional<gfx::Point> press = std::nullopt);

// Whether the keys in |widget|'s window are the app's UI's (bc_window_keyboard_is_ui). In
// window_views.cc, as on X11 the core follows the X server's focus.
bool KeyboardIsUI(views::Widget* widget);

// Whether a window of Chromium's own in |widget|'s window has the keyboard, such as an extension's
// popup or a tab's dialog (bc_window_keyboard_in_child). In window_views.cc.
bool KeyboardInChild(views::Widget* widget);

// The frame of |widget|'s pane whose page has the keyboard, in its window's content, or none
// (bc_window_keyboard_page). In window_views.cc.
std::optional<gfx::Rect> KeyboardPageFrame(views::Widget* widget);

// The title of the window of Chromium's own in |widget|'s with the keys (KeyboardInChild), such as
// a tab's dialog, or an empty one. In window_views.cc.
std::u16string ChildTitle(views::Widget* widget);

// Lends the keyboard in |widget|'s window to an overlay of the app's, or takes it back
// (bc_window_overlay_takes_keyboard).
void OverlayTakesKeyboard(views::Widget* widget, bool takes);

// A text field of the app's UI in |widget|'s window, or of an overlay over it, is focused with its
// caret at |caret|, in DIPs from the window's top left, or none is (bc_window_ui_text_input).
void SetUITextInput(views::Widget* widget, std::optional<gfx::Rect> caret);

// Answers on X11 the clipboard requests in Chromium's queue, and nothing else in it
// (bc_answer_clipboard_requests). In window_views.cc.
void AnswerClipboardRequests();

// From now on, on X11, a bubble of Chrome's on a page's Picture in Picture window, such as its
// automatic picture-in-picture prompt, is a window of its own, as it is on Wayland, not a child
// clipped to the Picture in Picture window, which Chrome would then grow to hold it (#1161). In
// window_views.cc.
void FloatPictureInPictureBubbles();

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_LINUX_WAYLAND_H_
