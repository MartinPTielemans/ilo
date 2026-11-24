//! Which pages show live in their pictures, on a host that records what the shell asks of the core
//! and answers a tile as the test says.

use std::rc::Rc;

use gpui::{Bounds, Pixels, point, px, size};

use super::{LiveTiles, page_in};
use crate::shell::recorder::Recorder;
use crate::tokens::shell::agent_pointer::RING;
use crate::tokens::shell::conversation::PICTURE_RADIUS;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The conversation's visible area, in the window.
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

#[test]
fn a_picture_wholly_in_view_shows_its_page_inside_its_ring() {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), vec![shown(30, picture)]);
    let at = page_in(picture);
    assert_eq!(at.origin.x, picture.origin.x + px(RING));
    assert_eq!(at.size.width, picture.size.width - px(2. * RING));

    // Drawn again where it was, nothing is asked; moved, the tile follows.
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), Vec::<String>::new());
    let moved = bounds(120., 200., 200., 150.);
    tiles.place(&[(30, moved)], area(), false);
    assert_eq!(host.take(), vec![shown(30, moved)]);
}

#[test]
fn a_picture_partly_out_of_view_keeps_its_picture() {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    host.take();
    // Scrolled so its top is above the area's, then back in.
    let out = bounds(120., 80., 200., 150.);
    tiles.place(&[(30, out)], area(), false);
    assert_eq!(host.take(), vec!["end tile 30".to_string()]);
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), vec![shown(30, picture)]);
    // Wholly in view at the area's very edge is in view.
    tiles.end_all();
    host.take();
    let edge = bounds(500., 350., 200., 150.);
    tiles.place(&[(30, edge)], area(), false);
    assert_eq!(host.take(), vec![shown(30, edge)]);
}

#[test]
fn no_from_the_core_leaves_the_picture_and_is_asked_again_only_once_it_moves() {
    let (host, tiles) = tiles(false);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), vec![shown(30, picture)]);
    assert!(tiles.live.borrow().is_empty());
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), Vec::<String>::new(), "asked again in place");
    // Nothing live, so nothing is ended.
    tiles.end_all();
    assert_eq!(host.take(), Vec::<String>::new());
    let moved = bounds(120., 200., 200., 150.);
    tiles.place(&[(30, moved)], area(), false);
    assert_eq!(host.take(), vec![shown(30, moved)]);
}

#[test]
fn a_tile_the_core_stops_showing_is_no_longer_live() {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    host.take();
    // Its tab was shown in a window, which ended the tile in the core, and the picture moved.
    host.shows_tiles.set(Some(false));
    let moved = bounds(120., 200., 200., 150.);
    tiles.place(&[(30, moved)], area(), false);
    assert_eq!(host.take(), vec![shown(30, moved)]);
    assert!(tiles.live.borrow().is_empty());
}

#[test]
fn a_page_watched_over_the_conversation_holds_every_tile() {
    let (host, tiles) = tiles(true);
    let (first, second) = (
        bounds(120., 120., 200., 150.),
        bounds(340., 120., 200., 150.),
    );
    tiles.place(&[(30, first), (31, second)], area(), false);
    host.take();
    tiles.place(&[(30, first), (31, second)], area(), true);
    let mut ended = host.take();
    ended.sort();
    assert_eq!(
        ended,
        vec!["end tile 30".to_string(), "end tile 31".to_string()]
    );
    tiles.place(&[(30, first), (31, second)], area(), false);
    assert_eq!(host.take(), vec![shown(30, first), shown(31, second)]);
}

#[test]
fn a_picture_no_longer_drawn_ends_its_tile() {
    let (host, tiles) = tiles(true);
    let (first, second) = (
        bounds(120., 120., 200., 150.),
        bounds(340., 120., 200., 150.),
    );
    tiles.place(&[(30, first), (31, second)], area(), false);
    host.take();
    tiles.place(&[(30, first)], area(), false);
    assert_eq!(host.take(), vec!["end tile 31".to_string()]);
}

#[test]
fn every_tile_ends_with_the_conversation() {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    host.take();
    drop(tiles);
    assert_eq!(host.take(), vec!["end tile 30".to_string()]);
}

/// A watch borrows a page's tile with nothing ended: while it's lent nothing here moves or ends
/// it, and given back where the watch left it, the next frame moves it to its picture.
#[test]
fn a_lent_tile_is_left_to_the_watch_and_taken_back_where_it_is() {
    let (host, tiles) = tiles(true);
    let picture = bounds(120., 120., 200., 150.);
    tiles.place(&[(30, picture)], area(), false);
    host.take();
    assert!(tiles.shown_any());

    assert_eq!(tiles.lend(30), Some(page_in(picture)));
    tiles.place(&[(30, picture)], area(), false);
    tiles.place(&[], area(), true);
    assert_eq!(
        host.take(),
        Vec::<String>::new(),
        "a lent tile is the watch's"
    );

    let grown = bounds(240., 60., 1042., 734.);
    tiles.take_back(30, Some(grown));
    tiles.place(&[(30, picture)], area(), false);
    assert_eq!(host.take(), vec![shown(30, picture)]);

    // Taken back after the user took the page over, the core shows none, so none ends.
    tiles.lend(30);
    tiles.take_back(30, None);
    tiles.place(&[], area(), false);
    assert_eq!(host.take(), Vec::<String>::new());
}
