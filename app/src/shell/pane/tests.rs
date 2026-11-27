//! The open tab's line as a pane lifted out of the sidebar's glass (#4398): which line takes it,
//! where it lies, the light its icon gives it, and what a line without one keeps. And the page
//! area, of which nothing moves for it.

use std::{rc::Rc, sync::Arc};

use gpui::{Entity, Hsla, Image, TestAppContext, VisualTestContext};

use super::{Drawn, PANES, stronger};
use crate::{
    palette::Tone,
    shell::{
        Command, Folders, PAGE_OUTLINES, Page, SHOWN_FILL, Shell, SidebarRight,
        browser::Split,
        recorder::Recorder,
        tile_theme::{MAIN_COLOURS_FOUND, main_colour},
    },
    theme::{self, Appearance},
    tokens::palette::SHOWN,
    tokens::selected::pane::{DARK_TOP, HOVERED, LIGHT_FILL},
    tokens::shell::{
        DIVIDER, FAVICON_SIZE, FAVICON_X, HAIRLINE, PAGE_OUTLINE, PAGE_OUTLINE_LIGHT, PLATE_HEIGHT,
        PLATE_INSET,
    },
};

/// Made-up pages' colours: a white one, a dark one and a blue one.
const WHITE: u32 = 0xffffff;
const DARK: u32 = 0x1d1d21;
const BLUE: u32 = 0x336699;

/// A drawn window whose Space has the favorite 20, the pinned tab 5 and today's tabs 10 and 11,
/// then 12 and 13 in a split view, showing 10. Tab 10's icon is red, 11's blue, 12's green and
/// 13's orange.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    cx.update(|_, cx| cx.set_reduce_motion(true));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "One".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_favorites(vec![(20, 1)], cx);
        let split = Split {
            tabs: vec![12, 13],
            ..Split::default()
        };
        shell.set_tabs(
            1,
            vec![5],
            vec![10, 11, 12, 13],
            vec![split],
            Folders::default(),
            cx,
        );
        for (tab, rgb) in [
            (10, [220, 40, 50]),
            (11, [40, 90, 220]),
            (12, [30, 170, 80]),
            (13, [240, 150, 20]),
        ] {
            shell.set_favicon(tab, Some(disc(rgb)), cx);
        }
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    (shell, cx)
}

/// A 32 px favicon of `paint`'s pixels, as the engine hands one over.
fn icon(paint: impl Fn(u32, u32) -> [u8; 4]) -> Arc<Image> {
    let image = image::RgbaImage::from_fn(32, 32, |x, y| image::Rgba(paint(x, y)));
    let mut png = Vec::new();
    (image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)).expect("a PNG");
    Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png))
}

/// A disc of one colour on nothing.
fn disc([r, g, b]: [u8; 3]) -> Arc<Image> {
    icon(move |x, y| {
        let inside = (x as f32 - 15.5).hypot(y as f32 - 15.5) <= 13.;
        if inside { [r, g, b, 255] } else { [0; 4] }
    })
}

/// The panes the next frames draw, each once, and the fill they lay the shown row's plain plate
/// in, if they lay one.
fn drawn(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Vec<Drawn>, Option<Hsla>) {
    PANES.take();
    SHOWN_FILL.set(None);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let mut panes = PANES.take();
    // A change can draw more than one frame before the window is still.
    panes.dedup();
    (panes, SHOWN_FILL.take())
}

