//! The broom on the New Tab line, on a host that answers its state and records its click.

use std::{rc::Rc, time::Duration};

use gpui::{
    Bounds, Entity, Modifiers, Pixels, SharedString, TestAppContext, VisualTestContext,
    accesskit::{Action, Live, Node, NodeId, Role},
    point, px,
};

use super::{Broom, DRAWN, FRAME, NAME, SORTING, failed_said, glyph_ink};
use crate::palette::{Palette, Tone};
use crate::product::PRODUCT;
use crate::shell::{
    CLEAR_HIT_OUT, Command, Folders, Shell, TODAY_HAIRLINE, TOOLS_FADE, recorder::Recorder,
    sidebar_keys::NEW_TAB,
};
use crate::tokens::shell::{BROOM_SIZE, DIVIDER_GAP, SIDEBAR_MIN, SIDEBAR_WIDTH};

/// A window showing a Space with `today` as its Today tabs, whose browser answers `broom`, read
/// as a screen reader reads it, for which the sidebar keeps what the pointer brings.
fn window(
    cx: &mut TestAppContext,
    today: Vec<u64>,
    broom: Option<Broom>,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = plain(cx, today, broom);
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    (shell, cx, host)
}

/// That window with no screen reader, the pointer nowhere yet.
fn plain(
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
    cx.run_until_parked();
    (shell, cx, host)
}

/// The broom the line has when drawn now: its state and how much of it, or None with no glyph.
fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<(Broom, f32)> {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    DRAWN.get().1
}

/// Puts the pointer over the sidebar, clear of the New Tab line, or over the page, and lets the
/// sidebar's fade for it end.
fn pointer(cx: &mut VisualTestContext, over: bool) {
    let at = match over {
        true => point(px(100.), px(300.)),
        false => point(px(600.), px(400.)),
    };
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
    cx.executor()
        .advance_clock(Duration::from_secs_f32(TOOLS_FADE * 2.));
    cx.run_until_parked();
}

/// Gives the sidebar the keyboard as Focus Sidebar does, with its ring on `row`, or takes the
/// ring away.
fn keys(shell: &Entity<Shell>, cx: &mut VisualTestContext, row: Option<u64>) {
    shell.update(cx, |shell, cx| match row {
        Some(row) => {
            shell.run(Command::FocusSidebar, "ui", cx);
            shell.key_row(row, cx);
        }
        None => shell.leave_keys("test", cx),
    });
    cx.run_until_parked();
}

