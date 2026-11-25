//! First run's window of its own (spec-first-run.md): a fixed 880 × 600 window, the only one on
//! screen until first run ends, drawing the flow the shell keeps. Its close is Skip Setup.

use gpui::{
    App, AppContext, Bounds, Context, Decorations, Entity, IntoElement, MouseButton, ParentElement,
    Render, Styled, Subscription, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point, prelude::*, px, rgb, rgba,
    size, svg,
};

use super::Shell;
use crate::settings_rows::named_button;
use crate::tokens::shell::onboarding::window as t;
use crate::tokens::shell::{WINDOW_BUTTON_GLYPH, WINDOW_BUTTON_SIZE, WINDOW_BUTTON_Y};

/// The window's view: the shell's flow, drawn here rather than over the browser window.
pub struct FirstRun {
    shell: Entity<Shell>,
    _shell: Subscription,
}

/// The window's options: its fixed size, centred, with the system's buttons on the Mac where the
/// drawing has them and the UI's own close off it. It opens unshown; the host shows it once it has
/// drawn, unless `shown`, for a host that has no such call.
pub fn window_options(glass: bool, shown: bool, cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(t::WIDTH), px(t::HEIGHT)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(env!("APP_NAME").into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(t::BUTTONS_X), px(t::BUTTONS_Y))),
        }),
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui::WindowDecorations::Client),
        kind: WindowKind::Normal,
        is_resizable: false,
        is_minimizable: false,
        show: shown,
        focus: shown,
        window_background: match glass {
            true => WindowBackgroundAppearance::Blurred,
            false => WindowBackgroundAppearance::Opaque,
        },
        ..Default::default()
    }
}

impl FirstRun {
    /// Opens the window for `shell`'s flow, unshown but where the host shows it.
    pub fn open(shell: Entity<Shell>, cx: &mut App) -> Option<WindowHandle<FirstRun>> {
        let (glass, shown) = {
            let shell = shell.read(cx);
            (shell.blurred, shell.browser.shows_first_run_at_open())
        };
        let options = window_options(glass, shown, cx);
        cx.open_window(options, move |window, cx| {
            // Closing it is Skip Setup, which ends the flow; the window goes as it ends.
            let closing = shell.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                closing
                    .update(cx, |shell, cx| shell.onboarding_skip_setup("close", cx))
                    .ok();
                false
            });
            cx.new(|cx| FirstRun {
                _shell: cx.observe(&shell, |_, _, cx| cx.notify()),
                shell,
            })
        })
        .ok()
    }

    /// The shell whose flow the window draws.
    #[cfg(test)]
    pub(crate) fn shell(&self) -> Entity<Shell> {
        self.shell.clone()
    }
}

impl Render for FirstRun {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let flow = (self.shell).update(cx, |shell, cx| shell.onboarding_layer(window, cx));
        let a11y = self.shell.read(cx).a11y;
        // Off the Mac the window manager draws no title bar, so the UI draws the close.
        let own_close = (!cfg!(target_os = "macos")
            && matches!(window.window_decorations(), Decorations::Client { .. }))
        .then(|| close_button(self.shell.clone(), a11y));
        div()
            .size_full()
            .relative()
            .font_family(crate::face::UI)
            .text_size(px(crate::tokens::text::BODY))
            .children(flow)
            .children(own_close)
    }
}

/// The close, as Settings' is off the Mac, where the Mac's sits: Skip Setup.
fn close_button(shell: Entity<Shell>, a11y: bool) -> impl IntoElement {
    let (x, colour) = super::super::WINDOW_BUTTONS[0];
    div()
        .id("first-run-close")
        .when(a11y, |close| named_button(close, "Close"))
        .absolute()
        .left(px(x - WINDOW_BUTTON_SIZE / 2.))
        .top(px(WINDOW_BUTTON_Y - WINDOW_BUTTON_SIZE / 2.))
        .size(px(WINDOW_BUTTON_SIZE))
        .rounded_full()
        .bg(rgb(colour))
        .group("first-run-close")
        // A press here closes the window, and doesn't move it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            shell.update(cx, |shell, cx| shell.onboarding_skip_setup("close", cx))
        })
        .child(
            svg()
                .path("icons/window-close.svg")
                .size(px(WINDOW_BUTTON_SIZE))
                .text_color(gpui::transparent_black())
                .group_hover("first-run-close", |style| {
                    style.text_color(rgba(WINDOW_BUTTON_GLYPH))
                }),
        )
}
