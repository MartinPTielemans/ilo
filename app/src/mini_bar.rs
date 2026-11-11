//! Arrange's mini bar (#2347): the small bar over the thing selected on the page, with the pin
//! while a floating thing is selected ("Stays on screen" or "Scrolls with page"), Hide, and Put back
//! once the thing has a change to take back. The mode's state gives what it shows (Shell's
//! `arrange_state`), and the bar draws it in the find bar's material. GPUI can't draw over the
//! page's native view, so as the find bar is, it's a window of its own that the host shows over the
//! browser window, as its child. It takes clicks on its buttons and never the keyboard, which stays
//! with the browser window, so with the page, whose keys the mode handles itself. What it asks of
//! its window goes through `Host`.
//!
//! The core sends the selection's box once a scroll rests and none while the page scrolls, so the
//! bar hides as a scroll starts and comes back at the box after, and nothing runs each frame.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, FontWeight, Global, Hsla, Pixels, SharedString, Size,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
    accesskit::{Role, Toggled},
    div, point,
    prelude::*,
    px, rgb, rgba, size, svg,
};

use crate::elevation::Level;
use crate::palette::{Palette, STATE_HOVERED, STATE_PRESSED};
use crate::tokens::find_bar::FILL;
use crate::tokens::mini_bar::*;
use crate::tokens::text::SMALL;

/// What the bar asks of its window.
pub trait Host {
    /// Shows the bar's window over the browser window `window`, or moves it there, at `place`, in
    /// points from that window's content's top left.
    fn show(&self, window: u64, place: Bounds<Pixels>);
    /// Hides it.
    fn hide(&self);
}

/// A button of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// The pin: a floating thing stays on screen, or scrolls with the page.
    Pin,
    Hide,
    PutBack,
}

impl Press {
    /// For the logs and the checks.
    pub fn name(self) -> &'static str {
        match self {
            Press::Pin => "pin",
            Press::Hide => "hide",
            Press::PutBack => "put back",
        }
    }
}

/// What the bar shows, and where, from the mode's state on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// The tab whose page the thing is on.
    pub tab: u64,
    /// The thing selected's box and its page's, in points from the window's content's top left.
    pub selection: Bounds<Pixels>,
    pub page: Bounds<Pixels>,
    /// While a floating thing is selected, whether it stays on screen.
    pub pin: Option<bool>,
    /// Whether the thing has a change Put back takes back.
    pub put_back: bool,
    /// The window's accent, which the pin is tinted with while the thing stays on screen.
    pub accent: Hsla,
}

/// What a press runs: the mode's ask on the tab's page (Shell::press_mini_bar).
pub type OnPress = Rc<dyn Fn(u64, Press, &mut App)>;

/// The bar's buttons for `shown`, in their order: each one's press, glyph and label.
pub fn buttons(shown: &Shown) -> Vec<(Press, &'static str, &'static str)> {
    let pin = shown.pin.map(|on_screen| {
        let label = match on_screen {
            true => "Stays on screen",
            false => "Scrolls with page",
        };
        (Press::Pin, "icons/pin.svg", label)
    });
    let put_back = (shown.put_back).then_some((Press::PutBack, "icons/undo-2.svg", "Put back"));
    pin.into_iter()
        .chain([(Press::Hide, "icons/eye-off.svg", "Hide")])
        .chain(put_back)
        .collect()
}

/// A made-up thing selected on `page`, a box at its right as the mock's, for the preview and the
/// checks: "selected" with Hide and Put back, "float" adding the pin as the thing stays on screen,
/// and "scrolls" with the pin as it scrolls with the page.
pub fn sample(kind: &str, page: Bounds<Pixels>, accent: Hsla) -> Option<Shown> {
    let pin = match kind {
        "selected" => None,
        "float" => Some(true),
        "scrolls" => Some(false),
        _ => return None,
    };
    // Whole points, so the box and the bar over it land on the pixel grid at 1x.
    let (width, height) = (
        (page.size.width * 0.28).round(),
        (page.size.height * 0.2).round(),
    );
    let at = point(
        (page.right() - width - page.size.width * 0.08).round(),
        (page.top() + page.size.height * 0.36).round(),
    );
    Some(Shown {
        tab: 0,
        selection: Bounds::new(at, size(width, height)),
        page,
        pin,
        put_back: true,
        accent,
    })
}

