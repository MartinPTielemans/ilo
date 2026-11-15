//! The theme picker's clicks, drags and keys, on a host that records what the picker asks of its
//! window and the themes it hands on.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, PlatformInput, TestAppContext, VisualTestContext, point, px};

use super::*;

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    themes: RefCell<Vec<(Theme, bool)>>,
}

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]) {
        let (_, tip, _) = corners[0];
        self.calls.borrow_mut().push(format!(
            "show {left:.1} {top:.1} tip {tip:.1} over {window}"
        ));
    }

    fn hide(&self) {
        self.calls.borrow_mut().push("hide".into());
    }

    fn detent(&self) {
        self.calls.borrow_mut().push("detent".into());
    }
}

/// The picker, shown for Space 7 with `theme`, its arrow at (262, 300) in a window 900 tall.
struct Picker {
    cx: VisualTestContext,
    window: WindowHandle<ThemePicker>,
    host: Rc<Recorder>,
}

impl Picker {
    fn new(theme: Option<Theme>, cx: &mut TestAppContext) -> Self {
        Self::opened(theme, false, cx)
    }

    /// The picker opened to the row's left if `left`, from a sidebar on the right.
    fn opened(theme: Option<Theme>, left: bool, cx: &mut TestAppContext) -> Self {
        let host = Rc::new(Recorder::default());
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for the picker");
        let recorded = host.clone();
        let changed: Changed =
            Rc::new(move |theme, done, _| recorded.themes.borrow_mut().push((theme, done)));
        // From browser window 2, as a context menu in a window behind another asks (#1432).
        let at = At::Row(point(262., 300.));
        cx.update(|cx| show(2, 7, theme, at, left, 900., changed, cx));
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        Picker { cx, window, host }
    }

    /// Where `x`, `y` in the card is in the window.
    fn at(x: f32, y: f32) -> Point<Pixels> {
        point(px(ARROW + x), px(y))
    }

    /// Where the field's point `x`, `y` is in the card.
    fn in_field(x: f32, y: f32) -> (f32, f32) {
        let middle = FIELD_INSET + FIELD / 2.;
        (middle + x * FIELD / 2., middle + y * FIELD / 2.)
    }

