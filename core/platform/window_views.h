// The core's native window and the views it puts in it, per platform.

#ifndef BROWSER_CORE_PLATFORM_WINDOW_VIEWS_H_
#define BROWSER_CORE_PLATFORM_WINDOW_VIEWS_H_

#include <cstddef>
#include <cstdint>
#include <optional>

#include "base/containers/span.h"
#include "base/functional/callback_forward.h"
#include "base/functional/callback_helpers.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/native_ui_types.h"

namespace content {
class RenderWidgetHostView;
}  // namespace content

namespace views {
class NativeWidget;
class Widget;
}  // namespace views

namespace browser_core::platform {

// The native window for |widget|, whose title bar is the app's: on the Mac, the UI fills it and
// the window's buttons sit in the app's header row. |leaving_full_screen| runs as the window starts
// to leave full screen, on a platform whose window can be taken out without finishing (the Mac's).
views::NativeWidget* NewNativeWidget(views::Widget* widget,
                                     base::RepeatingClosure leaving_full_screen);

// Fills |widget|'s window with the view pages are shown in, holding the app's |ui_view| under
// them: an NSView on the Mac, or null.
void AddWindowViews(views::Widget* widget, void* ui_view);

// A page's DevTools, docked: they fill the page's frame, and the page goes at |page_frame| in it,
// over them, or under them if |page_under|.
struct DockedDevTools {
  gfx::NativeView view;
  gfx::Rect page_frame;
  bool page_under;
};

// A page to show, at |frame| from the top-left of the window's content, with its DevTools if
// they're docked, and |color|, 0xRRGGBB, under it where it hasn't drawn, as the app's card is.
// |widget| is the view of its main frame, whose first frame on screen ends a switch to it, or null
// for a page in the system's engine.
struct Pane {
  gfx::NativeView page;
  gfx::Rect frame;
  std::optional<DockedDevTools> devtools;
  std::optional<uint32_t> color;
  content::RenderWidgetHostView* widget = nullptr;
};

// An extension's side panel page, at |frame| like a pane's. The app draws its header above it, so
// only its bottom corners are round.
struct SidePanelPage {
  gfx::NativeView page;
  gfx::Rect frame;
};

// Shows |panes| in place of the pages shown, with corners of |corner_radius|, and |side_panel| in
// place of the side panel shown. With more than one pane, as in a split view, the one at |focused|
// has a ring of |ring_color|, 0xRRGGBB, inside its edge. No panes shows no page. While |widget|
// isn't visible, the pages stay hidden. On the Mac, the pages a switch replaces stay over the ones
// it shows until those have drawn, as a page shown again shows nothing until then.
void ShowPages(views::Widget* widget,
               base::span<const Pane> panes,
               double corner_radius,
               size_t focused,
               uint32_t ring_color,
               std::optional<SidePanelPage> side_panel);

// Draws the panes shown in |widget|'s window, with their DevTools and ring, at |scale| of their
// size about the centre of their frames, without laying them out again, and keeps doing so as
// ShowPages lays them out, until it's 1 again. Clicks go to the UI around them on Linux, and to
// their frames on the Mac. On Windows it does nothing yet.
void ScalePages(views::Widget* widget, double scale);

// |point|, from the top-left of |page|, a page's view, as Chromium gives it, from where that
// top-left is laid out: as the app put it, whatever ScalePages draws. On Linux, where the scale is
// the view's own, Chromium gives it in the page drawn smaller; elsewhere it's unchanged.
gfx::PointF LaidOutPagePoint(gfx::NativeView page, const gfx::PointF& point);

// Keeps the pages shown in |widget|'s window where they are as it resizes, until ShowPages lays
// them out again: for a page going full screen, or back, which the app lays out once, at the
// window's new size. On the Mac, where the app hears of a resize before the pages are laid out at
// it, it does nothing.
void HoldPagesForResize(views::Widget* widget);

// Whether |widget|'s pages are held for a resize (HoldPagesForResize) that ShowPages hasn't ended.
bool PagesHeldForResize(views::Widget* widget);

// Hides the pages shown, their DevTools and the side panel, in place, for a window out of sight, so
// Chromium counts them hidden. ShowPages shows them again.
void HidePages(views::Widget* widget);

// Shows |page| in |view|, a view of the app's, which keeps it: at |frame| from the view's top-left,
// over what the view draws, with corners of |corner_radius|. For a page no window shows, such as a
// link preview's. Where the app can't hold a page in a view of its own, as on Linux, it shows over
// the pages of |widget|'s window, the one whose Browser holds it, at |frame| in its content, and
// the app's view over the window leaves it a hole. On Linux, |shown| runs once the first frame
// with the page at |frame| is on screen, unless ShowPageIn comes again first; Windows and the Mac
// drop it.
void ShowPageIn(views::Widget* widget,
                gfx::NativeView page,
                void* view,
                const gfx::Rect& frame,
                double corner_radius,
                base::OnceClosure shown);

// Draws |page|, which ShowPageIn shows, at |scale| of its frame's size about |about|, in the
// frame's coordinates, and at |opacity|, without laying it out again, until the next call or
// another page shown. Where it shows in the window, under the app's view; the Mac's page is in that
// view, which scales and fades it, and this does nothing.
void DrawPageIn(views::Widget* widget,
                gfx::NativeView page,
                float scale,
                const gfx::PointF& about,
                float opacity);

// Lets the pointer through |widget|, a window of the core's over the browser window's UI that
// shows something but takes nothing, such as a media card's video, to the UI under it. Views'
// accept_events does it on the Mac and Wayland, but not on X11 or Windows.
void PassPointerThrough(views::Widget* widget);

// Whether a page's dialog, such as Chrome's passkey dialog, needs a native window of its own to
// show over the pages, rather than being drawn in the window's own.
bool PageDialogsInOwnWindow();

// Whether a window's focus has settled by the time the window hears it's active, so the page can
// take the keyboard then, before the next key the window reads. Where it hasn't, the page takes it
// from a task posted then.
bool FocusSettledOnActivation();

// Whether the keyboard in |widget|'s window is the page's: in a page shown, or in nothing, as when
// the window opens or the page view that had it left the window. Not in DevTools or the side
// panel.
bool KeyboardForPage(views::Widget* widget);

// Gives the keyboard in |widget|'s window to the window itself, which KeyboardForPage counts as
// the page's.
void ReleaseKeyboard(views::Widget* widget);

// Whether an overlay of the app's over |widget|'s window, such as the command bar or a menu, has
// the keyboard, which takes it from the window on every platform: on the Mac a child window, key;
// on Windows an owned window, in front; on Linux lent (bc_window_overlay_takes_keyboard).
bool OverlayHasKeyboard(views::Widget* widget);

// Runs |let_go|, posted, once the overlay that has the keyboard of |widget|'s window lets it go: on
// the Mac as the overlay's window stops being key, on Windows as another window comes in front, on
// Linux as the app takes it back, unless X11's focus came back into the window.
void WhenOverlayLetsGo(views::Widget* widget, base::OnceClosure let_go);

// On Linux and Windows: the app's UI takes the keyboard in |widget|'s window, to keep
// (bc_window_ui_takes_keyboard) or until a page shows (bc_window_ui_holds_keyboard), which
// KeyboardForPage counts as the page's meanwhile. The Mac's window holds the keys for the next page
// itself.
void UITakesKeyboard(views::Widget* widget, bool until_page);

// Runs |change|, which may move the keyboard in |widget|'s window, then gives the keyboard back to
// what had it, if that's still in the window.
void KeepKeyboard(views::Widget* widget, base::OnceClosure change);

// Holds |page|, which no window shows, hidden in |widget|'s window until the runner returned runs,
// so it has the window's compositor to draw unseen with, as for a picture of it. Where a hidden
// page's view keeps its compositor, as on the Mac, or |page| is in a window already, it does
// nothing.
base::ScopedClosureRunner DrawUnseen(views::Widget* widget, gfx::NativeView page);

// Whether |view|, a page's, stretches the frame it last drew to fill itself as it resizes, until
// the page draws at the new size, rather than shows that frame at its old size in the top-left
// corner. Null for a page with no view yet.
void StretchOnResize(content::RenderWidgetHostView* view, bool stretch);

// On Linux and Windows: the frame of a tab's dialog, such as print preview, shown in |widget|'s
// window, from its content's top-left, if one shows (bc_window_dialog).
std::optional<gfx::Rect> DialogFrame(views::Widget* widget);

// On Windows: the widget of a tab's dialog, such as print preview, shown over |widget|'s window in
// a window of its own (PageDialogsInOwnWindow), or null.
views::Widget* PageDialog(views::Widget* widget);

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_WINDOW_VIEWS_H_
