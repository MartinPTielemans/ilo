//! First run's picture (spec-first-run.md, "The picture"): a small browser drawn from tokens, with
//! made-up content that is the same for everyone and that nothing in can be clicked, and over it
//! what shows the step's change. It's one drawing: a step is a `Scene`, which says only which of a
//! few overlays sit on the one window and how the Space's colour lies in it, so there are no
//! per-step copies of the window to drift apart.

use gpui::{
    AnyElement, BoxShadow, FontWeight, Hsla, PathBuilder, Pixels, Point, Rgba, SharedString,
    canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, rgb,
};

use super::{Colors, Step, with_alpha};
use crate::tokens::palette::INK;
use crate::tokens::shell::onboarding::picture as t;
pub(super) use crate::tokens::shell::onboarding::picture::SPACE;

/// What a step adds to the window: what changes, and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Scene {
    /// The Space's colour sweeps the window, where the step is the Space's own look.
    pub swept: bool,
    /// A thread at work in the sidebar, under New Tab.
    pub thread: bool,
    /// A caption over the picture.
    pub chip: Option<Chip>,
    /// The favorites' row ringed.
    pub ring: bool,
    /// A link from another app's message, which opens here.
    pub notice: bool,
    /// The command bar searching with the engine picked.
    pub bar: bool,
    /// The Spaces on a phone.
    pub phone: bool,
}

/// A caption over the picture: its icon and its words, where it has words. A chip that would
/// claim a count about the user's own things is drawn with none (spec-first-run.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Chip {
    Thread,
    Found,
    Favorites,
}

impl Chip {
    fn icon(self) -> Option<Glyph> {
        match self {
            Chip::Thread => Some(Glyph::Ring),
            Chip::Found => Some(Glyph::Check),
            Chip::Favorites => None,
        }
    }

    fn words(self) -> Option<&'static str> {
        match self {
            Chip::Thread => Some("A thread is a job you hand it, done in tabs of its own"),
            Chip::Found => None,
            Chip::Favorites => Some("Favorites sit here, in every Space"),
        }
    }
}

impl Scene {
    /// The step's scene, as values-first-run.md draws its screen.
    pub(super) fn of(step: Step) -> Scene {
        let plain = Scene::default();
        match step {
            Step::Intro | Step::OptIns => plain,
            Step::Assistant => Scene {
                thread: true,
                chip: Some(Chip::Thread),
                ..plain
            },
            Step::Import => Scene {
                chip: Some(Chip::Found),
                ..plain
            },
            Step::Favorites => Scene {
                ring: true,
                chip: Some(Chip::Favorites),
                ..plain
            },
            Step::DefaultBrowser => Scene {
                notice: true,
                ..plain
            },
            Step::SearchEngine => Scene { bar: true, ..plain },
            Step::Color | Step::Transparency => Scene {
                swept: true,
                ..plain
            },
            Step::Account => Scene {
                phone: true,
                ..plain
            },
        }
    }
}

/// The picture's inks in the window's appearance.
#[derive(Clone, Copy)]
struct Inks {
    text: u32,
    quiet: f32,
    faint: f32,
    shown: f32,
    fill: (u32, f32),
    rim: (u32, f32),
    edge: f32,
    shadow: f32,
    tile_line: (u32, f32),
    notice: u32,
    link: u32,
    bar: u32,
}

impl Inks {
    fn of(colors: Colors) -> Inks {
        match colors.ink == INK {
            true => Inks {
                text: t::LIGHT_TEXT,
                quiet: t::LIGHT_QUIET,
                faint: t::LIGHT_FAINT,
                shown: t::LIGHT_SHOWN,
                fill: (t::LIGHT_WINDOW_FILL, t::LIGHT_WINDOW_FILL_ALPHA),
                rim: (t::LIGHT_RIM, t::LIGHT_RIM_ALPHA),
                edge: t::LIGHT_EDGE_ALPHA,
                shadow: t::LIGHT_SHADOW_ALPHA,
                tile_line: (t::LIGHT_TILE_LINE, t::LIGHT_TILE_LINE_ALPHA),
                notice: t::LIGHT_NOTICE_FILL,
                link: t::LIGHT_LINK,
                bar: t::LIGHT_BAR_FILL,
            },
            false => Inks {
                text: t::TEXT,
                quiet: t::QUIET,
                faint: t::FAINT,
                shown: t::SHOWN,
                fill: (t::WINDOW_FILL, t::WINDOW_FILL_ALPHA),
                rim: (t::RIM, t::RIM_ALPHA),
                edge: t::EDGE_ALPHA,
                shadow: t::SHADOW_ALPHA,
                tile_line: (t::TILE_LINE, t::TILE_LINE_ALPHA),
                notice: t::NOTICE_FILL,
                link: t::LINK,
                bar: t::BAR_FILL,
            },
        }
    }

