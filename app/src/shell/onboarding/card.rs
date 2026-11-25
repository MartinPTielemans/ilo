//! First run's card (first-run-v9 §1): one 780 × 486 card, its heading and line at the top, the
//! step's controls in a column, a plane for the picture on the split steps, and the foot: Back,
//! the progress knot, an optional Skip and the main button. Everything is placed in the card's own
//! space, as the design gives it.

use gpui::{
    AnyElement, App, BoxShadow, Context, FontWeight, MouseButton, Rgba, SharedString, Stateful,
    StyleRefinement, div, point, prelude::*, px, rgb, svg,
};

use super::{Shell, Step, display_text, with_alpha};
use crate::product::drawn_mark_path;
use crate::tokens::shell::onboarding::card as t;

/// The card's inks in the system's appearance.
#[derive(Clone, Copy)]
pub(super) struct Inks {
    text: u32,
    on_main: u32,
    pub(super) light: bool,
}

impl Inks {
    pub(super) fn of(light: bool) -> Inks {
        match light {
            true => Inks {
                text: t::LIGHT_TEXT,
                on_main: t::LIGHT_ON_MAIN,
                light,
            },
            false => Inks {
                text: t::TEXT,
                on_main: t::ON_MAIN,
                light,
            },
        }
    }

    /// The text's ink at `alpha`.
    pub(super) fn text(&self, alpha: f32) -> Rgba {
        with_alpha(self.text, alpha)
    }

    /// The second ink: a line under a heading, a row's words, a bare button.
    pub(super) fn text_2(&self) -> Rgba {
        self.text(t::TEXT_2)
    }

    /// The card's sheet over the glass where there is some; with none behind it, opaque, as a
    /// sheet meant to be blurred would show the desktop sharp through it.
    pub(super) fn fill(&self, glass: bool) -> Rgba {
        let (fill, alpha) = match self.light {
            true => (t::LIGHT_FILL, t::LIGHT_FILL_ALPHA),
            false => (t::FILL, t::FILL_ALPHA),
        };
        with_alpha(fill, if glass { alpha } else { 1. })
    }

    pub(super) fn rim(&self) -> Rgba {
        with_alpha(
            t::RIM,
            if self.light {
                t::LIGHT_RIM_ALPHA
            } else {
                t::RIM_ALPHA
            },
        )
    }

    /// A group of rows' ground.
    pub(super) fn group(&self) -> Rgba {
        self.text(if self.light { t::LIGHT_GROUP } else { t::GROUP })
    }

    /// A quiet button's fill.
    pub(super) fn button(&self) -> Rgba {
        self.text(if self.light {
            t::LIGHT_BUTTON
        } else {
            t::BUTTON
        })
    }

    pub(super) fn plane(&self) -> Rgba {
        match self.light {
            true => with_alpha(t::LIGHT_PLANE_FILL, t::LIGHT_PLANE_ALPHA),
            false => with_alpha(t::PLANE_FILL, t::PLANE_ALPHA),
        }
    }

    pub(super) fn on_main(&self) -> Rgba {
        rgb(self.on_main)
    }
}

/// The card's keyboard ring, v9's: wider and fainter than the app's, as the card is its own
/// surface over the desktop.
pub(super) fn card_ring(inks: Inks) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    let color = inks.text(t::RING_ALPHA);
    move |style| {
        style.shadow(vec![BoxShadow {
            color: color.into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(t::RING),
            inset: false,
        }])
    }
}

/// An element placed at (`x`, `y`) in the card.
pub(super) fn at(x: f32, y: f32) -> gpui::Div {
    div().absolute().left(px(x)).top(px(y))
}

/// The step's heading and its line, at the column's start or centred over the card.
/// A heading's `title` in the display face at the headings' weight, a line `height` tall.
pub(super) fn heading_text(
    title: impl Into<SharedString>,
    size: f32,
    height: f32,
    cx: &mut App,
) -> gpui::Div {
    display_text(title, size, height, cx).font_weight(FontWeight(t::HEADING_WEIGHT))
}

pub(super) fn heading(
    (title, line): (SharedString, SharedString),
    centred: bool,
    inks: Inks,
    cx: &mut App,
) -> gpui::Div {
    let [x, y, size, height] = t::HEADING;
    let [gap, line_size, line_height] = t::LINE;
    let place = |row: gpui::Div| match centred {
        true => row.w(px(t::WIDTH)).flex().justify_center().text_center(),
        false => row,
    };
    let x = if centred { 0. } else { x };
    div()
        .child(
            place(at(x, y)).child(heading_text(title, size, height, cx).text_color(inks.text(1.))),
        )
        .when(!line.is_empty(), |both| {
            both.child(
                place(at(x, y + height + gap))
                    .text_size(px(line_size))
                    .line_height(px(line_height))
                    .text_color(inks.text_2())
                    .child(line),
            )
        })
}

