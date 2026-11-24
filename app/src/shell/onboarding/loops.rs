//! First run's loops (#4093): a step's feature at work in a small window in the sheet's slot,
//! drawn from made-up rows by the step's time alone. Nothing in one is read from the profile or
//! the core, and nothing it shows is kept. It plays only while its step shows on a visible window
//! (Shell::onboarding_looping); with Reduce Motion it's its last frame, still.

use gpui::{PathBuilder, Rgba, canvas, div, point, prelude::*, px};

use super::{Colors, Step, ease_in_out, ease_out, mix};
use crate::tokens::shell::onboarding::loops as t;

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

/// The welcome's: a pointer goes down the tabs, clicking the second, the third, then the first
/// again, and the page follows each click.
pub(super) fn welcome(t_: f32, colors: Colors) -> impl IntoElement {
    const ROWS: [&str; 3] = ["Field notes", "Trail map", "Recipes"];
    const ORDER: [usize; 3] = [1, 2, 0];
    let rest = (t::SIDEBAR + 110., t::ROWS_TOP + 4. * t::ROW + 40.);
    let mut pointer = rest;
    let (mut shown, mut before, mut faded) = (0, 0, 1.);
    let mut clicked = None;
    for (index, (&start, &row)) in t::WELCOME_AT.iter().zip(&ORDER).enumerate() {
        let from = match index {
            0 => rest,
            _ => row_point(ORDER[index - 1]),
        };
        if t_ >= start {
            let moved = ease_in_out(((t_ - start) / t::WELCOME_MOVE).min(1.));
            pointer = lerp(from, row_point(row), moved);
        }
        let click = start + t::WELCOME_MOVE + t::WELCOME_CLICK;
        if t_ >= click {
            (before, shown) = (shown, row);
            faded = ((t_ - click) / t::WELCOME_FADE).min(1.);
            clicked = Some(t_ - click);
        }
    }
    // The pointer comes at the start and goes at the end, so the loop starts again unseen.
    let seen = (((t_ - 0.4) / 0.4).clamp(0., 1.)).min(((t::PERIOD - 0.2 - t_) / 0.4).clamp(0., 1.));
    let rows = ROWS.iter().enumerate().map(|(index, name)| {
        let plate = match index {
            _ if index == shown => faded,
            _ if index == before => 1. - faded,
            _ => 0.,
        };
        row(name, index, plate, colors)
    });
    let pages = (0..ROWS.len()).map(|index| {
        let opacity = match index {
            _ if index == shown => faded,
            _ if index == before => 1. - faded,
            _ => 0.,
        };
        page(index, colors).opacity(opacity)
    });
    let ring = clicked
        .filter(|&since| since < t::WELCOME_FADE * 2.)
        .map(|since| {
            let grown = since / (t::WELCOME_FADE * 2.);
            let side = 10. + 10. * ease_out(grown);
            div()
                .absolute()
                .left(px(pointer.0 - side / 2.))
                .top(px(pointer.1 - side / 2.))
                .size(px(side))
                .rounded_full()
                .border_1()
                .border_color(colors.ink(0.5 * (1. - grown)))
        });
    window(
        colors,
        None,
        div()
            .absolute()
            .top_0()
            .left_0()
            .w(px(t::SIDEBAR))
            .children(rows),
        div().children(pages),
    )
    .children(ring)
    .child(arrow(pointer, colors).opacity(seen))
}