    fn ink(&self, alpha: f32) -> Rgba {
        with_alpha(self.text, alpha)
    }

    fn line(&self) -> Rgba {
        with_alpha(self.tile_line.0, self.tile_line.1)
    }
}

/// The picture for `scene`, the Space's colour `space` in it.
pub(super) fn picture(scene: Scene, space: u32, colors: Colors) -> AnyElement {
    let inks = Inks::of(colors);
    let [left, top, width, height] = t::WINDOW;
    let [shadow_y, shadow_blur] = t::SHADOW;
    let tint = match scene.swept {
        true => div().absolute().inset_0().bg(linear_gradient(
            135.,
            linear_color_stop(with_alpha(space, t::SWEPT_ALPHA), 0.),
            linear_color_stop(with_alpha(t::SPACE_END, t::SWEPT_ALPHA), 0.75),
        )),
        // A glow from the window's middle, softened past its edge.
        false => div()
            .absolute()
            .left(px((width - t::GLOW_SIZE) / 2.))
            .top(px((height - t::GLOW_SIZE) / 2.))
            .size(px(t::GLOW_SIZE))
            .rounded_full()
            .bg(with_alpha(space, t::GLOW_ALPHA))
            .shadow(vec![shadow(
                with_alpha(space, t::GLOW_ALPHA),
                0.,
                t::GLOW_BLUR,
            )]),
    };
    let window = div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .rounded(px(t::WINDOW_RADIUS))
        .bg(with_alpha(inks.fill.0, inks.fill.1))
        .shadow(vec![
            BoxShadow {
                inset: true,
                spread_radius: px(t::LINE),
                ..shadow(with_alpha(inks.rim.0, inks.rim.1), 0., 0.)
            },
            BoxShadow {
                spread_radius: px(t::LINE),
                ..shadow(with_alpha(0, inks.edge), 0., 0.)
            },
            shadow(with_alpha(0, inks.shadow), shadow_y, shadow_blur),
        ])
        .child(
            div()
                .size_full()
                .relative()
                .overflow_hidden()
                .rounded(px(t::WINDOW_RADIUS))
                .child(tint)
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .child(sidebar(scene, inks))
                        .child(page()),
                ),
        );
    div()
        .absolute()
        .inset_0()
        .child(window)
        .when(scene.ring, |picture| picture.child(ring(inks)))
        .when(scene.notice, |picture| picture.child(notice(inks)))
        .when(scene.bar, |picture| picture.child(bar(inks)))
        .when(scene.phone, |picture| picture.child(phone()))
        .children(scene.chip.map(|chip| caption(chip, inks)))
        .into_any_element()
}

/// A box shadow of `color`, `y` down and blurred by `blur`.
fn shadow(color: Rgba, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// A line of text at `size` (its size and line height), in `color`, on one line cut short.
fn text(words: impl Into<SharedString>, size: [f32; 2], color: impl Into<Hsla>) -> gpui::Div {
    let [size, line] = size;
    div()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(size))
        .line_height(px(line))
        .text_color(color)
        .child(words.into())
}

/// A small square of `color` with corners of `radius`, as a site's mark.
fn mark(size: f32, radius: f32, color: u32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius))
        .bg(rgb(color))
}