    fn click(&mut self, x: f32, y: f32) {
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

    fn theme(&mut self) -> Option<Theme> {
        let editing = self
            .window
            .read_with(&self.cx.cx, |picker, _| picker.editing())
            .unwrap();
        editing.and_then(|(_, theme)| theme)
    }

    fn open(&mut self) -> bool {
        self.cx.cx.update(|cx| is_open(cx))
    }

    /// Whether each theme handed on was kept, since the last look.
    fn handed_on(&mut self) -> Vec<bool> {
        self.host
            .themes
            .borrow_mut()
            .drain(..)
            .map(|(_, done)| done)
            .collect()
    }

    fn calls(&mut self) -> Vec<String> {
        self.host.calls.borrow().clone()
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    /// What the keyboard's ring is on.
    fn keyed(&mut self) -> Option<Target> {
        let picker = self.window.read_with(&self.cx.cx, |picker, _| {
            picker.editing.as_ref().and_then(|editing| editing.keyed)
        });
        picker.unwrap()
    }
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

#[gpui::test]
fn it_shows_over_the_window_that_asked(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    let calls = picker.calls();
    assert!(
        calls[0].starts_with("show ") && calls[0].ends_with(" over 2"),
        "{calls:?}"
    );
}

#[gpui::test]
fn a_click_on_the_empty_field_places_the_first_colour_there(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    let (x, y) = Picker::in_field(0.2, 0.45);
    picker.click(x, y);
    let theme = picker.theme().expect("no colour placed");
    assert!(
        close((theme.x, theme.y), (0.2, 0.45)),
        "at {}, {}",
        theme.x,
        theme.y
    );
    assert_eq!(theme.count, 1);
    assert_eq!(theme.intensity, DEFAULT_INTENSITY);
    // Handed on as it lands, and kept as the button comes up.
    assert_eq!(picker.handed_on(), vec![false, true]);
}

#[gpui::test]
fn a_click_up_before_the_next_frame_still_keeps_the_colour(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    let (x, y) = Picker::in_field(0.2, 0.45);
    // The driver's clickkey posts both events back to back, and a quick click can do the same.
    // The test context draws after every event; a window draws once a frame, so both go in one
    // update, with no draw between them.
    let at = Picker::at(x, y);
    picker.cx.update(|window, cx| {
        let down = MouseDownEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        };
        window.dispatch_event(PlatformInput::MouseDown(down), cx);
        let up = MouseUpEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 1,
        };
        window.dispatch_event(PlatformInput::MouseUp(up), cx);
    });
    picker.cx.run_until_parked();
    assert_eq!(picker.handed_on(), vec![false, true]);
    // The pointer isn't still holding the colour.
    let (x, y) = Picker::in_field(-0.5, -0.5);
    picker
        .cx
        .simulate_mouse_move(Picker::at(x, y), None, Modifiers::none());
    picker.cx.run_until_parked();
    let theme = picker.theme().unwrap();
    assert!(
        close((theme.x, theme.y), (0.2, 0.45)),
        "followed to {}, {}",
        theme.x,
        theme.y
    );
}

/// A click as the picker first appears, before it has drawn a frame of itself open, still places
/// the colour: GPUI hands it to the empty frame drawn as the window was made.
#[gpui::test]
fn a_click_before_the_first_frame_still_places_the_colour(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no window for the picker");
    let recorded = host.clone();
    let changed: Changed =
        Rc::new(move |theme, done, _| recorded.themes.borrow_mut().push((theme, done)));
    let (x, y) = Picker::in_field(0.2, 0.45);
    let at = Picker::at(x, y);
    // One update, so no frame comes between the opening and the click. Through the window rather
    // than its view, which the press updates.
    cx.update(|cx| {
        show(
            2,
            7,
            None,
            At::Row(point(262., 300.)),
            false,
            900.,
            changed,
            cx,
        );
        cx.update_window(window.into(), |_, window, cx| {
            let down = MouseDownEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            };
            window.dispatch_event(PlatformInput::MouseDown(down), cx);
            let up = MouseUpEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            };
            window.dispatch_event(PlatformInput::MouseUp(up), cx);
        })
        .unwrap();
    });
    let mut picker = Picker {
        cx: VisualTestContext::from_window(window.into(), cx),
        window,
        host,
    };
    picker.cx.run_until_parked();
    assert_eq!(picker.handed_on(), vec![false, true]);
    let theme = picker.theme().expect("no colour placed");
    assert!(
        close((theme.x, theme.y), (0.2, 0.45)),
        "at {}, {}",
        theme.x,
        theme.y
    );
}

#[gpui::test]
fn dragging_a_handle_moves_every_colour_and_keeps_them_on_letting_go(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.).with_count(3)), cx);
    let complement = picker.theme().unwrap().handle(1);
    // Taken off its middle, the handle keeps where the pointer took it.
    let (x, y) = Picker::in_field(0.5 + 0.05, 0.);
    let (to_x, to_y) = Picker::in_field(0.05, 0.3);
    picker.drag((x, y), (to_x, to_y));
    let theme = picker.theme().unwrap();
    assert!(
        close((theme.x, theme.y), (0., 0.3)),
        "at {}, {}",
        theme.x,
        theme.y
    );
    assert_eq!(theme.count, 3);
    assert_ne!(theme.handle(1), complement);
    assert_eq!(picker.handed_on(), vec![false, true]);
}

#[gpui::test]
fn dragging_another_handle_places_its_colour_alone(cx: &mut TestAppContext) {
    let start = Theme::at(0.5, 0.).with_count(3);
    let mut picker = Picker::new(Some(start), cx);
    let (hx, hy) = start.handle(2);
    picker.drag(Picker::in_field(hx, hy), Picker::in_field(0., -0.3));
    let theme = picker.theme().unwrap();
    assert!(
        close(theme.handle(2), (0., -0.3)),
        "the third at {:?}",
        theme.handle(2)
    );
    assert!(
        close((theme.x, theme.y), (0.5, 0.)),
        "the main one moved, to {}, {}",
        theme.x,
        theme.y
    );
    assert!(close(theme.handle(1), start.handle(1)), "the second moved");
    assert_eq!(theme.colors()[2], theme::color_at(0., -0.3, 1.));
    assert_eq!(picker.handed_on(), vec![false, true]);
    // − puts the second back at the main one's complement, the third gone.
    picker.click(REMOVE_X, COUNT_Y);
    let theme = picker.theme().unwrap();
    assert_eq!((theme.count, theme.places), (2, None));
}

