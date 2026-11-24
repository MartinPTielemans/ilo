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
    matches!(step, Step::Intro | Step::Import | Step::Color)
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
    let sidebar = column(div().children(rows), &[(1., 1.), (0., 1.)], &palette);
    let shown = window(&palette, &weights, sidebar, page(shown, &palette))
        .children(place("window"))
        .children(ring)
        .child(arrow(pointer, colors, place).opacity(seen));
    centred(shown)
}

/// The import's: another browser's window above ilo's, its bookmarks and then its tabs dimming
/// one by one as each lands in ilo's sidebar, the bookmarks as favorites and the tabs as tabs,
/// then the dots of the Spaces that came and what came, said in the page. All of it goes again
/// at the loop's end, so it starts as it began; its last frame, still, is what came.
pub(super) fn import(t_: f32, colors: Colors, place: Place) -> impl IntoElement {
    const BOOKMARKS: [&str; 4] = ["Calendar", "Mail", "Wave", "Code"];
    const TABS: [&str; 4] = ["Roadmap", "Budget", "Lakeside", "Parks"];
    const TILES: [&str; 4] = ["tile-0", "tile-1", "tile-2", "tile-3"];
    const ROWS: [&str; 4] = ["tab-0", "tab-1", "tab-2", "tab-3"];
    let palette = Palette::of(None, tone(colors));
    // How far the loop's end has taken it all away again; none in the still last frame.
    let out = match t_ >= t::PERIOD {
        true => 0.,
        false => ((t_ - (t::PERIOD - t::IMPORT_OUT)) / t::IMPORT_OUT).clamp(0., 1.),
    };
    let [bookmarks_at, tabs_at] = t::IMPORT_AT;
    // How far the `index`th of a kind starting at `start` has landed.
    // How far a part starting at `from` has come over `over`: none before it starts, as the
    // easing's first step is a hair over nothing.
    let come = |from: f32, over: f32| match t_ > from {
        true => ease_out(((t_ - from) / over).min(1.)) * (1. - out),
        false => 0.,
    };
    let landed =
        |start: f32, index: usize| come(start + index as f32 * t::IMPORT_STAGGER, t::IMPORT_LAND);
    let seen = |start: f32| come(start, t::IMPORT_FADE);
    // In the other browser, each dims as it lands.
    let dimmed = |landed: f32| 1. - (1. - t::IMPORT_DIM) * landed;
    let chip = |name: &'static str, landed: f32| {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .opacity(dimmed(landed))
            .child(
                div()
                    .size(px(7.))
                    .rounded(px(t::ICON_RADIUS))
                    .bg(palette.faint(0.3)),
            )
            .child(name)
    };
    let tabs = TABS.iter().enumerate().map(|(index, &name)| {
        chip(name, landed(tabs_at, index))
            .h(px(16.))
            .px(px(6.))
            .rounded(px(t::PLATE_RADIUS))
            .bg(palette.faint(if index == 0 { 0.1 } else { 0. }))
    });
    let bookmarks = (BOOKMARKS.iter().enumerate())
        .map(|(index, &name)| chip(name, landed(bookmarks_at, index)));
    let lights = (0..3).map(|_| div().size(px(6.)).rounded_full().bg(palette.faint(0.22)));
    let other = div()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::STRIP_HEIGHT))
        .px(px(10.))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(6.))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(palette.page)
        .border_1()
        .border_color(palette.faint(0.12))
        .text_size(px(t::TEXT_SIZE))
        .text_color(palette.text(0.75))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .children(lights)
                .child(div().ml(px(6.)).flex().gap(px(2.)).children(tabs)),
        )
        .child(div().flex().gap(px(12.)).pl(px(6.)).children(bookmarks))
        .children(place("other"));
    // In ilo's sidebar, the favorites as tiles above the tabs, each coming down into place.
    let tiles = (0..BOOKMARKS.len()).filter_map(|index| {
        let landed = landed(bookmarks_at, index);
        (landed > 0.).then(|| {
            div()
                .absolute()
                .top(px(t::TILES_TOP - t::IMPORT_DROP * (1. - landed)))
                .left(px(6. + index as f32 * (t::TILE + t::TILE_GAP)))
                .size(px(t::TILE))
                .rounded(px(t::PLATE_RADIUS))
                .bg(palette.shown(0.))
                .opacity(landed)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(7.))
                        .rounded(px(t::ICON_RADIUS))
                        .bg(palette.faint(0.3)),
                )
                .children(place(TILES[index]))
        })
    });
    let rows = TABS.iter().enumerate().filter_map(|(index, &name)| {
        let landed = landed(tabs_at, index);
        (landed > 0.).then(|| {
            let top = t::ROWS_TOP + index as f32 * t::ROW - t::IMPORT_DROP * (1. - landed);
            row(name, index, 0., &palette)
                .top(px(top))
                .opacity(landed)
                .children(place(ROWS[index]))
        })
    });
    // The Spaces that came with them, a dot each beside the one shown.
    let spaces = seen(t::IMPORT_SPACES);
    let dots = [(1., 1.), (0., spaces), (0., spaces)];
    let sidebar =
        column(div().children(tiles).children(rows), &dots, &palette).children(place("sidebar"));
    // What came, said in the page as the import's Done says it.
    let came = seen(t::IMPORT_CAME);
    let said = (came > 0.).then(|| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .opacity(came)
            .text_size(px(t::TEXT_SIZE))
            .text_color(palette.text(0.75))
            .child(div().size(px(9.)).rounded_full().bg(palette.faint(0.5)))
            .child("4 favorites, 4 tabs, 3 Spaces")
            .children(place("came"))
    });
    let ilo = window(
        &palette,
        &[(palette, 1.)],
        sidebar,
        div().size_full().children(said),
    )
    .children(place("window"));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(t::STRIP_GAP))
        .child(other)
        .child(ilo)
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
    let next = Palette::of(Some(&Theme::of_color(other)), tone);
    // With nothing picked, the Space has the ground a color gives it and no glow, as the window
    // behind has, rather than the untinted material, lighter than its page in the dark.
    let first = match picked {
        Some(color) => Palette::of(Some(&Theme::of_color(color)), tone),
        None => Palette {
            underlay: next.underlay,
            ..Palette::of(None, tone)
        },
    };
    let palettes = [first, next];
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
    let dots = [(1. - across, 1.), (across, 1.)];
    let sidebar = column(rows, &dots, &near).children(place("sidebar"));
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

/// The sidebar's column: its rows, clipped to it, and at its foot a dot for each Space, `dots`,
/// as big and as strong as its Space shows, and as much there as the loop has it. A window of one
/// Space has a second for the Space it would swipe to, as the real foot does.
fn column(rows: impl IntoElement, dots: &[(f32, f32)], palette: &Palette) -> gpui::Div {
    let dots = dots
        .iter()
        .filter(|(_, there)| *there > 0.)
        .map(|&(shown, there)| {
            div()
                .size(px(t::DOT + (t::DOT_SHOWN - t::DOT) * shown))
                .rounded_full()
                .bg(palette.faint(t::DOT_ALPHA + (1. - t::DOT_ALPHA) * shown))
                .opacity(there)
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

/// Where the pointer presses the `index`th row and rests on it, in the window: on the row past
/// its name's end, so it covers none of it.
fn row_point(index: usize) -> (f32, f32) {
    (
        t::SIDEBAR * t::PRESS_ACROSS,
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