/// The one pane the next frame draws.
fn the_pane(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Drawn {
    let (panes, plain) = drawn(shell, cx);
    assert_eq!(panes.len(), 1, "one pane, not {panes:?}");
    assert_eq!(plain, None, "a plain plate with the pane");
    panes[0]
}

fn select(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
}

fn show(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, color: Option<u32>) {
    shell.update(cx, |shell, cx| {
        shell.set_selected(tab, cx);
        shell.set_tab_background(tab, color, cx);
    });
}

fn hex(color: Hsla) -> u32 {
    theme::packed(color.to_rgb())
}

/// Whether `colour`, 0…1, is `rgb`, 0…255, within a step.
fn is([r, g, b]: [f32; 3], rgb: [u8; 3]) -> bool {
    [r, g, b]
        .iter()
        .zip(rgb)
        .all(|(c, want)| (c * 255. - want as f32).abs() <= 1.)
}

/// Where the icon's middle is from the pane's leading edge.
const ICON_MIDDLE: f32 = FAVICON_X - PLATE_INSET + FAVICON_SIZE / 2.;

#[gpui::test]
fn the_open_tabs_line_is_the_pane_in_its_plates_place_in_either_look(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    for appearance in [Appearance::Dark, Appearance::Light] {
        shell.update(cx, |shell, cx| {
            shell.run(Command::Appearance(appearance), "ui", cx)
        });
        cx.run_until_parked();
        let pane = the_pane(&shell, cx);
        let (width, page, tone) = shell.read_with(cx, |shell, _| {
            (shell.width(), shell.card_left(), shell.palette.tone)
        });
        assert_eq!(pane.look.tone, tone, "{appearance:?}");
        // Inset at both sides, as the plain shown plate: the sidebar shows between it and the page.
        assert_eq!(f32::from(pane.shape.left()), PLATE_INSET, "{appearance:?}");
        assert_eq!(f32::from(pane.shape.right()), width - PLATE_INSET);
        assert!(f32::from(pane.shape.right()) < page);
        assert_eq!(f32::from(pane.shape.size.height), PLATE_HEIGHT);
    }
}

#[gpui::test]
fn only_the_open_tabs_line_is_the_pane_as_tabs_switch(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let first = the_pane(&shell, cx);
    select(&shell, cx, 11);
    let second = the_pane(&shell, cx);
    // The next row down, and the same pane but for its light.
    assert!(second.shape.top() > first.shape.top());
    assert_eq!(second.shape.size, first.shape.size);
    assert!(is(second.look.glow.expect("its light").0, [40, 90, 220]));
}

#[gpui::test]
fn an_open_pinned_line_is_the_pane_as_a_tabs_is(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    select(&shell, cx, 5);
    let pane = the_pane(&shell, cx);
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert_eq!(f32::from(pane.shape.left()), PLATE_INSET);
    assert_eq!(f32::from(pane.shape.right()), width - PLATE_INSET);
}

/// An open split view is the open tab: its line takes the same pane, with no light, since it has
/// no one icon.
#[gpui::test]
fn an_open_split_views_line_is_the_pane_with_no_light(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    for tab in [12, 13] {
        select(&shell, cx, tab);
        let pane = the_pane(&shell, cx);
        assert_eq!(pane.look.glow, None, "tab {tab}");
        assert_eq!(f32::from(pane.shape.left()), PLATE_INSET, "tab {tab}");
    }
}

#[gpui::test]
fn the_light_is_the_icons_main_colour_behind_the_icon(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let (colour, middle) = the_pane(&shell, cx).look.glow.expect("its light");
    assert!(is(colour, [220, 40, 50]), "{colour:?}");
    assert_eq!(middle, ICON_MIDDLE);
    // An emoji in the favicon's place has no colour to give.
    shell.update(cx, |shell, cx| {
        let page = Page {
            icon: "🍋".into(),
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    assert_eq!(the_pane(&shell, cx).look.glow, None);
}

#[gpui::test]
fn an_icon_of_no_colour_gives_no_light(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    for rgb in [[0, 0, 0], [255, 255, 255], [128, 128, 128], [30, 32, 36]] {
        shell.update(cx, |shell, cx| shell.set_favicon(10, Some(disc(rgb)), cx));
        let pane = the_pane(&shell, cx);
        assert_eq!(pane.look.glow, None, "{rgb:?}");
    }
    // No favicon at all: the globe.
    shell.update(cx, |shell, cx| shell.set_favicon(10, None, cx));
    assert_eq!(the_pane(&shell, cx).look.glow, None);
}

/// The main colour is the most common hue, not the mean: a red icon with blue dots is red, where
/// its mean is a purple it has none of.
#[test]
fn the_main_colour_is_the_most_common_hue_not_the_mean() {
    let dotted = icon(
        |x, y| match (x % 4 == 0 && y % 4 == 0, x > 1 && x < 30 && y > 1) {
            (true, true) => [30, 60, 230, 255],
            (false, true) => [210, 30, 40, 255],
            _ => [0; 4],
        },
    );
    let colour = main_colour(&dotted).expect("a main colour");
    assert!(is(colour, [210, 30, 40]), "{colour:?}");
    // Colours scattered too thin across the wheel give none: twelve hues, each in the middle of
    // its own twelfth of it.
    let scattered = icon(|x, y| {
        let hue = ((x + y * 32) % 12) as f32 / 12. + 1. / 24.;
        let rgb = gpui::hsla(hue, 1., 0.5, 1.).to_rgb();
        let [r, g, b] = [rgb.r, rgb.g, rgb.b].map(|c| (c * 255.).round() as u8);
        [r, g, b, 255]
    });
    assert_eq!(main_colour(&scattered), None);
}

/// A tab switch decodes no favicon it has decoded before: each icon's main colour is found once.
#[gpui::test]
fn switching_tabs_decodes_each_icon_once(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    drawn(&shell, cx);
    select(&shell, cx, 11);
    drawn(&shell, cx);
    let found = MAIN_COLOURS_FOUND.get();
    for _ in 0..5 {
        for tab in [10, 11] {
            select(&shell, cx, tab);
            drawn(&shell, cx);
        }
    }
    assert_eq!(MAIN_COLOURS_FOUND.get(), found);
}

#[gpui::test]
fn under_the_pointer_the_pane_is_a_step_stronger(_: &mut TestAppContext) {
    for color in [DARK_TOP, LIGHT_FILL] {
        let rest = stronger(color, 0., 1.);
        let under = stronger(color, 1., 1.);
        assert_eq!(Hsla::from(gpui::rgba(color)), rest);
        assert!((under.a - rest.a - HOVERED).abs() < 1e-6);
        assert_eq!((under.h, under.s, under.l), (rest.h, rest.s, rest.l));
    }
}

#[gpui::test]
fn a_line_the_page_isnt_beside_keeps_the_plain_plate(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    assert_eq!(drawn(&shell, cx).0.len(), 1);
    // Developer Mode's line has its own look.
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "http://localhost:3000/".into(),
            developer_mode: true,
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    let shown = shell.read_with(cx, |shell, _| shell.palette.shown(0.));
    assert_eq!(drawn(&shell, cx), (vec![], Some(shown)));
    shell.update(cx, |shell, cx| shell.set_page(10, Page::default(), cx));
    assert_eq!(drawn(&shell, cx).0.len(), 1);
    // Hidden, the sidebar comes in over the page, which starts at the window's inset then: no
    // pane, whether it's in or brought out.
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "key", cx));
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |shell, _| shell.pane_shows()));
    assert_eq!(drawn(&shell, cx).0, vec![]);
}