/// The small sidebar: the window's buttons, the address, the favorites, then the Space's tabs.
fn sidebar(scene: Scene, inks: Inks) -> gpui::Div {
    let [width, padding, gap] = t::SIDEBAR;
    let [buttons_height, buttons_padding, buttons_gap, dot] = t::BUTTONS;
    let [address_height, address_padding] = t::ADDRESS;
    let [favorites_height, favorites_gap] = t::FAVORITES;
    let [_, _, _, row_mark] = t::ROW;
    let [roadmap, budget, open, parks, kestrel] = t::MARKS;
    let lights = div()
        .flex_none()
        .h(px(buttons_height))
        .px(px(buttons_padding))
        .flex()
        .items_center()
        .gap(px(buttons_gap))
        .children(t::LIGHTS.map(|color| mark(dot, t::BUTTON_RADIUS, color)));
    let address = div()
        .flex_none()
        .h(px(address_height))
        .px(px(address_padding))
        .flex()
        .items_center()
        .rounded(px(t::ADDRESS_RADIUS))
        .bg(inks.ink(t::FIELD))
        .child(text(
            "fieldbook.example",
            t::ADDRESS_TEXT,
            inks.ink(inks.quiet),
        ));
    let rounds = [false, false, true, false];
    let favorites = div()
        .flex_none()
        .h(px(favorites_height))
        .flex()
        .gap(px(favorites_gap))
        .children(t::FAVORITE_MARKS.iter().zip(rounds).map(|(&color, round)| {
            let radius = match round {
                true => t::FAVORITE_ROUND_RADIUS,
                false => t::FAVORITE_MARK_RADIUS,
            };
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(t::FAVORITE_RADIUS))
                .bg(inks.ink(t::CARD))
                .child(mark(t::FAVORITE_MARK, radius, color))
        }));
    let tab = |name: &'static str, color: u32, round: bool| {
        row(inks)
            .child(mark(
                row_mark,
                match round {
                    true => t::MARK_ROUND_RADIUS,
                    false => t::MARK_RADIUS,
                },
                color,
            ))
            .child(text(name, t::ROW_TEXT, inks.ink(1.)))
    };
    let space =
        row(inks).child(text("Home", t::ROW_TEXT, inks.ink(1.)).font_weight(FontWeight::SEMIBOLD));
    div()
        .flex_none()
        .w(px(width))
        .h_full()
        .px(px(padding))
        .flex()
        .flex_col()
        .gap(px(gap))
        .child(lights)
        .child(address)
        .child(favorites)
        .child(
            div()
                .flex()
                .flex_col()
                .child(space)
                .child(tab("Roadmap", roadmap, false))
                .child(tab("Budget 2027", budget, false))
                .child(new_tab(inks))
                .children(scene.thread.then(|| thread(inks)))
                .child(open_tab(open))
                .child(tab("The largest city parks", parks, true))
                .child(tab("The Kestrel 14, six mon…", kestrel, true)),
        )
}

/// A row of the small sidebar, without its contents.
fn row(_: Inks) -> gpui::Div {
    let [height, padding, gap, _] = t::ROW;
    div()
        .flex_none()
        .h(px(height))
        .px(px(padding))
        .flex()
        .items_center()
        .gap(px(gap))
}

/// New Tab's row: its plus, its label and the hairline that parts the pinned from today's tabs.
fn new_tab(inks: Inks) -> gpui::Div {
    let [height, size, line, plus] = t::NEW_TAB;
    let [_, padding, gap, slot] = t::ROW;
    div()
        .flex_none()
        .h(px(height))
        .px(px(padding))
        .flex()
        .items_center()
        .gap(px(gap))
        .child(
            div()
                .flex_none()
                .size(px(slot))
                .flex()
                .items_center()
                .justify_center()
                .child(glyph(Glyph::Plus, plus, inks.ink(inks.faint))),
        )
        .child(text("New Tab", [size, line], inks.ink(inks.faint)).flex_none())
        .child(div().flex_1().h(px(t::LINE)).bg(inks.ink(t::HAIRLINE)))
}

