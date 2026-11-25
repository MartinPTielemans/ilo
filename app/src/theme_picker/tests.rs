//! The theme panel's clicks, drags and keys, on a host that records what the panel asks of its
//! window and what it hands on: themes, the glass, and colours taken away.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, PlatformInput, TestAppContext, VisualTestContext, point, px};

use super::*;
use crate::space_transparency::START;
use crate::theme::DEFAULT_INTENSITY;

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    themes: RefCell<Vec<(Theme, bool)>>,
    glass: RefCell<Vec<(f32, bool)>>,
    cleared: RefCell<usize>,
}

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]) {
        let (x, y, _) = corners[0];
        self.calls.borrow_mut().push(format!(
            "show {left:.1} {top:.1} from {x:.1},{y:.1} over {window}"
        ));
    }

    fn hide(&self) {
        self.calls.borrow_mut().push("hide".into());
    }

    fn detent(&self) {
        self.calls.borrow_mut().push("detent".into());
    }
}

/// The panel, shown for Space 7 with `theme` and its glass at `START`, beside the name row at
/// (240, 120) in a window 900 tall.
struct Panel {
    cx: VisualTestContext,
    window: WindowHandle<ThemePicker>,
    host: Rc<Recorder>,
}

fn also_for(host: &Rc<Recorder>) -> Also {
    let (glass, cleared) = (host.clone(), host.clone());
    Also {
        transparency: START,
        glassed: Rc::new(move |value, done, _| glass.glass.borrow_mut().push((value, done))),
        cleared: Rc::new(move |_| *cleared.cleared.borrow_mut() += 1),
    }
}

fn changed_for(host: &Rc<Recorder>) -> Changed {
    let recorded = host.clone();
    Rc::new(move |theme, done, _| recorded.themes.borrow_mut().push((theme, done)))
}

const NAME_ROW: At = At::Beside {
    edge: 240.,
    top: 120.,
};

impl Panel {
    fn new(theme: Option<Theme>, cx: &mut TestAppContext) -> Self {
        let host = Rc::new(Recorder::default());
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for the panel");
        let (changed, also_) = (changed_for(&host), also_for(&host));
        // From browser window 2, as a context menu in a window behind another asks (#1432).
        cx.update(|cx| {
            show(2, 7, theme, NAME_ROW, false, 900., changed, cx);
            also(also_, cx);
        });
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        Panel { cx, window, host }
    }

    fn at(x: f32, y: f32) -> Point<Pixels> {
        point(px(x), px(y))
    }