/// How wide some words are, in a size and weight of the UI's font.
type Measure<'a> = &'a dyn Fn(&SharedString, f32, FontWeight) -> f32;

/// The bar's size for `shown`, a whole point wide, as the window is a whole number of pixels.
fn bar_size(shown: &Shown, measure: Measure) -> Size<Pixels> {
    let buttons = buttons(shown);
    let widths: f32 = (buttons.iter())
        .map(|(_, _, label)| {
            let label = measure(&SharedString::from(*label), SMALL, FontWeight::MEDIUM);
            BUTTON_PAD * 2. + GLYPH + GLYPH_GAP + label
        })
        .sum();
    let gaps = BUTTON_GAP * buttons.len().saturating_sub(1) as f32;
    let divider = match shown.pin {
        Some(_) => DIVIDER_GAP * 2. + 1. - BUTTON_GAP,
        None => 0.,
    };
    size(
        px((INSET * 2. + widths + gaps + divider).ceil()),
        px(HEIGHT),
    )
}

/// Where a bar `size` big goes over `selection` on `page`: its right end at the box's, `GAP` over
/// its top, or under its bottom where there's no room over it, and `EDGE` inside the page.
pub fn place(
    selection: Bounds<Pixels>,
    page: Bounds<Pixels>,
    size: Size<Pixels>,
) -> Bounds<Pixels> {
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    let (top, bottom) = (
        f32::from(page.top()) + EDGE,
        f32::from(page.bottom()) - EDGE,
    );
    let (left, right) = (
        f32::from(page.left()) + EDGE,
        f32::from(page.right()) - EDGE,
    );
    let over = f32::from(selection.top()) - GAP - height;
    let under = f32::from(selection.bottom()) + GAP;
    let y = match () {
        _ if over >= top => over,
        _ if under + height <= bottom => under,
        _ => top,
    };
    let x = (f32::from(selection.right()) - width)
        .min(right - width)
        .max(left);
    Bounds::new(point(px(x), px(y)), size)
}

struct Showing {
    window: u64,
    shown: Shown,
    on_press: OnPress,
    /// Whether it has drawn on screen, which the log says once, for the checks.
    drawn: bool,
}

pub struct MiniBar {
    host: Rc<dyn Host>,
    showing: Option<Showing>,
}

struct MiniBarWindow(WindowHandle<MiniBar>);

impl Global for MiniBarWindow {}

/// Opens the bar's window, hidden, ahead of time as the find bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<MiniBar>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(HEIGHT), px(HEIGHT)));
    open_at(host, bounds, false, cx)
}

/// Opens it at `bounds`, and on screen if `show`, as the preview does to show it alone.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<MiniBar>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The bar's rounded shape is drawn, and the shadow takes it, as the find bar's does.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|_| MiniBar {
            host,
            showing: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(MiniBarWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the mini bar: {error}");
            None
        }
    }
}

/// Shows the bar for `shown` over the browser window `window`, or moves it there, or hides it for
/// none. Its user calls it again as the selection or what it shows changes.
pub fn show(window: u64, shown: Option<(Shown, OnPress)>, cx: &mut App) {
    let Some(handle) = cx.try_global::<MiniBarWindow>().map(|bar| bar.0) else {
        return;
    };
    let _ = handle.update(cx, |bar, gpui_window, cx| {
        let Some((shown, on_press)) = shown else {
            if bar.showing.take().is_some() {
                bar.host.hide();
                cx.notify();
            }
            return;
        };
        let system = gpui_window.text_system().clone();
        let measure = |text: &SharedString, size: f32, weight| {
            crate::tooltip::text_width(text, size, weight, &system)
        };
        let at = place(shown.selection, shown.page, bar_size(&shown, &measure));
        let drawn = (bar.showing.as_ref()).is_some_and(|showing| showing.window == window);
        bar.showing = Some(Showing {
            window,
            shown,
            on_press,
            drawn,
        });
        bar.host.show(window, at);
        cx.notify();
    });
}

