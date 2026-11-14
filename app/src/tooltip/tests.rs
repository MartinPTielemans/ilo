//! When the tooltip shows and goes, on a host that records what it asks of its window, and where
//! it and its chips go.

use std::cell::RefCell;

use gpui::{TestAppContext, WindowTextSystem, point, size};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, over: Over, left: f32, top: f32) {
        let over = match over {
            Over::Window(window) => window.to_string(),
            Over::Overlay(overlay) => format!("overlay {overlay}"),
        };
        self.0
            .borrow_mut()
            .push(format!("show {left} {top} over {over}"));
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

fn open_tooltip(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx))
        .expect("no tooltip window");
    host
}

fn tip(text: &str) -> Tip {
    Tip::named(text.to_string().into())
}

/// The pointer comes onto what says `text` in the first browser window.
fn hover_on(cx: &mut TestAppContext, text: &str, anchor: Anchor) {
    hover_in(cx, 1, text, anchor);
}

fn hover_in(cx: &mut TestAppContext, window: u64, text: &str, anchor: Anchor) {
    cx.update(|cx| hover(window, tip(text), anchor, cx));
    cx.run_until_parked();
}

fn measured(cx: &mut TestAppContext, tip: &Tip) -> Layout {
    let system = cx.update(|cx| WindowTextSystem::new(cx.text_system().clone()));
    measure(tip, &system)
}

fn width(cx: &mut TestAppContext, text: &str) -> f32 {
    measured(cx, &tip(text)).width
}

/// Where a tooltip saying `text` goes for `anchor` in the first browser window, as the host is
/// told.
fn placed(cx: &mut TestAppContext, text: &str, anchor: Anchor) -> String {
    placed_in(cx, 1, text, anchor)
}

fn placed_in(cx: &mut TestAppContext, window: u64, text: &str, anchor: Anchor) -> String {
    let width = width(cx, text);
    let (left, top) = place(anchor, (width, HEIGHT));
    format!("show {left} {top} over {window}")
}

fn after(cx: &mut TestAppContext, host: &Recorder, time: Duration) -> Vec<String> {
    cx.executor().advance_clock(time);
    cx.run_until_parked();
    host.0.take()
}

/// Whether the delay's timer is running.
fn waiting(cx: &mut TestAppContext) -> bool {
    let window = cx.update(|cx| cx.global::<TooltipWindow>().0);
    cx.update(|cx| {
        window
            .update(cx, |tooltip, _, _| tooltip.wait.is_some())
            .unwrap()
    })
}

fn window() -> Size<Pixels> {
    size(px(1000.), px(700.))
}

/// A frame at (`left`, `top`) in the window, `width` by `height`.
fn frame(left: f32, top: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
}

/// Two favorites' tiles side by side, and a folder's plate under them.
fn tile_a() -> Anchor {
    Anchor::under(frame(108., 100., 80., 41.), window())
}

fn tile_b() -> Anchor {
    Anchor::under(frame(194., 100., 80., 41.), window())
}

fn folder_row() -> Anchor {
    Anchor::titled(frame(8., 300., 220., 36.), 35., window())
}

/// A tooltip 100 wide and `HEIGHT` tall.
const SIZE: (f32, f32) = (100., HEIGHT);

#[test]
fn a_name_is_the_text_and_no_name_is_none() {
    assert_eq!(text_for(" Docs ").unwrap(), "Docs");
    assert_eq!(text_for("Untitled").unwrap(), "Untitled");
    assert!(text_for("").is_none());
    assert!(text_for("  ").is_none());
}

#[gpui::test]
fn it_shows_after_the_delay_under_its_tile(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    assert!(waiting(cx));
    assert!(after(cx, &host, DELAY - Duration::from_millis(1)).is_empty());
    let shown = after(cx, &host, Duration::from_millis(1));
    assert_eq!(shown, [placed(cx, "Docs", tile_a())]);
}

/// Arc's shows 1.6 s after the pointer's last move, so a pointer still moving over a tile brings
/// none, and one that stops brings it the delay later.
#[gpui::test]
fn it_shows_only_once_the_pointer_rests(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    for _ in 0..4 {
        assert!(after(cx, &host, DELAY / 2).is_empty());
        cx.update(moved);
    }
    assert!(waiting(cx));
    assert!(after(cx, &host, DELAY - Duration::from_millis(1)).is_empty());
    assert_eq!(
        after(cx, &host, Duration::from_millis(1)),
        [placed(cx, "Docs", tile_a())]
    );
    // Once it shows, a move keeps it where it is, and starts no timer.
    cx.update(moved);
    assert!(!waiting(cx));
    assert!(after(cx, &host, DELAY * 2).is_empty());
}