    /// Where the pad's point (across, down) is on the panel.
    fn on_pad(across: f32, down: f32) -> (f32, f32) {
        layout::on_panel(across, down)
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

    fn theme(&mut self) -> Option<Theme> {
        let editing = self
            .window
            .read_with(&self.cx.cx, |picker, _| picker.editing())
            .unwrap();
        editing.and_then(|(_, theme)| theme)
    }

    fn glass(&mut self) -> f32 {
        let glass = self
            .window
            .read_with(&self.cx.cx, |picker, _| picker.glass())
            .unwrap();
        glass.expect("the panel isn't open")
    }

    fn chosen(&mut self) -> usize {
        let chosen = self.window.read_with(&self.cx.cx, |picker, _| {
            picker.editing.as_ref().map(|editing| editing.chosen)
        });
        chosen.unwrap().unwrap()
    }

    fn typing(&mut self) -> bool {
        let typing = self.window.read_with(&self.cx.cx, |picker, _| {
            (picker.editing.as_ref()).is_some_and(|editing| editing.typing.is_some())
        });
        typing.unwrap()
    }

    fn open(&mut self) -> bool {
        self.cx.cx.update(|cx| is_open(cx))
    }

    /// Whether each theme handed on was kept, since the last look.
    fn handed_on(&mut self) -> Vec<bool> {
        let themes = self.host.themes.borrow_mut().drain(..).collect::<Vec<_>>();
        themes.into_iter().map(|(_, done)| done).collect()
    }

    /// Each glass handed on and whether it was kept, since the last look.
    fn glassed(&mut self) -> Vec<(f32, bool)> {
        self.host.glass.borrow_mut().drain(..).collect()
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

    /// The middle of bar `bar`'s thumb at `value`.
    fn thumb(bar: Bar, value: f32) -> (f32, f32) {
        (layout::thumb_x(value), layout::track_y(bar))
    }
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

#[gpui::test]
fn it_hangs_beside_the_name_8_right_of_the_sidebar_with_its_top_at_the_row_s(
    cx: &mut TestAppContext,
) {
    let mut panel = Panel::new(None, cx);
    let calls = panel.calls();
    assert_eq!(
        calls[0], "show 248.0 120.0 from 0.0,0.0 over 2",
        "{calls:?}"
    );
}

#[test]
fn it_keeps_inside_the_window_and_opens_left_from_a_sidebar_on_the_right() {
    // Low in a short window, it rises to keep its margin off the bottom.
    let low = At::Beside {
        edge: 240.,
        top: 700.,
    };
    let (_, top, arrow) = placed(low, false, 800.);
    assert_eq!((top, arrow), (800. - MARGIN - HEIGHT, None));
    // From a sidebar on the right, it hangs left of the sidebar's edge, and grows from its top
    // right.
    let (left, top, _) = placed(NAME_ROW, true, 900.);
    assert_eq!((left, top), (240. - BESIDE - WIDTH, 120.));
    assert_eq!(corners(true)[0], (WIDTH, 0., RADIUS));
    // Beside a row, its head is level with the row.
    let (left, top, _) = placed(At::Row(point(262., 300.)), false, 900.);
    assert_eq!((left, top), (262. + BESIDE, 300. - PADDING - HEAD / 2.));
}

#[test]
fn the_boost_editor_keeps_the_form_today_s_picker_had() {
    // Its card centres on its arrow, as it did.
    let card = 400.;
    let (left, top, arrow_y) = placed_of(At::Row(point(250., 400.)), false, 1000., card);
    assert!((left - (250. + FORM_GAP - TIP_SHORT)).abs() < 0.01);
    assert_eq!((top, arrow_y), (400. - card / 2., Some(card / 2.)));
    let corners = corners_of(arrow_y, false, card);
    assert_eq!(corners[0], (0., card / 2., ARROW_TIP_RADIUS));
    assert!(corners.iter().all(|&(x, ..)| x <= FORM_WIDTH));
    assert_eq!(card_x(false), ARROW);
}

#[test]
fn every_part_lies_inside_the_panel_in_the_drawn_order() {
    let inside = |x: f32, y: f32| (0. ..=WIDTH).contains(&x) && (0. ..=HEIGHT).contains(&y);
    for index in 0..layout::palette_count() {
        let (x, y) = layout::disc(index);
        assert!(inside(x - SLOT / 2., y - SLOT / 2.) && inside(x + SLOT / 2., y + SLOT / 2.));
    }
    let (pad_left, pad_top) = layout::pad();
    assert_eq!(
        pad_left + PAD_WIDTH + PADDING,
        WIDTH,
        "the pad fills the inside"
    );
    assert!(layout::disc(0).1 + SLOT / 2. + GAP <= pad_top);
    let mut below = pad_top + PAD_HEIGHT;
    for bar in BARS {
        assert!(layout::bar_top(bar) >= below);
        below = layout::track_y(bar) + THUMB / 2.;
    }
    assert!(layout::note_top() >= below);
    assert_eq!(layout::note_top() + LABEL + PADDING, HEIGHT);
    let (left, length) = layout::track();
    assert_eq!(left + length, WIDTH - PADDING - LABEL_INSET);
    assert_eq!(layout::thumb_x(0.) - THUMB / 2., left);
    assert_eq!(layout::thumb_x(1.) + THUMB / 2., left + length);
}

#[test]
fn the_pad_and_the_field_are_one_place() {
    for down in [0., 0.25, 0.5, 0.75, 1.] {
        for across in [0., 0.2, 0.5, 0.9] {
            let (x, y) = layout::field_point(across, down);
            let (a, d) = layout::pad_point(x, y);
            assert!(
                close((a, d), (across, down)),
                "{across},{down} came back {a},{d}"
            );
        }
    }
    // The pad's top is light and its foot dark, as drawn.
    let lightness = |down: f32| {
        let (x, y) = layout::field_point(0., down);
        x.hypot(y)
    };
    assert!(lightness(0.) > lightness(1.));
}

#[test]
fn each_palette_is_known_by_its_colours_until_one_is_changed_by_hand() {
    assert_eq!(layout::palette_of(None), Some(0));
    for index in 1..layout::palette_count() {
        let made = layout::palette_theme(index, None).unwrap();
        assert_eq!(
            layout::palette_of(Some(&made)),
            Some(index),
            "palette {index}"
        );
        assert_eq!(made.count, layout::palette_colors(index).len());
        let moved = made.moved(0, made.x * 0.5, made.y * 0.5);
        assert_eq!(
            layout::palette_of(Some(&moved)),
            None,
            "palette {index} by hand"
        );
    }
    // Only the drawing's own is named.
    let named: Vec<_> = (0..layout::palette_count())
        .filter_map(layout::palette_name)
        .collect();
    assert_eq!(named, ["Dusk"]);
}

#[gpui::test]
fn a_press_on_the_empty_pad_places_the_first_colour_there(cx: &mut TestAppContext) {
    let mut panel = Panel::new(None, cx);
    let at = Panel::on_pad(0.3, 0.4);
    panel.click(at);
    let theme = panel.theme().expect("no colour placed");
    assert!(
        close(layout::handle(&theme, 0), at),
        "at {:?}",
        layout::handle(&theme, 0)
    );
    assert_eq!(theme.count, 1);
    assert_eq!(theme.intensity, DEFAULT_INTENSITY);
    // Handed on as it lands, and kept as the button comes up.
    assert_eq!(panel.handed_on(), vec![false, true]);
}

#[gpui::test]
fn a_click_up_before_the_next_frame_still_keeps_the_colour(cx: &mut TestAppContext) {
    let mut panel = Panel::new(None, cx);
    let (x, y) = Panel::on_pad(0.3, 0.4);
    // The driver's clickkey posts both events back to back, and a quick click can do the same.
    // The test context draws after every event; a window draws once a frame, so both go in one
    // update, with no draw between them.
    let at = Panel::at(x, y);
    panel.cx.update(|window, cx| {
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
    panel.cx.run_until_parked();
    assert_eq!(panel.handed_on(), vec![false, true]);
    // The pointer isn't still holding the colour.
    let away = Panel::on_pad(0.8, 0.9);
    panel
        .cx
        .simulate_mouse_move(Panel::at(away.0, away.1), None, Modifiers::none());
    panel.cx.run_until_parked();
    let theme = panel.theme().unwrap();
    assert!(
        close(layout::handle(&theme, 0), (x, y)),
        "it followed the pointer"
    );
}

/// A click as the panel first appears, before it has drawn a frame of itself open, still places
/// the colour: GPUI hands it to the empty frame drawn as the window was made.
#[gpui::test]
fn a_click_before_the_first_frame_still_places_the_colour(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no window for the panel");
    let changed = changed_for(&host);
    let (x, y) = Panel::on_pad(0.3, 0.4);
    let at = Panel::at(x, y);
    // One update, so no frame comes between the opening and the click. Through the window rather
    // than its view, which the press updates.
    cx.update(|cx| {
        show(2, 7, None, NAME_ROW, false, 900., changed, cx);
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
    let mut panel = Panel {
        cx: VisualTestContext::from_window(window.into(), cx),
        window,
        host,
    };
    panel.cx.run_until_parked();
    assert_eq!(panel.handed_on(), vec![false, true]);
    let theme = panel.theme().expect("no colour placed");
    assert!(close(layout::handle(&theme, 0), (x, y)));
}

#[gpui::test]
fn dragging_the_main_handle_moves_every_colour_and_keeps_them_on_letting_go(
    cx: &mut TestAppContext,
) {
    let start = Theme::at(0.5, 0.).with_count(3);
    let mut panel = Panel::new(Some(start), cx);
    let (hx, hy) = layout::handle(&start, 0);
    // Taken off its middle, the handle keeps where the pointer took it.
    let to = Panel::on_pad(0.6, 0.7);
    panel.drag((hx + 4., hy + 3.), (to.0 + 4., to.1 + 3.));
    let theme = panel.theme().unwrap();
    assert!(
        close(layout::handle(&theme, 0), to),
        "at {:?}",
        layout::handle(&theme, 0)
    );
    assert_eq!(theme.count, 3);
    assert_ne!(theme.handle(1), start.handle(1));
    assert_eq!(panel.handed_on(), vec![false, true]);
    assert_eq!(panel.chosen(), 0);
}

#[gpui::test]
fn dragging_another_handle_places_its_colour_alone_and_chooses_it(cx: &mut TestAppContext) {
    let start = Theme::at(0.5, 0.).with_count(3);
    let mut panel = Panel::new(Some(start), cx);
    let to = Panel::on_pad(0.1, 0.2);
    panel.drag(layout::handle(&start, 2), to);
    let theme = panel.theme().unwrap();
    assert!(
        close(layout::handle(&theme, 2), to),
        "the third at {:?}",
        layout::handle(&theme, 2)
    );
    assert!(close((theme.x, theme.y), (0.5, 0.)), "the main one moved");
    assert!(close(theme.handle(1), start.handle(1)), "the second moved");
    assert_eq!(panel.handed_on(), vec![false, true]);
    // The field now shows the third's value.
    assert_eq!(panel.chosen(), 2);
}

/// The handles are drawn at rest, then held from the press's own frame, which is the first to
/// listen for the drag's moves, and at rest again once let go, and a bar's thumb is drawn held the
/// same way (check.sh waits for each).
#[gpui::test]
fn a_held_handle_is_drawn_held_before_it_drags(cx: &mut TestAppContext) {
    let start = Theme::at(0.5, 0.).with_count(3);
    let mut panel = Panel::new(Some(start), cx);
    let drawn = |panel: &mut Panel| {
        let editing = panel.window.read_with(&panel.cx.cx, |picker, _| {
            (picker.editing.as_ref()).and_then(|editing| editing.drawn_handles)
        });
        editing.unwrap()
    };
    assert_eq!(drawn(&mut panel), Some((3, None)));
    let (x, y) = layout::handle(&start, 2);
    let left = MouseButton::Left;
    panel
        .cx
        .simulate_mouse_down(Panel::at(x, y), left, Modifiers::none());
    panel.cx.run_until_parked();
    assert_eq!(drawn(&mut panel), Some((3, Some(Drag::Handle(2, 0., 0.)))));
    panel
        .cx
        .simulate_mouse_up(Panel::at(x, y), left, Modifiers::none());
    panel.cx.run_until_parked();
    assert_eq!(drawn(&mut panel), Some((3, None)));
    let (x, y) = Panel::thumb(Bar::Transparency, START);
    panel
        .cx
        .simulate_mouse_down(Panel::at(x, y), left, Modifiers::none());
    panel.cx.run_until_parked();
    assert_eq!(
        drawn(&mut panel),
        Some((3, Some(Drag::Bar(Bar::Transparency))))
    );
}

#[gpui::test]
fn the_plus_adds_colours_up_to_three_and_chooses_the_new_one(cx: &mut TestAppContext) {
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    for count in 1..MAX_COLORS {
        panel.click(layout::chip_item(count, count));
        assert_eq!(panel.theme().unwrap().count, count + 1);
        assert_eq!(panel.chosen(), count);
    }
    assert_eq!(panel.handed_on(), vec![true, true]);
    // At three there's no plus anywhere along the chip.
    let theme = panel.theme();
    let y = layout::chip_item(3, 0).1;
    assert!((0..300).all(|x| target(x as f32, y, theme.as_ref()) != Some(Target::Add)));
    // A point's disc chooses its colour for the field.
    panel.click(layout::chip_item(3, 1));
    assert_eq!(panel.chosen(), 1);
    assert_eq!(panel.handed_on(), Vec::<bool>::new());
}

#[gpui::test]
fn a_palette_sets_its_colours_at_once_and_the_first_takes_them_away(cx: &mut TestAppContext) {
    let start = Theme {
        intensity: 0.7,
        ..Theme::at(0.5, 0.)
    };
    let mut panel = Panel::new(Some(start), cx);
    panel.click(layout::disc(1));
    let theme = panel.theme().unwrap();
    assert_eq!(layout::palette_of(Some(&theme)), Some(1));
    assert_eq!(theme.count, 2);
    // The rest of the look stays.
    assert_eq!(theme.intensity, 0.7);
    assert_eq!(panel.handed_on(), vec![true]);
    panel.click(layout::disc(0));
    assert_eq!(panel.theme(), None);
    assert_eq!(*panel.host.cleared.borrow(), 1);
    // With no colour, a palette gives its colours.
    panel.click(layout::disc(6));
    assert_eq!(layout::palette_of(panel.theme().as_ref()), Some(6));
}

#[gpui::test]
fn the_field_takes_six_hex_digits_and_refuses_anything_else(cx: &mut TestAppContext) {
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.).with_count(2)), cx);
    let (left, top) = layout::value_field();
    let field = (left + FIELD_WIDTH / 2., top + CHIP_HEIGHT / 2.);
    panel.click(field);
    assert!(panel.typing());
    panel.cx.simulate_input("e5605f");
    panel.keys("enter");
    assert!(!panel.typing());
    let theme = panel.theme().unwrap();
    assert_eq!(theme.main_color(), 0xe5605f);
    assert_eq!(panel.handed_on(), vec![true]);
    // With the sign too, for the colour chosen.
    panel.click(layout::chip_item(2, 1));
    panel.click(field);
    panel.cx.simulate_input("#5FA9E8");
    panel.keys("enter");
    let second = panel.theme().unwrap().colors()[1];
    let near = |a: u32, b: u32| {
        [16, 8, 0]
            .iter()
            .all(|shift| ((a >> shift) & 0xff).abs_diff((b >> shift) & 0xff) <= 24)
    };
    assert!(near(second, 0x5fa9e8), "the second is {second:06x}");
    panel.handed_on();
    // Anything else is refused and the old value stays.
    let before = panel.theme();
    panel.click(field);
    panel.cx.simulate_input("blue");
    panel.keys("enter");
    assert_eq!(panel.theme(), before);
    assert_eq!(panel.handed_on(), Vec::<bool>::new());
    // Esc in the field puts it back and the panel stays.
    panel.click(field);
    panel.cx.simulate_input("000000");
    panel.keys("escape");
    assert!(!panel.typing() && panel.open());
    assert_eq!(panel.theme(), before);
}

#[gpui::test]
fn intensity_sets_the_theme_s_and_waits_for_a_colour(cx: &mut TestAppContext) {
    let mut panel = Panel::new(None, cx);
    let (x, y) = Panel::thumb(Bar::Intensity, 0.5);
    panel.drag((x, y), (x + 60., y));
    assert_eq!(panel.theme(), None);
    assert_eq!(panel.handed_on(), Vec::<bool>::new());

    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    panel.click(Panel::thumb(Bar::Intensity, 0.25));
    let intensity = panel.theme().unwrap().intensity;
    assert!((intensity - 0.25).abs() < 0.01, "at {intensity}");
    assert_eq!(panel.handed_on(), vec![false, true]);
}

#[gpui::test]
fn transparency_moves_the_space_s_glass_and_keeps_it_on_letting_go(cx: &mut TestAppContext) {
    let mut panel = Panel::new(None, cx);
    assert_eq!(panel.glass(), START);
    // The whole bar is live, with no colour too: from solid to clear.
    panel.drag(
        Panel::thumb(Bar::Transparency, START),
        Panel::thumb(Bar::Transparency, 0.),
    );
    assert_eq!(panel.glass(), 0.);
    let glassed = panel.glassed();
    assert_eq!(glassed.last(), Some(&(0., true)));
    assert!(glassed[..glassed.len() - 1].iter().all(|&(_, done)| !done));
    panel.drag(
        Panel::thumb(Bar::Transparency, 0.),
        Panel::thumb(Bar::Transparency, 1.),
    );
    assert_eq!(panel.glass(), 1.);
    // The theme is left alone.
    assert_eq!(panel.handed_on(), Vec::<bool>::new());
}

#[gpui::test]
fn reset_gives_back_the_look_a_space_comes_with(cx: &mut TestAppContext) {
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    panel.drag(
        Panel::thumb(Bar::Transparency, START),
        Panel::thumb(Bar::Transparency, 0.9),
    );
    panel.glassed();
    let top = layout::note_top();
    panel.click((WIDTH - PADDING - RESET_WIDTH / 2., top + LABEL / 2.));
    assert_eq!(panel.theme(), None);
    assert_eq!(*panel.host.cleared.borrow(), 1);
    assert_eq!(panel.glassed(), vec![(START, true)]);
}

#[gpui::test]
fn a_drag_bumps_once_as_it_reaches_a_bar_s_end(cx: &mut TestAppContext) {
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    let detents = |panel: &Panel| {
        let calls = panel.host.calls.borrow();
        calls.iter().filter(|call| *call == "detent").count()
    };
    let (button, none) = (MouseButton::Left, Modifiers::none());
    let path = |panel: &mut Panel, points: &[(f32, f32)]| {
        let (x, y) = points[0];
        panel.cx.simulate_mouse_down(Panel::at(x, y), button, none);
        for &(x, y) in &points[1..] {
            panel.cx.simulate_mouse_move(Panel::at(x, y), button, none);
            panel.cx.run_until_parked();
        }
        let (x, y) = points[points.len() - 1];
        panel.cx.simulate_mouse_up(Panel::at(x, y), button, none);
        panel.cx.run_until_parked();
    };
    let (x, y) = Panel::thumb(Bar::Transparency, START);
    path(
        &mut panel,
        &[(x, y), (x - 30., y), (x - 400., y), (x - 420., y)],
    );
    assert_eq!(detents(&panel), 1);
    let (x, y) = Panel::thumb(Bar::Transparency, 0.);
    path(&mut panel, &[(x, y), (x + 50., y), (x + 400., y)]);
    assert_eq!(detents(&panel), 2);
}

#[test]
fn presses_land_on_what_is_drawn_there() {
    let theme = Theme::at(0.5, 0.).with_count(2);
    let theme = Some(&theme);
    for index in 0..layout::palette_count() {
        let (x, y) = layout::disc(index);
        assert_eq!(target(x, y, theme), Some(Target::Palette(index)));
    }
    let (x, y) = layout::chip_item(2, 1);
    assert_eq!(target(x, y, theme), Some(Target::Point(1)));
    let (x, y) = layout::chip_item(2, 2);
    assert_eq!(target(x, y, theme), Some(Target::Add));
    let (left, top) = layout::value_field();
    assert_eq!(target(left + 10., top + 10., theme), Some(Target::Value));
    let (x, y) = layout::handle(theme.unwrap(), 1);
    assert_eq!(target(x, y, theme), Some(Target::Handle(1)));
    let (x, y) = Panel::on_pad(0.5, 0.05);
    assert_eq!(target(x, y, theme), Some(Target::Pad));
    for bar in BARS {
        let (x, y) = Panel::thumb(bar, 0.5);
        assert_eq!(target(x, y, theme), Some(Target::Bar(bar)));
    }
    let note = layout::note_top();
    assert_eq!(
        target(WIDTH - PADDING - 4., note + 7., theme),
        Some(Target::Reset)
    );
    assert_eq!(
        target(PADDING + 4., note + 7., theme),
        None,
        "the note is only words"
    );
    // Without a colour the chip and the field aren't there: the pad answers.
    let (x, y) = layout::chip_item(1, 0);
    assert_eq!(target(x, y, None), Some(Target::Pad));
    assert_eq!(target(left + 10., top + 10., None), Some(Target::Pad));
}

/// Tab goes round the palettes, the chip, the field, each bar and Reset; left and right go along
/// a row or move a bar by a point, with shift by ten; space or return presses what's ringed.
#[gpui::test]
fn the_keyboard_goes_round_every_part_and_moves_the_bars(cx: &mut TestAppContext) {
    let theme = layout::palette_theme(1, None);
    let mut panel = Panel::new(theme, cx);
    assert_eq!(panel.keyed(), None);
    panel.keys("tab");
    assert_eq!(
        panel.keyed(),
        Some(Target::Appearance(Appearance::Automatic))
    );
    panel.keys("right right space");
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Dark);
    panel.keys("tab");
    // Onto the palette the Space has.
    assert_eq!(panel.keyed(), Some(Target::Palette(1)));
    panel.keys("right space");
    assert_eq!(layout::palette_of(panel.theme().as_ref()), Some(2));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Point(0)));
    panel.keys("right right");
    assert_eq!(panel.keyed(), Some(Target::Add));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Value));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Bar(Bar::Intensity)));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Bar(Bar::Transparency)));
    panel.glassed();
    panel.keys("right");
    assert!((panel.glass() - (START + 0.01)).abs() < 1e-4);
    panel.keys("shift-left");
    assert!((panel.glass() - (START - 0.09)).abs() < 1e-4);
    assert_eq!(panel.glassed().iter().filter(|(_, done)| *done).count(), 2);
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Reset));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Appearance(Appearance::Dark)));
    panel.keys("shift-tab");
    assert_eq!(panel.keyed(), Some(Target::Reset));
}

