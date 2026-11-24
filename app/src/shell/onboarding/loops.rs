//! First run's loops (#4093): a step's feature at work in a small window in the sheet's slot,
//! drawn from made-up rows by the step's time alone. The window is the real one's at a small size:
//! a Space's palette, its glow (glow.rs), its selected row's plate and its Space dots, all clipped
//! to the small window. Nothing in one is read from the profile or the core, and nothing it shows
//! is kept. It plays only while its step shows on a visible window (Shell::onboarding_looping),
//! from its first visible frame; with Reduce Motion it's its last frame, still.

use gpui::{AnyElement, PathBuilder, canvas, div, point, prelude::*, px};

use super::{Colors, Step, ease_in_out, ease_out};
use crate::glow;
use crate::palette::{Palette, Tone};
use crate::theme::Theme;
use crate::tokens::palette::INK;
use crate::tokens::shell::onboarding::loops as t;

/// What keeps where a loop draws a part, by its name, for a test (Shell::place).
pub(super) type Place<'a> = &'a dyn Fn(&'static str) -> Option<AnyElement>;

/// The steps with a loop in their slot.
pub(super) fn has_loop(step: Step) -> bool {
    matches!(step, Step::Intro | Step::Color)
}

/// Where a loop is `secs` after it started, or its last frame while `still`.
pub(super) fn at(secs: Option<f32>, still: bool) -> f32 {
    match secs {
        Some(secs) if !still => secs % t::PERIOD,
        _ => t::PERIOD,
    }
}

/// The welcome's: the pointer goes to a tab and presses it, and the tab takes the selected plate
/// and its page shows, as a tab switch does: the second, the third, then the first again. It
/// comes as the loop starts and goes before it ends, so the last frame is the window alone.
pub(super) fn welcome(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const ROWS: [&str; 3] = ["Field notes", "Trail map", "Recipes"];
    const ORDER: [usize; 3] = [1, 2, 0];
    const PLACES: [&str; 3] = ["row-0", "row-1", "row-2"];
    let palette = Palette::of(None, tone(colors));
    let rest = (t::SIDEBAR + 110., t::ROWS_TOP + 4. * t::ROW + 40.);
    let mut pointer = rest;
    let mut shown = 0;
    let mut pressed = None;
    for (index, (&start, &row)) in t::WELCOME_AT.iter().zip(&ORDER).enumerate() {
        let from = match index {
            0 => rest,
            _ => row_point(ORDER[index - 1]),
        };
        if t_ >= start {
            let moved = ease_in_out(((t_ - start) / t::WELCOME_MOVE).min(1.));
            pointer = lerp(from, row_point(row), moved);
        }
        let press = start + t::WELCOME_MOVE + t::WELCOME_CLICK;
        if t_ >= press {
            shown = row;
            pressed = Some(t_ - press);
        }
    }
    let seen = (t_ / t::POINTER_FADE)
        .min((t::PERIOD - t::POINTER_FADE - t_) / t::POINTER_FADE)
        .clamp(0., 1.);
    let rows = ROWS.iter().enumerate().map(|(index, name)| {
        let plate = if index == shown { 1. } else { 0. };
        row(name, index, plate, &palette).children(place(PLACES[index]))
    });
    // The press's ring round the pointer's tip, growing as it fades.
    let ring = pressed
        .filter(|&since| since < t::WELCOME_PRESS)
        .map(|since| {
            let grown = since / t::WELCOME_PRESS;
            let side = 10. + 10. * ease_out(grown);
            div()
                .absolute()
                .left(px(pointer.0 - side / 2.))
                .top(px(pointer.1 - side / 2.))
                .size(px(side))
                .rounded_full()
                .border_1()
                .border_color(palette.faint(0.5 * (1. - grown)))
        });
    let weights = [(palette, 1.)];
    let sidebar = column(div().children(rows), &weights, &palette);
    let shown = window(&palette, &weights, sidebar, page(shown, &palette))
        .children(place("window"))
        .children(ring)
        .child(arrow(pointer, colors, place).opacity(seen));
    centred(shown)
}

/// The colour's: the window glows with the colour picked, or none, and the sidebar's rows swipe
/// to another Space in another colour, the window's glow, its page and its Space dots going with
/// them, and back.
pub(super) fn colour(
    t_: f32,
    picked: Option<u32>,
    other: u32,
    colors: Colors,
    place: Place,
) -> impl IntoElement {
    const SPACES: [[&str; 3]; 2] = [["Inbox", "Notes", "Map"], ["Studio", "Sketches", "Palette"]];
    let tone = tone(colors);
    let palettes = [
        Palette::of(picked.map(Theme::of_color).as_ref(), tone),
        Palette::of(Some(&Theme::of_color(other)), tone),
    ];
    let [there, back] = t::COLOR_AT;
    let swiped = |start: f32| ease_in_out(((t_ - start) / t::COLOR_SWIPE).clamp(0., 1.));
    let across = swiped(there) - swiped(back);
    let weights = [(palettes[0], 1. - across), (palettes[1], across)];
    let near = palettes[if across < 0.5 { 0 } else { 1 }];
    // The Spaces side by side, slid along under the sidebar's column, which shows one at rest.
    let rows = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(-t::SIDEBAR * across))
        .w(px(2. * t::SIDEBAR))
        .flex()
        .children(
            SPACES
                .iter()
                .zip(palettes)
                .enumerate()
                .map(|(space, (rows, palette))| {
                    div().relative().w(px(t::SIDEBAR)).h_full().children(
                        rows.iter().enumerate().map(move |(index, name)| {
                            let named = (space == 1 && index == 0).then(|| place("next-space-row"));
                            row(name, index, if index == 0 { 1. } else { 0. }, &palette)
                                .children(named.flatten())
                        }),
                    )
                }),
        );
    let pages = div()
        .absolute()
        .inset_0()
        .child(page(0, &palettes[0]).opacity(1. - across))
        .child(page(1, &palettes[1]).opacity(across));
    let sidebar = column(rows, &weights, &near).children(place("sidebar"));
    let shown = window(&near, &weights, sidebar, pages).children(place("window"));
    centred(shown)
}