/// The handles are drawn at rest, then held from the press's own frame, which is the first to
/// listen for the drag's moves, and at rest again once let go, and the wave's knob is drawn held the
/// same way (check.sh waits for each).
#[gpui::test]
fn a_held_handle_is_drawn_held_before_it_drags(cx: &mut TestAppContext) {
    let start = Theme::at(0.5, 0.).with_count(3);
    let mut picker = Picker::new(Some(start), cx);
    let drawn = |picker: &mut Picker| {
        let editing = picker.window.read_with(&picker.cx.cx, |picker, _| {
            picker
                .editing
                .as_ref()
                .and_then(|editing| editing.drawn_handles)
        });
        editing.unwrap()
    };
    assert_eq!(drawn(&mut picker), Some((3, None)));
    let (x, y) = Picker::in_field(start.handle(2).0, start.handle(2).1);
    let left = MouseButton::Left;
    picker
        .cx
        .simulate_mouse_down(Picker::at(x, y), left, Modifiers::none());
    picker.cx.run_until_parked();
    assert_eq!(drawn(&mut picker), Some((3, Some(Drag::Handle(2, 0., 0.)))));
    picker
        .cx
        .simulate_mouse_up(Picker::at(x, y), left, Modifiers::none());
    picker.cx.run_until_parked();
    assert_eq!(drawn(&mut picker), Some((3, None)));
    // The wave's knob, likewise.
    let knob = Picker::at(knob_x(DEFAULT_INTENSITY), SLIDER_Y);
    picker.cx.simulate_mouse_down(knob, left, Modifiers::none());
    picker.cx.run_until_parked();
    assert_eq!(drawn(&mut picker), Some((3, Some(Drag::Intensity))));
}

#[gpui::test]
fn plus_and_minus_add_and_take_colours_from_one_to_three(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    picker.click(REMOVE_X, COUNT_Y);
    assert_eq!(picker.theme().unwrap().count, 1, "− with one colour");
    for count in [2, 3, 3] {
        picker.click(ADD_X, COUNT_Y);
        assert_eq!(picker.theme().unwrap().count, count);
    }
    picker.click(REMOVE_X, COUNT_Y);
    assert_eq!(picker.theme().unwrap().count, 2);
    // Each change that took is kept at once.
    assert_eq!(picker.handed_on(), vec![true, true, true]);
}

#[gpui::test]
fn a_swatch_gives_its_colour_and_keeps_the_rest(cx: &mut TestAppContext) {
    let theme = Theme {
        intensity: 0.4,
        grain: 0.3,
        ..Theme::at(0.5, 0.).with_count(2)
    };
    let mut picker = Picker::new(Some(theme), cx);
    picker.click(SWATCH_X + SWATCH_PITCH * 3., SWATCH_Y);
    let picked = picker.theme().unwrap();
    let colour = Theme {
        x: picked.x,
        y: picked.y,
        saturation: picked.saturation,
        ..theme
    };
    assert_eq!(picked, colour);
    let swatch = Theme::of_color(SWATCHES[3]);
    assert_eq!(
        picked,
        Theme {
            count: 2,
            intensity: 0.4,
            grain: 0.3,
            ..swatch
        }
    );
    assert_eq!(picked.main_color(), SWATCHES[3]);
    // Without a colour yet, a swatch is the first, as itself: slate, not the field's vivid blue.
    let mut picker = Picker::new(None, cx);
    picker.click(SWATCH_X + SWATCH_PITCH * 8., SWATCH_Y);
    assert_eq!(picker.theme(), Some(Theme::of_color(SWATCHES[8])));
    assert_eq!(picker.theme().unwrap().main_color(), SWATCHES[8]);
    // Its handle dragged takes the field's colour where it's let go.
    let slate = picker.theme().unwrap();
    let (x, y) = Picker::in_field(slate.x, slate.y);
    let (to_x, to_y) = Picker::in_field(slate.x, slate.y + 0.1);
    picker.drag((x, y), (to_x, to_y));
    let dragged = picker.theme().unwrap();
    assert_eq!(dragged.saturation, 1.);
    assert_eq!(
        dragged.main_color(),
        theme::color_at(dragged.x, dragged.y, 1.)
    );
}