#[gpui::test]
fn without_a_colour_the_keyboard_has_the_appearance_palettes_transparency_and_reset(
    cx: &mut TestAppContext,
) {
    let mut panel = Panel::new(None, cx);
    panel.keys("tab tab");
    assert_eq!(panel.keyed(), Some(Target::Palette(0)));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Bar(Bar::Transparency)));
    panel.keys("tab");
    assert_eq!(panel.keyed(), Some(Target::Reset));
}

#[gpui::test]
fn auto_light_and_dark_set_the_space_s_appearance_and_wait_for_a_first_colour(
    cx: &mut TestAppContext,
) {
    let segment = |index: usize| {
        let (left, top) = layout::segment(index);
        (left + SEGMENT_WIDTH / 2., top + SEGMENT_HEIGHT / 2.)
    };
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    panel.click(segment(2));
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Dark);
    assert_eq!(panel.handed_on(), vec![true]);
    // A palette keeps it.
    panel.click(layout::disc(3));
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Dark);

    // Chosen with no colour, the first colour takes it, from the pad or a palette.
    let mut panel = Panel::new(None, cx);
    panel.click(segment(1));
    assert_eq!(panel.handed_on(), Vec::<bool>::new());
    panel.click(Panel::on_pad(0.3, 0.4));
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Light);
    let mut panel = Panel::new(None, cx);
    panel.click(segment(2));
    panel.click(layout::disc(1));
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Dark);
}

