//! The mini bar's buttons and place, and its presses, on a host that records what it asks of its
//! window.

use std::cell::RefCell;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, RefCell<Vec<Bounds<Pixels>>>);

impl Host for Recorder {
    fn show(&self, window: u64, place: Bounds<Pixels>) {
        self.0.borrow_mut().push(format!("show {window}"));
        self.1.borrow_mut().push(place);
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// A page 1000 by 700 at 250, 40 in the window, as beside the sidebar.
fn page() -> Bounds<Pixels> {
    bounds(250., 40., 1000., 700.)
}

fn shown(pin: Option<bool>, put_back: bool) -> Shown {
    Shown {
        tab: 10,
        selection: bounds(600., 300., 300., 120.),
        page: page(),
        pin,
        put_back,
        accent: gpui::green(),
    }
}

fn labels(shown: &Shown) -> Vec<&'static str> {
    buttons(shown)
        .into_iter()
        .map(|(_, _, label)| label)
        .collect()
}

/// Hide always; Put back once the thing has a change; the pin first, saying where the thing is,
/// while a floating thing is selected.
#[test]
fn its_buttons_follow_the_thing_selected() {
    assert_eq!(labels(&shown(None, false)), ["Hide"]);
    assert_eq!(labels(&shown(None, true)), ["Hide", "Put back"]);
    assert_eq!(
        labels(&shown(Some(true), true)),
        ["Stays on screen", "Hide", "Put back"]
    );
    assert_eq!(
        labels(&shown(Some(false), true)),
        ["Scrolls with page", "Hide", "Put back"]
    );
}

/// Its right end at the box's, GAP over it; under it where there's no room over it; and EDGE
/// inside the page either way.
#[test]
fn it_sits_over_the_box_at_its_right_end_and_stays_in_the_page() {
    let bar = size(px(180.), px(HEIGHT));
    let over = place(bounds(600., 300., 300., 120.), page(), bar);
    assert_eq!(over, bounds(720., 300. - GAP - HEIGHT, 180., HEIGHT));

    // A box at the page's top: under it.
    let under = place(bounds(600., 45., 300., 120.), page(), bar);
    assert_eq!(under.origin.y, px(45. + 120. + GAP));

    // A box as tall as the page: at its top, over the box.
    let tall = place(bounds(600., 40., 300., 700.), page(), bar);
    assert_eq!(tall.origin.y, px(40. + EDGE));

    // A narrow box at the page's left edge: the bar starts EDGE in.
    let left = place(bounds(252., 300., 60., 40.), page(), bar);
    assert_eq!(left.origin.x, px(250. + EDGE));
    // And one past its right edge ends EDGE in.
    let right = place(bounds(1100., 300., 300., 40.), page(), bar);
    assert_eq!(right.origin.x, px(1250. - EDGE - 180.));
}

fn open_bar(cx: &mut TestAppContext) -> (Rc<Recorder>, WindowHandle<MiniBar>) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no mini bar window");
    (host, window)
}

type Pressed = Rc<RefCell<Vec<(u64, Press)>>>;

/// Shows `shown` over window 7, and makes its window the size it asked for, as the host does.
fn show_bar(cx: &mut TestAppContext, window: WindowHandle<MiniBar>, shown: Shown) -> Pressed {
    let pressed = Pressed::default();
    let on_press: OnPress = {
        let pressed = pressed.clone();
        Rc::new(move |tab, press, _| pressed.borrow_mut().push((tab, press)))
    };
    cx.update(|cx| super::show(7, Some((shown, on_press)), cx));
    let showing = cx.update(|cx| window.read(cx).unwrap().showing.is_some());
    assert!(showing, "the bar didn't take what it shows");
    pressed
}

/// Clicks the bar's last button, at its right end.
fn click_last(cx: &mut TestAppContext, host: &Recorder, window: WindowHandle<MiniBar>) {
    let place = *host.1.borrow().last().unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(place.size);
    visual.run_until_parked();
    let at = point(place.size.width - px(INSET + 6.), place.size.height / 2.);
    visual.simulate_click(at, Modifiers::none());
    visual.run_until_parked();
}

/// A press asks the mode on the tab the bar shows for, and the click leaves the keyboard where it
/// was; hidden for none, once.
#[gpui::test]
fn a_press_asks_the_mode_and_never_takes_the_keyboard(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    let pressed = show_bar(cx, window, shown(None, true));
    assert_eq!(host.0.take(), ["show 7"]);
    assert_eq!(cx.update(|cx| shown_over(cx)), Some(7));
    click_last(cx, &host, window);
    assert_eq!(pressed.take(), [(10, Press::PutBack)]);

    let pressed = show_bar(cx, window, shown(None, false));
    click_last(cx, &host, window);
    assert_eq!(pressed.take(), [(10, Press::Hide)]);
    let focused = cx.update(|cx| window.update(cx, |_, window, _| window.is_window_active()));
    assert!(!focused.unwrap(), "the bar took the keyboard");

    host.0.take();
    cx.update(|cx| super::show(7, None, cx));
    cx.update(|cx| super::show(7, None, cx));
    assert_eq!(host.0.take(), ["hide"], "hidden once");
    assert_eq!(cx.update(|cx| shown_over(cx)), None);
}

/// The bar's width is its buttons', each a glyph and its label, with the pin's divider.
#[test]
fn its_width_is_its_buttons() {
    let measure = |text: &SharedString, size: f32, _: FontWeight| text.len() as f32 * size / 10.;
    let button =
        |label: &str| BUTTON_PAD * 2. + GLYPH + GLYPH_GAP + label.len() as f32 * SMALL / 10.;
    let plain = bar_size(&shown(None, true), &measure);
    let expected = INSET * 2. + button("Hide") + BUTTON_GAP + button("Put back");
    assert_eq!(plain, size(px(expected.ceil()), px(HEIGHT)));
    let pinned = bar_size(&shown(Some(true), true), &measure);
    // The divider is a hairline DIVIDER_GAP from the pin and from Hide.
    let expected = expected + button("Stays on screen") + DIVIDER_GAP * 2. + 1.;
    assert_eq!(pinned, size(px(expected.ceil()), px(HEIGHT)));
}
