//! The open tab's line in its page's own colour, a shape of its own: which line takes it, where
//! it lies, and what a line without one keeps. And the page area, of which nothing moves for it.

use std::rc::Rc;

use gpui::{Corners, Entity, Hsla, TestAppContext, VisualTestContext, px, rgb};

use super::{OWN_SHAPE, OwnShape};
use crate::{
    palette::{TEXT, Tone, contrast, drawn as shown_as, over},
    shell::{
        Command, Folders, PAGE_OUTLINES, Page, SHOWN_FILL, Shell, SidebarRight, browser::Split,
        recorder::Recorder,
    },
    theme::{self, Appearance},
    tokens::palette::SHOWN,
    tokens::shell::{
        DIVIDER, HAIRLINE, PAGE_OUTLINE, PAGE_OUTLINE_LIGHT, PLATE_HEIGHT, PLATE_INSET,
        PLATE_RADIUS,
    },
};

/// Made-up pages' colours: a white one, a dark one and a blue one.
const PAGES: [u32; 3] = [0xffffff, 0x1d1d21, 0x336699];
const WHITE: u32 = PAGES[0];
const DARK: u32 = PAGES[1];
const BLUE: u32 = PAGES[2];

/// A drawn window whose Space has the favorite 20, the pinned tab 5 and today's tabs 10 and 11,
/// then 12 and 13 in a split view, showing 10.
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
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    (shell, cx)
}

/// The shape of its own the next frame paints, if it paints one, and the fill it lays the shown
/// row's plain plate in, if it lays one.
fn drawn(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Option<OwnShape>, Option<Hsla>) {
    OWN_SHAPE.set(None);
    SHOWN_FILL.set(None);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    (OWN_SHAPE.take(), SHOWN_FILL.take())
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

#[gpui::test]
fn the_open_tabs_line_is_a_shape_of_its_own_in_its_pages_colour(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 10, Some(BLUE));
    let (own, plain) = drawn(&shell, cx);
    let own = own.expect("the open tab's line has no shape of its own");
    assert_eq!(hex(own.fill), BLUE);
    assert_eq!(own.fill.a, 1.);
    // The page's colour is the line's whole fill: no plate of the window's lies with it.
    assert_eq!(plain, None);
    let (width, page) = shell.read_with(cx, |shell, _| (shell.width(), shell.card_left()));
    // Inset at both sides, as the plain shown plate: the sidebar shows between it and the page.
    assert_eq!(f32::from(own.shape.left()), PLATE_INSET);
    assert_eq!(f32::from(own.shape.right()), width - PLATE_INSET);
    assert!(f32::from(own.shape.right()) < page);
    assert_eq!(f32::from(own.shape.size.height), PLATE_HEIGHT);
    assert_eq!(own.corners, Corners::all(px(PLATE_RADIUS)));
}

#[gpui::test]
fn a_line_whose_page_reported_no_colour_keeps_the_plain_plate(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let shown = shell.read_with(cx, |shell, _| shell.palette.shown(0.));
    assert_eq!(drawn(&shell, cx), (None, Some(shown)));
    show(&shell, cx, 10, Some(WHITE));
    assert!(drawn(&shell, cx).0.is_some());
    // A page with no single colour any more: the plate is back.
    show(&shell, cx, 10, None);
    assert_eq!(drawn(&shell, cx), (None, Some(shown)));
}

#[gpui::test]
fn a_pages_colour_fills_its_own_tabs_line_and_no_other(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_tab_background(11, Some(BLUE), cx);
        shell.set_tab_background(5, Some(DARK), cx);
    });
    // Neither is the tab shown, whose own page has reported none.
    let shown = shell.read_with(cx, |shell, _| shell.palette.shown(0.));
    assert_eq!(drawn(&shell, cx), (None, Some(shown)));
    shell.read_with(cx, |shell, _| {
        for tab in [5, 10, 11, 12, 20] {
            assert_eq!(shell.page_fill(tab), None, "tab {tab}");
            assert_eq!(shell.line_palette(tab), shell.palette, "tab {tab}");
        }
    });
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    let (own, _) = drawn(&shell, cx);
    assert_eq!(own.map(|own| hex(own.fill)), Some(BLUE));
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.page_fill(5), None);
        assert_eq!(shell.page_fill(10), None);
    });
}

