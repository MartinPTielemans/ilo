//! Where the hover cards of the favorites' tiles go, and what a click on one asks for, on a host
//! that records them.

use std::rc::Rc;

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};

use crate::{
    hover_card::Anchor,
    shell::{Folders, LIST_TOP, PLATE_INSET, Row, Shell, TILE_HEIGHT, recorder::Recorder},
};

/// A drawn shell with the favorites 10, 11 and 12.
fn with_tiles(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_favorites(vec![(10, 1), (11, 1), (12, 1)], cx);
        for (tab, title) in [(10, "YouTube"), (11, "X"), (12, "Proton Mail: Sign-in")] {
            shell.set_title(tab, title.into(), cx);
        }
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// Where the card the shell asked for as the pointer moved onto `tab`'s tile from wherever it was
/// goes from, and the tile's frame.
fn hover_tile(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    host: &Recorder,
    tab: u64,
) -> (Anchor, gpui::Bounds<gpui::Pixels>) {
    let frame = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&tab]);
    host.card.borrow_mut().take();
    cx.simulate_mouse_move(frame.center(), None, Modifiers::none());
    cx.run_until_parked();
    let rest = host.card.borrow().clone().expect("no card asked for");
    assert_eq!(rest.card.tab, tab);
    (rest.anchor, frame)
}

/// The sidebar's rows' right edge in the window, which a card starts GAP past.
fn rows_right(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.read_with(cx, |shell, _| {
        f32::from(shell.sidebar_origin.get().x) + shell.width() - PLATE_INSET
    })
}

/// Each tile's card goes beside the sidebar's rows, not over the tiles beside it, its top at the
/// tile's, the pointer coming straight from the last tile's.
#[gpui::test]
fn each_tiles_card_goes_beside_the_rows_at_its_top(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tiles(cx);
    let right = rows_right(&shell, cx);
    for tab in [10, 11, 12, 11, 10] {
        let (anchor, frame) = hover_tile(&shell, cx, &host, tab);
        let window = shell.read_with(cx, |shell, _| shell.viewport);
        assert_eq!(anchor.window, window);
        assert_eq!(anchor.row.top(), frame.top(), "tile {tab}");
        assert_eq!(f32::from(anchor.row.right()), right, "tile {tab}");
    }
}

/// A tile's card starts at the tile's top, under the URL field, and its frame is the tile's
/// outside on a 1× display as on a 2× one, where GPUI draws the hairline round it 1 pt wide.
#[gpui::test]
fn a_tiles_card_starts_at_its_top_at_1x_and_2x(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tiles(cx);
    for (scale, tab) in [(1., 10), (2., 11), (1., 12)] {
        cx.simulate_scale_factor_change(scale);
        cx.run_until_parked();
        let (anchor, frame) = hover_tile(&shell, cx, &host, tab);
        assert_eq!(
            f32::from(anchor.row.top()),
            LIST_TOP,
            "tile {tab} at {scale}×"
        );
        assert_eq!(
            f32::from(frame.size.height),
            TILE_HEIGHT,
            "tile {tab} at {scale}×"
        );
    }
}

/// In a sidebar as narrow as it goes, where the third tile wraps to a second row, each tile's card
/// still starts at its own top.
#[gpui::test]
fn a_narrow_sidebars_tiles_have_cards_at_their_own_tops(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tiles(cx);
    shell.update(cx, |shell, cx| {
        shell.sidebar_width = 164.;
        cx.notify();
    });
    cx.run_until_parked();
    let right = rows_right(&shell, cx);
    for tab in [10, 11, 12] {
        let (anchor, frame) = hover_tile(&shell, cx, &host, tab);
        assert_eq!(
            (anchor.row.top(), f32::from(anchor.row.right())),
            (frame.top(), right),
            "tile {tab}"
        );
    }
}

