//! The edge knot's window. GPUI can't draw over the page's native view, so the tab the hidden
//! sidebar leaves at the window's edge while a thread needs the user is a window of its own, the
//! browser window's child. The shell draws the tab (shell/edge_knot.rs); this window only holds it.

use std::ffi::c_void;

use browser::log;
use browser::shell::Shell;
use browser::tokens::shell::edge_knot::{HEIGHT, MARGIN, WIDTH};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, point, prelude::*, px,
    size,
};

use crate::{ns_view, platform};

struct EdgeKnot {
    shell: Entity<Shell>,
}

impl Render for EdgeKnot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.shell
            .update(cx, |shell, cx| shell.render_edge_knot(window, cx))
    }
}

/// Opens the knot's window for the browser window whose UI draws in `ui_view`, hidden, ahead of
/// time, as the reveal's is. Returns its window.
pub fn open(shell: Entity<Shell>, ui_view: *mut c_void, cx: &mut App) -> Option<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WIDTH + MARGIN), px(HEIGHT + 2. * MARGIN)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // A panel the host keeps from ever taking the keyboard.
        kind: WindowKind::PopUp,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            // It draws only as what it shows changes while it shows, and the knot's next frame
            // while it moves, so a still tab costs nothing.
            let mut drawn = None;
            cx.observe(&shell, move |_, shell, cx| {
                let shell = shell.read(cx);
                let now = shell.edge_knot_shown().then(|| shell.edge_knot_look());
                if now.is_some() && (now != drawn || shell.edge_knot_moving()) {
                    cx.notify();
                }
                drawn = now;
            })
            .detach();
            EdgeKnot { shell }
        })
    });
    match window {
        Ok(window) => {
            if let Ok(view) = window.update(cx, |_, window, _| ns_view(window)) {
                unsafe { platform::host_add_edge_knot(ui_view, view, MARGIN.into()) };
            }
            Some(window.into())
        }
        Err(error) => {
            log!("couldn't open the edge knot: {error}");
            None
        }
    }
}
