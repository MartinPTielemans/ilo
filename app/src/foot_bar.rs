//! The foot bar: the bar a mode of the page shows over the page's bottom centre, such as Arrange's
//! (#2347), one line of words and its buttons, and pointing's (#2352), who asks over their words,
//! then a hint and the buttons. Its user gives what it says and the buttons; the bar draws them in
//! the find bar's material. GPUI can't draw over the page's native view, so as the find bar is,
//! it's a window of its own that the host shows over the browser window, as its child. It takes
//! clicks on its buttons and never the keyboard, which stays with the browser window, so with the
//! page, whose keys the mode handles itself. What it asks of its window goes through `Host`.
//!
//! There's one, which shows over one browser window at a time, as the toast does: a mode shows it
//! over its own window when that window comes in front.

use std::rc::Rc;

use gpui::{
    App, Bounds, Context, FontWeight, Global, Hsla, Pixels, Role, SharedString, Size,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, div, point, prelude::*, px, rgb, rgba, size, svg,
};

use crate::elevation::Level;
use crate::palette::{Palette, STATE_HOVERED, STATE_PRESSED};
use crate::tokens::find_bar::FILL;
use crate::tokens::foot_bar::*;

/// What the bar asks of its window.
pub trait Host {
    /// Shows the bar's window over the browser window `window`, or moves it there, at `place`.
    fn show(&self, window: u64, place: Place);
    /// Hides it.
    fn hide(&self);
}

/// Where the bar's window goes over its browser window, in points from the edges of that window's
/// content, which the host keeps as the window resizes: `size`, centred between `left` and
/// `right` in from its left and right edges, its bottom `bottom` over its bottom edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub size: Size<Pixels>,
}

/// What the bar says and its buttons.
#[derive(Clone)]
pub struct Bar {
    /// Who's asking, over the words, which makes the bar two lines tall.
    pub lead: Option<Lead>,
    /// Its words, which are its user's: an agent's question is the agent's own.
    pub words: SharedString,
    /// A quieter line before the buttons, such as how many notes there are.
    pub hint: Option<SharedString>,
    pub buttons: Vec<Button>,
}

/// Who's asking: a glyph's path in the assets, and a name, as "Claude asks".
#[derive(Clone, Debug, PartialEq)]
pub struct Lead {
    pub glyph: Option<&'static str>,
    pub text: SharedString,
}

/// A button, with its key's name after its label, as Send ⌘↩. A press runs it, while it's enabled.
#[derive(Clone)]
pub struct Button {
    pub label: SharedString,
    /// Its key as the bindings write it, such as "cmd-enter", which the bar names as menus do:
    /// Ctrl+Enter off the Mac.
    pub key: Option<&'static str>,
    /// The mode's main button, in the accent, as Done and Send.
    pub primary: bool,
    pub enabled: bool,
    /// For the logs and the checks.
    pub name: &'static str,
    pub run: Rc<dyn Fn(&mut App)>,
}

/// Made-up bars as each mode will show them, for the preview and the checks: "arrange", one line,
/// as Arrange's; "point", two, as pointing's; and "long", a question past its two lines.
pub fn sample(kind: &str) -> Option<Bar> {
    let button = |label: &str, key, primary, enabled| Button {
        label: label.to_owned().into(),
        key,
        primary,
        enabled,
        name: "sample",
        run: Rc::new(|_: &mut App| {}),
    };
    let question = match kind {
        "long" => {
            "Which one is “the box on the right”, the one with the recipes, or the video above \
             it, or the panel that slides in when the page scrolls past the comments? Show me."
        }
        _ => "Which one is “the box on the right”? Show me.",
    };
    match kind {
        "arrange" => Some(Bar {
            lead: None,
            words: "Traded the video with Trending".into(),
            hint: None,
            buttons: vec![
                button("Undo", Some("cmd-z"), false, true),
                button("Reset", None, false, true),
                button("Done", Some("escape"), true, true),
            ],
        }),
        "point" | "long" => Some(Bar {
            lead: Some(Lead {
                glyph: Some("icons/terminal.svg"),
                text: "Claude asks".into(),
            }),
            words: question.into(),
            hint: Some("Click what you mean".into()),
            buttons: vec![
                button("Not Now", Some("escape"), false, true),
                button("Send", Some("cmd-enter"), true, false),
            ],
        }),
        _ => None,
    }
}

