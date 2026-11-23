//! The picker as the user meets it: it shows the level it's given, a click on a picture, ← and →
//! with the keyboard on it and VoiceOver's press each pick the right level, and the line under it
//! says what the chosen one does.

use gpui::{
    Context, Entity, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext,
    accesskit::{Action, Node, NodeId, Role, Toggled},
    point, size,
};

use super::*;
use crate::tokens::settings::SIDE_GAP;

/// Where the picker sits in the test's window.
const INSET: f32 = 20.;
const WIDTH: f32 = 640.;

struct Shown {
    chosen: Transparency,
    picks: Vec<Transparency>,
    focus: FocusHandle,
}

impl Render for Shown {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        div()
            .track_focus(&self.focus)
            .size_full()
            .p(px(INSET))
            .child(picker(
                self.chosen,
                &Look::settings(true),
                Rc::default(),
                move |level, _, cx| {
                    view.update(cx, |view, cx| {
                        view.chosen = level;
                        view.picks.push(level);
                        cx.notify();
                    })
                    .ok();
                },
            ))
    }
}

fn shown(chosen: Transparency, cx: &mut TestAppContext) -> (Entity<Shown>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Shown {
        chosen,
        picks: Vec::new(),
        focus: cx.focus_handle(),
    });
    cx.simulate_resize(size(px(WIDTH), px(400.)));
    cx.update(|window, cx| {
        window.activate_window();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    (view, cx)
}

fn picks(view: &Entity<Shown>, cx: &mut VisualTestContext) -> Vec<Transparency> {
    view.read_with(cx, |view, _| view.picks.clone())
}

/// The middle of `level`'s picture: three alike, side by side across the picker, inside its room.
fn picture_at(level: Transparency) -> gpui::Point<gpui::Pixels> {
    let across = WIDTH - 2. * INSET - 2. * SIDE_ROOM;
    let each = (across - 2. * SIDE_GAP) / 3.;
    let x = INSET + SIDE_ROOM + level.index() as f32 * (each + SIDE_GAP) + each / 2.;
    let y = INSET + SIDE_ROOM + HEIGHT / 2.;
    point(px(x), px(y))
}

/// The node with `role` named `label` in what the picker last told VoiceOver.
fn named(cx: &mut VisualTestContext, role: Role, label: &str) -> (NodeId, Node) {
    let tree = cx.a11y_tree().expect("no accessibility tree");
    let found =
        (tree.nodes.iter()).find(|(_, node)| node.role() == role && node.label() == Some(label));
    found
        .cloned()
        .unwrap_or_else(|| panic!("no {role:?} named {label}"))
}

/// Which level the picker shows chosen, and the line it shows under them.
fn showing(cx: &mut VisualTestContext) -> (Vec<Transparency>, String) {
    let chosen = Transparency::ALL
        .into_iter()
        .filter(|level| {
            named(cx, Role::RadioButton, level.title()).1.toggled() == Some(Toggled::True)
        })
        .collect();
    let (_, group) = named(cx, Role::RadioGroup, "Transparency");
    (chosen, group.description().unwrap_or_default().to_string())
}

#[gpui::test]
fn it_shows_the_level_it_is_given_and_what_that_level_does(cx: &mut TestAppContext) {
    for level in Transparency::ALL {
        let (_, cx) = shown(level, cx);
        assert_eq!(showing(cx), (vec![level], level.line().to_string()));
    }
}

#[gpui::test]
fn a_click_on_each_picture_picks_its_level_and_the_line_follows(cx: &mut TestAppContext) {
    let (view, cx) = shown(Transparency::Standard, cx);
    cx.simulate_click(picture_at(Transparency::Standard), Modifiers::none());
    assert_eq!(
        picks(&view, cx),
        [],
        "a click on the chosen one picked it again"
    );
    for level in [
        Transparency::More,
        Transparency::Less,
        Transparency::Standard,
    ] {
        cx.simulate_click(picture_at(level), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(picks(&view, cx).last(), Some(&level));
        assert_eq!(showing(cx), (vec![level], level.line().to_string()));
    }
    assert_eq!(picks(&view, cx).len(), 3);
}

/// ⇥ reaches the three, and → and ← move the choice a level at a time, stopping at either end, as
/// radio buttons' arrows do; with ⌘ or another key held they're the window's.
#[gpui::test]
fn arrows_move_the_choice_once_the_picker_has_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = shown(Transparency::Standard, cx);
    cx.simulate_keystrokes("right");
    assert_eq!(picks(&view, cx), [], "→ picked with the picker not focused");
    cx.update(|window, cx| window.focus_next(cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-right alt-left");
    assert_eq!(picks(&view, cx), [], "a held key's arrow moved the choice");
    cx.simulate_keystrokes("right");
    assert_eq!(picks(&view, cx), [Transparency::More]);
    cx.simulate_keystrokes("right");
    assert_eq!(picks(&view, cx), [Transparency::More], "→ past More picked");
    cx.simulate_keystrokes("left left");
    assert_eq!(
        picks(&view, cx),
        [
            Transparency::More,
            Transparency::Standard,
            Transparency::Less
        ]
    );
    cx.simulate_keystrokes("left");
    assert_eq!(picks(&view, cx).len(), 3, "← past Less picked");
    assert_eq!(
        showing(cx),
        (
            vec![Transparency::Less],
            Transparency::Less.line().to_string()
        )
    );
}

#[gpui::test]
fn voiceover_presses_a_level(cx: &mut TestAppContext) {
    let (view, cx) = shown(Transparency::Less, cx);
    let (more, _) = named(cx, Role::RadioButton, "More");
    cx.simulate_a11y_action(more, Action::Click);
    cx.run_until_parked();
    assert_eq!(picks(&view, cx), [Transparency::More]);
    assert_eq!(showing(cx).0, [Transparency::More]);
}

/// What's kept reads back as the level it was, and nothing else reads as one.
#[test]
fn a_level_reads_back_from_its_key() {
    for level in Transparency::ALL {
        assert_eq!(Transparency::named(level.key()), Some(level));
    }
    assert_eq!(Transparency::named(""), None);
}

/// Each level lets more through than the one before it: the window's tint and what floats fall
/// from Less to More, in both looks.
#[test]
fn each_level_lets_more_through_than_the_last() {
    for dark in [true, false] {
        let tints = Transparency::ALL.map(|level| level.tint(dark));
        assert!(tints[0] > tints[1] && tints[1] > tints[2], "{tints:?}");
    }
    let fills = Transparency::ALL.map(Transparency::overlay_fill);
    assert!(fills[0] > fills[1] && fills[1] > fills[2], "{fills:?}");
}

/// The drawn material over white and over black gives the colours it was measured at.
#[test]
fn the_drawn_material_gives_its_measured_colours() {
    for (dark, white, black) in [
        (true, MATERIAL_OVER_WHITE, MATERIAL_OVER_BLACK),
        (false, LIGHT_MATERIAL_OVER_WHITE, LIGHT_MATERIAL_OVER_BLACK),
    ] {
        let fill = material(dark);
        for (under, measured) in [(1., white), (0., black)] {
            let got = under * (1. - fill.a) + fill.r * fill.a;
            assert!(
                (got - rgb(measured).r).abs() < 0.002,
                "{dark} {under}: {got}"
            );
        }
    }
}

/// Settings' row in a window of its own, the level kept in a folder of the test's.
struct Settings {
    glass: bool,
}

impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = crate::settings_rows::Look::settings(true);
        div()
            .size_full()
            .p(px(INSET))
            .children(row(&look, self.glass, Rc::default(), cx))
    }
}

/// A folder of the test's own, empty, for the level to be kept in.
fn folder(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("ilo-transparency-{}-{name}", std::process::id()));
    _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().into_owned()
}

