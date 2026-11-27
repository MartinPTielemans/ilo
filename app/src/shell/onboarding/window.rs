//! First run's window of its own (spec-first-run.md): the card's size, with no title bar and no
//! close, the only window on screen until first run ends, drawing the card the shell keeps.

use gpui::{
    App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, div, px, size,
};

use super::Shell;
use crate::tokens::shell::onboarding::card as t;

/// The window's view: the shell's card, drawn here rather than over the browser window.
pub struct FirstRun {
    shell: Entity<Shell>,
    _shell: Subscription,
}

/// The window's options: the card's size, centred, with nothing of the system's drawn round it,
/// as the card is the window. Clear outside the card's corners, with the system's blur behind it
/// where the window has glass. It opens unshown; the host shows it once it has drawn, unless
/// `shown`, for a host that has no such call.
pub fn window_options(glass: bool, shown: bool, cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(t::WIDTH), px(t::HEIGHT)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui::WindowDecorations::Client),
        kind: WindowKind::Normal,
        is_resizable: false,
        is_minimizable: false,
        show: shown,
        focus: shown,
        window_background: match glass {
            true => WindowBackgroundAppearance::Blurred,
            false => WindowBackgroundAppearance::Transparent,
        },
        ..Default::default()
    }
}

impl FirstRun {
    /// Opens the window for `shell`'s card, unshown but where the host shows it.
    pub fn open(shell: Entity<Shell>, cx: &mut App) -> Option<WindowHandle<FirstRun>> {
        let (glass, shown) = {
            let shell = shell.read(cx);
            (shell.blurred, shell.browser.shows_first_run_at_open())
        };
        let options = window_options(glass, shown, cx);
        cx.open_window(options, move |window, cx| {
            // The card has no close: a step is left by its own buttons, so the system's close
            // (a window manager's, ⌘W) does nothing. The window goes as first run ends.
            window.on_window_should_close(cx, |_, _| false);
            cx.new(|cx| FirstRun {
                _shell: cx.observe(&shell, |_, _, cx| cx.notify()),
                shell,
            })
        })
        .ok()
    }

    /// The shell whose card the window draws.
    #[cfg(test)]
    pub(crate) fn shell(&self) -> Entity<Shell> {
        self.shell.clone()
    }
}

impl Render for FirstRun {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The shell names what it draws for a screen reader by the window it draws in, this one.
        let card = (self.shell).update(cx, |shell, cx| {
            shell.a11y = window.is_a11y_active();
            shell.onboarding_layer(window, cx)
        });
        div()
            .size_full()
            .relative()
            .font_family(crate::face::UI)
            .text_size(px(crate::tokens::text::BODY))
            .children(card)
    }
}
