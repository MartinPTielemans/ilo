#include "browser_core/platform/linux/wayland.h"

#include <wayland-client-core.h>
#include <wayland-client-protocol.h>

#include <algorithm>
#include <cstring>
#include <memory>

#include "base/auto_reset.h"
#include "base/containers/flat_map.h"
#include "base/functional/bind.h"
#include "base/memory/raw_ptr.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/hooks.h"
#include "chrome/browser/shell_integration_linux.h"
#include "ui/aura/window.h"
#include "ui/aura/window_tree_host.h"
#include "ui/base/hit_test.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/rect_conversions.h"
#include "ui/gfx/geometry/size_conversions.h"
#include "ui/gfx/native_ui_types.h"
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

// Hears each configure a window acks, which Chromium does as it draws the first frame of the state
// the configure brought, and runs what waits for one with the state the window has. A state the
// browser asks for is the window's at once, and the compositor's configure with its bounds comes
// after, maybe after configures it had sent before, such as sway's answer to the last commit.
class ConfigureWatcher : public wl::WaylandProxy::Delegate {
 public:
  // The proxy's one delegate, which nothing else sets outside tests.
  static ConfigureWatcher& Get(wl::WaylandProxy* proxy) {
    static ConfigureWatcher* const watcher = [proxy] {
      auto* made = new ConfigureWatcher(proxy);
      proxy->SetDelegate(made);
      return made;
    }();
    return *watcher;
  }

  // The latest wait for a window replaces the one before it.
  void Wait(gfx::AcceleratedWidget window, base::OnceClosure done) {
    waiting_[window] = std::move(done);
  }

  // wl::WaylandProxy::Delegate:
  void OnWindowAdded(gfx::AcceleratedWidget) override {}
  void OnWindowRemoved(gfx::AcceleratedWidget window) override { waiting_.erase(window); }
  void OnWindowConfigured(gfx::AcceleratedWidget window, bool configured) override {
    auto waiting = waiting_.find(window);
    if (waiting == waiting_.end() || !configured || !proxy_->IsWindowStateConfigured(window)) {
      return;
    }
    base::OnceClosure done = std::move(waiting->second);
    waiting_.erase(waiting);
    // Posted, as the ack comes in the middle of Chromium's frame.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, std::move(done));
  }
  void OnWindowRoleAssigned(gfx::AcceleratedWidget) override {}

 private:
  explicit ConfigureWatcher(wl::WaylandProxy* proxy) : proxy_(proxy) {}

  const raw_ptr<wl::WaylandProxy> proxy_;
  base::flat_map<gfx::AcceleratedWidget, base::OnceClosure> waiting_;
};

// The window's content, in DIPs.
gfx::Size ContentSizeOf(views::Widget* widget) {
  return gfx::ScaleToFlooredSize(
      widget->GetNativeWindow()->GetHost()->GetBoundsInPixels().size(),
      1 / widget->GetNativeWindow()->GetHost()->device_scale_factor());
}

// The compositor global on Chromium's display, bound on a queue of the core's own, so the round
// trip that finds it dispatches none of Chromium's events. Its requests make regions, which have
// no events.
wl_compositor* Compositor(wl_display* display) {
  static wl_compositor* const compositor = [display] {
    static const wl_registry_listener listener = {
        .global =
            [](void* data, wl_registry* registry, uint32_t name, const char* interface,
               uint32_t version) {
              auto* found = static_cast<wl_compositor**>(data);
              if (!*found && std::strcmp(interface, wl_compositor_interface.name) == 0) {
                *found = static_cast<wl_compositor*>(wl_registry_bind(
                    registry, name, &wl_compositor_interface, std::min<uint32_t>(version, 4)));
              }
            },
        .global_remove = [](void*, wl_registry*, uint32_t) {},
    };
    wl_event_queue* queue = wl_display_create_queue(display);
    auto* wrapped = static_cast<wl_display*>(wl_proxy_create_wrapper(display));
    wl_proxy_set_queue(reinterpret_cast<wl_proxy*>(wrapped), queue);
    wl_registry* registry = wl_display_get_registry(wrapped);
    wl_proxy_wrapper_destroy(wrapped);
    wl_compositor* found = nullptr;
    wl_registry_add_listener(registry, &listener, &found);
    wl_display_roundtrip_queue(display, queue);
    wl_registry_destroy(registry);
    if (found) {
      wl_proxy_set_queue(reinterpret_cast<wl_proxy*>(found), nullptr);
    }
    wl_event_queue_destroy(queue);
    return found;
  }();
  return compositor;
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

void AfterStateConfigured(views::Widget* widget, base::OnceClosure done) {
  wl::WaylandProxy* proxy = wl::WaylandProxy::GetInstance();
  const gfx::AcceleratedWidget window =
      widget->GetNativeWindow()->GetHost()->GetAcceleratedWidget();
  if (!proxy || proxy->IsWindowStateConfigured(window)) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, std::move(done));
    return;
  }
  ConfigureWatcher::Get(proxy).Wait(window, std::move(done));
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

void SetInputRegionNow(views::Widget* widget, const std::vector<gfx::Rect>& region, float scale) {
  gfx::Size size;
  auto* surface = static_cast<wl_surface*>(WaylandSurfaceOf(widget, &size));
  if (!surface) {
    return;
  }
  wl_display* display = wl_proxy_get_display(reinterpret_cast<wl_proxy*>(surface));
  wl_compositor* compositor = Compositor(display);
  if (!compositor) {
    return;
  }
  // In the surface's coordinates, which are DIPs, as Chromium's own region is given.
  wl_region* input = nullptr;
  if (!region.empty()) {
    input = wl_compositor_create_region(compositor);
    for (const gfx::Rect& rect : region) {
      const gfx::Rect dips = gfx::ScaleToEnclosingRect(rect, 1 / scale);
      wl_region_add(input, dips.x(), dips.y(), dips.width(), dips.height());
    }
  }
  wl_surface_set_input_region(surface, input);
  if (input) {
    wl_region_destroy(input);
  }
  // With no buffer attached the commit changes only the region, and applies the subsurfaces'
  // state they committed, as Chromium's own commit of the region would.
  wl_surface_commit(surface);
  wl_display_flush(display);
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
