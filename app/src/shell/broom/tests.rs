//! The broom on the New Tab line, on a host that answers its state and records its click.

use std::rc::Rc;

use gpui::{
    Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext,
    accesskit::{Action, Live, Node, NodeId, Role},
    px,
};

use super::{Broom, FRAME, NAME, SORTING, failed_said, glyph_ink};
use crate::palette::{Palette, Tone};
use crate::product::PRODUCT;
use crate::shell::{CLEAR_HIT_OUT, Folders, Shell, TODAY_HAIRLINE, recorder::Recorder};
use crate::tokens::shell::{
    BROOM_SIZE, BROOM_UNAVAILABLE, DIVIDER_GAP, SIDEBAR_MIN, SIDEBAR_WIDTH,
};

const STATES: [Broom; 4] = [
    Broom::Idle,
    Broom::Sorting,
    Broom::Failed,
    Broom::NothingToSort,
];

/// A window showing a Space with `today` as its Today tabs, whose browser answers `broom`, read
/// as a screen reader reads it.
fn window(
    cx: &mut TestAppContext,
    today: Vec<u64>,
    broom: Option<Broom>,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    host.broom.set(broom);
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
    });
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    (shell, cx, host)
}

/// The browser answers `broom` from now on, and tells the shell, as the core does.
fn answer(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
    host: &Recorder,
    broom: Option<Broom>,
) {
    host.broom.set(broom);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// Where Clear, or the broom's glyph, was drawn.
fn frame(shell: &Entity<Shell>, cx: &mut VisualTestContext, key: &str) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| shell.button_frames.borrow()[key])
}

/// The node a screen reader has with `role` and `label`, if it has one.
fn node(cx: &mut VisualTestContext, role: Role, label: &str) -> Option<(NodeId, Node)> {
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    tree.nodes
        .into_iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label))
}

/// What the click at the middle of `at` asked of the browser, but for the tooltip it ends.
fn click(cx: &mut VisualTestContext, host: &Recorder, at: Bounds<Pixels>) -> Vec<String> {
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
    let calls = host.take().into_iter();
    calls.filter(|call| !call.starts_with("tooltip")).collect()
}

/// Where the node a screen reader has with `role` and `label` is, in the window's points.
fn place(cx: &mut VisualTestContext, role: Role, label: &str) -> Bounds<Pixels> {
    let (_, node) = node(cx, role, label).unwrap_or_else(|| panic!("no {role:?} called {label}"));
    let at = node.bounds().expect("a drawn node has its place");
    let scale = f64::from(cx.update(|window, _| window.scale_factor()));
    let point = |x: f64, y: f64| gpui::point(px((x / scale) as f32), px((y / scale) as f32));
    Bounds::from_corners(point(at.x0, at.y0), point(at.x1, at.y1))
}

fn close(a: Pixels, b: Pixels) -> bool {
    (a - b).abs() < px(0.01)
}

/// The broom is drawn exactly when Clear is: not where the browser answers none, and not on a day
/// with nothing, which has no Clear either.
#[gpui::test]
fn the_broom_is_drawn_exactly_when_clear_is(cx: &mut TestAppContext) {
    let (_, cx, _) = window(cx, vec![10, 11], None);
    assert!(node(cx, Role::Button, "Clear Today").is_some());
    assert!(node(cx, Role::Button, NAME).is_none(), "the browser's no");

    let (shell, cx, host) = window(cx, Vec::new(), Some(Broom::Idle));
    assert!(node(cx, Role::Button, "Clear Today").is_none());
    assert!(node(cx, Role::Button, NAME).is_none(), "a day with nothing");

    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    cx.run_until_parked();
    assert!(node(cx, Role::Button, "Clear Today").is_some());
    assert!(node(cx, Role::Button, NAME).is_some());
    answer(&shell, cx, &host, None);
    assert!(node(cx, Role::Button, NAME).is_none(), "taken away again");
}

