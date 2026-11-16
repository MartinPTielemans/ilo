//! The foot bar's size and place, and its buttons, on a host that records what it asks of its
//! window.

use std::cell::{Cell, RefCell};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, RefCell<Vec<Place>>);

impl Host for Recorder {
    fn show(&self, window: u64, place: Place) {
        self.0.borrow_mut().push(format!("show {window}"));
        self.1.borrow_mut().push(place);
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

/// Each character a tenth of the size wide, so widths are easy to add up.
fn measure(text: &SharedString, size: f32, _: FontWeight) -> f32 {
    text.chars().count() as f32 * size / 10.
}

fn button(label: &str, key: Option<&'static str>, primary: bool, enabled: bool) -> Button {
    Button {
        label: label.to_owned().into(),
        key,
        primary,
        enabled,
        name: "test",
        run: Rc::new(|_| {}),
    }
}

/// Arrange's: its words, then Undo, Reset and Done.
fn arrange() -> Bar {
    Bar {
        lead: None,
        words: "Click what to change".into(),
        hint: None,
        buttons: vec![
            button("Undo", Some("cmd-z"), false, false),
            button("Reset", None, false, true),
            button("Done", Some("escape"), true, true),
        ],
    }
}

/// Pointing's: who asks over the question, the hint, Not Now and Send.
fn ask(question: &str) -> Bar {
    Bar {
        lead: Some(Lead {
            glyph: Some("icons/terminal.svg"),
            text: "Claude asks".into(),
        }),
        words: question.to_owned().into(),
        hint: Some("Click what you mean".into()),
        buttons: vec![
            button("Not Now", Some("escape"), false, true),
            button("Send", Some("cmd-enter"), true, false),
        ],
    }
}

fn width(text: &str, size: f32) -> f32 {
    measure(&text.to_owned().into(), size, FontWeight::NORMAL)
}

fn button_width(label: &str, key: Option<&str>) -> f32 {
    BUTTON_PAD * 2.
        + width(label, TEXT)
        + key.map_or(0., |key| KEY_GAP + width(&key_text(key), TEXT))
}

/// Keys read as the menus write them on this OS, and escape as "esc".
#[test]
fn keys_are_named_as_menus_name_them() {
    assert_eq!(key_text("escape"), "esc");
    assert_eq!(
        key_text("cmd-enter"),
        crate::shell::keystroke_label("cmd-enter")
    );
    assert_eq!(key_text("cmd-z"), crate::shell::keystroke_label("cmd-z"));
}

/// Whether two widths are the same, but for float rounding.
fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// The bar's width, `content` rounded up to a whole point, and its words', `words` with the rest.
fn whole(content: f32, words: f32) -> (f32, f32) {
    (content.ceil(), words + content.ceil() - content)
}

#[test]
fn one_line_is_its_words_then_its_buttons() {
    let laid = layout(&arrange(), 2000., &measure);
    let buttons = button_width("Undo", Some("cmd-z"))
        + button_width("Reset", None)
        + button_width("Done", Some("escape"));
    let words = width("Click what to change", TEXT);
    let (bar, words) = whole(LEFT + words + LINE_GAP + GAP * 3. + buttons + RIGHT, words);
    assert_eq!(laid.lines, 1);
    assert!(laid.whole);
    assert!(same(laid.words, words));
    assert_eq!(laid.size, size(px(bar), px(HEIGHT)));
}

#[test]
fn two_lines_are_who_asks_over_the_words() {
    let question = "Which one is the box on the right?";
    let laid = layout(&ask(question), 2000., &measure);
    let after = TALL_GAP * 3.
        + width("Click what you mean", HINT)
        + button_width("Not Now", Some("escape"))
        + button_width("Send", Some("cmd-enter"));
    let words = width(question, TEXT);
    let (bar, words) = whole(TALL_LEFT + words + after + TALL_RIGHT, words);
    assert_eq!(laid.lines, 1);
    assert!(same(laid.words, words));
    assert_eq!(laid.size, size(px(bar), px(TALL_HEIGHT)));

    // A lead wider than the words sets the column's width.
    let laid = layout(&ask("Here?"), 2000., &measure);
    let lead = LEAD_GLYPH + LEAD_GLYPH_GAP + width("Claude asks", LEAD);
    assert!(laid.words >= lead && laid.words < lead + 1.);
}

/// #2352's call 4: a long question takes a second line and then ends in an ellipsis, and the bar
/// is never wider than its words' width.
#[test]
fn a_long_question_wraps_once_and_never_widens_the_bar() {
    let long = layout(&ask(&"a".repeat(1000)), 2000., &measure);
    let longer = layout(&ask(&"a".repeat(5000)), 2000., &measure);
    assert_eq!(long.lines, 2);
    assert!(!long.whole);
    assert!(long.words >= WORDS_WIDTH && long.words < WORDS_WIDTH + 1.);
    assert_eq!(long.size.height, px(TALL_HEIGHT + WORDS_LINE));
    assert_eq!(longer, long);
}

/// In a narrow window the words give way, and the buttons keep their size.
#[test]
fn a_narrow_page_shortens_the_words() {
    let wide = layout(&arrange(), 2000., &measure);
    let room = f32::from(wide.size.width) - 10.;
    let laid = layout(&arrange(), room, &measure);
    assert_eq!(laid.size.width, px(room));
    assert!(same(laid.words, wide.words - 10.));
    assert!(!laid.whole, "cut words end in an ellipsis");
    // With no room at all, there are no words, never fewer buttons.
    let laid = layout(&arrange(), 10., &measure);
    assert!(laid.words < 1.);
    assert!(same(
        f32::from(laid.size.width) - laid.words,
        f32::from(wide.size.width) - wide.words
    ));
}

#[test]
fn it_goes_over_the_pages_bottom() {
    let page = Bounds::new(point(px(250.), px(40.)), size(px(1000.), px(700.)));
    let content = size(px(1260.), px(750.));
    let bar = size(px(400.), px(40.));
    assert_eq!(
        place(page, content, bar),
        Place {
            left: 250.,
            right: 10.,
            bottom: 10. + BOTTOM,
            size: bar,
        }
    );
}

fn open_bar(cx: &mut TestAppContext) -> (Rc<Recorder>, WindowHandle<FootBar>) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), |_, _| Palette::default(), cx))
        .expect("no foot bar window");
    (host, window)
}

