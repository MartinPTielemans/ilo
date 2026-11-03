#include "browser_core/platform/linux/wayland.h"

#include <wayland-client-core.h>

#include <memory>

#include "base/auto_reset.h"
#include "base/functional/bind.h"
#include "browser_core/hooks.h"
#include "chrome/browser/shell_integration_linux.h"
#include "ui/aura/window.h"
#include "ui/aura/window_tree_host.h"
#include "ui/base/hit_test.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/size_conversions.h"
#include "ui/ozone/platform/wayland/host/proxy/wayland_proxy.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/window/frame_view.h"
#include "ui/views/window/native_frame_view.h"

// On Wayland the app's UI shares Chromium's connection, so it can be a subsurface of the browser
// window's surface, under it: a surface on another connection can't be put in Chromium's. Chromium
// draws the pages into its window as Chrome does, each in an aura window of its own, and leaves the
// rest of its surface clear and outside its input region, so the UI under it shows and takes the
// clicks there (window_views.cc). The engine's chromium-core-wayland.patch adds what this reaches
// into.

namespace browser_core::platform {

namespace {

// The resize edge's width, Arc's and Dia's outer 3 pt (docs/ux-spec.md › Dragging the window), here
// inside the window, so thinner than the card's margin, where the pointer stopped at the window's
// left edge brings the hidden sidebar out. The corners' length along each edge is Chrome's own
// frame's on Linux (OpaqueBrowserFrameView). In DIPs.
constexpr int kResizeEdge = 3;
constexpr int kResizeCorner = 16;

void (*g_app_dispatched)() = nullptr;

// What Chromium calls after each dispatch. The app's dispatch can call the core, and the core
// Chromium, whose round trips land here again: the app's dispatch isn't reentrant, and the outer one
// takes what the inner would have.
void DispatchApp() {
  static bool dispatching = false;
  if (dispatching) {
    return;
  }
  base::AutoReset<bool> reset(&dispatching, true);
  g_app_dispatched();
}

// The window's content, in DIPs.
gfx::Size ContentSizeOf(views::Widget* widget) {
  return gfx::ScaleToFlooredSize(
      widget->GetNativeWindow()->GetHost()->GetBoundsInPixels().size(),
      1 / widget->GetNativeWindow()->GetHost()->device_scale_factor());
}

}  // namespace

bool OnWayland() {
  return wl::WaylandProxy::GetInstance() != nullptr;
}

void* JoinWaylandDisplay(void (*dispatched)()) {
  wl::WaylandProxy* proxy = wl::WaylandProxy::GetInstance();
  if (!proxy) {
    return nullptr;
  }
  // Chromium calls the core, never the app: Clang's indirect-call check in Chromium's code would
  // trap on a function of the app's (cfi_ignores.txt).
  g_app_dispatched = dispatched;
  g_browser_core_wayland_dispatched = dispatched ? &DispatchApp : nullptr;
  return wl_proxy_get_display(reinterpret_cast<wl_proxy*>(proxy->GetRegistry()));
}

void* WaylandSurfaceOf(views::Widget* widget, gfx::Size* size) {
  wl::WaylandProxy* proxy = wl::WaylandProxy::GetInstance();
  if (!proxy) {
    return nullptr;
  }
  *size = ContentSizeOf(widget);
  return proxy->GetWlSurfaceForAcceleratedWidget(
      widget->GetNativeWindow()->GetHost()->GetAcceleratedWidget());
}

void SetUpWindow(views::Widget::InitParams& params, views::WidgetDelegate& delegate) {
  // Translucent, or the surface's opaque region lets the compositor skip drawing the UI under it.
  if (OnWayland()) {
    params.opacity = views::Widget::InitParams::WindowOpacity::kTranslucent;
  }
  // No frame of the window manager's, whose buttons would show beside the app's: on X11 the window
  // asks for none (_MOTIF_WM_HINTS), and under Wayland one would also make the compositor ignore
  // the input region.
  params.remove_standard_frame = true;
  // Named as Chrome's BrowserView names its windows (browser_native_widget_aura_linux.cc), which
  // the core's aren't: the dock matches the app id or WM_CLASS to the app's desktop file, and the
  // Wayland driver tells a browser window from Settings, which has none, by it.
  params.wm_class_name = shell_integration_linux::GetProgramClassName();
  params.wm_class_class = shell_integration_linux::GetProgramClassClass();
  params.wayland_app_id = params.wm_class_class;
  // A frame view that draws nothing, in place of GTK's title bar, with Chromium's own frameless
  // resizing: a press in the band at its edges asks the compositor or window manager to resize
  // the window (xdg_toplevel.resize, _NET_WM_MOVERESIZE), and the pointer there takes the edge's
  // cursor. The app's header moves the window (bc_window_start_move).
  delegate.SetFrameViewFactory(
      base::BindRepeating([](views::Widget* widget) -> std::unique_ptr<views::FrameView> {
        auto frame = std::make_unique<views::NativeFrameView>(widget);
        // Unretained: the frame view holds the callback, and the widget holds the frame view.
        frame->set_non_client_hit_test_callback(base::BindRepeating(
            [](views::FrameView* frame, views::Widget* widget, const gfx::Point& point) {
              const int edge = ResizeEdgeOf(widget);
              if (!edge) {
                return static_cast<int>(HTNOWHERE);
              }
              return frame->GetHTComponentForFrame(point, gfx::Insets(edge), kResizeCorner,
                                                   kResizeCorner,
                                                   widget->widget_delegate()->CanResize());
            },
            base::Unretained(frame.get()), base::Unretained(widget)));
        return frame;
      }));
}

int ResizeEdgeOf(views::Widget* widget) {
  return widget->IsMaximized() || widget->IsFullscreen() ? 0 : kResizeEdge;
}

}  // namespace browser_core::platform