/// The tab open in the small window, on its page's own ground and joined to it.
fn open_tab(color: u32) -> gpui::Div {
    let [height, padding, gap, size] = t::ROW;
    div()
        .flex_none()
        .w(px(t::OPEN_TAB))
        .h(px(height))
        .px(px(padding))
        .flex()
        .items_center()
        .gap(px(gap))
        .rounded_l(px(t::OPEN_TAB_RADIUS))
        .bg(rgb(t::PAGE))
        .child(mark(size, t::MARK_RADIUS, color))
        .child(text(
            "The Lakeside loop · Fie…",
            t::ROW_TEXT,
            rgb(t::PAGE_INK),
        ))
}

/// A thread at work in the sidebar: its ring, its name, what it's doing and the sites it reads.
fn thread(inks: Inks) -> gpui::Div {
    let [padding_y, padding_x, gap, ring, indent] = t::THREAD;
    let [first, second, third] = t::THREAD_MARKS;
    let site = |color: u32, faded: bool| {
        mark(t::THREAD_MARK, t::THREAD_MARK / 2., color)
            .when(faded, |mark| mark.opacity(t::THREAD_LATER))
    };
    div().pb(px(gap)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(gap))
            .py(px(padding_y))
            .px(px(padding_x))
            .rounded(px(t::THREAD_RADIUS))
            .bg(inks.ink(t::CARD))
            .border_1()
            .border_color(inks.ink(t::THREAD_LINE_ALPHA))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(padding_x - 1.))
                    .child(glyph(Glyph::Ring, ring, inks.ink(1.)))
                    .child(
                        text("Best price, Kestrel 14", t::ROW_TEXT, inks.ink(1.))
                            .font_weight(FontWeight::SEMIBOLD),
                    ),
            )
            .child(div().pl(px(indent)).child(text(
                "Reading the price on Hexa",
                t::THREAD_SUB_TEXT,
                inks.ink(inks.quiet),
            )))
            .child(
                div()
                    .pl(px(indent))
                    .flex()
                    .items_center()
                    .gap(px(gap - 1.))
                    .child(site(first, false))
                    .child(site(second, false))
                    .child(site(third, true))
                    .child(div().flex_1())
                    .child(text("2 of 3", t::THREAD_COUNT_TEXT, inks.ink(inks.faint))),
            ),
    )
}

/// The small page: a made-up journal's article, the same in both appearances.
fn page() -> gpui::Div {
    let [head_height, head_padding, head_gap, head_mark] = t::PAGE_HEAD;
    let [padding_y, padding_x, gap] = t::ARTICLE;
    let [sky_top, sky_foot] = t::SKY;
    let [sun_left, sun_top, sun] = t::SUN_AT;
    let hill = |index: usize| {
        let [left, top, width] = [0, 1, 2].map(|at| t::HILLS_AT[index * 3 + at]);
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(width))
            .rounded_full()
            .bg(rgb(t::HILLS[index]))
    };
    let scene = div()
        .flex_none()
        .relative()
        .h(px(t::SCENE_HEIGHT))
        .overflow_hidden()
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(sky_top), 0.),
            linear_color_stop(rgb(sky_foot), 1.),
        ))
        .child(
            div()
                .absolute()
                .left(px(sun_left))
                .top(px(sun_top))
                .size(px(sun))
                .rounded_full()
                .bg(rgb(t::SUN)),
        )
        .children((0..t::HILLS.len()).map(hill));
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .py(px(t::PAGE_INSET))
        .pr(px(t::PAGE_INSET))
        .child(
            div()
                .size_full()
                .overflow_hidden()
                .rounded(px(t::PAGE_RADIUS))
                .bg(rgb(t::PAGE))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .h(px(head_height))
                        .px(px(head_padding))
                        .flex()
                        .items_center()
                        .gap(px(head_gap))
                        .child(mark(head_mark, t::MARK_RADIUS, t::MARKS[2]))
                        .child(
                            text("Fieldbook", t::PAGE_HEAD_TEXT, rgb(t::PAGE_INK))
                                .font_weight(FontWeight::SEMIBOLD),
                        ),
                )
                .child(scene)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(gap))
                        .py(px(padding_y))
                        .px(px(padding_x))
                        .child(
                            text("JOURNAL · 6 MIN", t::KICKER_TEXT, rgb(t::KICKER))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(
                            paragraph(
                                "The Lakeside loop, before the first snow",
                                t::TITLE_TEXT,
                                rgb(t::PAGE_INK),
                            )
                            .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(paragraph(
                            "The path leaves the car park by the boathouse and keeps the water on \
                             your left for the first hour.",
                            t::BODY_TEXT,
                            rgb(t::PAGE_QUIET),
                        )),
                ),
        )
}