/// Shows `bar` over window 7, and makes its window the size it asked for, as the host does.
fn show_bar(cx: &mut TestAppContext, host: &Recorder, window: WindowHandle<FootBar>, bar: Bar) {
    let page = Bounds::new(point(px(250.), px(40.)), size(px(1000.), px(700.)));
    cx.update(|cx| show(7, page, size(px(1260.), px(750.)), bar, cx));
    let place = *host.1.borrow().last().unwrap();
    VisualTestContext::from_window(window.into(), cx).simulate_resize(place.size);
    cx.run_until_parked();
}

/// Clicks the bar's last button, at its right end.
fn click_last(cx: &mut TestAppContext, host: &Recorder, window: WindowHandle<FootBar>) {
    let place = *host.1.borrow().last().unwrap();
    let at = point(
        place.size.width - px(RIGHT.max(TALL_RIGHT) + 4.),
        place.size.height / 2.,
    );
    VisualTestContext::from_window(window.into(), cx).simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn a_press_runs_its_button_and_a_dimmed_one_does_nothing(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    let ran = Rc::new(Cell::new(0));
    let counting = |enabled| {
        let ran = ran.clone();
        let mut bar = arrange();
        let done = bar.buttons.last_mut().unwrap();
        done.enabled = enabled;
        done.run = Rc::new(move |_| ran.set(ran.get() + 1));
        bar
    };
    show_bar(cx, &host, window, counting(true));
    assert_eq!(host.0.take(), ["show 7"]);
    click_last(cx, &host, window);
    assert_eq!(ran.get(), 1);

    show_bar(cx, &host, window, counting(false));
    click_last(cx, &host, window);
    assert_eq!(ran.get(), 1, "a dimmed button does nothing");
}

#[gpui::test]
fn its_user_hides_it_and_it_never_takes_the_keyboard(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    show_bar(cx, &host, window, ask("Which one?"));
    assert_eq!(cx.update(|cx| shown_over(cx)), Some(7));
    click_last(cx, &host, window);
    // A click on it leaves the keyboard where it was.
    let focused = cx.update(|cx| window.update(cx, |_, window, _| window.is_window_active()));
    assert!(!focused.unwrap());
    cx.update(hide);
    cx.update(hide);
    assert_eq!(host.0.take(), ["show 7", "hide"], "hidden once");
    assert_eq!(cx.update(|cx| shown_over(cx)), None);
}

#[gpui::test]
fn a_bar_that_grows_draws_once_its_window_has_grown(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    let ran = Rc::new(RefCell::new(Vec::new()));
    let saying = |words: &str, name: &'static str| {
        let mut bar = arrange();
        bar.words = words.to_owned().into();
        let ran = ran.clone();
        bar.buttons.last_mut().unwrap().run = Rc::new(move |_| ran.borrow_mut().push(name));
        bar
    };
    show_bar(cx, &host, window, saying("Short", "before"));
    let page = Bounds::new(point(px(250.), px(40.)), size(px(1000.), px(700.)));
    let wider = saying("Inner scrolls with the page, over it", "after");
    // The host sizes the window after the update that asked: until it has, the bar drawn, and the
    // one a click presses, is the one before, not the new one cut at the old size (#2991).
    cx.update(|cx| show(7, page, size(px(1260.), px(750.)), wider, cx));
    cx.run_until_parked();
    let places = host.1.borrow().clone();
    let (before, after) = (places[places.len() - 2], places[places.len() - 1]);
    assert!(after.size.width > before.size.width);
    let done = |place: Place| {
        point(
            place.size.width - px(RIGHT.max(TALL_RIGHT) + 4.),
            place.size.height / 2.,
        )
    };
    VisualTestContext::from_window(window.into(), cx)
        .simulate_click(done(before), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(*ran.borrow(), ["before"]);
    VisualTestContext::from_window(window.into(), cx).simulate_resize(after.size);
    cx.run_until_parked();
    click_last(cx, &host, window);
    assert_eq!(*ran.borrow(), ["before", "after"]);
}

/// Words that change twice before a frame, as Windows' log had them in Arrange: the window at the
/// size of the bar between draws that one, not the newest cut to it.
#[gpui::test]
fn a_bar_replaced_before_it_drew_draws_when_its_window_fits_it(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    let ran = Rc::new(RefCell::new(Vec::new()));
    let saying = |words: &str, name: &'static str| {
        let mut bar = arrange();
        bar.words = words.to_owned().into();
        let ran = ran.clone();
        bar.buttons.last_mut().unwrap().run = Rc::new(move |_| ran.borrow_mut().push(name));
        bar
    };
    show_bar(
        cx,
        &host,
        window,
        saying("Inner scrolls with the page, over it", "first"),
    );
    let page = Bounds::new(point(px(250.), px(40.)), size(px(1000.), px(700.)));
    let (between, last) = (
        saying("Let go where it goes", "between"),
        saying("Go", "last"),
    );
    cx.update(|cx| {
        show(7, page, size(px(1260.), px(750.)), between, cx);
        show(7, page, size(px(1260.), px(750.)), last, cx);
    });
    cx.run_until_parked();
    let places = host.1.borrow().clone();
    let [first, between, last] = places[places.len() - 3..] else {
        unreachable!()
    };
    assert!(first.size.width > between.size.width && between.size.width > last.size.width);
    let done = |place: Place| {
        point(
            place.size.width - px(RIGHT.max(TALL_RIGHT) + 4.),
            place.size.height / 2.,
        )
    };
    let mut press = |place: Place| {
        let mut window = VisualTestContext::from_window(window.into(), cx);
        window.simulate_resize(place.size);
        window.run_until_parked();
        window.simulate_click(done(place), Modifiers::none());
        window.run_until_parked();
    };
    press(first);
    press(between);
    press(last);
    assert_eq!(*ran.borrow(), ["first", "between", "last"]);
}

