//! The pictures as the user meets them: a click on a drawing, ← and → with the keyboard on them,
//! and what VoiceOver reads and presses, for Sidebar position's pair and Appearance's three.

use gpui::{
    Context, Entity, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext,
    accesskit::{Node, NodeId},
    point, size,
};

use super::*;
use crate::palette::{INK, PAPER};

struct Pair {
    right: bool,
    picks: Vec<bool>,
    logged: Rc<RefCell<Vec<String>>>,
    focus: FocusHandle,
}

impl Render for Pair {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pair = cx.entity().downgrade();
        div()
            .track_focus(&self.focus)
            .size_full()
            .p(px(20.))
            .child(picker(
                &Look::settings(false),
                rgb(INK),
                false,
                self.right,
                Buttons::HERE,
                window.is_a11y_active(),
                self.logged.clone(),
                move |side, _, cx| {
                    pair.update(cx, |pair, cx| {
                        pair.right = side;
                        pair.picks.push(side);
                        cx.notify();
                    })
                    .ok();
                },
            ))
    }
}

fn pair(right: bool, cx: &mut TestAppContext) -> (Entity<Pair>, &mut VisualTestContext) {
    let (pair, cx) = cx.add_window_view(|_, cx| Pair {
        right,
        picks: Vec::new(),
        logged: Rc::default(),
        focus: cx.focus_handle(),
    });
    cx.simulate_resize(size(px(400.), px(200.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = pair.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (pair, cx)
}

/// Where the drawing of `side` was drawn, its centre, as check.sh reads it.
fn at(pair: &Entity<Pair>, side: bool, cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let prefix = format!(
        "browser: settings sidebar position option {} at ",
        title(side)
    );
    let logged = pair.read_with(cx, |pair, _| pair.logged.borrow().clone());
    let line = (logged.iter())
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{} wasn't drawn: {logged:?}", title(side)));
    let (x, y) = line.split_once(' ').expect("x and y");
    point(px(x.parse().unwrap()), px(y.parse().unwrap()))
}

fn picks(pair: &Entity<Pair>, cx: &mut VisualTestContext) -> Vec<bool> {
    pair.read_with(cx, |pair, _| pair.picks.clone())
}

#[gpui::test]
fn a_click_on_the_other_drawing_picks_it_and_one_on_the_chosen_does_nothing(
    cx: &mut TestAppContext,
) {
    let (pair, cx) = pair(false, cx);
    let left = at(&pair, false, cx);
    cx.simulate_click(left, Modifiers::none());
    assert_eq!(
        picks(&pair, cx),
        [false; 0],
        "a click on the chosen one picked it again"
    );
    let right = at(&pair, true, cx);
    assert!(right.x > left.x, "Right isn't drawn right of Left");
    cx.simulate_click(right, Modifiers::none());
    assert_eq!(picks(&pair, cx), [true]);
    let left = at(&pair, false, cx);
    cx.simulate_click(left, Modifiers::none());
    assert_eq!(picks(&pair, cx), [true, false]);
}

/// ⇥ reaches the pair, and → and ← move the choice, each once, as a pair of radio buttons'
/// arrows do; with ⌘ or another key held they're the window's.
#[gpui::test]
fn arrows_move_the_choice_once_the_pair_has_the_keyboard(cx: &mut TestAppContext) {
    let (pair, cx) = pair(false, cx);
    cx.simulate_keystrokes("right");
    assert_eq!(
        picks(&pair, cx),
        [false; 0],
        "→ picked with the pair not focused"
    );
    cx.update(|window, cx| window.focus_next(cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("left");
    assert_eq!(
        picks(&pair, cx),
        [false; 0],
        "← picked Left, which was chosen"
    );
    cx.simulate_keystrokes("cmd-right alt-right");
    assert_eq!(
        picks(&pair, cx),
        [false; 0],
        "a held key's → moved the choice"
    );
    cx.simulate_keystrokes("right");
    assert_eq!(picks(&pair, cx), [true]);
    cx.simulate_keystrokes("right");
    assert_eq!(picks(&pair, cx), [true], "→ past Right picked again");
    cx.simulate_keystrokes("left");
    assert_eq!(picks(&pair, cx), [true, false]);
}

/// The node with `role` named `label` in what the pair last told VoiceOver.
fn named(cx: &mut VisualTestContext, role: Role, label: &str) -> (NodeId, Node) {
    let tree = cx.a11y_tree().expect("no accessibility tree");
    let found =
        (tree.nodes.iter()).find(|(_, node)| node.role() == role && node.label() == Some(label));
    found
        .cloned()
        .unwrap_or_else(|| panic!("no {role:?} named {label}"))
}

#[gpui::test]
fn voiceover_reads_a_pair_of_options_with_one_chosen_and_presses_the_other(
    cx: &mut TestAppContext,
) {
    let (pair, cx) = pair(true, cx);
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    named(cx, Role::RadioGroup, "Sidebar position");
    let (left, node) = named(cx, Role::RadioButton, "Left");
    assert_eq!(node.toggled(), Some(Toggled::False));
    let (_, node) = named(cx, Role::RadioButton, "Right");
    assert_eq!(node.toggled(), Some(Toggled::True));
    cx.simulate_a11y_action(left, Action::Click);
    cx.run_until_parked();
    assert_eq!(picks(&pair, cx), [false]);
    let (_, node) = named(cx, Role::RadioButton, "Left");
    assert_eq!(node.toggled(), Some(Toggled::True));
}

struct Appearances {
    chosen: Appearance,
    picks: Vec<Appearance>,
    logged: Rc<RefCell<Vec<String>>>,
    focus: FocusHandle,
}

impl Render for Appearances {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        div()
            .track_focus(&self.focus)
            .size_full()
            .p(px(20.))
            .child(appearance_picker(
                &Look::settings(false),
                rgb(INK),
                self.chosen,
                Buttons::HERE,
                window.is_a11y_active(),
                self.logged.clone(),
                move |appearance, _, cx| {
                    view.update(cx, |view, cx| {
                        view.chosen = appearance;
                        view.picks.push(appearance);
                        cx.notify();
                    })
                    .ok();
                },
            ))
    }
}

fn appearances(
    chosen: Appearance,
    cx: &mut TestAppContext,
) -> (Entity<Appearances>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Appearances {
        chosen,
        picks: Vec::new(),
        logged: Rc::default(),
        focus: cx.focus_handle(),
    });
    cx.simulate_resize(size(px(600.), px(200.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// Where `appearance`'s drawing was drawn, its centre, as check.sh reads it.
fn drawn_at(
    view: &Entity<Appearances>,
    appearance: Appearance,
    cx: &mut VisualTestContext,
) -> gpui::Point<gpui::Pixels> {
    let prefix = format!(
        "browser: settings appearance option {} at ",
        appearance_title(appearance)
    );
    let logged = view.read_with(cx, |view, _| view.logged.borrow().clone());
    let line = (logged.iter())
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{appearance:?} wasn't drawn: {logged:?}"));
    let (x, y) = line.split_once(' ').expect("x and y");
    point(px(x.parse().unwrap()), px(y.parse().unwrap()))
}

fn picked(view: &Entity<Appearances>, cx: &mut VisualTestContext) -> Vec<Appearance> {
    view.read_with(cx, |view, _| view.picks.clone())
}

#[gpui::test]
fn appearance_draws_automatic_light_and_dark_in_a_row_and_a_click_picks_one(
    cx: &mut TestAppContext,
) {
    let (view, cx) = appearances(Appearance::Automatic, cx);
    let [automatic, light, dark] = APPEARANCES.map(|each| drawn_at(&view, each, cx));
    assert!(
        automatic.x < light.x && light.x < dark.x && automatic.y == dark.y,
        "not Automatic, Light, Dark in a row: {automatic:?} {light:?} {dark:?}"
    );
    cx.simulate_click(automatic, Modifiers::none());
    assert_eq!(
        picked(&view, cx),
        [],
        "a click on the chosen one picked it again"
    );
    cx.simulate_click(dark, Modifiers::none());
    assert_eq!(picked(&view, cx), [Appearance::Dark]);
    let light = drawn_at(&view, Appearance::Light, cx);
    cx.simulate_click(light, Modifiers::none());
    assert_eq!(picked(&view, cx), [Appearance::Dark, Appearance::Light]);
}

/// → walks Automatic, Light, Dark and stops there, and ← walks back, as three radio buttons'
/// arrows do.
#[gpui::test]
fn arrows_walk_the_three_and_stop_at_the_ends(cx: &mut TestAppContext) {
    let (view, cx) = appearances(Appearance::Automatic, cx);
    cx.update(|window, cx| window.focus_next(cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("left");
    assert_eq!(picked(&view, cx), [], "← before Automatic picked");
    cx.simulate_keystrokes("right right right");
    assert_eq!(picked(&view, cx), [Appearance::Light, Appearance::Dark]);
    cx.simulate_keystrokes("cmd-left left left");
    assert_eq!(
        picked(&view, cx),
        [
            Appearance::Light,
            Appearance::Dark,
            Appearance::Light,
            Appearance::Automatic
        ]
    );
}

#[gpui::test]
fn voiceover_reads_appearance_as_three_options_with_one_chosen(cx: &mut TestAppContext) {
    let (view, cx) = appearances(Appearance::Light, cx);
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    named(cx, Role::RadioGroup, "Appearance");
    for each in APPEARANCES {
        let (_, node) = named(cx, Role::RadioButton, appearance_title(each));
        let chosen = if each == Appearance::Light {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(node.toggled(), Some(chosen), "{each:?}");
    }
    let (dark, _) = named(cx, Role::RadioButton, "Dark");
    cx.simulate_a11y_action(dark, Action::Click);
    cx.run_until_parked();
    assert_eq!(picked(&view, cx), [Appearance::Dark]);
}

/// Who shares reads without the colours: each list's marks have a shape no other list's has, and
/// under each scope the Spaces that share show one list and those that don't show different ones.
#[test]
fn share_favorites_tells_who_shares_by_the_marks_shape_too() {
    let shapes = LISTS.map(|(_, mark)| mark);
    assert!(
        shapes[0] != shapes[1] && shapes[1] != shapes[2] && shapes[0] != shapes[2],
        "two lists share a shape: {shapes:?}"
    );
    // The first two Spaces are one profile's, the third another's.
    let shared = |scope| {
        let [a, b, c] = lists(scope);
        (a == b, b == c)
    };
    assert_eq!(shared(FavoritesScope::Every), (true, true));
    assert_eq!(shared(FavoritesScope::Profile), (true, false));
    assert_eq!(shared(FavoritesScope::Space), (false, false));
}

struct Shared {
    chosen: FavoritesScope,
    picks: Vec<FavoritesScope>,
    logged: Rc<RefCell<Vec<String>>>,
}

impl Render for Shared {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        div().size_full().p(px(20.)).child(favorites_picker(
            &Look::settings(true),
            rgb(PAPER),
            true,
            self.chosen,
            window.is_a11y_active(),
            self.logged.clone(),
            move |scope, _, cx| {
                view.update(cx, |view, cx| {
                    view.chosen = scope;
                    view.picks.push(scope);
                    cx.notify();
                })
                .ok();
            },
        ))
    }
}

#[gpui::test]
fn share_favorites_draws_its_three_scopes_and_a_click_or_voiceover_picks_one(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, _| Shared {
        chosen: FavoritesScope::Profile,
        picks: Vec::new(),
        logged: Rc::default(),
    });
    cx.simulate_resize(size(px(600.), px(200.)));
    cx.run_until_parked();
    let at = |scope: FavoritesScope, cx: &mut VisualTestContext| {
        let prefix = format!(
            "browser: settings share favorites option {} at ",
            scope.caption()
        );
        let logged = view.read_with(cx, |view, _| view.logged.borrow().clone());
        let line = (logged.iter())
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap_or_else(|| panic!("{scope:?} wasn't drawn: {logged:?}"));
        let (x, y) = line.split_once(' ').expect("x and y");
        point(px(x.parse::<f32>().unwrap()), px(y.parse::<f32>().unwrap()))
    };
    let [every, profile, space] = FavoritesScope::ALL.map(|scope| at(scope, cx));
    assert!(
        every.x < profile.x && profile.x < space.x,
        "not in the menu's order"
    );
    cx.simulate_click(space, Modifiers::none());
    let picks = view.read_with(cx, |view, _| view.picks.clone());
    assert_eq!(picks, [FavoritesScope::Space]);
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    named(cx, Role::RadioGroup, "Share favorites");
    let (_, node) = named(cx, Role::RadioButton, "In each Space");
    assert_eq!(node.toggled(), Some(Toggled::True));
    let (every, _) = named(cx, Role::RadioButton, "In every Space");
    cx.simulate_a11y_action(every, Action::Click);
    cx.run_until_parked();
    let picks = view.read_with(cx, |view, _| view.picks.clone());
    assert_eq!(picks, [FavoritesScope::Space, FavoritesScope::Every]);
}