/// The window's tone, as the flow's colours have it.
fn tone(colors: Colors) -> Tone {
    match colors.ink == INK {
        true => Tone::Light,
        false => Tone::Dark,
    }
}

/// The sidebar's column: its rows, clipped to it, and at its foot a dot for each Space, as big and
/// as strong as its Space shows. A window of one Space has a second for the Space it would swipe
/// to, as the real foot does.
fn column(rows: impl IntoElement, weights: &[(Palette, f32)], palette: &Palette) -> gpui::Div {
    let shown = |index: usize| match weights.len() {
        1 => {
            if index == 0 {
                1.
            } else {
                0.
            }
        }
        _ => weights.get(index).map_or(0., |(_, weight)| *weight),
    };
    let dots = (0..2).map(|index| {
        let weight = shown(index);
        div()
            .size(px(t::DOT + (t::DOT_SHOWN - t::DOT) * weight))
            .rounded_full()
            .bg(palette.faint(t::DOT_ALPHA + (1. - t::DOT_ALPHA) * weight))
    });
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(px(t::SIDEBAR))
        .overflow_hidden()
        .child(rows)
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(t::DOTS_BOTTOM))
                .flex()
                .justify_center()
                .items_center()
                .gap(px(t::DOT_GAP))
                .children(dots),
        )
}

/// The small window: its ground and the glows of the Spaces it shows, each as much as it shows,
/// its lights, its sidebar's column and its page card. What a loop adds over it goes in it, in its
/// terms, and nothing of it is drawn outside it.
fn window(
    palette: &Palette,
    weights: &[(Palette, f32)],
    sidebar: impl IntoElement,
    page: impl IntoElement,
) -> gpui::Div {
    let glows = weights.iter().flat_map(|&(palette, weight)| {
        [
            (glow::Corner::Foot, palette.glow),
            (glow::Corner::Far, palette.far_glow),
        ]
        .into_iter()
        .filter(move |(_, color)| color.a > 0. && weight > 0.)
        .map(move |(corner, color)| {
            glow::glow_scaled(corner, false, color, color.a * weight, t::SCALE)
        })
    });
    let lights = div()
        .absolute()
        .top(px(12.))
        .left(px(12.))
        .flex()
        .gap(px(5.))
        .children((0..3).map(|_| div().size(px(7.)).rounded_full().bg(palette.faint(0.22))));
    div()
        .relative()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::WINDOW_HEIGHT))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.underlay)
        .border_1()
        .border_color(palette.faint(0.12))
        .children(glows)
        .child(lights)
        .child(sidebar)
        .child(
            div()
                .absolute()
                .top(px(6.))
                .right(px(6.))
                .bottom(px(6.))
                .left(px(t::SIDEBAR))
                .overflow_hidden()
                .rounded(px(t::PLATE_RADIUS))
                .bg(palette.page)
                .child(page),
        )
}