/// The room the line's hairline has when drawn now.
fn hairline(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Pixels {
    TODAY_HAIRLINE.set(None);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    TODAY_HAIRLINE.get().expect("the line has a hairline").0
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

/// No broom where the browser answers none, and none on a day with nothing, which has no Clear
/// either.
#[gpui::test]
fn no_broom_where_the_browser_has_none_or_the_day_has_nothing(cx: &mut TestAppContext) {
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

/// The glyph is drawn only where it makes sense. Where a click would sort: with the pointer over
/// the sidebar, anywhere in it, or the keys on the New Tab line, and not otherwise. While a sort
/// runs and while its failure is said: whatever the pointer does. With nothing to sort, and where
/// the browser has no broom: never.
#[gpui::test]
fn the_glyph_is_drawn_only_where_it_makes_sense(cx: &mut TestAppContext) {
    let (shell, cx, host) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    pointer(cx, false);
    assert_eq!(shown(&shell, cx), None, "the pointer over the page");
    pointer(cx, true);
    assert_eq!(shown(&shell, cx), Some((Broom::Idle, 1.)));
    pointer(cx, false);
    assert_eq!(shown(&shell, cx), None, "the pointer gone again");

    keys(&shell, cx, Some(NEW_TAB));
    assert_eq!(shown(&shell, cx), Some((Broom::Idle, 1.)), "the keys on it");
    keys(&shell, cx, Some(10));
    assert_eq!(shown(&shell, cx), None, "the keys on a tab's row");
    keys(&shell, cx, None);

    for (state, over, drawn) in [
        (Broom::Sorting, false, true),
        (Broom::Failed, false, true),
        (Broom::Sorting, true, true),
        (Broom::Failed, true, true),
        (Broom::NothingToSort, false, false),
        (Broom::NothingToSort, true, false),
    ] {
        pointer(cx, over);
        answer(&shell, cx, &host, Some(state));
        let expected = drawn.then_some((state, 1.));
        assert_eq!(shown(&shell, cx), expected, "{state:?}, over {over}");
    }
    pointer(cx, true);
    answer(&shell, cx, &host, None);
    assert_eq!(shown(&shell, cx), None, "the browser's no");
}

/// With motion reduced the glyph comes and goes at once, with no fade.
#[gpui::test]
fn with_motion_reduced_the_glyph_comes_and_goes_at_once(cx: &mut TestAppContext) {
    let (shell, cx, _) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    cx.update(|_, cx| cx.set_reduce_motion(true));
    cx.simulate_mouse_move(point(px(100.), px(300.)), None, Modifiers::none());
    assert_eq!(shown(&shell, cx), Some((Broom::Idle, 1.)));
    cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
    assert_eq!(shown(&shell, cx), None);
}

/// Nothing of the broom runs on a tab switch: the browser is asked to start nothing, and what the
/// line shows of it is as it was, with the pointer over the sidebar and away from it.
#[gpui::test]
fn a_tab_switch_runs_nothing_of_the_broom(cx: &mut TestAppContext) {
    let (shell, cx, host) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    for over in [true, false] {
        pointer(cx, over);
        let before = shown(&shell, cx);
        assert_eq!(before.is_some(), over);
        host.take();
        for tab in [11, 12, 10] {
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
            cx.run_until_parked();
        }
        let asked = host.take();
        assert!(
            !asked.iter().any(|call| call.contains("broom")),
            "{asked:?}"
        );
        assert_eq!(shown(&shell, cx), before, "over {over}");
        assert!(!shell.read_with(cx, |shell, _| shell.breath.pending()));
    }
}

/// Whether the glyph shows is decided from what the shell has: as the pointer comes and goes and
/// the keys move, nothing of the broom is asked of the browser, which starts nothing.
#[gpui::test]
fn nothing_is_started_or_asked_to_decide_whether_the_glyph_shows(cx: &mut TestAppContext) {
    let (shell, cx, host) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    host.take();
    pointer(cx, true);
    assert!(shown(&shell, cx).is_some());
    keys(&shell, cx, Some(NEW_TAB));
    pointer(cx, false);
    assert!(shown(&shell, cx).is_some());
    keys(&shell, cx, None);
    assert!(shown(&shell, cx).is_none());
    let asked = host.take();
    assert!(
        !asked.iter().any(|call| call.contains("broom")),
        "{asked:?}"
    );
}

/// Runs the window's next frame, and returns how many had asked for it.
fn next_frame(cx: &mut VisualTestContext) -> usize {
    let asked = cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    asked
}

/// The pointer comes over the sidebar and stays: how many frames were asked for until none was,
/// a quarter of the sidebar's fade apart, and how much of the broom the line had part of the way.
fn frames_of_the_pointer_coming(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
) -> (usize, Option<(Broom, f32)>) {
    pointer(cx, false);
    while next_frame(cx) > 0 {}
    cx.simulate_mouse_move(point(px(100.), px(300.)), None, Modifiers::none());
    cx.run_until_parked();
    let (mut frames, mut part) = (0, None);
    for step in 0..40 {
        cx.executor()
            .advance_clock(Duration::from_secs_f32(TOOLS_FADE / 4.));
        if next_frame(cx) == 0 {
            break;
        }
        frames += 1;
        if step == 1 {
            part = DRAWN.get().1;
            let tools = shell.read_with(cx, |shell, _| shell.tools_opacity());
            assert!(tools > 0. && tools < 1., "{tools}");
            assert_eq!(part.map(|(_, shown)| shown).unwrap_or(tools), tools);
        }
    }
    (frames, part)
}

/// The pointer over the sidebar adds no timer and no frame of the broom's own: the glyph comes on
/// the fade the sidebar already runs for its URL field's button, as far along as that is, in as
/// many frames as a sidebar with no broom takes, and once that has ended nothing asks for another
/// frame and no timer is out, however long the pointer stays.
#[gpui::test]
fn the_glyph_comes_on_the_sidebars_own_fade_and_wakes_nothing(cx: &mut TestAppContext) {
    let (shell, without, _) = plain(cx, vec![10, 11, 12, 13], None);
    let (plain_frames, none) = frames_of_the_pointer_coming(&shell, without);
    assert!(plain_frames > 0, "the sidebar's fade asks for its frames");
    assert_eq!(none, None);

    let (shell, cx, _) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    let (frames, part) = frames_of_the_pointer_coming(&shell, cx);
    assert_eq!(frames, plain_frames);
    assert!(matches!(part, Some((Broom::Idle, shown)) if shown > 0. && shown < 1.));

    assert_eq!(shown(&shell, cx), Some((Broom::Idle, 1.)));
    assert_eq!(next_frame(cx), 0);
    assert!(shell.read_with(cx, |shell, _| shell.tools_fade.is_none()));
    assert!(!shell.read_with(cx, |shell, _| shell.breath.pending()));
    let built = DRAWN.get().0;
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(next_frame(cx), 0);
    assert_eq!(DRAWN.get().0, built, "the line was built with nothing new");
}

/// The menu command is the other way in: it is offered with the pointer away and no glyph drawn,
/// and it starts the sort.
#[gpui::test]
fn the_menu_command_sorts_with_no_glyph_drawn(cx: &mut TestAppContext) {
    let (shell, cx, host) = plain(cx, vec![10, 11, 12, 13], Some(Broom::Idle));
    pointer(cx, false);
    assert_eq!(shown(&shell, cx), None);
    let command = Command::SortIntoFolders;
    assert!(shell.read_with(cx, |shell, _| shell.offers(command)));
    shell.update(cx, |shell, cx| shell.update_menus(cx));
    assert!(host.menu_bar.borrow().broom_offered);
    host.take();
    shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"start broom".to_owned()));
}

/// Clear keeps its place with the broom and without, at the line's end, and the hairline alone
/// gives way: the glyph sits the line's gap left of Clear's word and the hairline ends as far
/// before the glyph. The broom takes clicks up to where Clear starts taking them. Nothing on the
/// line moves as a sort starts, and with nothing to sort the hairline has its room back. (A failed
/// sort's word is longer than Clear and ends where Clear did, so the glyph stands the gap before
/// it: a_failed_sort_says_so_in_clears_place.)
#[gpui::test]
fn clear_keeps_its_place_and_the_hairline_alone_gives_way(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx, vec![10, 11], None);
    let alone = place(cx, Role::Button, "Clear Today");
    let line = hairline(&shell, cx);

    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let clear = place(cx, Role::Button, "Clear Today");
    let broom = place(cx, Role::Button, NAME);
    let glyph = frame(&shell, cx, FRAME);
    assert_eq!(clear, alone);
    assert!(close(glyph.size.width, px(BROOM_SIZE)) && close(glyph.size.height, px(BROOM_SIZE)));
    // Clear's box is the word and what takes its clicks before it.
    assert!(close(
        glyph.right(),
        clear.left() + px(CLEAR_HIT_OUT) - px(DIVIDER_GAP)
    ));
    assert!(close(glyph.center().y, clear.center().y));
    assert!(close(broom.right(), clear.left()));
    assert!(close(broom.center().x, glyph.center().x));
    assert!(close(
        hairline(&shell, cx),
        line - px(BROOM_SIZE + DIVIDER_GAP)
    ));

    for state in [Broom::Idle, Broom::Sorting] {
        answer(&shell, cx, &host, Some(state));
        assert_eq!(frame(&shell, cx, FRAME), glyph, "{state:?}");
        assert_eq!(place(cx, Role::Button, "Clear Today"), clear, "{state:?}");
    }
    answer(&shell, cx, &host, Some(Broom::NothingToSort));
    assert!(node(cx, Role::Button, NAME).is_none());
    assert_eq!(place(cx, Role::Button, "Clear Today"), clear);
    assert!(close(hairline(&shell, cx), line));
}