/// The plane the split steps' picture stands on, empty until each step's picture lands.
pub(super) fn plane(inks: Inks) -> gpui::Div {
    let [x, y, width, height] = t::PLANE;
    at(x, y)
        .w(px(width))
        .h(px(height))
        .rounded(px(t::PLANE_RADIUS))
        .bg(inks.plane())
}

/// A bare button: its words in the second ink, the text's under the pointer.
pub(super) fn bare(id: &'static str, label: SharedString, inks: Inks) -> Stateful<gpui::Div> {
    let [height, radius, _, _] = t::BUTTON_SIZE;
    div()
        .id(id)
        .flex_none()
        .h(px(height))
        .px(px(t::BARE_INSET))
        .flex()
        .items_center()
        .rounded(px(radius))
        .text_size(px(t::BUTTON_TEXT))
        .text_color(inks.text_2())
        .hover(move |style| style.text_color(inks.text(1.)))
        .focus_visible(card_ring(inks))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// A quiet button: the button fill, its words in the text's ink, no key.
pub(super) fn quiet(id: &'static str, label: SharedString, inks: Inks) -> Stateful<gpui::Div> {
    let [height, radius, inset, _] = t::BUTTON_SIZE;
    div()
        .id(id)
        .flex_none()
        .h(px(height))
        .px(px(inset))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .bg(inks.button())
        .text_size(px(t::BUTTON_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(inks.text(1.))
        .hover(|style| style.opacity(0.9))
        .focus_visible(card_ring(inks))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// The step's main button, `button` with its words in it: the text's ink as its fill and the key
/// that presses it, `key`, in a cap at its end.
pub(super) fn main_button(
    button: Stateful<gpui::Div>,
    key: SharedString,
    inks: Inks,
) -> Stateful<gpui::Div> {
    let [height, radius, inset, gap] = t::BUTTON_SIZE;
    let [cap, cap_radius, cap_text, cap_alpha] = t::CAP;
    let chip = div()
        .h(px(cap))
        .min_w(px(cap))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(cap_radius))
        .bg(Rgba {
            a: cap_alpha,
            ..inks.on_main()
        })
        .text_size(px(cap_text))
        .font_weight(FontWeight::NORMAL)
        .child(key);
    button
        .flex_none()
        .relative()
        .h(px(height))
        .pl(px(inset))
        .pr(px(t::MAIN_END))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(gap))
        .rounded(px(radius))
        .bg(inks.text(1.))
        .text_color(inks.on_main())
        .text_size(px(t::BUTTON_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.opacity(0.9))
        .focus_visible(card_ring(inks))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(chip)
}

/// The Welcome's taller button, a column wide.
pub(super) fn welcome_button(button: Stateful<gpui::Div>) -> Stateful<gpui::Div> {
    let [height, radius, size] = t::WELCOME_BUTTON;
    let [width, _] = t::WELCOME_COLUMN;
    button
        .w(px(width))
        .h(px(height))
        .rounded(px(radius))
        .text_size(px(size))
}

/// The mark `size` across, drawn `fraction` of the way along its cord over its faint whole.
pub(super) fn knot(size: f32, fraction: f32, ink: Rgba) -> gpui::Svg {
    svg()
        .path(drawn_mark_path(size, fraction))
        .size(px(size))
        .flex_none()
        .text_color(ink)
}

/// The mark whole, `size` across.
pub(super) fn whole_knot(size: f32, ink: Rgba) -> gpui::Svg {
    svg()
        .path(crate::product::mark_path(size))
        .size(px(size))
        .flex_none()
        .text_color(ink)
}

/// Where a step's foot runs along the card: the whole width on the centred steps, the control
/// column on the split ones, so nothing of the foot sits on the picture.
pub(super) fn foot_span(step: Step) -> (f32, f32) {
    let side = t::FOOT_SIDE;
    let [_, column] = t::COLUMN;
    match step {
        Step::Assistant | Step::Tidy => (side, side + column),
        Step::Space => (side, side + t::SPACE_FOOT),
        _ => (side, t::WIDTH - side),
    }
}

/// The foot along `span`: `back` at its start, the progress knot `progress` of the way centred in
/// it, and `trail` (Skip, the main button) at its end.
pub(super) fn foot(
    (start, end): (f32, f32),
    back: Option<AnyElement>,
    progress: Option<f32>,
    trail: Vec<AnyElement>,
    inks: Inks,
) -> gpui::Div {
    let [top, height] = t::FOOT;
    let knot = progress.map(|fraction| {
        let size = t::PROGRESS;
        at((end - start - size) / 2., (height - size) / 2.).child(knot(
            size,
            fraction,
            inks.text(1.),
        ))
    });
    at(start, top)
        .w(px(end - start))
        .h(px(height))
        .flex()
        .items_center()
        .gap(px(t::FOOT_GAP))
        .children(knot)
        .children(back)
        .child(div().flex_1())
        .children(trail)
}

/// A group of rows on the card, a hairline between each, as Settings' groups are.
pub(super) fn rows(rows: Vec<gpui::AnyElement>, width: f32, inks: Inks) -> gpui::Div {
    let count = rows.len();
    div()
        .w(px(width))
        .flex()
        .flex_col()
        .rounded(px(crate::tokens::radius::ROW))
        .bg(inks.group())
        .children(rows.into_iter().enumerate().map(move |(at, row)| {
            div()
                .when(at + 1 < count, |row| {
                    row.border_b_1().border_color(inks.text(t::ROW_LINE))
                })
                .child(row)
        }))
}

/// A row: its name over its words, then `end`, the row's control.
pub(super) fn row(
    name: SharedString,
    words: SharedString,
    end: Option<AnyElement>,
    inks: Inks,
) -> gpui::Div {
    let [height, inset, gap] = t::ROW;
    let [name_size, name_line] = t::ROW_NAME;
    let [words_size, words_line] = t::ROW_WORDS;
    div()
        .relative()
        .h(px(height))
        .px(px(inset))
        .flex()
        .items_center()
        .gap(px(gap))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(name_size))
                        .line_height(px(name_line))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(inks.text(1.))
                        .child(name),
                )
                .child(
                    div()
                        .text_size(px(words_size))
                        .line_height(px(words_line))
                        .text_color(inks.text_2())
                        .child(words),
                ),
        )
        .children(end)
}