/// The small window centred in the slot.
fn centred(shown: gpui::Div) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(shown)
}

/// A tab's row in the sidebar, the `index`th, with the selected plate as much as `plate`.
fn row(name: &'static str, index: usize, plate: f32, palette: &Palette) -> gpui::Div {
    div()
        .absolute()
        .top(px(t::ROWS_TOP + index as f32 * t::ROW))
        .left(px(6.))
        .w(px(t::SIDEBAR - 12.))
        .h(px(t::ROW))
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(t::PLATE_RADIUS))
                .bg(palette.shown(0.))
                .opacity(plate),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .px(px(7.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(t::TEXT_SIZE))
                .text_color(palette.text(0.75))
                .child(
                    div()
                        .size(px(9.))
                        .rounded(px(t::ICON_RADIUS))
                        .bg(palette.faint(0.3)),
                )
                .child(name),
        )
}

/// A made-up page: a heading's bar and a few lines, laid out its own way for each `index`.
fn page(index: usize, palette: &Palette) -> gpui::Div {
    let palette = *palette;
    let bar = move |width: f32, height: f32, alpha: f32| {
        div()
            .w(px(width))
            .h(px(height))
            .rounded(px(t::BAR_RADIUS))
            .bg(palette.faint(alpha))
    };
    let lines = move |count: usize| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children((0..count).map(move |line| bar(150. - 22. * (line % 3) as f32, 5., 0.14)))
    };
    let body = match index {
        0 => div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(bar(180., 64., 0.1))
            .child(lines(3)),
        1 => div()
            .flex()
            .gap(px(10.))
            .child(bar(84., 110., 0.1))
            .child(lines(6)),
        _ => div().flex().flex_col().gap(px(10.)).child(lines(4)).child(
            div()
                .flex()
                .gap(px(8.))
                .children((0..3).map(move |_| bar(54., 40., 0.1))),
        ),
    };
    div()
        .absolute()
        .inset_0()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(bar(96., 9., 0.3))
        .child(body)
}

/// The middle of the `index`th row's name, where the pointer presses it, in the window.
fn row_point(index: usize) -> (f32, f32) {
    (
        t::SIDEBAR * 0.45,
        t::ROWS_TOP + (index as f32 + 0.5) * t::ROW,
    )
}

fn lerp(from: (f32, f32), to: (f32, f32), t_: f32) -> (f32, f32) {
    (from.0 + (to.0 - from.0) * t_, from.1 + (to.1 - from.1) * t_)
}

/// The pointer, its tip at `at` in the window.
fn arrow(at: (f32, f32), colors: Colors, place: Place) -> gpui::Div {
    // An arrow's outline, its tip at the origin, as a system pointer is drawn.
    const OUTLINE: [(f32, f32); 7] = [
        (0., 0.),
        (0., 14.),
        (3.6, 10.6),
        (6.2, 16.),
        (8.4, 15.),
        (5.8, 9.8),
        (10.6, 9.8),
    ];
    let (ink, ground) = (colors.ink(0.9), colors.ground);
    div()
        .absolute()
        .left(px(at.0))
        .top(px(at.1))
        .size(px(18.))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let shape = |grow: f32| {
                        let mut path = PathBuilder::fill();
                        let mut points = OUTLINE
                            .iter()
                            .map(|&(x, y)| bounds.origin + point(px(x * grow), px(y * grow)));
                        if let Some(first) = points.next() {
                            path.move_to(first);
                        }
                        for each in points {
                            path.line_to(each);
                        }
                        path.close();
                        path.build().ok()
                    };
                    // The ground's edge round the ink, so it reads on any page.
                    if let Some(edge) = shape(1.15) {
                        window.paint_path(edge, ground);
                    }
                    if let Some(tip) = shape(1.) {
                        window.paint_path(tip, ink);
                    }
                },
            )
            .size_full(),
        )
        .children(place("pointer"))
}