/// Only a click on the broom starts a sort, one for a click, and none while one runs. A press on
/// the broom is never the line's, which would open the command bar. With nothing to sort there is
/// no broom to press: a press where its glyph was is the line's.
#[gpui::test]
fn a_click_starts_a_sort_only_where_the_broom_says_it_would(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    for (state, starts) in [
        (Broom::Idle, true),
        (Broom::Sorting, false),
        (Broom::Failed, true),
    ] {
        answer(&shell, cx, &host, Some(state));
        let glyph = frame(&shell, cx, FRAME);
        let asked = click(cx, &host, glyph);
        match starts {
            true => assert_eq!(asked, ["start broom"], "{state:?}"),
            false => assert!(asked.is_empty(), "{state:?}: {asked:?}"),
        }
        assert!(!shell.read_with(cx, |shell, _| shell.new_tab), "{state:?}");
    }

    // A sort begun since the broom was drawn, which the shell hasn't been told of yet.
    answer(&shell, cx, &host, Some(Broom::Idle));
    let glyph = frame(&shell, cx, FRAME);
    host.broom.set(Some(Broom::Sorting));
    assert!(click(cx, &host, glyph).is_empty());

    answer(&shell, cx, &host, Some(Broom::NothingToSort));
    let asked = click(cx, &host, glyph);
    assert!(!asked.contains(&"start broom".to_owned()), "{asked:?}");
    assert!(shell.read_with(cx, |shell, _| shell.new_tab));
}