#[gpui::test]
fn the_wave_sets_the_intensity_and_the_dial_the_grain(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    let knob = knob_x(DEFAULT_INTENSITY);
    picker.drag((knob, SLIDER_Y), (knob_x(0.25), SLIDER_Y + 10.));
    assert!((picker.theme().unwrap().intensity - 0.25).abs() < 0.001);
    // Handed on as the knob is taken and moved, and kept as it's let go.
    assert_eq!(picker.handed_on(), vec![false, false, true]);
    // A quarter turn from the top, then on past the top, where it stops.
    let right = (DIAL.0 + DIAL_RING, DIAL.1);
    picker.drag((DIAL.0, DIAL.1 - DIAL_RING), right);
    assert!((picker.theme().unwrap().grain - 0.25).abs() < 0.001);
    let left = (DIAL.0 - DIAL_RING, DIAL.1);
    let past_top = (DIAL.0 + 5., DIAL.1 - DIAL_RING);
    let (button, none) = (MouseButton::Left, Modifiers::none());
    picker
        .cx
        .simulate_mouse_down(Picker::at(left.0, left.1), button, none);
    for (x, y) in [(DIAL.0 - 5., DIAL.1 - DIAL_RING), past_top] {
        picker
            .cx
            .simulate_mouse_move(Picker::at(x, y), button, none);
        picker.cx.run_until_parked();
    }
    picker
        .cx
        .simulate_mouse_up(Picker::at(past_top.0, past_top.1), button, none);
    picker.cx.run_until_parked();
    assert_eq!(picker.theme().unwrap().grain, 1.);
}