/// A drawn shell with the favorite 10, Docs, and the Today tabs 1 and 2.
fn with_docs(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Work".into(), None, 0, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_favorites(vec![(10, 0)], cx);
        shell.set_title(10, "Docs".into(), cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

fn cards_for(host: &Recorder, tab: u64) -> usize {
    let rest = format!("card rest {tab} ");
    host.take()
        .iter()
        .filter(|call| call.starts_with(&rest))
        .count()
}

/// A drop that leaves the pointer on a tile, which the drag had it hover, asks for the tile's
/// card once the tile has settled: no hover change comes to ask for it, and not before, as a tile
/// still sliding would put it beside where the tile was.
#[gpui::test]
fn a_tile_under_the_pointer_after_a_drag_has_its_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_docs(cx);
    let (x, y) = shell.update(cx, |shell, _| {
        let [left, top, width] = shell.tile_places(1)[0];
        (
            left + width / 2.,
            shell.tiles_top() + top + TILE_HEIGHT / 2.,
        )
    });
    shell.update(cx, |shell, _| shell.press_tile(10, point(px(x), px(y))));
    shell.update(cx, |shell, cx| shell.drag_to(point(px(x), px(y + 30.)), cx));
    assert!(shell.read_with(cx, |shell, _| shell.dragging().is_some()));
    // The tile is hovered under the drag, which keeps its card.
    shell.update(cx, |shell, cx| shell.hover(Row::Tab(10), true, cx));
    cx.run_until_parked();
    assert_eq!(cards_for(&host, 10), 0);

    shell.update(cx, |shell, cx| {
        assert!(shell.cancel_drag(cx));
        // Its tile is still sliding home.
        shell
            .tile_slides
            .insert(10, ([0.; 3], shell.clock.now(), crate::shell::Curve::Slide));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cards_for(&host, 10), 0, "while it slides");

    shell.update(cx, |shell, cx| {
        shell.tile_slides.clear();
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cards_for(&host, 10), 1);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(cards_for(&host, 10), 0, "only once");
}

/// A click on a tile shows its tab and gives its page the keyboard, as a press on a row does.
#[gpui::test]
fn a_click_on_a_tile_gives_its_page_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tiles(cx);
    let middle = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&11].center());
    cx.simulate_mouse_move(middle, None, Modifiers::none());
    cx.simulate_click(middle, Modifiers::none());
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        calls.ends_with(&["select 11".into(), "focus page".into()]),
        "{calls:?}"
    );
}

/// At MOST_FAVORITES no tab becomes another: Add to Favorites is dimmed and leaves them, the hover
/// card has no Add to Favorites, and a row dragged over the tiles stays a row. One short of it, the
/// drag makes it a tile.
#[gpui::test]
fn the_favorites_take_no_more_than_the_most(cx: &mut TestAppContext) {
    use crate::{
        hover_card::Action,
        shell::{
            Place,
            drag::detents::{X, drag_to, press, shell_with},
            favorites::MOST_FAVORITES,
        },
    };

    let (shell, host) = shell_with(cx, &[1], &[2], Folders::default());
    let favorites = |count: u64| (100..100 + count).map(|tab| (tab, 0)).collect::<Vec<_>>();
    let over_tiles = |cx: &mut TestAppContext| {
        press(&shell, cx, 2);
        let top = shell.read_with(cx, |shell, _| shell.tiles_top());
        drag_to(&shell, cx, X, top + TILE_HEIGHT / 2.);
        let to = shell.read_with(cx, |shell, _| shell.drag.as_ref().map(|drag| drag.to));
        shell.update(cx, |shell, cx| shell.cancel_drag(cx));
        to
    };
    shell.update(cx, |shell, cx| {
        shell.set_favorites(favorites(MOST_FAVORITES as u64 - 1), cx)
    });
    assert!(matches!(over_tiles(cx), Some(Place::Favorite(_))));

    shell.update(cx, |shell, cx| {
        shell.set_favorites(favorites(MOST_FAVORITES as u64), cx)
    });
    assert!(matches!(over_tiles(cx), Some(Place::List(..))));
    let actions = shell.read_with(cx, |shell, _| shell.card_actions(1));
    assert_eq!(actions, [Action::Unpin]);
    host.take_edits();
    shell.update(cx, |shell, cx| shell.favorite_tab(1, usize::MAX, cx));
    assert!(host.take_edits().is_empty(), "a thirteenth favorite");
    // A favorite still moves among them.
    shell.update(cx, |shell, cx| shell.favorite_tab(100, 3, cx));
    assert_eq!(host.take_edits(), ["favorite 100 3"]);
}