/// A tiling manager on X11 can refuse the bar's window its size and give it a tile (#2900): the
/// bar keeps its size, centred over the tile's bottom, and in a tile narrower than it, its words
/// give way so its buttons stay in the tile.
#[gpui::test]
fn in_a_tile_it_didnt_ask_for_it_shows_whole_and_centred(cx: &mut TestAppContext) {
    let (host, window) = open_bar(cx);
    let ran = Rc::new(Cell::new(0));
    let mut bar = arrange();
    bar.words = "Inner scrolls with the page, over it".into();
    let counting = ran.clone();
    bar.buttons.last_mut().unwrap().run = Rc::new(move |_| counting.set(counting.get() + 1));
    show_bar(cx, &host, window, bar);
    let asked = host.1.borrow().last().unwrap().size;
    // Its last button's right end, in a tile `width` wide and 800 tall with the bar centred over
    // its bottom.
    let mut press_in = |width: Pixels, bar_width: Pixels| {
        let tile = size(width, px(800.));
        let mut window = VisualTestContext::from_window(window.into(), cx);
        window.simulate_resize(tile);
        window.run_until_parked();
        let right = (width + bar_width) / 2.;
        let at = point(
            right - px(RIGHT + 4.),
            px(800. - BOTTOM) - asked.height / 2.,
        );
        window.simulate_click(at, Modifiers::none());
        window.run_until_parked();
    };
    press_in(asked.width * 2., asked.width);
    assert_eq!(
        ran.get(),
        1,
        "the bar is its own size, over the bottom of a wider tile"
    );
    let narrow = asked.width - px(60.);
    press_in(narrow, narrow - px(BOTTOM * 2.));
    assert_eq!(ran.get(), 2, "its words give way in a narrower tile");
}