/// A drag that moves before the picker has drawn the hold still moves the knob and the dial: a
/// quick flick, or the driver's press and dragto, puts both in one frame (#2605).
#[gpui::test]
fn a_drag_that_moves_before_the_hold_is_drawn_still_moves(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    let knob = knob_x(DEFAULT_INTENSITY);
    let quarter = (DIAL.0 + DIAL_RING, DIAL.1);
    let drags = [
        ((knob, SLIDER_Y), (knob_x(0.25), SLIDER_Y)),
        ((DIAL.0, DIAL.1 - DIAL_RING), quarter),
    ];
    for (from, to) in drags {
        let (from, to) = (Picker::at(from.0, from.1), Picker::at(to.0, to.1));
        // One update, with no draw between the press and the move, as in one frame.
        picker.cx.update(|window, cx| {
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
        // The hold is drawn, then it's let go where it moved to.
        picker.cx.run_until_parked();
        picker
            .cx
            .simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        picker.cx.run_until_parked();
    }
    let theme = picker.theme().unwrap();
    assert!(
        (theme.intensity - 0.25).abs() < 0.001,
        "intensity {}",
        theme.intensity
    );
    assert!((theme.grain - 0.25).abs() < 0.001, "grain {}", theme.grain);
}

#[gpui::test]
fn without_a_colour_only_the_field_swatches_and_appearance_answer(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    picker.click(ADD_X, COUNT_Y);
    picker.click(knob_x(0.5), SLIDER_Y);
    picker.click(DIAL.0 + DIAL_RING, DIAL.1);
    assert_eq!(picker.theme(), None);
    assert!(picker.handed_on().is_empty());
    // The appearance chosen first goes with the first colour.
    picker.click(APPEARANCE_X[2], APPEARANCE_Y);
    let (x, y) = Picker::in_field(0., 0.5);
    picker.click(x, y);
    assert_eq!(picker.theme().unwrap().appearance, Appearance::Dark);
}

#[gpui::test]
fn escape_closes_it_and_a_click_outside_leaves_it_closed(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    assert!(picker.open());
    picker.cx.simulate_keystrokes("escape");
    picker.cx.run_until_parked();
    assert!(!picker.open());
    assert_eq!(picker.calls().last().map(String::as_str), Some("hide"));
    // The host's dismissal after it changes nothing more.
    picker.cx.cx.update(dismissed);
    assert!(!picker.open());
}

#[test]
fn the_card_centres_on_its_arrow_inside_the_window() {
    // Room above and below: centred, the tip at the card's middle.
    let (left, top, arrow_y) = placed(At::Row(point(250., 400.)), false, 1000.);
    assert!((left - (250. + GAP - TIP_SHORT)).abs() < 0.01);
    assert_eq!((top, arrow_y), (400. - HEIGHT / 2., Some(HEIGHT / 2.)));
    // Near the top, the card stays in and the arrow moves up it, clear of the corner.
    let (_, top, arrow_y) = placed(At::Row(point(250., 60.)), false, 1000.);
    assert_eq!((top, arrow_y), (MARGIN, Some(52.)));
    let (_, top, arrow_y) = placed(At::Row(point(250., 10.)), false, 1000.);
    assert_eq!(
        (top, arrow_y),
        (MARGIN, Some(RADIUS + ARROW + ARROW_FOOT_RADIUS))
    );
    // Near the bottom, the same.
    let (_, top, arrow_y) = placed(At::Row(point(250., 950.)), false, 1000.);
    assert_eq!((top, arrow_y), (1000. - MARGIN - HEIGHT, Some(950. - top)));
}

/// From the Space's menu the card has no arrow and sits 14 inside the sidebar's edge and the browser
/// window's top, as Arc 1.171's does inside its 382 × 534 window there, growing in from its top left.
#[test]
fn from_the_menu_the_card_sits_against_the_sidebar_at_the_window_s_top() {
    let (left, top, arrow_y) = placed(At::Edge(240.), false, 1000.);
    assert_eq!(arrow_y, None);
    let (card_left, card_top) = (left + ARROW, top);
    assert_eq!((card_left, card_top), (254., 14.));
    let corners = corners(None, false);
    assert_eq!(corners.len(), 4, "the card has an arrow");
    assert_eq!(
        corners[0],
        (ARROW, 0., RADIUS),
        "it doesn't grow in from its top left"
    );
    assert!(
        corners.iter().all(|&(x, _, _)| x >= ARROW),
        "the card reaches into the arrow's room"
    );
}

/// From a sidebar on the right, the picker opens toward the page: to the left of what opened it,
/// its arrow on its right edge with the same gap, and against the sidebar's left edge from the
/// Space's menu, growing in from its top right.
#[test]
fn from_a_sidebar_on_the_right_the_card_opens_to_the_left() {
    let (left, top, arrow_y) = placed(At::Row(point(750., 400.)), true, 1000.);
    assert!((left + WIDTH - TIP_SHORT - (750. - GAP)).abs() < 0.01);
    assert_eq!((top, arrow_y), (400. - HEIGHT / 2., Some(HEIGHT / 2.)));
    let arrowed = corners(arrow_y, true);
    assert_eq!(arrowed[0], (WIDTH, HEIGHT / 2., ARROW_TIP_RADIUS));
    assert!(
        arrowed
            .iter()
            .all(|&(x, _, _)| x == WIDTH || x <= CARD_WIDTH),
        "the card reaches into the arrow's room"
    );
    // Clockwise from the tip, as the outline rounds each corner the way it bulges: down the
    // arrow's lower side first.
    assert!(arrowed[1].1 > arrowed[0].1, "{arrowed:?}");

    let (left, top, arrow_y) = placed(At::Edge(760.), true, 1000.);
    assert_eq!(arrow_y, None);
    assert_eq!((left + CARD_WIDTH, top), (746., 14.));
    let card = corners(None, true);
    assert_eq!(card[0], (CARD_WIDTH, 0., RADIUS), "not from its top right");
    assert_eq!(card[1], (CARD_WIDTH, HEIGHT, RADIUS), "not clockwise");
}

/// Opened to the left, the card starts at its window's left, and presses land on what's drawn.
#[gpui::test]
fn opened_to_the_left_a_press_lands_on_the_field(cx: &mut TestAppContext) {
    let mut picker = Picker::opened(None, true, cx);
    let (x, y) = Picker::in_field(0.2, 0.45);
    picker
        .cx
        .simulate_click(point(px(x), px(y)), Modifiers::none());
    picker.cx.run_until_parked();
    let theme = picker.theme().expect("no colour placed");
    assert!(
        close((theme.x, theme.y), (0.2, 0.45)),
        "at {}, {}",
        theme.x,
        theme.y
    );
}

#[test]
fn presses_land_on_what_is_drawn_there() {
    let theme = Theme::at(0.5, 0.).with_count(2);
    let (x, y) = Picker::in_field(0.5, 0.);
    assert_eq!(target(x, y, Some(&theme)), Some(Target::Handle(0)));
    let (x, y) = Picker::in_field(-0.5, 0.);
    assert_eq!(target(x, y, Some(&theme)), Some(Target::Handle(1)));
    let (x, y) = Picker::in_field(0., 0.5);
    assert_eq!(target(x, y, Some(&theme)), Some(Target::Field));
    assert_eq!(
        target(APPEARANCE_X[1], APPEARANCE_Y, Some(&theme)),
        Some(Target::Appearance(Appearance::Light))
    );
    assert_eq!(
        target(SWATCH_X + SWATCH_PITCH * 8., SWATCH_Y, None),
        Some(Target::Swatch(8))
    );
    // Between two swatches, nothing.
    assert_eq!(target(SWATCH_X + SWATCH_PITCH / 2., SWATCH_Y, None), None);
    assert_eq!(
        target(TRACK_LEFT + 1., SLIDER_Y, None),
        Some(Target::Intensity)
    );
    assert_eq!(
        target(DIAL.0, DIAL.1 + DIAL_RING, None),
        Some(Target::Grain)
    );
}

#[test]
fn minus_and_plus_read_on_the_field_and_off_reads_as_off() {
    let paper = Hsla::from(rgb(palette::PAPER));
    let light = Theme {
        appearance: Appearance::Light,
        ..Theme::of_color(SWATCHES[0])
    };
    let dark = Theme {
        appearance: Appearance::Dark,
        ..Theme::of_color(SWATCHES[8])
    };
    for theme in [light, dark] {
        let palette = Palette::of(Some(&theme), Tone::Dark);
        let states = [
            ("enabled", true, false, 0.65),
            ("pressed", true, true, 0.9),
            ("disabled", false, false, 0.3),
            ("disabled, pressed", false, true, 0.3),
        ];
        for (state, enabled, pressed, alpha) in states {
            let color = count_color(&palette, enabled, pressed);
            assert_eq!(
                color,
                paper.opacity(alpha),
                "{state} on {:?}",
                theme.appearance
            );
        }
        // On the plain field, where there are no dots, over the card as it's drawn off the Mac:
        // enabled reads as a glyph and pressed more, and disabled stands out more than a dot does
        // but less than a glyph has to, so it reads as there and off.
        let card = palette::over(rgb(palette::OVERLAY_DARK), palette.card, palette.card.a);
        let field = palette::over(card, palette.ink, FIELD_FILL);
        let on_field =
            |color: Hsla| palette::contrast(palette::over(field, palette.ink, color.a), field);
        let enabled = on_field(count_color(&palette, true, false));
        let pressed = on_field(count_color(&palette, true, true));
        let disabled = on_field(count_color(&palette, false, false));
        let dot = on_field(paper.opacity(DOT_ALPHA));
        assert!(enabled >= palette::GLYPH, "enabled {enabled:.2}");
        assert!(
            pressed > enabled,
            "pressed {pressed:.2}, enabled {enabled:.2}"
        );
        let off = dot < disabled && disabled < palette::GLYPH;
        assert!(off, "disabled {disabled:.2}, dot {dot:.2}");
    }
}

#[test]
fn the_field_has_no_dots_under_minus_and_plus() {
    let dots = dots_image();
    let bytes = dots.as_bytes(0).unwrap();
    let side = (FIELD * 2.) as usize;
    let reach = COUNT_TARGET / 2.;
    let middle = COUNT_Y - FIELD_INSET;
    for at in [REMOVE_X, ADD_X].map(|x| x - FIELD_INSET) {
        // Every 2× pixel inside the target is clear, and the grid goes on just outside it.
        let mut around = 0;
        for y in 0..side {
            for x in 0..side {
                let centre = ((x as f32 + 0.5) / 2., (y as f32 + 0.5) / 2.);
                let distance = (centre.0 - at).hypot(centre.1 - middle);
                let lit = bytes[(y * side + x) * 4 + 3] > 0;
                if distance < reach - 0.5 {
                    assert!(!lit, "a dot at {centre:?}");
                } else if lit && distance < reach + DOT_PITCH + 1. {
                    around += 1;
                }
            }
        }
        assert!(around > 0, "no dots round the target at {at}");
    }
}

/// A drag gives one detent as it reaches a stop: a handle the field's rim, the knob an end and the
/// dial the top. Moving along the rim, held at the stop, gives no more, and leaving and coming
/// back gives another.
#[gpui::test]
fn a_drag_bumps_once_as_it_reaches_a_stop(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    let detents = |picker: &Picker| {
        picker
            .host
            .calls
            .borrow()
            .iter()
            .filter(|call| *call == "detent")
            .count()
    };
    let (button, none) = (MouseButton::Left, Modifiers::none());
    let path = |picker: &mut Picker, points: &[(f32, f32)]| {
        let (x, y) = points[0];
        picker
            .cx
            .simulate_mouse_down(Picker::at(x, y), button, none);
        for &(x, y) in &points[1..] {
            picker
                .cx
                .simulate_mouse_move(Picker::at(x, y), button, none);
            picker.cx.run_until_parked();
        }
        let (x, y) = points[points.len() - 1];
        picker.cx.simulate_mouse_up(Picker::at(x, y), button, none);
        picker.cx.run_until_parked();
    };
    let field = Picker::in_field;
    path(
        &mut picker,
        &[
            field(0.5, 0.),
            field(0.8, 0.),
            field(1.2, 0.),
            field(1.2, 0.3),
        ],
    );
    assert_eq!(detents(&picker), 1);
    let theme = picker.theme().unwrap();
    path(
        &mut picker,
        &[field(theme.x, theme.y), field(0.5, 0.), field(1.3, 0.)],
    );
    assert_eq!(detents(&picker), 2);
    let knob = knob_x(DEFAULT_INTENSITY);
    path(
        &mut picker,
        &[
            (knob, SLIDER_Y),
            (knob_x(0.5), SLIDER_Y),
            (knob_x(0.) - 20., SLIDER_Y),
        ],
    );
    assert_eq!(detents(&picker), 3);
}

/// Tab goes round the appearance buttons, − and +, the swatches and the wave, left and right go
/// along each, and space or return presses what's ringed (#1383).
#[gpui::test]
fn the_keyboard_goes_round_the_buttons_swatches_and_wave(cx: &mut TestAppContext) {
    let mut picker = Picker::new(Some(Theme::at(0.5, 0.)), cx);
    // Nothing ringed, nothing pressed.
    picker.keys("space right");
    assert_eq!((picker.keyed(), picker.handed_on()), (None, vec![]));
    // The appearance chosen first, then along to the next.
    picker.keys("tab");
    assert_eq!(
        picker.keyed(),
        Some(Target::Appearance(Appearance::Automatic))
    );
    picker.keys("right space");
    assert_eq!(picker.theme().unwrap().appearance, Appearance::Light);
    // + first, as − doesn't answer with one colour.
    picker.keys("tab");
    assert_eq!(picker.keyed(), Some(Target::Add));
    picker.keys("enter enter enter");
    assert_eq!(picker.theme().unwrap().count, 3);
    picker.keys("left space");
    assert_eq!(picker.theme().unwrap().count, 2);
    // The swatches from the first, stopping at the last.
    picker.keys("tab right right right enter");
    assert_eq!(picker.theme().unwrap().main_color(), SWATCHES[3]);
    picker.keys(&["right"; 12].join(" "));
    assert_eq!(picker.keyed(), Some(Target::Swatch(SWATCHES.len() - 1)));
    // The wave's knob a step at a time, onto the steps.
    picker.keys("tab left");
    assert_eq!(picker.keyed(), Some(Target::Intensity));
    assert!((picker.theme().unwrap().intensity - 0.75).abs() < 1e-4);
    picker.keys(&["right"; 8].join(" "));
    assert_eq!(picker.theme().unwrap().intensity, 1.);
    // Each press that changed something was kept, as a click's is: + twice, not at three.
    let handed_on = picker.handed_on();
    assert!(
        handed_on.len() == 11 && handed_on.iter().all(|&done| done),
        "{handed_on:?}"
    );
    // Round to the appearance chosen, and back.
    picker.keys("tab");
    assert_eq!(picker.keyed(), Some(Target::Appearance(Appearance::Light)));
    picker.keys("shift-tab");
    assert_eq!(picker.keyed(), Some(Target::Intensity));
    // A click takes the ring away; escape still closes.
    picker.click(APPEARANCE_X[2], APPEARANCE_Y);
    assert_eq!(picker.keyed(), None);
    picker.keys("escape");
    assert!(!picker.open());
}

/// Without a colour, tab passes − and + and the wave, which don't answer, and a swatch picked from
/// the keyboard is the first colour (#1383).
#[gpui::test]
fn without_a_colour_the_keyboard_has_the_appearance_and_swatches(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    picker.keys("tab right right");
    assert_eq!(picker.keyed(), Some(Target::Appearance(Appearance::Dark)));
    picker.keys("space tab");
    assert_eq!(picker.keyed(), Some(Target::Swatch(0)));
    picker.keys("tab");
    assert_eq!(picker.keyed(), Some(Target::Appearance(Appearance::Dark)));
    picker.keys("shift-tab right enter");
    let theme = picker.theme().expect("the swatch gave no colour");
    assert_eq!(
        (theme.main_color(), theme.appearance),
        (SWATCHES[1], Appearance::Dark)
    );
    // With it, the wave comes next.
    picker.keys("tab");
    assert_eq!(picker.keyed(), Some(Target::Intensity));
}

/// The node with `role` named `label` in what the picker last told VoiceOver.
fn named(
    picker: &mut Picker,
    role: Role,
    label: &str,
) -> (gpui::accesskit::NodeId, gpui::accesskit::Node) {
    let tree = picker.cx.a11y_tree().expect("no accessibility tree");
    let found = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label));
    found
        .cloned()
        .unwrap_or_else(|| panic!("no {role:?} named {label}"))
}