fn settings<'a>(dir: &str, glass: bool, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    let dir = dir.to_string();
    cx.update(|cx| keep_in(dir, cx));
    let (_, cx) = cx.add_window_view(|_, _| Settings { glass });
    cx.simulate_resize(size(px(WIDTH), px(400.)));
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    cx
}

#[gpui::test]
fn the_row_shows_the_stored_level(cx: &mut TestAppContext) {
    let dir = folder("shows");
    std::fs::write(format!("{dir}/transparency"), "more").unwrap();
    let cx = settings(&dir, true, cx);
    assert_eq!(showing(cx).0, [Transparency::More]);
}

#[gpui::test]
fn a_pick_in_the_row_is_kept_and_shown(cx: &mut TestAppContext) {
    let dir = folder("keeps");
    let cx = settings(&dir, true, cx);
    assert_eq!(
        showing(cx).0,
        [Transparency::Standard],
        "none kept is Standard"
    );
    for level in [
        Transparency::Less,
        Transparency::More,
        Transparency::Standard,
    ] {
        let (id, _) = named(cx, Role::RadioButton, level.title());
        cx.simulate_a11y_action(id, Action::Click);
        cx.run_until_parked();
        assert_eq!(stored_in(&dir), level);
        assert_eq!(showing(cx), (vec![level], level.line().to_string()));
        assert_eq!(cx.update(|_, cx| super::level(cx)), level);
    }
    assert!(
        !std::path::Path::new(&format!("{dir}/transparency")).exists(),
        "Standard left a file"
    );
}

#[gpui::test]
fn the_row_is_absent_where_the_host_has_no_glass(cx: &mut TestAppContext) {
    let dir = folder("absent");
    let cx = settings(&dir, false, cx);
    let tree = cx.a11y_tree().expect("no accessibility tree");
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some(NAME)),
        "a Transparency control was drawn"
    );
}