/// The browser window the bar shows over, while it shows.
pub fn shown_over(cx: &App) -> Option<u64> {
    let handle = cx.try_global::<MiniBarWindow>()?.0;
    let bar = handle.read(cx).ok()?;
    bar.showing.as_ref().map(|showing| showing.window)
}

impl MiniBar {
    fn press(&mut self, press: Press, cx: &mut Context<Self>) {
        let Some(showing) = &self.showing else {
            return;
        };
        log!("browser: mini bar {} pressed", press.name());
        let (tab, on_press) = (showing.shown.tab, showing.on_press.clone());
        // After this update, as what it runs may show the bar again or hide it.
        cx.defer(move |cx| on_press(tab, press, cx));
    }
}

impl Render for MiniBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().font_family(crate::face::UI);
        let Some(showing) = &mut self.showing else {
            return root;
        };
        // GPUI draws a window only once the system reports it visible, some frames after the host
        // shows it, so the line comes with the first frame that's seen. check.sh reads it.
        if !showing.drawn {
            match window.is_visible() {
                true => log!("browser: mini bar drawn over {}", showing.window),
                false => window.request_animation_frame(),
            }
            showing.drawn = window.is_visible();
        }
        let shown = &showing.shown;
        let a11y = window.is_a11y_active();
        let buttons = buttons(shown)
            .into_iter()
            .flat_map(|(press, glyph, label)| {
                let on = press == Press::Pin && shown.pin == Some(true);
                let button = div()
                    .id(("mini-bar-button", press as usize))
                    .h(px(BUTTON_HEIGHT))
                    .px(px(BUTTON_PAD))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(GLYPH_GAP))
                    .rounded(px(BUTTON_RADIUS))
                    .when(on, |button| button.bg(shown.accent.opacity(PIN_ON)))
                    .hover(|style| style.bg(Palette::on_dark(STATE_HOVERED)))
                    .active(|style| style.bg(Palette::on_dark(STATE_PRESSED)))
                    .when(a11y, |button| {
                        let toggled = match on {
                            true => Toggled::True,
                            false => Toggled::False,
                        };
                        button
                            .role(Role::Button)
                            .aria_label(label)
                            .when(press == Press::Pin, |button| button.aria_toggled(toggled))
                    })
                    .on_click(cx.listener(move |bar, _, _, cx| bar.press(press, cx)))
                    .child(svg().path(glyph).size(px(GLYPH)).text_color(rgba(INK)))
                    .child(label);
                // The pin's divider, DIVIDER_GAP from the buttons either side.
                let divider = (press == Press::Pin).then(|| {
                    div()
                        .flex_none()
                        .mx(px(DIVIDER_GAP - BUTTON_GAP))
                        .w(px(1.))
                        .h(px(DIVIDER_HEIGHT))
                        .bg(rgba(DIVIDER))
                        .into_any_element()
                });
                [Some(button.into_any_element()), divider]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<AnyElement>>()
            });
        root.child(
            div()
                .id("mini-bar")
                .when(a11y, |bar| {
                    bar.role(Role::Toolbar).aria_label("The thing selected")
                })
                .size_full()
                .relative()
                .flex()
                .items_center()
                .gap(px(BUTTON_GAP))
                .p(px(INSET))
                .rounded(px(RADIUS))
                .bg(rgb(FILL))
                .text_size(px(SMALL))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgba(INK))
                .children(buttons)
                // The popover level's rim, as the find bar's, its own layer so the border doesn't
                // move the bar's parts.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(RADIUS))
                        .border(px(0.5))
                        .border_color(Level::Popover.rim()),
                ),
        )
    }
}

#[cfg(test)]
mod tests;