/// The check disc a row shows once what it offered is done.
pub(super) fn check_disc(inks: Inks) -> gpui::Div {
    let [_, _, size] = t::BROOM;
    div()
        .flex_none()
        .size(px(size + 2.))
        .rounded_full()
        .bg(inks.text(1.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path("icons/check.svg")
                .size(px(size - 2.))
                .text_color(inks.on_main()),
        )
}

impl Shell {
    /// Tidy tabs' card: "Put away after" and the time, over a bar of the build's times. A click or
    /// a drag on the bar puts it at the stop nearest; the arrows move it a stop.
    pub(super) fn tidy_card(&self, inks: Inks, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let tidy = (self.onboarding.as_ref()).and_then(|onboarding| onboarding.tidy.clone())?;
        let [top, height, inset] = t::TIDY;
        let [width, bar_height, thumb] = t::TIDY_BAR;
        let [tick, tick_alpha] = t::TIDY_TICK;
        let [shadow_y, shadow_blur, shadow_alpha] = t::TIDY_THUMB_SHADOW;
        let [_, column] = t::COLUMN;
        let last = tidy.stops.len().saturating_sub(1).max(1);
        // Each stop's centre along the bar, the thumb's half in from either end.
        let x = move |at: usize| thumb / 2. + (width - thumb) * at as f32 / last as f32;
        let thumb_at = x(tidy.at);
        let words = tidy.stops[tidy.at].1;
        let ticks = (0..tidy.stops.len()).map(move |each| {
            at(x(each) - tick / 2., (bar_height - tick) / 2.)
                .size(px(tick))
                .rounded_full()
                .bg(inks.text(tick_alpha))
        });
        // Where the bar is drawn, which a click's place along it is read against.
        let drawn: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>> =
            Default::default();
        let stop_at = move |bounds: Option<gpui::Bounds<gpui::Pixels>>,
                            pointer: gpui::Point<gpui::Pixels>| {
            let along = bounds.map_or(0., |bounds| f32::from(pointer.x - bounds.origin.x));
            let along = ((along - thumb / 2.) / (width - thumb)).clamp(0., 1.);
            (along * last as f32).round() as usize
        };
        let (pressed, moved) = (drawn.clone(), drawn.clone());
        let count = tidy.stops.len();
        let bar = div()
            .id("onboarding-tidy")
            .relative()
            .w(px(width))
            .h(px(bar_height))
            .tab_index(0)
            .when_some(self.first_stop(Step::Tidy), |bar, stop| {
                bar.track_focus(&stop)
            })
            .focus_from_keys_only()
            .rounded_full()
            .focus_visible(card_ring(inks))
            .when(self.a11y, |bar| {
                bar.role(gpui::accesskit::Role::Slider)
                    .aria_label("Put away after")
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value((count - 1) as f64)
                    .aria_numeric_value_step(1.)
                    .aria_numeric_value(tidy.at as f64)
                    .on_a11y_action(gpui::accesskit::Action::Increment, {
                        let shell = cx.weak_entity();
                        move |_, _, cx| {
                            shell
                                .update(cx, |shell, cx| shell.onboarding_tidy_by(1, cx))
                                .ok();
                        }
                    })
                    .on_a11y_action(gpui::accesskit::Action::Decrement, {
                        let shell = cx.weak_entity();
                        move |_, _, cx| {
                            shell
                                .update(cx, |shell, cx| shell.onboarding_tidy_by(-1, cx))
                                .ok();
                        }
                    })
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.onboarding_tidy(stop_at(pressed.get(), event.position), cx);
                }),
            )
            .on_mouse_move(
                cx.listener(move |shell, event: &gpui::MouseMoveEvent, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        shell.onboarding_tidy(stop_at(moved.get(), event.position), cx);
                    }
                }),
            )
            .child(
                gpui::canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .bg(inks.text(t::TIDY_TRACK))
                    .child(
                        div()
                            .h_full()
                            .w(px(thumb_at))
                            .rounded_full()
                            .bg(inks.text(t::TIDY_FILLED)),
                    ),
            )
            .children(ticks)
            .children(self.place(Step::Tidy, "tidy"))
            .child(
                at(thumb_at - thumb / 2., (bar_height - thumb) / 2.)
                    .size(px(thumb))
                    .rounded_full()
                    .bg(rgb(t::TIDY_THUMB))
                    .shadow(vec![BoxShadow {
                        color: gpui::hsla(0., 0., 0., shadow_alpha),
                        offset: point(px(0.), px(shadow_y)),
                        blur_radius: px(shadow_blur),
                        spread_radius: px(0.),
                        inset: false,
                    }]),
            );
        let [name_size, name_line] = t::ROW_NAME;
        Some(
            div()
                .w(px(column))
                .h(px(height))
                .mt(px(top - t::BODY_TOP))
                .px(px(inset))
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(inset / 2.))
                .rounded(px(crate::tokens::radius::ROW))
                .bg(inks.group())
                .text_size(px(name_size))
                .line_height(px(name_line))
                .text_color(inks.text(1.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child("Put away after")
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(words)),
                )
                .child(bar),
        )
    }

    /// The broom's row, itself the control, where the window has a broom: only with threads on and
    /// the broom there at all, which `Browser::broom` answers whatever today's tabs number.
    pub(super) fn broom_row(&self, inks: Inks, cx: &App) -> Option<gpui::Div> {
        if !self.broom_card(cx) {
            return None;
        }
        let [top, height, glyph] = t::BROOM;
        let [tidy_top, tidy_height, _] = t::TIDY;
        let [_, column] = t::COLUMN;
        let [name_size, name_line] = t::ROW_NAME;
        let [words_size, words_line] = t::ROW_WORDS;
        let [_, inset, gap] = t::ROW;
        Some(
            div()
                .relative()
                .w(px(column))
                .h(px(height))
                .mt(px(top - tidy_top - tidy_height))
                .px(px(inset))
                .flex()
                .items_center()
                .gap(px(gap))
                .rounded(px(crate::tokens::radius::ROW))
                .hover(move |row| row.bg(inks.group()))
                .child(
                    svg()
                        .path("icons/broom.svg")
                        .size(px(glyph))
                        .flex_none()
                        .text_color(inks.text(1.)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(name_size))
                                .line_height(px(name_line))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(inks.text(1.))
                                .child("Or sweep them into folders"),
                        )
                        .child(
                            div()
                                .text_size(px(words_size))
                                .line_height(px(words_line))
                                .text_color(inks.text_2())
                                .child("Click the broom above today's tabs."),
                        ),
                )
                .children(self.place(Step::Tidy, "broom")),
        )
    }

    /// Whether the window has a broom: only with an assistant, and only where the broom can be at
    /// all.
    pub(super) fn broom_card(&self, cx: &App) -> bool {
        self.browser.threads(cx) == Some(true) && self.browser.broom().is_some()
    }
}