/// The tiles take Arc 1.171's rows: the most columns, up to 4, that keep a tile 47 wide, then the
/// fewest that need no more rows, a short last row on the left. The widths and counts are the
/// ones the outside Arc measurement read from its accessibility tree.
#[gpui::test]
fn the_tiles_take_arcs_rows(cx: &mut TestAppContext) {
    let (shell, cx, _) = with_tiles(cx);
    let rows = |width: f32, count: usize, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, _| shell.sidebar_width = width);
        let places = shell.read_with(cx, |shell, _| shell.tile_places(count));
        let mut rows: Vec<usize> = Vec::new();
        let mut top = None;
        for [_, place_top, _] in &places {
            match top == Some(*place_top) {
                true => *rows.last_mut().unwrap() += 1,
                false => rows.push(1),
            }
            top = Some(*place_top);
        }
        rows
    };
    for (count, want) in [
        (1, vec![1]),
        (2, vec![2]),
        (3, vec![3]),
        (4, vec![4]),
        (5, vec![3, 2]),
        (6, vec![3, 3]),
        (7, vec![4, 3]),
        (8, vec![4, 4]),
        (9, vec![3, 3, 3]),
    ] {
        assert_eq!(rows(347., count, cx), want, "{count} at 347");
    }
    for (width, count, want) in [
        (230., 4, vec![4]),
        (227., 4, vec![2, 2]),
        (200., 3, vec![3]),
        (200., 4, vec![2, 2]),
        (200., 5, vec![3, 2]),
        (140., 3, vec![2, 1]),
        (100., 2, vec![1, 1]),
    ] {
        assert_eq!(rows(width, count, cx), want, "{count} at {width}");
    }

    // A tile keeps its width on a short last row, and its left edge is on the half point.
    shell.update(cx, |shell, _| shell.sidebar_width = 228.);
    let places = shell.read_with(cx, |shell, _| shell.tile_places(5));
    assert!(
        places.iter().all(|place| place[2] == places[0][2]),
        "{places:?}"
    );
    assert_eq!(places[3][0], places[0][0]);
    assert!(
        places.iter().all(|place| (place[0] * 2.).fract() == 0.),
        "{places:?}"
    );
    assert_eq!(places[1][1] - places[0][1], 0.);
    assert_eq!(places[3][1], TILE_HEIGHT + crate::shell::TILE_GAP);
}

/// A drawn shell whose favorites are the split 10 and 11, focused on 11, and 12, with the Today
/// tab 1.
fn with_split_tile(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        let split = crate::shell::Split {
            tabs: vec![10, 11],
            ..Default::default()
        };
        shell.set_split_favorites(
            vec![(10, 1), (11, 1), (12, 1)],
            vec![split],
            [(10, 11)].into(),
            cx,
        );
        for (tab, title) in [(10, "Q4 plan"), (11, "Standup notes"), (12, "Inbox")] {
            shell.set_title(tab, title.into(), cx);
        }
        shell.set_selected(1, cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// A favorite split is one favorite, its first tab's tile, and a click on it or its ⌘ key shows it
/// focused where it was focused last, which follows the page of it shown.
#[gpui::test]
fn a_favorite_split_is_one_tile_shown_where_it_was_focused(cx: &mut TestAppContext) {
    use crate::shell::Command;

    let (shell, cx, host) = with_split_tile(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.favorites().to_vec()),
        [10, 12]
    );
    let middle = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&10].center());
    cx.simulate_mouse_move(middle, None, Modifiers::none());
    cx.simulate_click(middle, Modifiers::none());
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        calls.ends_with(&["select 11".into(), "focus page".into()]),
        "{calls:?}"
    );
    // The core shows it, then another tab, and ⌘1 shows it focused where it was.
    shell.update(cx, |shell, cx| {
        shell.set_selected(11, cx);
        shell.set_selected(1, cx);
        shell.run(Command::SelectTab(0), "ui", cx);
    });
    let calls = host.take();
    assert!(calls.contains(&"select 11".into()), "{calls:?}");
    // A click in the other page focuses it, and the split shows focused there next.
    shell.update(cx, |shell, cx| {
        shell.set_selected(10, cx);
        shell.set_selected(1, cx);
        shell.run(Command::SelectTab(0), "ui", cx);
    });
    let calls = host.take();
    assert!(calls.contains(&"select 10".into()), "{calls:?}");
}

/// The split's tile is shown, and ringed, while either of its pages is the tab shown.
#[gpui::test]
fn a_split_tile_is_shown_with_either_page(cx: &mut TestAppContext) {
    let (shell, cx, _) = with_split_tile(cx);
    for (selected, shown) in [(1, false), (10, true), (11, true), (12, false)] {
        shell.update(cx, |shell, cx| shell.set_selected(selected, cx));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.tile_shown(10)),
            shown,
            "with {selected} shown"
        );
    }
}

