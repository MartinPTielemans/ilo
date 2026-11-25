//! Which pages show live in their pictures, on a host that records what the shell asks of the core
//! and answers a tile as the test says.

use std::rc::Rc;

use gpui::{Bounds, Pixels, TestAppContext, point, px, size};

use super::{LiveTiles, page_in};
use crate::shell::recorder::Recorder;
use crate::tokens::shell::agent_pointer::RING;
use crate::tokens::shell::conversation::PICTURE_RADIUS;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The rail's visible area, in the window.
fn area() -> Bounds<Pixels> {
    bounds(100., 100., 600., 400.)
}

/// A host whose core answers each tile with `shows`, and the conversation's tiles on it.
fn tiles(shows: bool) -> (Rc<Recorder>, Rc<LiveTiles>) {
    let host = Rc::new(Recorder::default());
    host.shows_tiles.set(Some(shows));
    let tiles = LiveTiles::new(host.clone());
    (host, tiles)
}

/// What the host records for `tab`'s tile in the picture drawn at `picture`.
fn shown(tab: u64, picture: Bounds<Pixels>) -> String {
    let at = page_in(picture);
    format!(
        "tile {tab} at {} {} {} {} radius {}",
        f32::from(at.origin.x),
        f32::from(at.origin.y),
        f32::from(at.size.width),
        f32::from(at.size.height),
        PICTURE_RADIUS - RING
    )
}

/// What the host records as `tab`'s tile ends: the picture that stands in its place is asked for
/// first, of the page as it is then.
fn ended_pictured(tab: u64) -> Vec<String> {
    vec![format!("task picture {tab}"), format!("end tile {tab}")]
}

#[gpui::test]
fn a_picture_wholly_in_view_shows_its_page_inside_its_ring(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, picture)]);
    let at = page_in(picture);
    assert_eq!(at.origin.x, picture.origin.x + px(RING));
    assert_eq!(at.size.width, picture.size.width - px(2. * RING));

    // Drawn again where it was, nothing is asked; moved, the tile follows.
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), Vec::<String>::new());
    let moved = bounds(120., 200., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, moved)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, moved)]);
}

#[gpui::test]
fn a_picture_partly_out_of_view_keeps_its_picture(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    host.take();
    // Scrolled so its top is above the area's, then back in.
    let out = bounds(120., 80., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, out)], area(), false, cx));
    assert_eq!(host.take(), ended_pictured(30));
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, picture)]);
    // Wholly in view at the area's very edge is in view.
    cx.update(|cx| tiles.end_all(cx));
    host.take();
    let edge = bounds(500., 350., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, edge)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, edge)]);
}

#[gpui::test]
fn no_from_the_core_leaves_the_picture_and_is_asked_again_only_once_it_moves(
    cx: &mut TestAppContext,
) {
    let (host, tiles) = tiles(false);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, picture)]);
    assert!(tiles.live.borrow().is_empty());
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), Vec::<String>::new(), "asked again in place");
    // Nothing live, so nothing is ended.
    cx.update(|cx| tiles.end_all(cx));
    assert_eq!(host.take(), Vec::<String>::new());
    let moved = bounds(120., 200., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, moved)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, moved)]);
}

#[gpui::test]
fn a_tile_the_core_stops_showing_is_no_longer_live(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    host.take();
    // Its tab was shown in a window, which ended the tile in the core, and the picture moved.
    host.shows_tiles.set(Some(false));
    let moved = bounds(120., 200., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, moved)], area(), false, cx));
    assert_eq!(
        host.take(),
        vec![shown(30, moved), "task picture 30".to_string()]
    );
    assert!(tiles.live.borrow().is_empty());
}

#[gpui::test]
fn a_page_watched_over_the_conversation_holds_every_tile(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let (first, second) = (
        bounds(120., 120., 200., 150.),
        bounds(340., 120., 200., 150.),
    );
    cx.update(|cx| tiles.place(&[(30, first), (31, second)], area(), false, cx));
    host.take();
    cx.update(|cx| tiles.place(&[(30, first), (31, second)], area(), true, cx));
    let ended = host.take();
    assert_eq!(ended.len(), 4, "{ended:?}");
    for tab in [30, 31] {
        let at = ended
            .iter()
            .position(|line| *line == format!("task picture {tab}"));
        assert_eq!(ended.get(at.unwrap() + 1), Some(&format!("end tile {tab}")));
    }
    cx.update(|cx| tiles.place(&[(30, first), (31, second)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, first), shown(31, second)]);
}