#[gpui::test]
fn with_the_sidebar_on_the_right_the_pane_keeps_both_its_insets(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    cx.update(|_, cx| cx.set_global(SidebarRight(true)));
    let pane = the_pane(&shell, cx);
    let (left, width) = shell.read_with(cx, |shell, _| (shell.column_x(), shell.width()));
    assert_eq!(f32::from(pane.shape.left()), left + PLATE_INSET);
    assert_eq!(f32::from(pane.shape.right()), left + width - PLATE_INSET);
    // The icon is at the line's left on either side, and so is its light.
    assert_eq!(pane.look.glow.map(|glow| glow.1), Some(ICON_MIDDLE));
}

/// A page's colour no longer reaches its tab's line: the pane and the inks are the window's
/// whatever the page reports.
#[gpui::test]
fn a_pages_colour_doesnt_reach_its_tabs_line(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let plain = the_pane(&shell, cx);
    for color in [WHITE, DARK, BLUE] {
        show(&shell, cx, 10, Some(color));
        assert_eq!(the_pane(&shell, cx), plain, "{color:06x}");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.page_fill(10), None);
            assert_eq!(shell.line_palette(10), shell.palette);
        });
    }
}

/// The page keeps its place, its corners and its own outline whatever its tab's line wears or
/// its page reports: the line at the window's side and none at the docked sidebar's.
#[gpui::test]
fn nothing_of_the_page_area_moves_for_its_tabs_pane(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    for right in [false, true] {
        cx.update(|_, cx| cx.set_global(SidebarRight(right)));
        for color in [None, Some(WHITE), Some(DARK), Some(BLUE)] {
            show(&shell, cx, 10, color);
            PAGE_OUTLINES.take();
            drawn(&shell, cx);
            let lines = PAGE_OUTLINES.take();
            let (frame, corner, radius, tone) = shell.read_with(cx, |shell, _| {
                let (frame, corner) = shell.page_frame(viewport);
                (frame, corner, shell.card_radius(), shell.palette.tone)
            });
            assert_eq!(corner, radius, "{color:?}, right {right}");
            let line: Hsla = match tone {
                Tone::Dark => gpui::rgba(PAGE_OUTLINE).into(),
                Tone::Light => gpui::rgba(PAGE_OUTLINE_LIGHT).into(),
            };
            assert_eq!(
                lines,
                [(frame, radius, Some(right), line)],
                "{color:?}, right {right}"
            );
        }
    }
}

#[gpui::test]
fn an_open_favorites_tile_still_fills_with_its_pages_colour(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 20, Some(WHITE));
    // A tile is its own shape already: no pane.
    assert_eq!(drawn(&shell, cx).0, vec![]);
    let look = |shell: &Shell| shell.open_look(shell.page_fill(20), None);
    let (fill, width, _) = shell.read_with(cx, |shell, _| look(shell));
    assert_eq!((hex(fill), width), (WHITE, HAIRLINE));
    // A dark page's tile has a hairline of the ink a dark page's shape has, firmer than a line's,
    // which the idle tiles beside it would hide.
    show(&shell, cx, 20, Some(DARK));
    let (fill, width, ring) = shell.read_with(cx, |shell, _| look(shell));
    let on = shell.read_with(cx, |shell, _| shell.line_palette(20));
    assert_eq!((hex(fill), width, ring), (DARK, HAIRLINE, on.faint(SHOWN)));
    assert!(ring.a > on.faint(DIVIDER).a);
    show(&shell, cx, 20, None);
    shell.read_with(cx, |shell, _| assert_eq!(shell.page_fill(20), None));
}