/// A browser with no broom, as one whose core can't make today's folders has: no glyph, no
/// item in the divider's menu or the Tabs menu, no bar entry, and the command run anyway asks
/// for no sort.
#[gpui::test]
fn a_browser_with_no_broom_offers_a_sort_by_no_door(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], None);
    assert!(node(cx, Role::Button, NAME).is_none());
    shell.update(cx, |shell, cx| {
        shell.divider_menu(gpui::point(px(100.), px(200.)), cx)
    });
    let titles: Vec<SharedString> = (host.entries.borrow().iter())
        .map(|entry| entry.title.clone())
        .collect();
    assert!(
        !titles.iter().any(|title| title.as_ref() == NAME),
        "{titles:?}"
    );
    assert!(!host.menu_bar.borrow().broom_offered);
    let command = Command::SortIntoFolders;
    assert!(!shell.read_with(cx, |shell, _| shell.offers(command)));
    host.take();
    shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
    cx.run_until_parked();
    assert!(!host.take().contains(&"start broom".to_owned()));
}

/// A screen reader hears the broom by what it does, there for it with no pointer, and as
/// unavailable under its other name while it sorts. With nothing to sort it hears none.
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
    assert!(node(cx, Role::Button, NAME).is_none());
    assert!(node(cx, Role::Button, SORTING).is_none());
}

/// A sort that changed nothing: its word stands in Clear's place while the browser says so, a
/// screen reader is told in a sentence with the product's name as it comes, the glyph is there and
/// sorts again, and a press where Clear was is neither Clear's nor the line's. Then Clear is back
/// where it was.
#[gpui::test]
fn a_failed_sort_says_so_in_clears_place(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    let clear = frame(&shell, cx, "clear");
    let ends = place(cx, Role::Button, "Clear Today").right() - px(CLEAR_HIT_OUT);
    assert!(node(cx, Role::Status, &failed_said()).is_none());

    answer(&shell, cx, &host, Some(Broom::Failed));
    assert!(node(cx, Role::Button, "Clear Today").is_none());
    let (_, word) = node(cx, Role::Status, &failed_said()).expect("the word");
    assert_eq!(word.value(), Some(failed_said().as_str()));
    assert_eq!(word.live(), Some(Live::Polite));
    assert!(failed_said().starts_with(PRODUCT));
    let (_, broom) = node(cx, Role::Button, NAME).expect("the glyph, back");
    assert!(!broom.is_disabled());
    // The word ends where Clear's did, from the line's gap after the glyph.
    let word = place(cx, Role::Status, &failed_said());
    let glyph = frame(&shell, cx, FRAME);
    assert!(close(word.left(), glyph.right() + px(DIVIDER_GAP)));
    assert!(close(word.right(), ends));
    assert!(click(cx, &host, clear).is_empty());

    answer(&shell, cx, &host, Some(Broom::Idle));
    assert!(
        node(cx, Role::Status, &failed_said()).is_none(),
        "said once"
    );
    assert_eq!(frame(&shell, cx, "clear"), clear);
    assert_eq!(click(cx, &host, clear), ["clear"]);
}

/// Under the pointer the glyph is lit, in both tones.
#[test]
fn the_glyph_lights_under_the_pointer() {
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        assert!(glyph_ink(&palette, true).a > glyph_ink(&palette, false).a);
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
/// the broom before it, and with a failed sort's word in Clear's place.
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

/// A Space beside the one shown is drawn with no broom, which can't know that Space's state. The
/// moment it is the one shown, its broom is what the browser answers for it.
#[gpui::test]
fn a_space_shows_its_own_brooms_state_the_moment_it_is_the_one_shown(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![10, 11], Some(Broom::Idle));
    shell.update(cx, |shell, cx| {
        let space = |id| (id, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space(1), space(2)], cx);
        shell.set_tabs(
            2,
            Vec::new(),
            vec![20, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    let (_, broom) = node(cx, Role::Button, NAME).expect("the first Space's");
    assert!(!broom.is_disabled());
    let glyph = frame(&shell, cx, FRAME);

    // The browser's answer is for the Space the window shows, and the other has nothing to sort.
    host.broom.set(Some(Broom::NothingToSort));
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.run_until_parked();
    assert!(node(cx, Role::Button, NAME).is_none(), "the second Space's");
    let asked = click(cx, &host, glyph);
    assert!(!asked.contains(&"start broom".to_owned()), "{asked:?}");
}