/// With the broom, Clear sits the glyph and the line's gap left of where it ends alone, and the
/// glyph's box ends there instead, on Clear's middle. The broom takes clicks from where Clear
/// stops taking them. Nothing on the line moves as the broom goes from one state to another, the
/// failed sort's word included, which gives way where it has no room.
#[gpui::test]
fn clear_sits_the_glyph_and_the_gap_left_of_its_place_alone(cx: &mut TestAppContext) {
    let (_, cx, _) = window(cx, vec![10, 11], None);
    let alone = place(cx, Role::Button, "Clear Today");

    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let clear = place(cx, Role::Button, "Clear Today");
    let broom = place(cx, Role::Button, NAME);
    let glyph = frame(&shell, cx, FRAME);
    assert!(close(
        alone.right() - clear.right(),
        px(BROOM_SIZE + DIVIDER_GAP)
    ));
    assert!(close(
        clear.left(),
        alone.left() - px(BROOM_SIZE + DIVIDER_GAP)
    ));
    assert!(close(glyph.size.width, px(BROOM_SIZE)) && close(glyph.size.height, px(BROOM_SIZE)));
    // Clear's box is the word and what takes its clicks past it.
    assert!(close(glyph.right(), alone.right() - px(CLEAR_HIT_OUT)));
    assert!(close(
        glyph.left(),
        clear.right() - px(CLEAR_HIT_OUT) + px(DIVIDER_GAP)
    ));
    assert!(close(glyph.center().y, clear.center().y));
    assert!(close(broom.left(), clear.right()));
    assert!(close(broom.center().x, glyph.center().x));

    for state in STATES {
        answer(&shell, cx, &host, Some(state));
        assert_eq!(frame(&shell, cx, FRAME), glyph, "{state:?}");
        if state != Broom::Failed {
            assert_eq!(place(cx, Role::Button, "Clear Today"), clear, "{state:?}");
        }
    }
}

/// Only a click on the broom starts a sort, one for a click, and none while one runs or there is
/// nothing to sort. A press on the broom is never the line's, which would open the command bar.
#[gpui::test]
fn a_click_starts_a_sort_only_where_the_broom_says_it_would(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let glyph = frame(&shell, cx, FRAME);
    let clear = frame(&shell, cx, "clear");

    for (state, starts) in [
        (Broom::Idle, true),
        (Broom::Sorting, false),
        (Broom::NothingToSort, false),
        (Broom::Failed, true),
    ] {
        answer(&shell, cx, &host, Some(state));
        let asked = click(cx, &host, glyph);
        match starts {
            true => assert_eq!(asked, ["start broom"], "{state:?}"),
            false => assert!(asked.is_empty(), "{state:?}: {asked:?}"),
        }
        assert!(!shell.read_with(cx, |shell, _| shell.new_tab), "{state:?}");
    }

    // A sort begun since the broom was drawn, which the shell hasn't been told of yet.
    answer(&shell, cx, &host, Some(Broom::Idle));
    host.broom.set(Some(Broom::Sorting));
    assert!(click(cx, &host, glyph).is_empty());

    // A press on the line before Clear is the line's.
    let line = Bounds::centered_at(
        gpui::point(clear.left() - px(20.), clear.center().y),
        gpui::size(px(2.), px(2.)),
    );
    click(cx, &host, line);
    assert!(shell.read_with(cx, |shell, _| shell.new_tab));
}

/// A screen reader hears the broom by what it does, and as unavailable where a press would do
/// nothing: under its other name while it sorts, and under its own with nothing to sort.
#[gpui::test]
fn a_screen_reader_hears_the_broom_and_whether_it_can_sort(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let (id, broom) = node(cx, Role::Button, NAME).expect("the broom");
    assert!(!broom.is_disabled() && broom.supports_action(Action::Click));
    host.take();
    cx.simulate_a11y_action(id, Action::Click);
    cx.run_until_parked();
    assert!(host.take().contains(&"start broom".to_owned()));

    answer(&shell, cx, &host, Some(Broom::Sorting));
    assert!(node(cx, Role::Button, NAME).is_none());
    let (_, sorting) = node(cx, Role::Button, SORTING).expect("the broom at work");
    assert!(sorting.is_disabled());

    answer(&shell, cx, &host, Some(Broom::NothingToSort));
    let (id, nothing) = node(cx, Role::Button, NAME).expect("the broom, still in its place");
    assert!(nothing.is_disabled());
    host.take();
    cx.simulate_a11y_action(id, Action::Click);
    cx.run_until_parked();
    assert!(!host.take().contains(&"start broom".to_owned()));
}

/// A sort that changed nothing: its word stands in Clear's place while the browser says so, a
/// screen reader is told in a sentence with the product's name as it comes, the glyph is back and
/// sorts again, and a press where Clear was is neither Clear's nor the line's. Then Clear is back
/// where it was.
#[gpui::test]
fn a_failed_sort_says_so_in_clears_place(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let clear = frame(&shell, cx, "clear");
    assert!(node(cx, Role::Status, &failed_said()).is_none());

    answer(&shell, cx, &host, Some(Broom::Failed));
    assert!(node(cx, Role::Button, "Clear Today").is_none());
    let (_, word) = node(cx, Role::Status, &failed_said()).expect("the word");
    assert_eq!(word.value(), Some(failed_said().as_str()));
    assert_eq!(word.live(), Some(Live::Polite));
    assert!(failed_said().starts_with(PRODUCT));
    let (_, broom) = node(cx, Role::Button, NAME).expect("the glyph, back");
    assert!(!broom.is_disabled());
    // The word ends where Clear's did, the line's gap before the glyph.
    let word = place(cx, Role::Status, &failed_said());
    let glyph = frame(&shell, cx, FRAME);
    assert!(close(word.right(), glyph.left() - px(DIVIDER_GAP)));
    assert!(click(cx, &host, clear).is_empty());

    answer(&shell, cx, &host, Some(Broom::Idle));
    assert!(
        node(cx, Role::Status, &failed_said()).is_none(),
        "said once"
    );
    assert_eq!(frame(&shell, cx, "clear"), clear);
    assert_eq!(click(cx, &host, clear), ["clear"]);
}