#[gpui::test]
fn a_move_with_nothing_waiting_shows_nothing(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    cx.update(moved);
    assert!(!waiting(cx));
    assert!(after(cx, &host, DELAY * 2).is_empty());
}

#[gpui::test]
fn leaving_before_the_delay_cancels_it(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    after(cx, &host, DELAY / 2);
    cx.update(dismiss);
    // Nothing shows, and nothing is hidden that never showed.
    assert!(after(cx, &host, DELAY * 4).is_empty());
    assert!(!waiting(cx));
}

#[gpui::test]
fn it_goes_when_dismissed_and_leaves_no_timer(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    assert_eq!(after(cx, &host, DELAY).len(), 1);
    cx.update(dismiss);
    assert_eq!(host.0.take(), ["hide"]);
    // A second dismissal has nothing to hide.
    cx.update(dismiss);
    assert!(host.0.take().is_empty());
    assert!(!waiting(cx));
}

#[gpui::test]
fn the_next_tile_shows_at_once(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    after(cx, &host, DELAY);
    // The pointer leaves the first tile and crosses the gap to the next.
    cx.update(dismiss);
    assert_eq!(after(cx, &host, GRACE / 2), ["hide"]);
    hover_on(cx, "Mail", tile_b());
    assert_eq!(host.0.take(), [placed(cx, "Mail", tile_b())]);
}

/// Three tiles across a 228 pt sidebar in a wide window, the owner's: their tooltips of a long, a
/// medium and a short title are each placed from their own width, never the last one's, going
/// along them and back, each one's square corner under the tile's middle.
#[gpui::test]
fn each_tooltip_is_placed_from_its_own_width_whichever_was_before_it(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    let tile = |middle: f32| {
        Anchor::under(
            frame(middle - 33., 90., 66., 41.),
            size(px(1400.), px(900.)),
        )
    };
    let titles = [
        ("Creating A World That Doesn't Exist", tile(41.)),
        ("Elon Musk on X: a post / X", tile(114.)),
        ("Proton Mail: Sign-in", tile(187.)),
    ];
    let widths: Vec<f32> = titles.iter().map(|&(text, _)| width(cx, text)).collect();
    assert!(widths[0] > widths[1] && widths[1] > widths[2], "{widths:?}");
    for index in [0, 1, 2, 1, 0] {
        let (text, anchor) = titles[index];
        // The pointer leaves the last tile and crosses to this one.
        cx.update(dismiss);
        hover_on(cx, text, anchor);
        let mut shown = host.0.take();
        shown.extend(after(cx, &host, DELAY));
        let want = anchor.x + BUTTON_X;
        let over = format!("show {want} {} over 1", 110.5 + BUTTON_Y);
        assert_eq!(shown.last().unwrap(), &over, "{text}");
    }
}

/// With two browser windows, the tooltip shows over the one the pointer is in, whichever is in
/// front: its tile's tooltip waits there, and one showing goes over to the next window's at once
/// (#1432).
#[gpui::test]
fn it_shows_over_the_window_the_pointer_is_in(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_in(cx, 2, "Docs", tile_a());
    assert_eq!(
        after(cx, &host, DELAY),
        [placed_in(cx, 2, "Docs", tile_a())]
    );
    cx.update(dismiss);
    host.0.take();
    hover_in(cx, 1, "Mail", tile_b());
    assert_eq!(host.0.take(), [placed_in(cx, 1, "Mail", tile_b())]);
    hover_in(cx, 2, "Docs", tile_a());
    assert_eq!(host.0.take(), [placed_in(cx, 2, "Docs", tile_a())]);
    // One waiting for its delay in one window shows in the window the pointer went to.
    cx.update(dismiss);
    after(cx, &host, GRACE * 2);
    hover_in(cx, 1, "Mail", tile_b());
    after(cx, &host, DELAY / 2);
    hover_in(cx, 2, "Docs", tile_a());
    assert_eq!(
        after(cx, &host, DELAY),
        [placed_in(cx, 2, "Docs", tile_a())]
    );
}

#[gpui::test]
fn the_delay_starts_again_after_it_went(cx: &mut TestAppContext) {
    let host = open_tooltip(cx);
    hover_on(cx, "Docs", tile_a());
    after(cx, &host, DELAY);
    cx.update(dismiss);
    assert_eq!(after(cx, &host, GRACE), ["hide"]);
    hover_on(cx, "Mail", tile_b());
    assert!(after(cx, &host, DELAY / 2).is_empty());
    assert_eq!(after(cx, &host, DELAY / 2).len(), 1);
}

