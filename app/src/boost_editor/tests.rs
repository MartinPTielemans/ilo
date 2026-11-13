//! The Boost editor's presses, drags and keys, on a host that records what it asks of its window
//! and what it asks of the Boost.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, PlatformInput, TestAppContext, VisualTestContext, point, px};

use super::*;
use crate::tokens::theme_picker::ARROW;

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    requests: RefCell<Vec<Request>>,
}

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32, _: &[(f32, f32, f32)]) {
        self.calls
            .borrow_mut()
            .push(format!("show {left:.1} {top:.1} over {window}"));
    }

    fn resize(&self, height: f32, corners: &[(f32, f32, f32)]) {
        // The card's bottom corners are at its height.
        let bottom = corners.iter().map(|corner| corner.1).fold(0., f32::max);
        self.calls
            .borrow_mut()
            .push(format!("resize {height:.0} bottom {bottom:.0}"));
    }

    fn hide(&self) {
        self.calls.borrow_mut().push("hide".into());
    }
}

/// The editor, shown for news.example from browser window 2, its arrow at (240, 60) in a window
/// 800 tall.
struct Editor {
    cx: VisualTestContext,
    window: WindowHandle<BoostEditor>,
    host: Rc<Recorder>,
}

impl Editor {
    fn new(style: Style, cx: &mut TestAppContext) -> Self {
        cx.update(crate::text_input::bind_keys);
        let host = Rc::new(Recorder::default());
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for the editor");
        let recorded = host.clone();
        let requested: Requested =
            Rc::new(move |request, _| recorded.requests.borrow_mut().push(request));
        let at = At::Row(point(240., 60.));
        cx.update(|cx| {
            let palette = Palette::default();
            let site = "news.example".to_string();
            show(
                2, site, None, style, true, at, false, 800., palette, requested, cx,
            )
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        // The host makes its window the key one as it shows it.
        (window.update(&mut cx.cx, |_, window, _| window.activate_window())).unwrap();
        cx.run_until_parked();
        Editor { cx, window, host }
    }

    fn at(x: f32, y: f32) -> Point<Pixels> {
        point(px(ARROW + x), px(y))
    }

    fn click(&mut self, (x, y): (f32, f32)) {
        self.cx.simulate_click(Self::at(x, y), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        let left = MouseButton::Left;
        self.cx
            .simulate_mouse_down(Self::at(from.0, from.1), left, Modifiers::none());
        self.cx.run_until_parked();
        self.cx
            .simulate_mouse_move(Self::at(to.0, to.1), left, Modifiers::none());
        self.cx.run_until_parked();
        self.cx
            .simulate_mouse_up(Self::at(to.0, to.1), left, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// A drag whose press and first move come in one frame, as a quick flick's or the driver's
    /// do, with the editor drawn only before it's let go (#2605).
    fn quick_drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        let (from, to) = (Self::at(from.0, from.1), Self::at(to.0, to.1));
        self.cx.update(|window, cx| {
            let down = MouseDownEvent {
                position: from,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            };
            window.dispatch_event(PlatformInput::MouseDown(down), cx);
            let moved = MouseMoveEvent {
                position: to,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            };
            window.dispatch_event(PlatformInput::MouseMove(moved), cx);
        });
        self.cx.run_until_parked();
        self.cx
            .simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn style(&mut self) -> Style {
        self.window
            .read_with(&self.cx.cx, |editor, _| {
                editor.editing.as_ref().map(|editing| editing.style.clone())
            })
            .unwrap()
            .expect("not editing")
    }

    fn menu(&mut self) -> bool {
        self.window
            .read_with(&self.cx.cx, |editor, _| {
                editor.editing.as_ref().is_some_and(|editing| editing.menu)
            })
            .unwrap()
    }

    fn open(&mut self) -> bool {
        self.cx.cx.update(|cx| is_open(cx))
    }

    /// What it asked of the Boost since the last look.
    fn requests(&mut self) -> Vec<Request> {
        self.host.requests.borrow_mut().drain(..).collect()
    }

    fn calls(&mut self) -> Vec<String> {
        self.host.calls.borrow().clone()
    }
}

/// Where the middle of what `wanted` is lies in the card, found by its target.
fn find(wanted: Target, style: &Style, menu: bool) -> (f32, f32) {
    for y in (0..height_of(style) as i32).map(|y| y as f32 + 0.5) {
        let xs: Vec<f32> = (0..CARD_WIDTH as i32)
            .map(|x| x as f32 + 0.5)
            .filter(|&x| target(x, y, style, menu) == Some(wanted))
            .collect();
        if let (Some(first), Some(last)) = (xs.first(), xs.last()) {
            return ((first + last) / 2., y + 4.);
        }
    }
    panic!("nothing in the card is {wanted:?}");
}

#[test]
fn each_control_is_where_it_is_drawn() {
    let style = Style::default();
    let (shuffle, switch) = header_buttons();
    let header = HEADER / 2.;
    assert_eq!(
        target(switch + 1., header, &style, false),
        Some(Target::Switch)
    );
    assert_eq!(
        target(shuffle + 1., header, &style, false),
        Some(Target::Shuffle)
    );
    assert_eq!(
        target(NAME_LEFT + 4., header, &style, false),
        Some(Target::Name)
    );
    assert_eq!(
        target(INSET + 4., header, &style, false),
        None,
        "the site's icon took a press for the name"
    );
    let (gx, gy) = in_card(REST[0]);
    assert_eq!(target(gx, gy, &style, false), Some(Target::Dot(0)));
    let (lx, ly) = in_card(REST[1]);
    assert_eq!(target(lx, ly, &style, false), Some(Target::Dot(1)));
    let corner = (FIELD_INSET + 4., FIELD_TOP + 4.);
    assert_eq!(
        target(corner.0, corner.1, &style, false),
        Some(Target::Field)
    );
    assert_eq!(
        target(INSET + 4., INVERT_Y, &style, false),
        Some(Target::Invert)
    );
    assert_eq!(
        target(CONTROL_X + 4., SIZE_Y, &style, false),
        Some(Target::Smaller)
    );
    assert_eq!(
        target(segment_x(2) + 4., CASE_Y, &style, false),
        Some(Target::Case(Some(Case::Upper)))
    );
    assert_eq!(
        target(INSET + 4., FOOTER_Y, &style, false),
        Some(Target::EditCss)
    );
    // Between the lines, nothing.
    assert_eq!(target(CARD_WIDTH - INSET - 2., SIZE_Y, &style, false), None);
}

#[test]
fn only_the_faces_this_computer_has_are_offered() {
    let faces = faces();
    assert_eq!(faces[0], None, "the site's own comes first");
    assert_eq!(
        faces.contains(&Some(Face::Rounded)),
        cfg!(target_os = "macos")
    );
    let style = Style::default();
    for face in &faces {
        find(Target::Face(*face), &style, false);
    }
    // A face with no family of its own here draws its sample in the UI's.
    let families = families(&["DejaVu Serif".to_string()]);
    let serif = families.iter().find(|(face, _)| *face == Some(Face::Serif));
    assert_eq!(
        serif.map(|(_, family)| family.as_ref()),
        Some("DejaVu Serif")
    );
    let mono = families.iter().find(|(face, _)| *face == Some(Face::Mono));
    assert_eq!(
        mono.map(|(_, family)| family.as_ref()),
        Some(crate::face::UI)
    );
}

#[gpui::test]
fn it_shows_over_the_window_that_asked(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    let calls = editor.calls();
    assert_eq!(calls[0], "resize 496 bottom 496", "{calls:?}");
    assert!(calls[1].ends_with(" over 2"), "{calls:?}");
    assert!(editor.open());
}

#[gpui::test]
fn a_drag_shows_as_it_goes_and_is_kept_as_it_ends(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    let from = in_card(REST[0]);
    let to = in_card(Dot { x: 0.5, y: 0.5 });
    editor.drag(from, to);
    let kept: Vec<bool> = editor
        .requests()
        .into_iter()
        .map(|request| match request {
            Request::Style(_, kept) => kept,
            other => panic!("asked {other:?}"),
        })
        .collect();
    assert!(
        kept.len() >= 2 && !kept[0] && kept[kept.len() - 1],
        "{kept:?}"
    );
    let style = editor.style();
    let ground = style.ground.expect("no ground");
    assert!(
        (ground.x - 0.5).abs() < 0.02 && (ground.y - 0.5).abs() < 0.02,
        "{ground:?}"
    );
    assert!(style.dragged, "the hint stays after the first drag");
    assert_eq!(style.links, None, "the links' dot wasn't placed");
}

/// A dot and a slider still move when the drag's first move comes before the editor has drawn
/// the press (#2605).
#[gpui::test]
fn a_drag_that_moves_before_the_press_is_drawn_still_moves(cx: &mut TestAppContext) {
    let open = Style {
        advanced: true,
        ..Style::default()
    };
    let mut editor = Editor::new(open, cx);
    editor.quick_drag(in_card(REST[0]), in_card(Dot { x: 0.5, y: 0.5 }));
    let ground = editor
        .style()
        .ground
        .expect("the dot dragged left no ground");
    assert!(
        (ground.x - 0.5).abs() < 0.02 && (ground.y - 0.5).abs() < 0.02,
        "{ground:?}"
    );
    let y = slider_y(Adjust::Contrast);
    let middle = (TRACK.0 + TRACK.1) / 2.;
    editor.quick_drag((middle, y), (TRACK.0 + (TRACK.1 - TRACK.0) * 0.76, y));
    assert_eq!(editor.style().adjust[0], 125);
}

#[gpui::test]
fn command_z_takes_each_change_back_and_the_hint_stays_gone(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    editor.drag(in_card(REST[1]), in_card(Dot { x: -0.5, y: 0. }));
    editor.click((INSET + 4., INVERT_Y));
    assert!(editor.style().invert);
    editor.keys("cmd-z");
    let style = editor.style();
    assert!(!style.invert && style.links.is_some(), "{style:?}");
    editor.keys("cmd-z");
    let style = editor.style();
    assert_eq!(style.links, None);
    assert!(style.dragged);
    assert!(matches!(
        editor.requests().last(),
        Some(Request::Style(_, true))
    ));
}

#[gpui::test]
fn the_size_steps_within_its_range(cx: &mut TestAppContext) {
    let style = Style {
        size: LARGEST - SIZE_STEP,
        ..Style::default()
    };
    let mut editor = Editor::new(style, cx);
    let larger = find(Target::Larger, &editor.style(), false);
    editor.click(larger);
    editor.click(larger);
    assert_eq!(editor.style().size, LARGEST);
    let smaller = find(Target::Smaller, &editor.style(), false);
    editor.click(smaller);
    assert_eq!(editor.style().size, LARGEST - SIZE_STEP);
}

#[gpui::test]
fn a_face_and_a_case_are_kept(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    editor.click(find(
        Target::Face(Some(Face::Mono)),
        &Style::default(),
        false,
    ));
    editor.click(find(
        Target::Case(Some(Case::Lower)),
        &Style::default(),
        false,
    ));
    let style = editor.style();
    assert_eq!(
        (style.face, style.case),
        (Some(Face::Mono), Some(Case::Lower))
    );
    editor.click(find(Target::Face(None), &Style::default(), false));
    assert_eq!(editor.style().face, None);
}

#[gpui::test]
fn escape_closes_the_menu_and_then_the_editor(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    editor.click((NAME_LEFT + 4., HEADER / 2.));
    assert!(editor.menu());
    editor.keys("escape");
    assert!(!editor.menu() && editor.open());
    editor.keys("escape");
    assert!(!editor.open());
    assert_eq!(editor.calls().last().map(String::as_str), Some("hide"));
}

#[gpui::test]
fn reset_takes_every_edit_away_but_the_name(cx: &mut TestAppContext) {
    let style = Style {
        invert: true,
        size: 120,
        name: Some("Quiet news".into()),
        dragged: true,
        ..Style::default()
    };
    let mut editor = Editor::new(style, cx);
    editor.click((NAME_LEFT + 4., HEADER / 2.));
    editor.click(find(Target::Item(Item::Reset), &Style::default(), true));
    let style = editor.style();
    assert!(!style.invert && style.size == 100, "{style:?}");
    assert_eq!(style.name.as_deref(), Some("Quiet news"));
    editor.keys("cmd-z");
    assert!(editor.style().invert);
}

#[gpui::test]
fn rename_keeps_what_is_typed_and_escape_leaves_the_name(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    editor.click((NAME_LEFT + 4., HEADER / 2.));
    editor.click(find(Target::Item(Item::Rename), &Style::default(), true));
    editor.cx.simulate_input("Quiet news");
    editor.keys("enter");
    assert_eq!(editor.style().name.as_deref(), Some("Quiet news"));
    assert!(editor.open(), "enter in the field closes only the field");

    editor.click((NAME_LEFT + 4., HEADER / 2.));
    editor.click(find(Target::Item(Item::Rename), &Style::default(), true));
    editor.cx.simulate_input("Other");
    editor.keys("escape");
    assert_eq!(editor.style().name.as_deref(), Some("Quiet news"));
    assert!(editor.open());
}

#[gpui::test]
fn edit_css_edit_script_zap_and_delete_hand_over_to_the_window(cx: &mut TestAppContext) {
    for (wanted, request) in [
        (Target::EditCss, Request::EditCss),
        (Target::EditScript, Request::EditScript),
        (Target::Zap, Request::Zap),
    ] {
        let mut editor = Editor::new(Style::default(), cx);
        editor.click(find(wanted, &Style::default(), false));
        assert_eq!(editor.requests(), vec![request]);
        assert!(!editor.open());
    }
    let mut editor = Editor::new(Style::default(), cx);
    editor.click((NAME_LEFT + 4., HEADER / 2.));
    editor.click(find(Target::Item(Item::Delete), &Style::default(), true));
    assert_eq!(editor.requests(), vec![Request::Delete]);
    assert!(!editor.open());
}

#[gpui::test]
fn the_switch_turns_the_boost_off_and_on(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    let switch = find(Target::Switch, &Style::default(), false);
    editor.click(switch);
    editor.click(switch);
    assert_eq!(
        editor.requests(),
        vec![Request::On(false), Request::On(true)]
    );
}

/// Advanced's chip is at Invert lightness's line's end, open or closed; Reset and the sliders take
/// presses only while it's open, and what's under them moves down by as much as the card grows.
#[test]
fn advanced_moves_what_is_under_it() {
    let closed = Style::default();
    let open = Style {
        advanced: true,
        ..Style::default()
    };
    for style in [&closed, &open] {
        assert_eq!(
            target(ADVANCED_X + 4., INVERT_Y, style, false),
            Some(Target::Advanced)
        );
        assert_eq!(
            target(CONTROL_RIGHT - 1., INVERT_Y, style, false),
            Some(Target::Advanced)
        );
    }
    let reset = (RESET_X + 4., INVERT_Y);
    assert_eq!(target(reset.0, reset.1, &closed, false), None);
    assert_eq!(
        target(reset.0, reset.1, &open, false),
        Some(Target::ResetAdvanced)
    );
    for adjust in Adjust::ALL {
        let y = slider_y(adjust);
        assert_eq!(
            target(TRACK.0, y, &open, false),
            Some(Target::Slider(adjust))
        );
        assert_eq!(
            target(TRACK.1, y, &open, false),
            Some(Target::Slider(adjust))
        );
        assert_ne!(
            target(TRACK.0, y, &closed, false),
            Some(Target::Slider(adjust))
        );
    }
    // The track starts where Size's stepper and Case's segment do.
    assert_eq!(TRACK.0, CONTROL_X);
    assert_eq!(height_of(&closed), HEIGHT);
    assert_eq!(height_of(&open), HEIGHT + ADVANCED_HEIGHT);
    let shift = ADVANCED_HEIGHT;
    for (x, y, wanted) in [
        (CONTROL_X + 4., SIZE_Y, Target::Smaller),
        (segment_x(2) + 4., CASE_Y, Target::Case(Some(Case::Upper))),
        (INSET + 4., FOOTER_Y, Target::EditCss),
    ] {
        assert_eq!(target(x, y, &closed, false), Some(wanted));
        assert_eq!(target(x, y + shift, &open, false), Some(wanted));
    }
    // The last slider's line ends above the rule over the faces.
    let last = slider_y(Adjust::Saturation) + SLIDER_PITCH / 2.;
    assert!(last <= RULES[0] + shift, "{last}");
}

/// Advanced opens and closes the card's lower part, the host making the card as tall, and it's
/// kept with the Boost without being an edit ⌘Z takes back.
#[gpui::test]
fn advanced_opens_and_closes_as_the_boost_keeps_it(cx: &mut TestAppContext) {
    let mut editor = Editor::new(Style::default(), cx);
    let chip = (ADVANCED_X + 4., INVERT_Y);
    editor.click(chip);
    assert!(editor.style().advanced);
    let calls = editor.calls();
    assert_eq!(
        calls.last().map(String::as_str),
        Some("resize 600 bottom 600"),
        "{calls:?}"
    );
    assert!(matches!(
        editor.requests().last(),
        Some(Request::Style(style, true)) if style.advanced
    ));
    editor.keys("cmd-z");
    assert!(editor.style().advanced, "⌘Z closed Advanced");
    editor.click(chip);
    assert!(!editor.style().advanced);
    let calls = editor.calls();
    assert_eq!(
        calls.last().map(String::as_str),
        Some("resize 496 bottom 496")
    );
}

/// A Boost left with Advanced open shows it open, the card as tall before it shows.
#[gpui::test]
fn advanced_shows_as_it_was_left(cx: &mut TestAppContext) {
    let style = Style {
        advanced: true,
        ..Style::default()
    };
    let mut editor = Editor::new(style, cx);
    let calls = editor.calls();
    assert_eq!(calls[0], "resize 600 bottom 600", "{calls:?}");
}

/// A slider's drag shows as it goes and is kept as it ends, on its step, from 100 % in the middle.
#[gpui::test]
fn a_slider_drag_is_kept_on_its_step(cx: &mut TestAppContext) {
    let open = Style {
        advanced: true,
        ..Style::default()
    };
    let mut editor = Editor::new(open, cx);
    let y = slider_y(Adjust::Contrast);
    let middle = (TRACK.0 + TRACK.1) / 2.;
    assert_eq!(value_at(Adjust::Contrast, middle), 100);
    let quarter = TRACK.0 + (TRACK.1 - TRACK.0) * 0.76;
    editor.drag((middle, y), (quarter, y));
    let kept: Vec<bool> = editor
        .requests()
        .into_iter()
        .filter_map(|request| match request {
            Request::Style(_, kept) => Some(kept),
            _ => None,
        })
        .collect();
    assert_eq!(kept.last(), Some(&true), "{kept:?}");
    assert_eq!(editor.style().adjust, [125, 100, 100]);
    // Past the track's end, the most.
    let y = slider_y(Adjust::Saturation);
    editor.drag((middle, y), (CARD_WIDTH, y));
    assert_eq!(editor.style().adjust, [125, 100, 200]);
    editor.keys("cmd-z");
    assert_eq!(editor.style().adjust, [125, 100, 100], "a drag is one step");
}

/// The arrow keys move the slider last pressed a step at a time, each a step for ⌘Z, and nothing
/// once a press lands elsewhere.
#[gpui::test]
fn arrow_keys_move_the_focused_slider(cx: &mut TestAppContext) {
    let open = Style {
        advanced: true,
        ..Style::default()
    };
    let mut editor = Editor::new(open, cx);
    editor.keys("right");
    assert_eq!(editor.style().adjust, [100; 3], "no slider was pressed");
    let middle = (TRACK.0 + TRACK.1) / 2.;
    editor.click((middle, slider_y(Adjust::Brightness)));
    editor.keys("right right up");
    assert_eq!(editor.style().adjust, [100, 115, 100]);
    editor.keys("left down");
    assert_eq!(editor.style().adjust, [100, 105, 100]);
    editor.keys("cmd-z");
    assert_eq!(editor.style().adjust, [100, 110, 100]);
    for _ in 0..12 {
        editor.keys("right");
    }
    assert_eq!(editor.style().adjust, [100, 150, 100], "past its most");
    editor.click((INSET + 4., INVERT_Y));
    editor.keys("left");
    assert_eq!(
        editor.style().adjust,
        [100, 150, 100],
        "a press elsewhere kept the keys"
    );
}

/// Reset puts Advanced's three back and leaves the rest; Reset All Edits puts them back too, and
/// leaves Advanced open.
#[gpui::test]
fn both_resets_put_the_sliders_back(cx: &mut TestAppContext) {
    let style = Style {
        invert: true,
        adjust: [110, 90, 0],
        advanced: true,
        ..Style::default()
    };
    let mut editor = Editor::new(style.clone(), cx);
    editor.click((RESET_X + 4., INVERT_Y));
    let reset = editor.style();
    assert_eq!(reset.adjust, [100; 3]);
    assert!(reset.invert && reset.advanced, "{reset:?}");
    editor.keys("cmd-z");
    assert_eq!(editor.style().adjust, [110, 90, 0]);
    editor.click((NAME_LEFT + 4., HEADER / 2.));
    editor.click(find(Target::Item(Item::Reset), &Style::default(), true));
    let reset = editor.style();
    assert_eq!(reset.adjust, [100; 3]);
    assert!(!reset.invert && reset.advanced, "{reset:?}");
}