#[gpui::test]
fn voiceover_reads_the_picker_and_moves_its_sliders(cx: &mut TestAppContext) {
    let theme = Theme {
        intensity: 0.4,
        grain: 0.3,
        ..Theme::at(0.5, 0.).with_count(2)
    };
    let mut picker = Picker::new(Some(theme), cx);
    picker.cx.simulate_a11y_activation();
    picker.cx.run_until_parked();
    named(&mut picker, Role::Dialog, "Space Theme");
    named(&mut picker, Role::Group, "Colors, 2");
    let (_, automatic) = named(&mut picker, Role::RadioButton, "Automatic");
    assert_eq!(automatic.toggled(), Some(Toggled::True));
    let (dark, _) = named(&mut picker, Role::RadioButton, "Dark");
    let (_, intensity) = named(&mut picker, Role::Slider, "Intensity");
    assert_eq!(intensity.numeric_value(), Some(40.));
    let (_, grain) = named(&mut picker, Role::Slider, "Grain");
    assert_eq!(grain.numeric_value(), Some(30.));

    // Its press is a click where the plate is drawn.
    picker.cx.simulate_a11y_action(dark, Action::Click);
    picker.cx.run_until_parked();
    assert_eq!(picker.theme().unwrap().appearance, Appearance::Dark);
    let (add, _) = named(&mut picker, Role::Button, "Add Color");
    picker.cx.simulate_a11y_action(add, Action::Click);
    picker.cx.run_until_parked();
    assert_eq!(picker.theme().unwrap().count, 3);
    named(&mut picker, Role::Group, "Colors, 3");
    let (_, add) = named(&mut picker, Role::Button, "Add Color");
    assert!(add.is_disabled(), "+ at three colours");
    let (slate, _) = named(&mut picker, Role::Button, "Slate");
    picker.cx.simulate_a11y_action(slate, Action::Click);
    picker.cx.run_until_parked();
    assert_eq!(picker.theme().unwrap().main_color(), SWATCHES[8]);

    // Increment and decrement move to the next tenth and stop at the ends, each move kept.
    picker.handed_on();
    let (intensity, _) = named(&mut picker, Role::Slider, "Intensity");
    picker.cx.simulate_a11y_action(intensity, Action::Increment);
    picker.cx.run_until_parked();
    assert!((picker.theme().unwrap().intensity - 0.5).abs() < 0.001);
    let (grain, _) = named(&mut picker, Role::Slider, "Grain");
    for _ in 0..4 {
        picker.cx.simulate_a11y_action(grain, Action::Decrement);
        picker.cx.run_until_parked();
    }
    assert_eq!(picker.theme().unwrap().grain, 0.);
    assert_eq!(picker.handed_on(), vec![true, true, true, true]);
    let (_, grain) = named(&mut picker, Role::Slider, "Grain");
    assert_eq!(grain.numeric_value(), Some(0.));
}

#[gpui::test]
fn voiceover_places_the_first_colour_from_the_empty_field(cx: &mut TestAppContext) {
    let mut picker = Picker::new(None, cx);
    picker.cx.simulate_a11y_activation();
    picker.cx.run_until_parked();
    let (field, node) = named(&mut picker, Role::Group, "Colors, none");
    assert_eq!(node.description(), Some(HINT));
    let (_, intensity) = named(&mut picker, Role::Slider, "Intensity");
    assert!(intensity.is_disabled(), "the wave without a colour");
    picker.cx.simulate_a11y_action(field, Action::Click);
    picker.cx.run_until_parked();
    let theme = picker.theme().expect("no colour placed");
    assert_eq!(theme.count, 1);
    named(&mut picker, Role::Group, "Colors, 1");
}

#[test]
fn every_swatch_has_its_own_name() {
    let names = SWATCHES.map(swatch_name);
    assert_eq!(
        names,
        [
            "White", "Pink", "Purple", "Red", "Orange", "Yellow", "Green", "Blue", "Slate"
        ]
    );
}