/// Text at `size` that wraps.
fn paragraph(words: &'static str, size: [f32; 2], color: impl Into<Hsla>) -> gpui::Div {
    let [size, line] = size;
    div()
        .text_size(px(size))
        .line_height(px(line))
        .text_color(color)
        .child(words)
}

/// A caption over the picture, at its top left.
fn caption(chip: Chip, inks: Inks) -> gpui::Div {
    let [left, top, height, gap, start, end, icon] = t::CHIP;
    let words = chip.words();
    let start = match chip.icon() {
        Some(_) => start,
        None => end,
    };
    // With no words, the icon sits in the middle of a round chip.
    let end = if words.is_some() { end } else { start };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .h(px(height))
        .pl(px(start))
        .pr(px(end))
        .flex()
        .items_center()
        .gap(px(gap))
        .rounded(px(t::CHIP_RADIUS))
        .bg(inks.ink(t::BUTTON))
        .children(chip.icon().map(|kind| glyph(kind, icon, inks.ink(1.))))
        .children(words.map(|words| text(words, t::CHIP_TEXT, inks.ink(1.))))
}

/// The favorites' row ringed, with a halo.
fn ring(inks: Inks) -> gpui::Div {
    let [left, top, width, height, line, halo] = t::RING;
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .rounded(px(t::RING_RADIUS))
        .shadow(vec![
            BoxShadow {
                spread_radius: px(line),
                ..shadow(inks.ink(t::RING_ALPHA), 0., 0.)
            },
            BoxShadow {
                spread_radius: px(halo),
                ..shadow(inks.ink(t::HALO_ALPHA), 0., 0.)
            },
        ])
}