#[test]
fn a_tooltips_square_corner_is_off_the_middle_of_its_tile() {
    // The tile spans 108 to 188 and 100 to 141, so its middle is (148, 120.5) whichever point of
    // it the pointer is on, and the tooltip goes right from 8 right of it and 7 under it, for a
    // name of any width.
    let (left, top) = place(tile_a(), SIZE);
    assert_eq!((left, top), (148. + BUTTON_X, 120.5 + BUTTON_Y));
    assert_eq!(place(tile_a(), (60., HEIGHT)).0, 156.);
    assert_eq!(place(tile_a(), (300., HEIGHT)).0, 156.);
    assert_eq!(near(tile_a(), 300., HEIGHT), Near::TopLeft);
}

#[test]
fn a_folders_tooltip_has_its_text_over_the_title() {
    // The title starts 35 in from the plate's left, and the text 11 in from the tooltip's.
    let (left, top) = place(folder_row(), SIZE);
    assert_eq!(left + TEXT_X, 8. + 35.);
    assert_eq!(top, 336. + GAP);
}

#[test]
fn a_tooltip_near_the_right_edge_mirrors_and_goes_left_from_its_tile() {
    // A tile at the right edge, 965 across: 100 more from 973 would cross 992, so the square
    // corner is the top right, 8 left of the tile's middle, and the tooltip ends there.
    let right = Anchor::under(frame(930., 100., 70., 41.), window());
    let (left, top) = place(right, SIZE);
    assert_eq!((left + 100., top), (965. - BUTTON_X, 120.5 + BUTTON_Y));
    assert_eq!(near(right, 100., HEIGHT), Near::TopRight);
    // Right up to the inset it still goes right, and past it by a point it mirrors.
    let fits = Anchor::under(frame(844., 100., 80., 41.), window());
    assert_eq!(place(fits, SIZE).0, 892.);
    assert_eq!(near(fits, 100., HEIGHT), Near::TopLeft);
    let crosses = Anchor::under(frame(845., 100., 80., 41.), window());
    assert_eq!(place(crosses, SIZE).0 + 100., 885. - BUTTON_X);
    assert_eq!(near(crosses, 100., HEIGHT), Near::TopRight);
}

#[test]
fn a_tooltip_stays_inside_the_window_across() {
    // A tile at the left edge keeps its square corner off its middle, 20 + 8 in.
    let left = Anchor::under(frame(0., 100., 40., 41.), window());
    assert_eq!(place(left, SIZE).0, 28.);
    // And one with its middle nearer the edge than the inset has the corner at the inset.
    let edge = Anchor::under(frame(-10., 100., 10., 41.), window());
    assert_eq!(place(edge, SIZE).0, EDGE_INSET);
    // A folder's text near the right edge.
    let folder = Anchor::titled(frame(900., 300., 100., 36.), 35., window());
    assert_eq!(place(folder, SIZE).0, 1000. - 100. - EDGE_INSET);
    // Room for the tooltip but not the insets, in either direction: the left one is kept.
    let narrow = Anchor::under(frame(8., 100., 80., 41.), size(px(110.), px(700.)));
    assert_eq!(place(narrow, SIZE).0, EDGE_INSET);
}

/// The sidebar's three tiles, each 66.7 wide on the 8 pt gaps, and the tooltips of titles as long
/// as the owner's, in a wide window.
#[test]
fn a_tooltip_over_a_tile_goes_right_from_its_middle_whatever_its_width() {
    let tile = |left: f32| Anchor::under(frame(left, 90., 66.7, 41.), size(px(1400.), px(900.)));
    // The last tile, a short title: its corner is off the tile's middle, 8 right and 7 under it.
    // The tile is 153.3 to 220, so a tooltip's middle is nowhere near the sidebar's.
    let (left, top) = place(tile(153.3), (146., HEIGHT));
    assert!((left - (153.3 + 33.35 + BUTTON_X)).abs() < 0.01);
    assert_eq!(top, 110.5 + BUTTON_Y);
    // A long title over the first tile starts off the tile's middle and goes over the page.
    let (left, _) = place(tile(8.), (306., HEIGHT));
    assert!((left - (8. + 33.35 + BUTTON_X)).abs() < 0.01);
    assert!(left + 306. > 228.);
    // A tooltip as wide as its window is the same in the middle of a wide window.
    let (left, _) = place(tile(700.), (WINDOW_WIDTH, HEIGHT));
    assert!((left - (700. + 33.35 + BUTTON_X)).abs() < 0.01);
}