#[gpui::test]
fn an_open_pinned_line_is_a_shape_of_its_own_as_a_tabs_is(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 5, Some(WHITE));
    let (own, plain) = drawn(&shell, cx);
    let own = own.expect("the open pinned line has no shape of its own");
    assert_eq!((hex(own.fill), plain), (WHITE, None));
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert_eq!(f32::from(own.shape.left()), PLATE_INSET);
    assert_eq!(f32::from(own.shape.right()), width - PLATE_INSET);
    assert_eq!(f32::from(own.shape.size.height), PLATE_HEIGHT);
    assert_eq!(own.corners, Corners::all(px(PLATE_RADIUS)));
}

#[gpui::test]
fn a_split_views_line_keeps_the_plain_plate_whatever_its_panes_colours(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_tab_background(12, Some(BLUE), cx);
        shell.set_tab_background(13, Some(DARK), cx);
    });
    let shown = shell.read_with(cx, |shell, _| shell.palette.shown(0.));
    for pane in [12, 13] {
        shell.update(cx, |shell, cx| shell.set_selected(pane, cx));
        assert_eq!(drawn(&shell, cx), (None, Some(shown)), "pane {pane} shown");
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.page_fill(pane), None);
            assert_eq!(shell.line_palette(pane), shell.palette);
        });
    }
}

#[gpui::test]
fn a_dark_page_has_a_hairline_round_its_line_and_a_light_one_none(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 10, Some(WHITE));
    let (own, _) = drawn(&shell, cx);
    let own = own.expect("the open tab's line has no shape of its own");
    assert_eq!(own.outline, None);
    show(&shell, cx, 10, Some(DARK));
    let (own, _) = drawn(&shell, cx);
    let on = shell.read_with(cx, |shell, _| shell.line_palette(10));
    assert_eq!(on.tone, Tone::Dark);
    assert_eq!(own.and_then(|own| own.outline), Some(on.faint(DIVIDER)));
}

#[gpui::test]
fn the_lines_inks_are_the_ones_that_read_on_its_pages_colour(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    for (color, tone) in [(WHITE, Tone::Light), (DARK, Tone::Dark)] {
        show(&shell, cx, 10, Some(color));
        shell.read_with(cx, |shell, _| {
            let on = shell.line_palette(10);
            assert_eq!(on, shell.palette.on_fill(rgb(color)));
            assert_eq!(on.tone, tone);
            // The rows beside it keep the window's.
            assert_eq!(shell.line_palette(11), shell.palette);
        });
    }
}

/// What the engine reports for most pages in a dark window: white for a page with no background
/// of its own, and black for its own error page.
#[gpui::test]
fn a_white_page_and_a_black_one_in_a_dark_window_both_keep_a_title_that_reads(
    cx: &mut TestAppContext,
) {
    let (shell, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::Appearance(Appearance::Dark), "ui", cx)
    });
    cx.run_until_parked();
    let dark = shell.read_with(cx, |shell, _| shell.palette.tone);
    assert_eq!(dark, Tone::Dark, "the made-up window isn't a dark one");
    for (color, tone) in [(0xffffff, Tone::Light), (0x000000, Tone::Dark)] {
        show(&shell, cx, 10, Some(color));
        let (own, plain) = drawn(&shell, cx);
        let own = own.expect("the open tab's line has no shape of its own");
        assert_eq!((hex(own.fill), plain), (color, None), "{color:06x}");
        // A black page in a dark window is told from it by its hairline alone.
        assert_eq!(own.outline.is_some(), tone == Tone::Dark, "{color:06x}");
        let on = shell.read_with(cx, |shell, _| shell.line_palette(10));
        assert_eq!(on.tone, tone, "{color:06x}");
        let (fill, title) = (rgb(color), on.text(0.));
        let reads = contrast(
            shown_as(over(fill, title.to_rgb(), title.a)),
            shown_as(fill),
        );
        assert!(reads >= TEXT, "{color:06x}: the title reads at {reads}");
    }
}