/// Another app's message with a link in it, which opens here, and the arrow that says so.
fn notice(inks: Inks) -> gpui::Div {
    let [left, top, height, start, end, gap, tile] = t::NOTICE;
    let [shadow_y, shadow_blur] = t::SHADOW;
    let message = div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .h(px(height))
        .pl(px(start))
        .pr(px(end))
        .flex()
        .items_center()
        .gap(px(gap))
        .rounded(px(t::NOTICE_RADIUS))
        .bg(with_alpha(inks.notice, t::NOTICE_ALPHA))
        .border_1()
        .border_color(inks.line())
        .shadow(vec![shadow(
            with_alpha(0, inks.shadow),
            shadow_y / 2.,
            shadow_blur / 2.,
        )])
        .child(
            div()
                .flex_none()
                .size(px(tile))
                .rounded(px(t::NOTICE_TILE_RADIUS))
                .bg(rgb(t::NOTICE_TILE))
                .flex()
                .items_center()
                .justify_center()
                .child(glyph(Glyph::Bubble, tile / 2., rgb(t::PAGE))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(text("Juno", t::CHIP_TEXT, inks.ink(1.)).font_weight(FontWeight::SEMIBOLD))
                .child(
                    div()
                        .flex()
                        .child(text(
                            "Saturday's route: ",
                            t::CHIP_TEXT,
                            inks.ink(inks.quiet),
                        ))
                        .child(
                            text(
                                "fieldbook.example/lakeside-loop",
                                t::CHIP_TEXT,
                                rgb(inks.link),
                            )
                            .underline(),
                        ),
                ),
        );
    let [arrow_left, arrow_top, arrow] = t::ARROW;
    div().absolute().inset_0().child(message).child(
        div()
            .absolute()
            .left(px(arrow_left))
            .top(px(arrow_top))
            .child(glyph(Glyph::Arrow, arrow, inks.ink(1.))),
    )
}

/// The command bar searching with the engine the step picks: what's typed, the search and a tab.
fn bar(inks: Inks) -> gpui::Div {
    let [
        left,
        top,
        width,
        padding,
        input,
        input_padding,
        result,
        result_mark,
    ] = t::BAR;
    let [search, tab] = t::BAR_MARKS;
    let [shadow_y, shadow_blur] = t::SHADOW;
    let found = |mark_color: u32, name: &'static str, kind: &'static str, shown: bool| {
        div()
            .h(px(result))
            .px(px(input_padding - 1.))
            .flex()
            .items_center()
            .gap(px(input_padding - 1.))
            .rounded(px(t::BAR_ROW_RADIUS))
            .when(shown, |row| row.bg(inks.ink(inks.shown)))
            .child(mark(result_mark, t::BAR_MARK_RADIUS, mark_color))
            .child(text(name, t::CHIP_TEXT, inks.ink(1.)).flex_1())
            .child(text(kind, t::ADDRESS_TEXT, inks.ink(inks.quiet)).flex_none())
    };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .p(px(padding))
        .flex()
        .flex_col()
        .gap(px(t::BAR_GAP))
        .rounded(px(t::BAR_RADIUS))
        .bg(rgb(inks.bar))
        .border_1()
        .border_color(inks.line())
        .shadow(vec![shadow(
            with_alpha(0, inks.shadow),
            shadow_y,
            shadow_blur,
        )])
        .child(
            div()
                .h(px(input))
                .px(px(input_padding))
                .flex()
                .items_center()
                .child(text("lakeside loop map", t::BAR_TEXT, inks.ink(1.)))
                .child(div().w(px(t::LINE)).h(px(t::BAR_TEXT[0])).bg(inks.ink(1.))),
        )
        .child(found(search, "lakeside loop map", "Search Lantern ↩", true))
        .child(found(
            tab,
            "The Lakeside loop · Fieldbook",
            "Open tab",
            false,
        ))
}

/// The Spaces on a phone, which sync brings there.
fn phone() -> gpui::Div {
    let [left, top, width, height, padding_y, padding_x, gap] = t::PHONE;
    let [island_width, island_height] = t::ISLAND;
    let [space_width, space_height, space_padding] = t::PHONE_SPACE;
    let [
        home,
        home_end,
        work,
        work_end,
        studio,
        studio_end,
        reading,
        reading_end,
    ] = t::PHONE_SPACES;
    let space = |name: &'static str, from: u32, to: u32| {
        div()
            .w(px(space_width))
            .h(px(space_height))
            .p(px(space_padding))
            .flex()
            .items_end()
            .rounded(px(t::PHONE_SPACE_RADIUS))
            .bg(linear_gradient(
                135.,
                linear_color_stop(rgb(from), 0.),
                linear_color_stop(rgb(to), 1.),
            ))
            .child(text(name, t::CHIP_TEXT, rgb(t::PHONE_TEXT)).font_weight(FontWeight::SEMIBOLD))
    };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .py(px(padding_y))
        .px(px(padding_x))
        .flex()
        .flex_col()
        .gap(px(gap))
        .rounded(px(t::PHONE_RADIUS))
        .bg(rgb(t::PHONE_FILL))
        .border_1()
        .border_color(with_alpha(t::RIM, t::RIM_ALPHA))
        .child(
            div().flex().justify_center().child(
                div()
                    .w(px(island_width))
                    .h(px(island_height))
                    .rounded(px(t::ISLAND_RADIUS))
                    .bg(rgb(t::ISLAND_FILL)),
            ),
        )
        .child(
            text("Spaces", t::PHONE_TITLE_TEXT, rgb(t::PHONE_TEXT))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(space_padding))
                .child(space("Home", home, home_end))
                .child(space("Work", work, work_end))
                .child(space("Studio", studio, studio_end))
                .child(space("Reading", reading, reading_end)),
        )
}

/// The picture's few drawn glyphs, as the drawing's paths have them on a 16-point square (the
/// arrow on a 40-point one).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Glyph {
    Ring,
    Check,
    Plus,
    Bubble,
    Arrow,
}