#[test]
fn a_tooltip_goes_above_when_there_is_no_room_below() {
    // 41 tall at the window's bottom, its middle at 670.5: the tooltip's bottom goes 7 over it.
    let bottom = Anchor::under(frame(8., 650., 80., 41.), window());
    let (_, top) = place(bottom, SIZE);
    assert_eq!(top, 670.5 - BUTTON_Y - HEIGHT);
    assert_eq!(near(bottom, 100., HEIGHT), Near::BottomLeft);
    // It fits below with exactly no room to spare.
    let snug = Anchor::under(
        frame(8., 700. - HEIGHT - BUTTON_Y - 20., 80., 40.),
        window(),
    );
    assert_eq!(place(snug, SIZE).1, 700. - HEIGHT);
    // A folder's row goes above with the gap.
    let low = Anchor::titled(frame(8., 650., 220., 36.), 35., window());
    assert_eq!(place(low, SIZE).1, 650. - GAP - HEIGHT);
    // It never goes past the window's top.
    let short = Anchor::under(frame(8., 10., 80., 41.), size(px(1000.), px(60.)));
    assert_eq!(place(short, SIZE).1, 0.);
}

#[test]
fn a_frame_gives_the_anchor_its_middle_and_edges() {
    let anchor = Anchor::under(frame(108., 100., 80., 41.), window());
    assert_eq!((anchor.x, anchor.top, anchor.bottom), (148., 100., 141.));
    assert_eq!(anchor.window, (1000., 700.));
}

#[gpui::test]
fn a_long_name_is_as_wide_as_its_window(cx: &mut TestAppContext) {
    assert_eq!(width(cx, &"x".repeat(400)), WINDOW_WIDTH);
    assert!(width(cx, "Docs") < 100.);
}

/// The header's back button, and the footer's Library button in a 700 tall window.
fn back_button() -> Bounds<Pixels> {
    frame(122., 7.5, 32., 32.)
}

fn library_button() -> Bounds<Pixels> {
    frame(8., 700. - 41., 32., 32.)
}

#[test]
fn a_footer_buttons_tooltip_is_beside_it_and_centred_on_it() {
    // Arc's: 3.5 past the plate's right edge, its middle at the plate's.
    let (left, top) = place(Anchor::beside(library_button(), window(), false), SIZE);
    assert_eq!(left, 8. + 32. + BESIDE);
    assert_eq!(top + HEIGHT / 2., 700. - 41. + 16.);
}

/// From a sidebar on the right, a footer button's tooltip is beside it toward the page, the same
/// gap off the plate's left edge, its square corner the top right, at the button.
#[test]
fn from_a_sidebar_on_the_right_a_footer_tooltip_is_to_its_left() {
    let button = frame(1000. - 40., 700. - 41., 32., 32.);
    let anchor = Anchor::beside(button, window(), true);
    let (left, top) = place(anchor, SIZE);
    assert_eq!(left + SIZE.0, 1000. - 40. - BESIDE);
    assert_eq!(top + HEIGHT / 2., 700. - 41. + 16.);
    assert_eq!(near(anchor, SIZE.0, HEIGHT), Near::TopRight);
    // At the window's left edge it keeps EDGE_INSET from it.
    let far = Anchor::beside(frame(20., 300., 32., 32.), window(), true);
    assert_eq!(place(far, SIZE).0, EDGE_INSET);
}

#[test]
fn a_tooltip_beside_stays_inside_the_window() {
    // A button whose middle is nearer the window's bottom than half the tooltip's height.
    let low = Anchor::beside(frame(8., 690., 32., 10.), window(), false);
    assert_eq!(place(low, SIZE).1, 700. - HEIGHT);
    // One at the right edge: the tooltip ends EDGE_INSET from the window's edge.
    let right = Anchor::beside(frame(950., 300., 32., 32.), window(), false);
    assert_eq!(place(right, SIZE).0, 1000. - 100. - EDGE_INSET);
}

#[test]
fn a_header_buttons_tooltip_hangs_off_its_glyphs_bottom_right() {
    // Arc's: the fill's top left at the glyph's centre + (8, 7), the corner of a 16 pt box around
    // it, so it covers the plate's lower right.
    let (left, top) = place(Anchor::under(back_button(), window()), SIZE);
    assert_eq!((left, top), (122. + 16. + 8., 7.5 + 16. + 7.));
}