/// The split's card has both pages, the focused one's title and every page's host, and its star
/// takes the whole split to the top of Today, which ⌘Z makes a favorite split again.
#[gpui::test]
fn a_split_tiles_card_shows_its_pages_and_removes_it_whole(cx: &mut TestAppContext) {
    use crate::hover_card::Action;

    let (shell, cx, host) = with_split_tile(cx);
    let card = shell
        .read_with(cx, |shell, _| shell.card(10))
        .expect("no card");
    let panes: Vec<u64> = card.panes.iter().map(|(tab, _)| *tab).collect();
    assert_eq!((panes, card.focused), (vec![10, 11], 11));
    assert_eq!(card.title.as_ref(), "Standup notes");
    assert_eq!(card.actions, [Action::Unfavorite]);
    // Its second page has no tile, so no card of its own.
    assert!(shell.read_with(cx, |shell, _| shell.card(11)).is_none());
    host.take_edits();
    shell.update(cx, |shell, cx| shell.move_tab_to(10, false, 0, 0, cx));
    assert_eq!(host.take_edits(), ["move 10 false 0 0"]);
    // The core has made it a split at the top of Today.
    shell.update(cx, |shell, cx| {
        let split = crate::shell::Split {
            tabs: vec![10, 11],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11, 1],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_favorites(vec![(12, 1)], cx);
        shell.run(crate::shell::Command::Undo, "ui", cx);
    });
    assert_eq!(host.take_edits(), ["favorite split 10 0"]);
}

/// ⌘W on a favorite split's page shown closes both its pages and keeps its tile, as a pinned
/// split's do.
#[gpui::test]
fn closing_a_favorite_split_closes_its_pages_and_keeps_its_tile(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_split_tile(cx);
    shell.update(cx, |shell, cx| {
        shell.set_selected(11, cx);
        shell.run(crate::shell::Command::CloseTab, "ui", cx);
    });
    assert_eq!(host.split_closes.take(), [11]);
}

/// Separate All Tabs on the split's tile leaves its first page the favorite and takes the others
/// to the top of Today.
#[gpui::test]
fn separating_a_favorite_split_keeps_its_first_page_the_favorite(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_split_tile(cx);
    host.take_edits();
    shell.update(cx, |shell, cx| shell.separate_favorite(10, cx));
    assert_eq!(host.take_edits(), ["separate 11", "move 11 false 0 0"]);
}

/// ⌘Z after Separate All Tabs makes the favorite split again at its tile, its first page joined
/// to the others through the top of Today, and ⇧⌘Z separates it again (#1776).
#[gpui::test]
fn separating_a_favorite_split_undoes(cx: &mut TestAppContext) {
    use crate::shell::{Command, Split};

    let (shell, cx, host) = with_split_tile(cx);
    shell.update(cx, |shell, cx| shell.separate_favorite(10, cx));
    host.take_edits();
    host.take();
    // The core has left 10 the favorite and put 11 at the top of Today.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![11, 1],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_split_favorites(vec![(10, 1), (12, 1)], Vec::new(), [].into(), cx);
        shell.run(Command::Undo, "ui", cx);
    });
    assert_eq!(
        host.take_edits(),
        [
            "move 10 false 0 0",
            "split 11 10 false",
            "favorite split 10 0"
        ]
    );
    // Tab 1 stays shown, where the core shows the split it joins.
    assert_eq!(host.take().last().map(String::as_str), Some("select 1"));
    // The core has made them the favorite split again.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        let split = Split {
            tabs: vec![10, 11],
            ..Default::default()
        };
        shell.set_split_favorites(
            vec![(10, 1), (11, 1), (12, 1)],
            vec![split],
            [(10, 11)].into(),
            cx,
        );
        shell.run(Command::Redo, "ui", cx);
    });
    assert_eq!(host.take_edits(), ["separate 11", "move 11 false 0 0"]);
}

/// A split's row dragged over the tiles is a tile there, its pages one favorite once it's let go,
/// a split counting one of the most.
#[gpui::test]
fn a_split_dragged_onto_the_tiles_is_one_favorite(cx: &mut TestAppContext) {
    use crate::shell::{
        Place, Split,
        drag::detents::{X, drag_to, press, shell_with},
    };

    let (shell, host) = shell_with(cx, &[], &[1, 2, 3], Folders::default());
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![1, 2],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2, 3],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_favorites(vec![(100, 0)], cx);
    });
    press(&shell, cx, 1);
    let top = shell.read_with(cx, |shell, _| shell.tiles_top());
    drag_to(&shell, cx, X, top + TILE_HEIGHT / 2.);
    let to = shell.read_with(cx, |shell, _| shell.drag.as_ref().map(|drag| drag.to));
    assert!(matches!(to, Some(Place::Favorite(_))), "{to:?}");
    host.take_edits();
    shell.update(cx, |shell, cx| {
        // Let go once it has turned into its tile.
        if let Some(drag) = shell.drag.as_mut() {
            drag.shape.start = None;
        }
        shell.drop_row(cx)
    });
    let edits = host.take_edits();
    assert!(
        edits
            .iter()
            .any(|edit| edit.starts_with("favorite split 1 ")),
        "{edits:?}"
    );
}