/// How a button names `key`: escape as "esc", as the offer's and the onboarding's chips do, and
/// any other as a menu shows it.
fn key_text(key: &str) -> SharedString {
    match key {
        "escape" => "esc".into(),
        key => crate::shell::keystroke_label(key).into(),
    }
}

/// How the bar is laid out: its size, how wide its words may be, how many lines they take, and
/// whether they fit whole.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    size: Size<Pixels>,
    words: f32,
    lines: usize,
    whole: bool,
}

/// How wide some words are, in a size and weight of the UI's font.
type Measure<'a> = &'a dyn Fn(&SharedString, f32, FontWeight) -> f32;

/// Lays `bar` out to be no wider than `room`: words that don't fit give way, wrapping to a second
/// line in a two-line bar first, and the hint and the buttons keep their size.
fn layout(bar: &Bar, room: f32, measure: Measure) -> Layout {
    let button = |button: &Button| {
        let label = measure(&button.label, TEXT, FontWeight::MEDIUM);
        let key = (button.key).map_or(0., |key| {
            KEY_GAP + measure(&key_text(key), TEXT, FontWeight::NORMAL)
        });
        BUTTON_PAD * 2. + label + key
    };
    let tall = bar.lead.is_some();
    let (gap, edges) = match tall {
        true => (TALL_GAP, TALL_LEFT + TALL_RIGHT),
        false => (GAP, LEFT + RIGHT),
    };
    // What follows the words, with the gap before each.
    let after: f32 = (bar.hint.iter())
        .map(|hint| gap + measure(hint, HINT, FontWeight::NORMAL))
        .chain(bar.buttons.iter().map(|each| gap + button(each)))
        .sum();
    let after = after
        + if tall || bar.buttons.is_empty() {
            0.
        } else {
            LINE_GAP
        };
    let measured = measure(&bar.words, TEXT, FontWeight::NORMAL);
    let room_for_words = (room - edges - after).max(0.);
    let (words, lines, height) = match &bar.lead {
        Some(lead) => {
            let glyph = lead.glyph.map_or(0., |_| LEAD_GLYPH + LEAD_GLYPH_GAP);
            let lead = glyph + measure(&lead.text, LEAD, FontWeight::NORMAL);
            let lines = if measured > WORDS_WIDTH { 2 } else { 1 };
            let column = measured.min(WORDS_WIDTH).max(lead).min(room_for_words);
            (column, lines, TALL_HEIGHT + WORDS_LINE * (lines - 1) as f32)
        }
        None => (measured.min(room_for_words), 1, HEIGHT),
    };
    // A whole point wide, the words taking what's over, as the window is a whole number of pixels.
    let width = (edges + words + after).ceil();
    Layout {
        size: size(px(width), px(height)),
        words: width - edges - after,
        lines,
        whole: lines == 1 && measured <= words,
    }
}

/// Where the bar goes over a page at `page` in a window's content `content` big.
fn place(page: Bounds<Pixels>, content: Size<Pixels>, size: Size<Pixels>) -> Place {
    Place {
        left: f32::from(page.left()),
        right: f32::from(content.width - page.right()),
        bottom: f32::from(content.height - page.bottom()) + BOTTOM,
        size,
    }
}

struct Shown {
    window: u64,
    bar: Bar,
    layout: Layout,
    /// Whether it has drawn on screen, which the log says once, for the checks.
    drawn: bool,
}

pub struct FootBar {
    host: Rc<dyn Host>,
    /// The palette of a browser window, by its id, for the main button's accent.
    look: Box<dyn Fn(u64, &App) -> Palette>,
    shown: Option<Shown>,
}