#[test]
fn a_name_alone_has_its_text_11_in_and_10_before_the_end() {
    let layout = layout(60., &[]);
    let alone = Layout {
        width: 11. + 60. + 10.,
        text: 60.,
        truncated: false,
        chips: vec![],
    };
    assert_eq!(layout, alone);
}

#[test]
fn chips_follow_the_text_on_arcs_pitch() {
    // View Library ⇧⌘L: 8 after the text, 18.5 wide on a pitch of 22, and 10 after the last.
    let layout = layout(70., &[CHIP, CHIP, CHIP]);
    let first = 11. + 70. + 8.;
    assert_eq!(
        layout.chips,
        [(first, 18.5), (first + 22., 18.5), (first + 44., 18.5)]
    );
    assert_eq!(layout.width, first + 44. + 18.5 + 10.);
    assert!(!layout.truncated);
}

#[test]
fn a_wider_chip_moves_the_next() {
    // A key named in words, as Space is, between two glyphs.
    let layout = layout(50., &[CHIP, 40., CHIP]);
    let first = 11. + 50. + 8.;
    assert_eq!(
        layout.chips,
        [
            (first, 18.5),
            (first + 22., 40.),
            (first + 22. + 43.5, 18.5)
        ]
    );
    assert_eq!(layout.width, first + 22. + 43.5 + 18.5 + 10.);
}

#[test]
fn a_long_name_truncates_and_its_keys_stay_whole() {
    let layout = layout(1000., &[CHIP, CHIP]);
    assert_eq!(layout.width, WINDOW_WIDTH);
    assert!(layout.truncated);
    // The chips end 10 from the window's end, and the text stops 8 before them.
    let &(last, width) = layout.chips.last().unwrap();
    assert_eq!(last + width + 10., WINDOW_WIDTH);
    assert_eq!(11. + layout.text + 8., layout.chips[0].0);
}

#[gpui::test]
fn a_keys_chip_is_square_unless_its_name_is_wider(cx: &mut TestAppContext) {
    let keys = |keys: &[&str]| Tip {
        text: "View Library".into(),
        keys: keys.iter().map(|key| key.to_string().into()).collect(),
    };
    let glyphs = measured(cx, &keys(&["⇧", "⌘", "L"]));
    assert!(glyphs.chips.iter().all(|&(_, width)| width == CHIP));
    let words = measured(cx, &keys(&["⌘", "Space"]));
    assert_eq!(words.chips[0].1, CHIP);
    assert!(words.chips[1].1 > CHIP);
    // The same text with keys is wider by its chips, 8 before them and none of its end changes.
    let name = measured(cx, &tip("View Library"));
    assert_eq!(glyphs.width, name.width + 8. + 3. * CHIP + 2. * CHIP_GAP);
}

#[test]
fn the_log_names_the_keys_as_a_menu_does() {
    let tip = Tip {
        text: "View Library".into(),
        keys: vec!["⇧".into(), "⌘".into(), "L".into()],
    };
    assert_eq!(label(&tip), "View Library ⇧⌘L");
    assert_eq!(label(&Tip::named("Docs".into())), "Docs");
}

#[test]
fn the_capsule_is_round_but_at_what_it_names() {
    // Beside a button, under a tile and under a row, the corner at what it names is the top left.
    let beside = Anchor::beside(library_button(), window(), false);
    assert_eq!(near(beside, 100., HEIGHT), Near::TopLeft);
    assert_eq!(near(tile_a(), 100., HEIGHT), Near::TopLeft);
    assert_eq!(near(folder_row(), 100., HEIGHT), Near::TopLeft);
    // Above it for want of room below, that corner is the bottom left, or the bottom right when
    // the tooltip goes left of a tile near the right edge.
    let bottom = Anchor::under(frame(8., 650., 80., 41.), window());
    assert_eq!(near(bottom, 100., HEIGHT), Near::BottomLeft);
    let corner = Anchor::under(frame(930., 650., 70., 41.), window());
    assert_eq!(near(corner, 100., HEIGHT), Near::BottomRight);
    // A footer button's tooltip never goes above, whatever the window's height, and never mirrors.
    let low = Anchor::beside(frame(8., 690., 32., 10.), window(), false);
    assert_eq!(near(low, 100., HEIGHT), Near::TopLeft);
    let right = Anchor::beside(frame(950., 300., 32., 32.), window(), false);
    assert_eq!(near(right, 100., HEIGHT), Near::TopLeft);
}
