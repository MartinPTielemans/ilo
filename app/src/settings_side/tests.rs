//! The pair as the user meets it: a click on a drawing, ← and → with the keyboard on it, and what
//! VoiceOver reads and presses.

use gpui::{
    Context, Entity, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext,
    accesskit::{Node, NodeId},
    point, size,
};

use super::*;
use crate::palette::INK;

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
