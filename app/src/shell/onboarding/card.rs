//! First run's card (first-run-v9 §1): one 780 × 486 card, its heading and line at the top, the
//! step's controls in a column, a plane for the picture on the split steps, and the foot: Back,
//! the progress knot, an optional Skip and the main button. Everything is placed in the card's own
//! space, as the design gives it.

use gpui::{
    AnyElement, App, BoxShadow, FontWeight, MouseButton, Rgba, SharedString, Stateful,
    StyleRefinement, div, point, prelude::*, px, rgb, svg,
};

use super::{Step, with_alpha};
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

    /// A field's fill, and the recovery words' plate.
    pub(super) fn field(&self) -> Rgba {
        self.text(if self.light { t::LIGHT_FIELD } else { t::FIELD })
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
/// The headings' weight in the display face: v9's semibold, the face's 600.
const HEADING_WEIGHT: FontWeight = FontWeight::SEMIBOLD;

/// A heading's `title` in the display face at the headings' weight, a line `height` tall.
pub(super) fn heading_text(
    title: impl Into<SharedString>,
    size: f32,
    height: f32,
    cx: &mut App,
) -> gpui::Div {
    crate::shell::display_text_weighted(title, size, height, HEADING_WEIGHT, cx)
}

pub(super) fn heading(
    said: (SharedString, SharedString),
    centred: bool,
    inks: Inks,
    cx: &mut App,
) -> gpui::Div {
    let [_, y, _, _] = t::HEADING;
    heading_at(said, centred, y, inks, cx)
}

/// As `heading`, its top at `y`, as the Welcome's states place theirs lower under the knot.
pub(super) fn heading_at(
    (title, line): (SharedString, SharedString),
    centred: bool,
    y: f32,
    inks: Inks,
    cx: &mut App,
) -> gpui::Div {
    let [x, _, size, height] = t::HEADING;
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
        .hover(|style| style.opacity(t::BUTTON_HOVER))
        .focus_visible(card_ring(inks))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}

/// A small quiet button, as the recovery words' Copy is.
pub(super) fn small(id: &'static str, label: SharedString, inks: Inks) -> Stateful<gpui::Div> {
    let [height, radius, inset, size] = t::SMALL_BUTTON;
    quiet(id, label, inks)
        .h(px(height))
        .px(px(inset))
        .rounded(px(radius))
        .text_size(px(size))
}

/// The step's main button, `button` with its words in it: the text's ink as its fill and the key
/// that presses it, `key`, in a cap at its end, if it has one.
pub(super) fn main_button(
    button: Stateful<gpui::Div>,
    key: Option<SharedString>,
    inks: Inks,
) -> Stateful<gpui::Div> {
    let [height, radius, inset, gap] = t::BUTTON_SIZE;
    let [cap, cap_radius, cap_text, cap_alpha] = t::CAP;
    let [rest, hovered, pressed] = main_fills(inks);
    let end = if key.is_some() { t::MAIN_END } else { inset };
    let chip = key.map(|key| {
        div()
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
            .child(key)
    });
    button
        .flex_none()
        .relative()
        .h(px(height))
        .pl(px(inset))
        .pr(px(end))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(gap))
        .rounded(px(radius))
        .bg(rest)
        .text_color(inks.on_main())
        .text_size(px(t::BUTTON_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(move |style| style.bg(hovered))
        .active(move |style| style.bg(pressed))
        .focus_visible(card_ring(inks))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(chip)
}

/// The main button's fill at rest, under the pointer and pressed: the text's ink, then a step and
/// a further step toward its label's, as every button filled with ink goes in both looks.
pub(super) fn main_fills(inks: Inks) -> [Rgba; 3] {
    let rest = inks.text(1.);
    let (hovered, pressed) = crate::palette::ink_states(rest, inks.on_main());
    [rest, hovered, pressed]
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

/// The card's sheet and rim at `plate`: x, y, width, height and corners.
pub(super) fn plate(plate: super::exit::Plate, fill: Rgba, rim: Rgba) -> gpui::Div {
    let (x, y, w, h, radius) = plate;
    at(x, y)
        .w(px(w))
        .h(px(h))
        .rounded(px(radius))
        .bg(fill)
        .border_1()
        .border_color(rim)
}

#[cfg(test)]
thread_local! {
    /// How many whole knots have been drawn, for a test of what a frame draws.
    pub(super) static KNOTS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The mark whole, `size` across.
pub(super) fn whole_knot(size: f32, ink: Rgba) -> gpui::Svg {
    #[cfg(test)]
    KNOTS.set(KNOTS.get() + 1);
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
    match step.split() {
        true => (side, side + column(step)),
        false => (side, t::WIDTH - side),
    }
}

/// How wide a step's column is: a pictured step's own, beside its picture or centred without it,
/// and the centred steps' otherwise.
pub(super) fn column(step: Step) -> f32 {
    let [_, column] = t::COLUMN;
    match step {
        Step::Assistant => column,
        Step::Space => t::SPACE_FOOT,
        _ => t::CENTRED_COLUMN,
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
    let size = t::CHECK_DISC;
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