#[gpui::test]
fn escape_closes_it_and_a_click_outside_leaves_it_closed(cx: &mut TestAppContext) {
    let mut panel = Panel::new(Some(Theme::at(0.5, 0.)), cx);
    assert!(panel.open());
    panel.keys("escape");
    assert!(!panel.open());
    assert_eq!(panel.calls().last().map(String::as_str), Some("hide"));
    // The host's dismissal after it changes nothing more.
    panel.cx.cx.update(dismissed);
    assert!(!panel.open());
}

#[gpui::test]
fn closing_mid_drag_keeps_what_was_dragged(cx: &mut TestAppContext) {
    let mut panel = Panel::new(None, cx);
    let (x, y) = Panel::thumb(Bar::Transparency, START);
    let left = MouseButton::Left;
    panel
        .cx
        .simulate_mouse_down(Panel::at(x, y), left, Modifiers::none());
    panel.cx.run_until_parked();
    panel
        .cx
        .simulate_mouse_move(Panel::at(x + 40., y), left, Modifiers::none());
    panel.cx.run_until_parked();
    panel.keys("escape");
    assert_eq!(panel.glassed().last().map(|&(_, done)| done), Some(true));
}

/// The node with `role` named `label` in what the panel last told VoiceOver.
fn named(
    panel: &mut Panel,
    role: Role,
    label: &str,
) -> (gpui::accesskit::NodeId, gpui::accesskit::Node) {
    let tree = panel.cx.a11y_tree().expect("no accessibility tree");
    let found = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label));
    found
        .cloned()
        .unwrap_or_else(|| panic!("no {role:?} named {label}"))
}