/// With nothing to sort the glyph is drawn at a part of its own strength, and the pointer lights
/// it only where a click would sort.
#[test]
fn the_glyph_dims_with_nothing_to_sort_and_lights_only_where_it_sorts() {
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        let idle = glyph_ink(&palette, Broom::Idle, false);
        let nothing = glyph_ink(&palette, Broom::NothingToSort, false);
        assert!((nothing.a - idle.a * BROOM_UNAVAILABLE).abs() < 0.001);
        assert_eq!(glyph_ink(&palette, Broom::NothingToSort, true), nothing);
        assert_eq!(glyph_ink(&palette, Broom::Failed, false), idle);
        for state in [Broom::Idle, Broom::Failed] {
            assert!(glyph_ink(&palette, state, true).a > idle.a, "{state:?}");
        }
    }
}

/// The ring turns only while the broom sorts: a frame with it asks for the next, and once the
/// glyph is back no timer is left out, so a broom at rest wakes nothing.
#[gpui::test]
fn only_a_sorting_broom_asks_for_the_next_frame(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    assert!(!shell.read_with(cx, |shell, _| shell.breath.pending()));
    answer(&shell, cx, &host, Some(Broom::Sorting));
    assert!(shell.read_with(cx, |shell, _| shell.breath.pending()));
    answer(&shell, cx, &host, Some(Broom::Idle));
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |shell, _| shell.breath.pending()));
}

/// Resting on the broom says its name, hung from its glyph, and leaving it, or pressing it, ends
/// the tooltip. Under the pointer the glyph is lit, and not once the pointer has left.
#[gpui::test]
fn the_broom_says_its_name_under_the_pointer(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let glyph = frame(&shell, cx, FRAME);
    let tips = |cx: &mut VisualTestContext, at: gpui::Point<Pixels>| {
        host.take();
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take().into_iter();
        calls
            .filter(|call| call.starts_with("tooltip"))
            .collect::<Vec<_>>()
    };
    assert_eq!(tips(cx, glyph.center()), [format!("tooltip {NAME} ")]);
    let hung = host
        .anchors
        .borrow()
        .last()
        .map(|anchor| (anchor.top, anchor.bottom));
    assert_eq!(
        hung,
        Some((f32::from(glyph.top()), f32::from(glyph.bottom())))
    );
    assert!(shell.read_with(cx, |shell, _| shell.broom_hovered));
    assert_eq!(tips(cx, gpui::point(px(100.), px(300.))), ["tooltip none"]);
    assert!(!shell.read_with(cx, |shell, _| shell.broom_hovered));

    tips(cx, glyph.center());
    host.take();
    cx.simulate_mouse_down(glyph.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().iter().any(|call| call == "tooltip none"));
}

/// As the sidebar narrows, the line's hairline is drawn while it has the line's own gap of room
/// and not in less, where it would be a dot between the label and Clear: with Clear alone, with
/// the broom after it, and with a failed sort's word in Clear's place.
#[gpui::test]
fn a_hairline_shorter_than_the_lines_gap_is_not_drawn(cx: &mut TestAppContext) {
    for broom in [None, Some(Broom::Idle), Some(Broom::Failed)] {
        let (shell, cx, _) = window(cx, vec![10, 11], broom);
        let (mut whole, mut stub) = (false, false);
        // From wider than the sidebar opens, where the failed sort's word still leaves the line
        // room, to narrower than it goes.
        for width in (SIDEBAR_MIN as u32 / 2..=SIDEBAR_WIDTH as u32 + 120).rev() {
            TODAY_HAIRLINE.set(None);
            shell.update(cx, |shell, cx| {
                shell.sidebar_width = width as f32;
                cx.notify();
            });
            cx.run_until_parked();
            let (room, drawn) = TODAY_HAIRLINE.get().unwrap_or((px(0.), false));
            assert_eq!(drawn, room >= px(DIVIDER_GAP), "{broom:?} at {width}");
            whole |= drawn;
            stub |= !drawn && room > px(0.);
        }
        assert!(
            whole && stub,
            "{broom:?}: a line, then room for less than one"
        );
    }
}
