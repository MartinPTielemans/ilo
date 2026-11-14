//! The sidebar reveal's window. GPUI can't draw over the page's native view, so the panel the
//! hidden sidebar slides out in is a window of its own, the browser window's child, at its left
//! edge. The shell draws the panel; this window only holds it.

use std::ffi::c_void;

use browser::log;
use browser::shell::{SIDEBAR_WIDTH, Shell};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, point, prelude::*, px,
    size,
};
#[cfg(target_os = "linux")]
use gpui::{DispatchPhase, MouseExitEvent};

use crate::{ns_view, platform, update_window, window_with_view};

struct Reveal {
    shell: Entity<Shell>,
}

impl Render for Reveal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self
            .shell
            .update(cx, |shell, cx| shell.render_reveal(window, cx));
        // On Linux the page is Chromium's, whose pointer the host doesn't hear, so this window
        // says where the pointer is over it, and as it leaves, that it's past the panel.
        #[cfg(target_os = "linux")]
        let panel = {
            let (moved, left) = (self.shell.clone(), self.shell.clone());
            gpui::div()
                .size_full()
                .on_mouse_move(move |event, window, cx| {
                    let (x, width) = (event.position.x.into(), window.viewport_size().width);
                    moved.update(cx, |shell, cx| {
                        let x = shell.from_reveal(x, width.into());
                        shell.pointer_at(x, cx)
                    });
                })
                // The window's own leave rather than the div's hover, which GPUI counts only after
                // a move over it: a press with none before it, on a panel that came out under a
                // still pointer, would leave the pointer's going unheard. It's past the panel once
                // the move that took it off is through: onto a row's card, the card hears that
                // move after this window hears it leave, and the card keeps the sidebar out (#830).
                // A panel already gone, docked under the pointer, hears its leave only with the
                // next move, which may be the one to the edge that brings it out again.
                .child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            let left = left.downgrade();
                            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                                if phase == DispatchPhase::Capture {
                                    let left = left.clone();
                                    cx.spawn(async move |cx| {
                                        left.update(cx, |shell, cx| {
                                            if shell.reveal_shown() {
                                                shell.pointer_at(f32::MAX, cx);
                                            }
                                        })
                                        .ok();
                                    })
                                    .detach();
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .child(panel)
                .into_any_element()
        };
        panel
    }
}

/// Opens the reveal's window for the browser window whose UI draws in `ui_view`, hidden. Opening it
/// ahead of time keeps window setup off the path from the pointer reaching the edge to the panel
/// moving. Returns its window.
pub fn open(shell: Entity<Shell>, ui_view: *mut c_void, cx: &mut App) -> Option<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(SIDEBAR_WIDTH), px(SIDEBAR_WIDTH)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // A panel that doesn't take the keyboard when clicked.
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
            // It draws only while the panel is out.
            cx.observe(&shell, |_, shell, cx| {
                if shell.read(cx).reveal_shown() {
                    cx.notify();
                }
            })
            .detach();
            Reveal { shell }
        })
    });
    match window {
        Ok(window) => {
            if let Ok(view) = window.update(cx, |_, window, _| ns_view(window)) {
                unsafe { platform::host_add_reveal(ui_view, view, pointer_moved) };
            }
            Some(window.into())
        }
        Err(error) => {
            log!("couldn't open the sidebar reveal: {error}");
            None
        }
    }
}

extern "C" fn pointer_moved(ui_view: *mut c_void, x: f64) {
    update_window(window_with_view(ui_view), |shell, cx| {
        shell.pointer_at(x as f32, cx)
    });
}