#[gpui::test]
fn voiceover_reads_the_panel_and_moves_its_bars(cx: &mut TestAppContext) {
    let theme = Theme {
        intensity: 0.4,
        ..layout::palette_theme(1, None).unwrap()
    };
    let mut panel = Panel::new(Some(theme), cx);
    panel.cx.simulate_a11y_activation();
    panel.cx.run_until_parked();
    named(&mut panel, Role::Dialog, "Space Theme");
    named(&mut panel, Role::Group, "Colors, 2");
    let (_, automatic) = named(&mut panel, Role::RadioButton, "Automatic");
    assert_eq!(automatic.toggled(), Some(Toggled::True));
    let (dark, _) = named(&mut panel, Role::RadioButton, "Dark");
    panel.cx.simulate_a11y_action(dark, Action::Click);
    panel.cx.run_until_parked();
    assert_eq!(panel.theme().unwrap().appearance, Appearance::Dark);
    let (_, dusk) = named(&mut panel, Role::RadioButton, "Dusk");
    assert_eq!(dusk.toggled(), Some(Toggled::True));
    let (none, _) = named(&mut panel, Role::RadioButton, "No color");
    let (_, intensity) = named(&mut panel, Role::Slider, "Intensity");
    assert_eq!(intensity.numeric_value(), Some(40.));
    let (_, glass) = named(&mut panel, Role::Slider, "Transparency");
    assert_eq!(glass.numeric_value(), Some(40.));
    let (add, _) = named(&mut panel, Role::Button, "Add Color");
    panel.cx.simulate_a11y_action(add, Action::Click);
    panel.cx.run_until_parked();
    named(&mut panel, Role::Group, "Colors, 3");

    // Increment and decrement move a point, each move kept.
    panel.glassed();
    let (glass, _) = named(&mut panel, Role::Slider, "Transparency");
    panel.cx.simulate_a11y_action(glass, Action::Increment);
    panel.cx.run_until_parked();
    assert!((panel.glass() - 0.41).abs() < 1e-4);
    assert_eq!(panel.glassed().last().map(|&(_, done)| done), Some(true));

    panel.cx.simulate_a11y_action(none, Action::Click);
    panel.cx.run_until_parked();
    assert_eq!(panel.theme(), None);
    let (_, intensity) = named(&mut panel, Role::Slider, "Intensity");
    assert!(intensity.is_disabled(), "Intensity without a colour");
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