struct FootBarWindow(WindowHandle<FootBar>);

impl Global for FootBarWindow {}

/// Opens the bar's window, hidden, ahead of time as the find bar's is. `look` gives a browser
/// window's palette, by its id.
pub fn open(
    host: Rc<dyn Host>,
    look: impl Fn(u64, &App) -> Palette + 'static,
    cx: &mut App,
) -> Option<WindowHandle<FootBar>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(HEIGHT), px(HEIGHT)));
    open_at(host, look, bounds, false, cx)
}

/// Opens it at `bounds`, and on screen if `show`, as the preview does to show it alone.
pub fn open_at(
    host: Rc<dyn Host>,
    look: impl Fn(u64, &App) -> Palette + 'static,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<FootBar>> {
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
        cx.new(|_| FootBar {
            host,
            look: Box::new(look),
            shown: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(FootBarWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the foot bar: {error}");
            None
        }
    }
}

/// Shows `bar` over the page at `page` in the content of the browser window `window`, `content`
/// big, or shows it there instead of what it showed. Its user calls it again as what it says
/// changes, or the page moves.
pub fn show(window: u64, page: Bounds<Pixels>, content: Size<Pixels>, bar: Bar, cx: &mut App) {
    crate::lazy_window::ready::<FootBar>(cx);
    let Some(handle) = cx.try_global::<FootBarWindow>().map(|bar| bar.0) else {
        return;
    };
    let _ = handle.update(cx, |foot_bar, gpui_window, cx| {
        let system = gpui_window.text_system().clone();
        let measure = |text: &SharedString, size: f32, weight| {
            crate::tooltip::text_width(text, size, weight, &system)
        };
        let room = f32::from(page.size.width) - BOTTOM * 2.;
        let layout = layout(&bar, room, &measure);
        let place = place(page, content, layout.size);
        foot_bar.shown = Some(Shown {
            window,
            bar,
            layout,
            drawn: false,
        });
        foot_bar.host.show(window, place);
        cx.notify();
    });
}

/// Hides the bar, if it shows.
pub fn hide(cx: &mut App) {
    let Some(handle) = cx.try_global::<FootBarWindow>().map(|bar| bar.0) else {
        return;
    };
    let _ = handle.update(cx, |foot_bar, _, cx| {
        if foot_bar.shown.take().is_some() {
            foot_bar.host.hide();
            cx.notify();
        }
    });
}

/// The browser window the bar shows over, while it shows.
pub fn shown_over(cx: &App) -> Option<u64> {
    let handle = cx.try_global::<FootBarWindow>()?.0;
    handle
        .read(cx)
        .ok()?
        .shown
        .as_ref()
        .map(|shown| shown.window)
}

impl FootBar {
    fn press(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(button) = (self.shown.as_ref()).and_then(|shown| shown.bar.buttons.get(index))
        else {
            return;
        };
        if !button.enabled {
            return;
        }
        log!("browser: foot bar {} pressed", button.name);
        let run = button.run.clone();
        // After this update, as what it runs may show the bar again or hide it.
        cx.defer(move |cx| run(cx));
    }
}

impl Render for FootBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().font_family(crate::face::UI);
        let Some(shown) = &mut self.shown else {
            return root;
        };
        // GPUI draws a window only once the system reports it visible, some frames after the host
        // shows it, so the line comes with the first frame that's seen. check.sh reads it.
        if !shown.drawn {
            match window.is_visible() {
                true => log!("browser: foot bar drawn over {}", shown.window),
                false => window.request_animation_frame(),
            }
            shown.drawn = window.is_visible();
        }
        let shown = &*shown;
        let palette = (self.look)(shown.window, cx);
        let a11y = window.is_a11y_active();
        let (bar, layout) = (&shown.bar, shown.layout);
        let tall = bar.lead.is_some();
        // VoiceOver reads who's asking with the words.
        let bar_said = match &bar.lead {
            Some(lead) => format!("{}: {}", lead.text, bar.words),
            None => bar.words.to_string(),
        };
        let (radius, left, right, gap) = match tall {
            true => (TALL_RADIUS, TALL_LEFT, TALL_RIGHT, TALL_GAP),
            false => (RADIUS, LEFT, RIGHT, GAP),
        };
        let words = div()
            .flex_none()
            .w(px(layout.words))
            .text_color(rgba(INK))
            .text_size(px(TEXT))
            .line_height(px(WORDS_LINE));
        // Words that fit are never cut: GPUI decides where to cut by adding up each character's
        // width without kerning, which comes out wider than the words.
        let words = match (layout.lines, layout.whole) {
            (1, true) => words.whitespace_nowrap(),
            (1, false) => words.truncate(),
            (lines, _) => words.line_clamp(lines).text_ellipsis(),
        }
        .child(bar.words.clone());
        let words = match &bar.lead {
            Some(lead) => div()
                .flex()
                .flex_col()
                .gap(px(LINES_GAP))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(LEAD_GLYPH_GAP))
                        .text_size(px(LEAD))
                        .text_color(rgba(QUIET))
                        .children(lead.glyph.map(|path| {
                            svg()
                                .path(path)
                                .size(px(LEAD_GLYPH))
                                .text_color(rgba(QUIET))
                        }))
                        .child(lead.text.clone()),
                )
                .child(words),
            None => words.mr(px(if bar.buttons.is_empty() { 0. } else { LINE_GAP })),
        };
        let buttons = (bar.buttons.iter().enumerate()).map(|(index, button)| {
            let (plate, ink): (Hsla, Hsla) = match button.primary {
                true => (palette.accent.into(), palette.accent_label.into()),
                false => (rgba(BUTTON_PLATE).into(), rgba(INK).into()),
            };
            let key_ink = match button.primary {
                true => ink.opacity(PRIMARY_KEY),
                false => rgba(QUIET).into(),
            };
            let enabled = button.enabled;
            div()
                .id(("foot-bar-button", index))
                .h(px(BUTTON_HEIGHT))
                .px(px(BUTTON_PAD))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(KEY_GAP))
                .rounded(px(BUTTON_RADIUS))
                .bg(plate)
                .text_color(ink)
                .font_weight(FontWeight::MEDIUM)
                .when(!enabled, |button| button.opacity(DISABLED))
                .when(enabled, |button| {
                    button
                        .hover(|style| style.bg(Palette::on_dark(STATE_HOVERED)))
                        .active(|style| style.bg(Palette::on_dark(STATE_PRESSED)))
                })
                .when(a11y, |each| {
                    each.role(Role::Button)
                        .aria_label(button.label.to_string())
                        .when(!enabled, |each| {
                            each.a11y_synthetic_children(|builder| {
                                builder.parent_node().set_disabled()
                            })
                        })
                })
                .on_click(cx.listener(move |bar, _, _, cx| bar.press(index, cx)))
                .child(button.label.clone())
                .children(button.key.map(|key| {
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(key_ink)
                        .child(key_text(key))
                }))
        });
        root.child(
            div()
                .id("foot-bar")
                .when(a11y, |bar| bar.role(Role::Toolbar).aria_label(bar_said))
                .size_full()
                .relative()
                .flex()
                .items_center()
                .gap(px(gap))
                .pl(px(left))
                .pr(px(right))
                .rounded(px(radius))
                .bg(rgb(FILL))
                .text_size(px(TEXT))
                .child(words)
                .children(bar.hint.clone().map(|hint| {
                    div()
                        .flex_none()
                        .text_size(px(HINT))
                        .text_color(rgba(QUIET))
                        .child(hint)
                }))
                .children(buttons)
                // The popover level's rim, as the find bar's, its own layer so the border doesn't
                // move the bar's parts.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(radius))
                        .border(px(0.5))
                        .border_color(Level::Popover.rim()),
                ),
        )
    }
}

#[cfg(test)]
mod tests;