impl Glyph {
    /// The square it's drawn on, and the width of its stroke there, or none where it's filled.
    fn frame(self) -> (f32, Option<f32>) {
        match self {
            Glyph::Ring => (16., Some(1.8)),
            Glyph::Check => (16., Some(1.8)),
            Glyph::Plus => (10., Some(1.2)),
            Glyph::Bubble => (16., None),
            Glyph::Arrow => (40., Some(2.)),
        }
    }
}

/// `kind` at `size`, in `color`.
fn glyph(kind: Glyph, size: f32, color: impl Into<Hsla>) -> gpui::Div {
    let color = color.into();
    div().flex_none().size(px(size)).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let (square, stroke) = kind.frame();
                let scale = f32::from(bounds.size.width) / square;
                let at = |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
                let mut paths = Vec::new();
                let new = || match stroke {
                    Some(width) => PathBuilder::stroke(px(width * scale)),
                    None => PathBuilder::fill(),
                };
                match kind {
                    Glyph::Ring => {
                        // The track, then the part done: a quarter from the top.
                        let mut track = new();
                        circle(&mut track, at(8., 8.), 6.25 * scale);
                        paths.push((track, Some(0.2)));
                        let mut done = new();
                        done.move_to(at(8., 1.75));
                        let radius = px(6.25 * scale);
                        done.arc_to(point(radius, radius), px(0.), false, true, at(14.25, 8.));
                        paths.push((done, None));
                    }
                    Glyph::Check => {
                        let mut path = new();
                        path.move_to(at(3.5, 8.3));
                        path.line_to(at(6.5, 11.3));
                        path.line_to(at(12.5, 4.7));
                        paths.push((path, None));
                    }
                    Glyph::Plus => {
                        let mut path = new();
                        path.move_to(at(5., 1.5));
                        path.line_to(at(5., 8.5));
                        path.move_to(at(1.5, 5.));
                        path.line_to(at(8.5, 5.));
                        paths.push((path, None));
                    }
                    Glyph::Bubble => {
                        let mut path = new();
                        path.move_to(at(8., 2.5));
                        path.cubic_bezier_to(at(2., 7.5), at(4.7, 2.5), at(2., 4.7));
                        path.cubic_bezier_to(at(4., 11.2), at(2., 9.), at(2.8, 10.3));
                        path.line_to(at(3.5, 13.5));
                        path.line_to(at(6.1, 12.2));
                        path.cubic_bezier_to(at(8., 12.5), at(6.7, 12.4), at(7.3, 12.5));
                        path.cubic_bezier_to(at(14., 7.5), at(11.3, 12.5), at(14., 10.3));
                        path.cubic_bezier_to(at(8., 2.5), at(14., 4.7), at(11.3, 2.5));
                        path.close();
                        paths.push((path, None));
                    }
                    Glyph::Arrow => {
                        let mut path = new();
                        path.move_to(at(4., 6.));
                        path.cubic_bezier_to(at(32., 32.), at(20., 6.), at(30., 14.));
                        path.move_to(at(25.5, 27.));
                        path.line_to(at(32., 32.));
                        path.line_to(at(35.5, 24.5));
                        paths.push((path, None));
                    }
                }
                for (path, alpha) in paths {
                    let color = match alpha {
                        Some(alpha) => Hsla {
                            a: color.a * alpha,
                            ..color
                        },
                        None => color,
                    };
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full(),
    )
}

/// A whole circle round `centre`, as two halves.
fn circle(path: &mut PathBuilder, centre: Point<Pixels>, radius: f32) {
    let radius = px(radius);
    let top = centre - point(px(0.), radius);
    let bottom = centre + point(px(0.), radius);
    path.move_to(top);
    path.arc_to(point(radius, radius), px(0.), false, true, bottom);
    path.arc_to(point(radius, radius), px(0.), false, true, top);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A step's scene adds only what it changes: the welcome's is the window alone.
    #[test]
    fn the_welcome_is_the_window_alone() {
        assert_eq!(Scene::of(Step::Intro), Scene::default());
    }

    /// No chip claims a count about the user's own things: the import's has no words.
    #[test]
    fn the_import_chip_claims_no_count() {
        let chip = Scene::of(Step::Import).chip;
        assert_eq!(chip, Some(Chip::Found));
        assert_eq!(Chip::Found.words(), None);
    }
}