#[gpui::test]
fn a_picture_no_longer_drawn_ends_its_tile(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let (first, second) = (
        bounds(120., 120., 200., 150.),
        bounds(340., 120., 200., 150.),
    );
    cx.update(|cx| tiles.place(&[(30, first), (31, second)], area(), false, cx));
    host.take();
    cx.update(|cx| tiles.place(&[(30, first)], area(), false, cx));
    assert_eq!(host.take(), ended_pictured(31));
}

#[gpui::test]
fn every_tile_ends_with_the_conversation(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    host.take();
    drop(tiles);
    assert_eq!(host.take(), vec!["end tile 30".to_string()]);
}

/// A watch borrows a page's tile with nothing ended: while it's lent nothing here moves or ends
/// it, and given back where the watch left it, the next frame moves it to its picture.
#[gpui::test]
fn a_lent_tile_is_left_to_the_watch_and_taken_back_where_it_is(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    host.take();
    assert!(tiles.shown_any());

    assert_eq!(tiles.lend(30), Some(page_in(picture)));
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    cx.update(|cx| tiles.place(&[], area(), true, cx));
    assert_eq!(
        host.take(),
        Vec::<String>::new(),
        "a lent tile is the watch's"
    );

    let grown = bounds(240., 60., 1042., 734.);
    tiles.take_back(30, Some(grown));
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    assert_eq!(host.take(), vec![shown(30, picture)]);

    // Taken back after the user took the page over, the core shows none, so none ends.
    tiles.lend(30);
    tiles.take_back(30, None);
    cx.update(|cx| tiles.place(&[], area(), false, cx));
    assert_eq!(host.take(), Vec::<String>::new());
}

#[gpui::test]
fn every_way_a_tile_ends_asks_first_for_its_tabs_picture(cx: &mut TestAppContext) {
    // A picture from before the tile went live stood in its place, and jumped forward when it was
    // live again.
    let (host, tiles) = tiles(true);
    let (first, second) = (
        bounds(120., 120., 200., 150.),
        bounds(340., 120., 200., 150.),
    );
    let live = |cx: &mut TestAppContext| {
        cx.update(|cx| tiles.place(&[(30, first)], area(), false, cx));
        host.take();
    };
    // Scrolled half out.
    live(cx);
    let half_out = bounds(120., 20., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, half_out)], area(), false, cx));
    assert_eq!(host.take(), ended_pictured(30), "scrolled out");
    // Pushed out by a newer picture.
    live(cx);
    let pushed = bounds(620., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(31, first), (30, pushed)], area(), false, cx));
    let lines = host.take();
    assert_eq!(lines[..2], ended_pictured(30)[..], "pushed out: {lines:?}");
    cx.update(|cx| tiles.end_all(cx));
    host.take();
    // Held by a page watched over the conversation.
    live(cx);
    cx.update(|cx| tiles.place(&[(30, first)], area(), true, cx));
    assert_eq!(host.take(), ended_pictured(30), "held");
    // The conversation not drawn.
    live(cx);
    cx.update(|cx| tiles.end_all(cx));
    assert_eq!(host.take(), ended_pictured(30), "not drawn");
    // The core saying no to a live tile.
    live(cx);
    host.shows_tiles.set(Some(false));
    cx.update(|cx| tiles.place(&[(30, second)], area(), false, cx));
    assert_eq!(
        host.take(),
        vec![shown(30, second), "task picture 30".to_string()],
        "the core's no"
    );
}

#[gpui::test]
fn the_picture_an_end_asked_for_is_known_by_its_answer(cx: &mut TestAppContext) {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    cx.update(|cx| tiles.place(&[(30, picture)], area(), false, cx));
    cx.update(|cx| tiles.end_all(cx));
    host.take();
    let request = *tiles.asked.borrow().keys().next().unwrap();
    assert_eq!(tiles.asked_at_end(request), Some(30));
    assert_eq!(tiles.asked_at_end(request), None, "answered once");
}