#[gpui::test]
fn with_the_sidebar_on_the_right_the_line_keeps_both_its_insets(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    // The side is the setting's, which the shell reads as it draws.
    cx.update(|_, cx| cx.set_global(SidebarRight(true)));
    show(&shell, cx, 10, Some(BLUE));
    let (own, _) = drawn(&shell, cx);
    let own = own.expect("the open tab's line has no shape of its own");
    let (left, width) = shell.read_with(cx, |shell, _| (shell.column_x(), shell.width()));
    assert_eq!(f32::from(own.shape.left()), left + PLATE_INSET);
    assert_eq!(f32::from(own.shape.right()), left + width - PLATE_INSET);
    assert_eq!(own.corners, Corners::all(px(PLATE_RADIUS)));
}

/// The page keeps its place, its corners and its own outline whatever its tab's line wears: the
/// line at the window's side and none at the docked sidebar's for a light page or one with no
/// colour, and a dark page's hairline round all of it.
#[gpui::test]
fn nothing_of_the_page_area_moves_for_its_tabs_line(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    let viewport = cx.update(|window, _| window.viewport_size());
    for right in [false, true] {
        cx.update(|_, cx| cx.set_global(SidebarRight(right)));
        show(&shell, cx, 10, None);
        drawn(&shell, cx);
        let place = |shell: &Shell| (shell.page_frame(viewport), shell.card_left());
        let plain = shell.read_with(cx, |shell, _| place(shell));
        for color in [None, Some(WHITE), Some(DARK), Some(BLUE)] {
            show(&shell, cx, 10, color);
            PAGE_OUTLINES.take();
            drawn(&shell, cx);
            let lines = PAGE_OUTLINES.take();
            let (now, radius, tone, on) = shell.read_with(cx, |shell, _| {
                let on = shell.line_palette(10);
                (place(shell), shell.card_radius(), shell.palette.tone, on)
            });
            assert_eq!(now, plain, "{color:?}, right {right}");
            let ((frame, corner), _) = now;
            assert_eq!(corner, radius, "{color:?}, right {right}");
            let lit = color.is_some() && on.tone == Tone::Dark;
            let line = match (lit, tone) {
                (true, _) => on.faint(DIVIDER),
                (false, Tone::Dark) => gpui::rgba(PAGE_OUTLINE).into(),
                (false, Tone::Light) => gpui::rgba(PAGE_OUTLINE_LIGHT).into(),
            };
            let open = (!lit).then_some(right);
            assert_eq!(
                lines,
                [(frame, radius, open, line)],
                "{color:?}, right {right}"
            );
        }
    }
}

#[gpui::test]
fn a_line_the_page_isnt_beside_keeps_the_plain_plate(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 10, Some(BLUE));
    assert!(
        shell
            .read_with(cx, |shell, _| shell.page_fill(10))
            .is_some()
    );
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
    assert_eq!(drawn(&shell, cx), (None, Some(shown)));
    shell.update(cx, |shell, cx| shell.set_page(10, Page::default(), cx));
    assert!(drawn(&shell, cx).0.is_some());
    // Hidden, the sidebar comes in over the page, which starts at the window's inset then.
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "key", cx));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |shell, _| shell.page_fill(10)), None);
}

#[gpui::test]
fn an_open_favorites_tile_fills_with_its_pages_colour_where_it_stands(cx: &mut TestAppContext) {
    let (shell, cx) = window(cx);
    show(&shell, cx, 20, Some(WHITE));
    // A tile is its own shape already: no line's.
    assert_eq!(drawn(&shell, cx).0, None);
    let look = |shell: &Shell| shell.open_look(shell.page_fill(20), None);
    let (fill, width, _) = shell.read_with(cx, |shell, _| look(shell));
    assert_eq!((hex(fill), width), (WHITE, HAIRLINE));
    // A dark page's tile has a hairline of the ink a dark page's shape has, firmer than the
    // line's, which the idle tiles beside it would hide.
    show(&shell, cx, 20, Some(DARK));
    let (fill, width, ring) = shell.read_with(cx, |shell, _| look(shell));
    let on = shell.read_with(cx, |shell, _| shell.line_palette(20));
    assert_eq!((hex(fill), width, ring), (DARK, HAIRLINE, on.faint(SHOWN)));
    assert!(ring.a > on.faint(DIVIDER).a);
    // Until its page reports one, the tile keeps the look its icon gives it.
    show(&shell, cx, 20, None);
    shell.read_with(cx, |shell, _| assert_eq!(shell.page_fill(20), None));
}