/// The colour's: the window glows with the colour picked; two fingers swipe to another Space,
/// which glows with its own, and back.
pub(super) fn colour(t_: f32, picked: Rgba, other: Rgba, colors: Colors) -> impl IntoElement {
    const SPACES: [[&str; 3]; 2] = [["Inbox", "Notes", "Map"], ["Studio", "Sketches", "Palette"]];
    let [there, back] = t::COLOR_AT;
    let swiped = |start: f32| ease_in_out(((t_ - start) / t::COLOR_SWIPE).clamp(0., 1.));
    let across = swiped(there) - swiped(back);
    // Two fingers show a moment before each swipe and go after it, moving its way.
    let fingers = [there, back].into_iter().find_map(|start| {
        let near = t_ - (start - 0.3);
        (0. ..t::COLOR_SWIPE + 0.6).contains(&near).then(|| {
            let way = if start == there { -1. } else { 1. };
            let seen = (near / 0.2)
                .min((t::COLOR_SWIPE + 0.6 - near) / 0.2)
                .min(1.);
            (way * 40. * swiped(start), seen)
        })
    });
    let sidebar = div()
        .absolute()
        .top_0()
        .left(px(-t::SIDEBAR * across))
        .w(px(2. * t::SIDEBAR))
        .flex()
        .children(SPACES.iter().map(|rows| {
            div().w(px(t::SIDEBAR)).children(
                rows.iter().enumerate().map(|(index, name)| {
                    row(name, index, if index == 0 { 1. } else { 0. }, colors)
                }),
            )
        }));
    let fingers = fingers.map(|(moved, seen)| {
        let middle = t::SIDEBAR / 2. + moved;
        div()
            .absolute()
            .left(px(middle - 10.))
            .bottom(px(28.))
            .flex()
            .gap(px(6.))
            .opacity(seen)
            .children((0..2).map(|_| div().size(px(7.)).rounded_full().bg(colors.ink(0.45))))
    });
    window(
        colors,
        Some(mix(picked, other, across)),
        sidebar,
        page(0, colors),
    )
    .children(fingers)
}

/// The small window: its glow from the sidebar's foot, its sidebar and its page.
fn window(
    colors: Colors,
    glow: Option<Rgba>,
    sidebar: impl IntoElement,
    page: impl IntoElement,
) -> gpui::Div {
    let lights = div()
        .absolute()
        .top(px(12.))
        .left(px(12.))
        .flex()
        .gap(px(5.))
        .children((0..3).map(|_| div().size(px(7.)).rounded_full().bg(colors.ink(0.22))));
    let shown = div()
        .relative()
        .flex_none()
        .w(px(t::WINDOW_WIDTH))
        .h(px(t::WINDOW_HEIGHT))
        .overflow_hidden()
        .rounded(px(t::WINDOW_RADIUS))
        .bg(colors.ground)
        .border_1()
        .border_color(colors.ink(0.12))
        .children(glow.map(foot_glow))
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
                .bg(colors.ink(0.05))
                .child(page),
        );
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(shown)
}

/// A Space's colour from the window's bottom left, fading out: rings of it, each fainter out.
fn foot_glow(color: Rgba) -> gpui::Div {
    let (width, height) = (t::WINDOW_WIDTH * 1.1, t::WINDOW_HEIGHT * 1.2);
    div()
        .absolute()
        .left_0()
        .bottom_0()
        .children((0..5).map(move |ring| {
            let scale = 1. - ring as f32 * 0.18;
            let (w, h) = (width * scale, height * scale);
            div()
                .absolute()
                .left(px(-w / 2.))
                .bottom(px(-h / 2.))
                .w(px(w))
                .h(px(h))
                .rounded_full()
                .bg(Rgba { a: 0.14, ..color })
        }))
}

/// A tab's row in the sidebar, the `index`th, its plate shown as much as `plate`.
fn row(name: &'static str, index: usize, plate: f32, colors: Colors) -> gpui::Div {
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
                .bg(colors.ink(0.12))
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
                .text_color(colors.ink(0.75))
                .child(
                    div()
                        .size(px(9.))
                        .rounded(px(t::ICON_RADIUS))
                        .bg(colors.ink(0.3)),
                )
                .child(name),
        )
}

/// A made-up page: a heading's bar and a few lines, laid out its own way for each `index`.
fn page(index: usize, colors: Colors) -> gpui::Div {
    let bar = |width: f32, height: f32, alpha: f32| {
        div()
            .w(px(width))
            .h(px(height))
            .rounded(px(t::BAR_RADIUS))
            .bg(colors.ink(alpha))
    };
    let lines = |count: usize| {
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
                .children((0..3).map(|_| bar(54., 40., 0.1))),
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

/// The middle of the `index`th row's name, where the pointer clicks it, in the window.
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
fn arrow(at: (f32, f32), colors: Colors) -> gpui::Div {
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
}
