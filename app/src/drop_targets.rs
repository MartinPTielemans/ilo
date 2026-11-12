//! The window over the page that draws Add Split View's targets while a tab dragged out of the
//! sidebar is over it, the agent's pointer and a new download's flight (`Shell::over_page`). As
//! the zoom HUD is, it's a window of its own that the host shows over the browser window, as its
//! child, and the mouse goes through it, so the drag goes on in the browser window.

use std::ffi::c_void;

use browser::log;
use browser::shell::Shell;
use gpui::{
    App, Bounds, Context, Global, TitlebarOptions, WeakEntity, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point, prelude::*, px, size,
};

use crate::platform;

pub struct DropTargets {
    /// The shell of the browser window it shows over, which draws the targets.
    shell: WeakEntity<Shell>,
}

struct DropTargetsWindow(WindowHandle<DropTargets>);

impl Global for DropTargetsWindow {}

/// Opens the targets' window, hidden, for every browser window, ahead of time as the HUD does. The
/// host makes it as big as the browser window's content as it shows it.
pub fn open(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The page shows through everywhere but the targets and the tab.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|_| DropTargets {
            shell: WeakEntity::new_invalid(),
        })
    });
    match window {
        Ok(window) => {
            if let Ok(view) = window.update(cx, |_, window, _| crate::ns_view(window)) {
                unsafe { platform::host_add_drop_targets(view) };
                cx.set_global(DropTargetsWindow(window));
            }
        }
        Err(error) => log!("couldn't open the drop targets' window: {error}"),
    }
}

/// Shows the targets' window over the browser window whose UI draws in `ui_view`, drawing it
/// again for the drag's move, or hides it.
pub fn show(shown: bool, ui_view: *mut c_void, cx: &mut App) {
    let Some(window) = cx
        .try_global::<DropTargetsWindow>()
        .map(|targets| targets.0)
    else {
        return;
    };
    if shown {
        window
            .update(cx, |targets, _, cx| {
                targets.shell = crate::shell_with_view(ui_view);
                cx.notify();
            })
            .ok();
        unsafe { platform::host_show_drop_targets(ui_view) };
    } else {
        unsafe { platform::host_hide_drop_targets() };
    }
}

impl Render for DropTargets {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.shell.upgrade() {
            Some(shell) => shell.read(cx).over_page(window, cx),
            None => div().into_any_element(),
        }
    }
}
