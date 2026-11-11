//! When the hover card shows, swaps and goes, on a host that records what it asks of its window,
//! where it goes, and what its pictures do to it.

use std::cell::RefCell;

use gpui::{TestAppContext, point};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32, width: f32, height: f32) {
        self.0
            .borrow_mut()
            .push(format!("show {left} {top} {width} {height} over {window}"));
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }
}

/// What the card told the shell, in order.
type Told = Rc<RefCell<Vec<String>>>;

fn open_card(cx: &mut TestAppContext) -> (Rc<Recorder>, Told) {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx))
        .expect("no card window");
    (host, Told::default())
}

fn window() -> Size<Pixels> {
    size(px(1000.), px(700.))
}

/// The row of the tab `tab`, the sidebar's rows being 36 apart from 100 down.
fn row(tab: u64) -> Anchor {
    let top = 100. + 36. * tab as f32;
    Anchor {
        row: Bounds::new(point(px(8.), px(top)), size(px(224.), px(34.))),
        window: window(),
        left: false,
    }
}

fn card(tab: u64, pictured: bool) -> Card {
    Card {
        tab,
        title: format!("Tab {tab}").into(),
        host: "example.com".into(),
        icon: Icon::Favicon(None),
        pictured,
        paused: false,
        actions: vec![Action::Pin, Action::Favorite, Action::Split],
        panes: Vec::new(),
        focused: tab,
        ring: gpui::black(),
    }
}

fn rest_on(cx: &mut TestAppContext, told: &Told, card: Card) {
    rest_in(cx, told, 1, card);
}

/// The pointer rests on `card`'s row in the sidebar of browser window `window`.
fn rest_in(cx: &mut TestAppContext, told: &Told, window: u64, card: Card) {
    let told = told.clone();
    let events = Rc::new(move |tab: u64, event: Event, _: &mut App| {
        told.borrow_mut().push(match event {
            Event::Shown => format!("shown {tab}"),
            Event::Act(action) => format!("{} {tab}", action.label()),
            Event::Pointer(on) => format!("pointer {} {tab}", on as u8),
        })
    });
    let anchor = row(card.tab);
    cx.update(|cx| {
        hover(
            Hover::Rest(Rest {
                window,
                card,
                anchor,
                events,
            }),
            cx,
        )
    });
    cx.run_until_parked();
}

fn ask(cx: &mut TestAppContext, what: Hover) {
    cx.update(|cx| hover(what, cx));
    cx.run_until_parked();
}

fn after(cx: &mut TestAppContext, host: &Recorder, time: Duration) -> Vec<String> {
    cx.executor().advance_clock(time);
    cx.run_until_parked();
    host.0.take()
}

/// Where the host is told a card of `tab` goes, `height` tall, in browser window 1.
fn shown(tab: u64, height: f32) -> String {
    shown_in(1, tab, height)
}

fn shown_in(window: u64, tab: u64, height: f32) -> String {
    let (left, top) = place(row(tab), height);
    format!("show {left} {top} {WIDTH} {height} over {window}")
}

/// A card with a picture's box but no picture yet.
fn waiting_height() -> f32 {
    height(picture_height(true, None))
}

fn card_window(cx: &mut TestAppContext) -> WindowHandle<HoverCard> {
    cx.update(|cx| cx.global::<CardWindow>().0)
}

/// A picture `width` by `height` pixels, of no colour.
fn picture(width: u32, height: u32) -> PagePicture {
    PagePicture::from_bgra(vec![0; (width * height * 4) as usize], width, height).unwrap()
}

#[gpui::test]
fn a_card_shows_once_the_pointer_has_rested_on_its_row_for_dia_s_half_second(
    cx: &mut TestAppContext,
) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    let before = Duration::from_millis(490);
    assert!(
        after(cx, &host, before).is_empty(),
        "a passing pointer opens nothing"
    );
    assert_eq!(
        after(cx, &host, Duration::from_millis(10)),
        [shown(1, waiting_height())]
    );
    assert_eq!(*told.borrow(), ["shown 1"]);
    assert!(
        DWELL < crate::tooltip::DELAY,
        "the card comes before a tooltip would"
    );
}

#[gpui::test]
fn the_pointer_leaving_before_the_dwell_opens_nothing(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    ask(cx, Hover::Left(1));
    assert!(after(cx, &host, DWELL * 2).is_empty());
    assert!(told.borrow().is_empty());
}

#[gpui::test]
fn resting_again_on_the_same_row_keeps_its_dwell(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL / 2);
    rest_on(cx, &told, card(1, true));
    assert_eq!(after(cx, &host, DWELL / 2), [shown(1, waiting_height())]);
}

#[gpui::test]
fn the_next_row_swaps_in_at_once_while_a_card_shows(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    // The pointer comes onto the next row before the one it left hears so, as GPUI may order them.
    rest_on(cx, &told, card(2, true));
    ask(cx, Hover::Left(1));
    assert_eq!(host.0.take(), [shown(2, waiting_height())]);
    assert!(
        after(cx, &host, LEAVE * 4).is_empty(),
        "leaving the old row leaves the new card"
    );
    assert_eq!(*told.borrow(), ["shown 1", "shown 2"]);
}

#[gpui::test]
fn it_shows_over_the_window_whose_row_the_pointer_rests_on(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    // A window behind another still hears the pointer, so its row's card shows over it (#1432).
    rest_in(cx, &told, 2, card(4, true));
    assert_eq!(after(cx, &host, DWELL), [shown_in(2, 4, waiting_height())]);
    rest_in(cx, &told, 1, card(1, true));
    assert_eq!(host.0.take(), [shown_in(1, 1, waiting_height())]);
    ask(cx, Hover::Close);
    after(cx, &host, FADE * 4);
    // The pointer crossing from one window's row to the other's before the dwell ends shows the
    // card it rests on, over its own window.
    rest_in(cx, &told, 1, card(1, true));
    after(cx, &host, DWELL / 2);
    rest_in(cx, &told, 2, card(4, true));
    assert_eq!(after(cx, &host, DWELL), [shown_in(2, 4, waiting_height())]);
}

#[gpui::test]
fn the_next_row_swaps_in_at_once_after_the_old_one_is_left(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    ask(cx, Hover::Left(1));
    rest_on(cx, &told, card(2, true));
    assert_eq!(host.0.take(), [shown(2, waiting_height())]);
    assert!(after(cx, &host, LEAVE * 4).is_empty());
}

#[gpui::test]
fn leaving_the_rows_closes_the_card_after_a_moment(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    ask(cx, Hover::Left(1));
    assert!(
        after(cx, &host, LEAVE).is_empty(),
        "it waits, then fades out"
    );
    assert_eq!(after(cx, &host, FADE), ["hide"]);
    // The next row waits for its dwell again.
    rest_on(cx, &told, card(2, true));
    assert!(host.0.take().is_empty());
    assert_eq!(after(cx, &host, DWELL), [shown(2, waiting_height())]);
}

#[gpui::test]
fn the_pointer_on_the_card_keeps_it(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    ask(cx, Hover::Left(1));
    let window = card_window(cx);
    cx.update(|cx| {
        window
            .update(cx, |card, _, cx| card.on_card(true, cx))
            .unwrap()
    });
    assert!(
        after(cx, &host, LEAVE * 4).is_empty(),
        "it stays while the pointer is on it"
    );
    cx.update(|cx| {
        window
            .update(cx, |card, _, cx| card.on_card(false, cx))
            .unwrap()
    });
    assert_eq!(after(cx, &host, LEAVE + FADE), ["hide"]);
    // The shell hears it come and go, as a hidden sidebar counts the card as its own (#830).
    assert_eq!(*told.borrow(), ["shown 1", "pointer 1 1", "pointer 0 1"]);
}

#[gpui::test]
fn a_card_ended_under_the_pointer_tells_the_shell_it_s_off(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    let window = card_window(cx);
    cx.update(|cx| {
        window
            .update(cx, |card, _, cx| card.on_card(true, cx))
            .unwrap()
    });
    ask(cx, Hover::Close);
    assert_eq!(*told.borrow(), ["shown 1", "pointer 1 1", "pointer 0 1"]);
    assert_eq!(after(cx, &host, FADE), ["hide"]);
}

#[gpui::test]
fn a_press_a_scroll_or_a_menu_closes_it_at_once(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    ask(cx, Hover::Close);
    // Its window stays while it fades out, and goes as the fade ends.
    assert!(after(cx, &host, FADE - Duration::from_millis(10)).is_empty());
    assert_eq!(after(cx, &host, Duration::from_millis(10)), ["hide"]);
    // And one waiting for its dwell never comes.
    rest_on(cx, &told, card(2, true));
    ask(cx, Hover::Close);
    assert!(after(cx, &host, DWELL * 2).is_empty());
}

#[gpui::test]
fn any_key_closes_it_as_a_press_does(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    // The browser window, which has the keyboard while the card shows.
    let browser = cx.add_empty_window().window_handle();
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    cx.simulate_keystrokes(browser, "x");
    assert_eq!(after(cx, &host, FADE), ["hide"]);
    // And one waiting for its dwell never comes.
    rest_on(cx, &told, card(2, true));
    cx.simulate_keystrokes(browser, "shift-a");
    assert!(after(cx, &host, DWELL * 2).is_empty());
}

#[gpui::test]
fn a_card_back_before_its_window_hid_stays(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    let window = card_window(cx);
    ask(cx, Hover::Close);
    // As a press on its button can show it again within the fade, on the shell's say.
    let events = Rc::new(|_, _, _: &mut App| {});
    let rest = Rest {
        window: 1,
        card: card(2, true),
        anchor: row(2),
        events,
    };
    cx.update(|cx| {
        window
            .update(cx, |card, _, cx| card.show(rest, cx))
            .unwrap()
    });
    assert_eq!(after(cx, &host, FADE * 4), [shown(2, waiting_height())]);
}

/// Whether `pose` is `scale` big and `opacity` opaque, to a thousandth, as the curves solve.
fn near(pose: Pose, scale: f32, opacity: f32) -> bool {
    (pose.scale - scale).abs() < 0.001 && (pose.opacity - opacity).abs() < 0.001
}

#[test]
fn it_grows_from_0_92_as_it_fades_in_over_100_ms() {
    let start = Pose::arriving(0.);
    assert!(near(start, 0.92, 0.), "{start:?}");
    let half = Pose::arriving(0.05);
    assert!(
        half.opacity > 0.5 && half.opacity < 1.,
        "it eases out: {half:?}"
    );
    let faded_in = Pose::arriving(0.1);
    assert!((faded_in.opacity - 1.).abs() < 0.001, "{faded_in:?}");
    assert!(
        faded_in.scale > 0.92 && faded_in.scale < 1.,
        "the spring goes on: {faded_in:?}"
    );
    // ζ 0.75 swings a little past its size before it settles.
    assert!((0..60).any(|frame| Pose::arriving(frame as f32 / 120.).scale > 1.));
    assert!(near(Pose::arriving(1.), 1., 1.));
}

#[test]
fn it_springs_back_toward_0_92_as_it_fades_out_over_100_ms() {
    let rest = Pose {
        scale: 1.,
        opacity: 1.,
    };
    assert!(near(rest.going(0.), 1., 1.));
    let gone = rest.going(FADE.as_secs_f32());
    assert!(gone.opacity.abs() < 0.001, "{gone:?}");
    // The 0.2 s spring has reached 0.92 and swung a little past it by then.
    assert!(gone.scale < 0.92 && gone.scale > 0.9, "{gone:?}");
    assert!(rest.going(0.05).scale < 1., "it shrinks as it fades");
    // From midway in, it goes from where it was.
    let midway = Pose::arriving(0.05);
    assert!(near(midway.going(0.), midway.scale, midway.opacity));
    assert!(midway.going(0.05).opacity < midway.opacity);
}

#[gpui::test]
fn a_button_closes_the_card_then_tells_the_shell(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    let window = card_window(cx);
    cx.update(|cx| {
        window
            .update(cx, |card, _, cx| card.press(Action::Favorite, cx))
            .unwrap()
    });
    assert_eq!(*told.borrow(), ["shown 1", "favorite 1"]);
    assert_eq!(after(cx, &host, FADE), ["hide"]);
}

#[gpui::test]
fn a_picture_coming_while_its_card_shows_takes_the_page_s_shape(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    let request = cx.update(|cx| asking(1, cx));
    assert!(is_card_request(request));
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    // A page 1200 by 800 points, at 2×, scaled to fit the card's pixels.
    cx.update(|cx| answered(request, Some(picture(528, 352)), cx));
    cx.run_until_parked();
    assert_eq!(host.0.take(), [shown(1, height(INNER * 352. / 528.))]);
    // Its next card has it at once.
    ask(cx, Hover::Close);
    after(cx, &host, FADE);
    rest_on(cx, &told, card(1, true));
    assert_eq!(
        after(cx, &host, DWELL),
        [shown(1, height(INNER * 352. / 528.))]
    );
}

/// A tab renamed while its card shows, or waits for its dwell, shows its new name, not the one it
/// had when the pointer came onto its row (#1452), and the card stays where it is. Another tab's
/// change leaves it be.
#[gpui::test]
fn a_card_shows_its_tab_s_name_as_it_is_now(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    let window = card_window(cx);
    let title = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            window
                .read(cx)
                .ok()?
                .drawn
                .as_ref()
                .map(|drawn| drawn.card.title.clone())
        })
    };
    let renamed = |tab: u64, name: &str| Card {
        title: name.to_string().into(),
        ..card(tab, true)
    };
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL);
    ask(cx, Hover::Changed(renamed(1, "Inbox")));
    assert_eq!(title(cx).as_deref(), Some("Inbox"));
    assert_eq!(host.0.take(), [shown(1, waiting_height())]);
    ask(cx, Hover::Changed(renamed(2, "Elsewhere")));
    assert_eq!(title(cx).as_deref(), Some("Inbox"));
    assert!(host.0.take().is_empty());

    ask(cx, Hover::Close);
    after(cx, &host, FADE);
    rest_on(cx, &told, card(2, true));
    after(cx, &host, DWELL / 2);
    ask(cx, Hover::Changed(renamed(2, "Calendar")));
    assert_eq!(after(cx, &host, DWELL / 2), [shown(2, waiting_height())]);
    assert_eq!(title(cx).as_deref(), Some("Calendar"));
}

#[gpui::test]
fn an_unloaded_or_closed_tab_s_picture_goes(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    let request = cx.update(|cx| asking(1, cx));
    cx.update(|cx| answered(request, Some(picture(528, 352)), cx));
    cx.update(|cx| forget(1, cx));
    rest_on(cx, &told, card(1, true));
    assert_eq!(after(cx, &host, DWELL), [shown(1, waiting_height())]);
}

#[gpui::test]
fn memory_pressure_takes_every_picture(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    for tab in [1, 2] {
        let request = cx.update(|cx| asking(tab, cx));
        cx.update(|cx| answered(request, Some(picture(528, 352)), cx));
    }
    cx.update(forget_all);
    rest_on(cx, &told, card(2, true));
    assert_eq!(after(cx, &host, DWELL), [shown(2, waiting_height())]);
}

#[gpui::test]
fn the_tab_shown_has_no_picture_box(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    let request = cx.update(|cx| asking(1, cx));
    cx.update(|cx| answered(request, Some(picture(528, 352)), cx));
    rest_on(cx, &told, card(1, false));
    assert_eq!(after(cx, &host, DWELL), [shown(1, height(0.))]);
}

#[test]
fn the_picture_s_box_keeps_to_its_shares() {
    assert_eq!(picture_height(false, Some((528., 352.))), 0.);
    assert_eq!(picture_height(true, None), INNER * PICTURE_DEFAULT);
    // A tall narrow page is cut to the most, a wide one to the least.
    assert_eq!(
        picture_height(true, Some((300., 600.))),
        INNER * PICTURE_MOST
    );
    assert_eq!(
        picture_height(true, Some((600., 100.))),
        INNER * PICTURE_LEAST
    );
    // Its tallest fits the pixels the core is asked for at the window's scale, and a Retina
    // window's are twice a 1× window's each way.
    let (across, down) = picture_pixels(1.);
    assert!(across as f32 >= INNER && down as f32 >= INNER * PICTURE_MOST);
    assert_eq!(picture_pixels(2.), (2 * across, 2 * down));
    assert!(height(INNER * PICTURE_MOST) + 2. * WINDOW_MARGIN <= WINDOW_HEIGHT);
}

#[test]
fn a_card_goes_beside_its_row_and_stays_inside_the_window() {
    let tall = height(INNER * PICTURE_MOST);
    let (left, top) = place(row(1), tall);
    assert_eq!((left, top), (8. + 224. + GAP, 136.));
    // Near the window's bottom it rises to keep EDGE from it.
    let low = Anchor {
        row: Bounds::new(point(px(8.), px(660.)), size(px(224.), px(34.))),
        ..row(0)
    };
    assert_eq!(place(low, tall).1, 700. - tall - EDGE);
    // In a window too narrow for it, it keeps EDGE from the right.
    let narrow = Anchor {
        window: size(px(400.), px(700.)),
        ..row(0)
    };
    assert_eq!(place(narrow, tall).0, 400. - WIDTH - EDGE);
}

/// With the sidebar on the right, a card opens on its row's left, toward the page, as far from it as
/// on the left, and keeps EDGE from the window's left edge (#2375).
#[test]
fn a_card_opens_toward_the_page_from_a_sidebar_on_the_right() {
    let tall = height(INNER * PICTURE_MOST);
    let mirrored = Anchor {
        row: Bounds::new(point(px(768.), px(136.)), size(px(224.), px(34.))),
        left: true,
        ..row(1)
    };
    let (left, top) = place(mirrored, tall);
    assert_eq!((left, top), (768. - GAP - WIDTH, 136.));
    let narrow = Anchor {
        row: Bounds::new(point(px(176.), px(136.)), size(px(224.), px(34.))),
        window: size(px(400.), px(700.)),
        ..mirrored
    };
    assert_eq!(place(narrow, tall).0, EDGE);
}

/// Each button, clicked where the card draws it, as the pointer comes onto the card from the row,
/// makes its call on that first click (#818).
#[gpui::test]
fn a_click_on_each_button_makes_its_call(cx: &mut TestAppContext) {
    let (host, told) = open_card(cx);
    let window = card_window(cx);
    let actions = card(1, true).actions;
    let count = actions.len() as f32;
    let first = WIDTH - BUTTON_INSET - count * BUTTON - (count - 1.) * BUTTON_GAP;
    let top = PADDING + picture_height(true, None) + TEXT_TOP + TITLE_LINE;
    for (at, action) in actions.into_iter().enumerate() {
        rest_on(cx, &told, card(1, true));
        after(cx, &host, DWELL + Duration::from_secs(1));
        ask(cx, Hover::Left(1));
        let x = WINDOW_MARGIN + first + at as f32 * (BUTTON + BUTTON_GAP) + BUTTON / 2.;
        let y = WINDOW_MARGIN + top + BUTTON / 2.;
        let mut window_cx = gpui::VisualTestContext::from_window(window.into(), cx);
        window_cx.simulate_mouse_move(point(px(x), px(y)), None, gpui::Modifiers::none());
        window_cx.simulate_click(point(px(x), px(y)), gpui::Modifiers::none());
        cx.run_until_parked();
        let said = format!("{} 1", action.label());
        assert!(told.borrow().contains(&said), "{said}: {:?}", told.borrow());
        after(cx, &host, LEAVE + FADE);
        told.borrow_mut().clear();
    }
}

/// The card's window keeps the last place it saw the pointer, on the button a click left it on,
/// while the pointer goes on over the page. The next row's card, and its picture changing its
/// height there, don't take that for the pointer coming onto the card or leaving it, which would
/// end the card under a pointer resting on its row (#818's VM run, a pinned tab's card).
#[gpui::test]
fn a_card_after_a_click_doesn_t_take_the_pointer_left_on_a_button_for_its_own(
    cx: &mut TestAppContext,
) {
    let (host, told) = open_card(cx);
    let window = card_window(cx);
    let count = card(1, true).actions.len() as f32;
    let first = WIDTH - BUTTON_INSET - count * BUTTON - (count - 1.) * BUTTON_GAP;
    let top = PADDING + picture_height(true, None) + TEXT_TOP + TITLE_LINE;
    rest_on(cx, &told, card(1, true));
    after(cx, &host, DWELL + Duration::from_secs(1));
    let at = point(
        px(WINDOW_MARGIN + first + BUTTON / 2.),
        px(WINDOW_MARGIN + top + BUTTON / 2.),
    );
    let mut window_cx = gpui::VisualTestContext::from_window(window.into(), cx);
    window_cx.simulate_mouse_move(at, None, gpui::Modifiers::none());
    window_cx.simulate_click(at, gpui::Modifiers::none());
    after(cx, &host, LEAVE + FADE);
    told.borrow_mut().clear();
    // The pointer rests on the next row; the card's window hears nothing of it.
    let request = cx.update(|cx| asking(2, cx));
    rest_on(cx, &told, card(2, true));
    assert_eq!(after(cx, &host, DWELL), [shown(2, waiting_height())]);
    // A wide page, whose picture makes the card shorter than the place the click was.
    cx.update(|cx| answered(request, Some(picture(528, 100)), cx));
    cx.run_until_parked();
    let shorter = height(picture_height(true, Some((528., 100.))));
    assert_eq!(host.0.take(), [shown(2, shorter)]);
    assert!(
        after(cx, &host, (LEAVE + FADE) * 4).is_empty(),
        "it stays while its row is rested on"
    );
    assert_eq!(*told.borrow(), ["shown 2"]);
}

#[gpui::test]
fn a_picture_taken_before_the_cards_window_is_made_is_the_windows(cx: &mut TestAppContext) {
    let kept = cx.update(|cx| asking(1, cx));
    let dropped = cx.update(|cx| asking(2, cx));
    cx.update(|cx| answered(kept, Some(picture(528, 352)), cx));
    cx.update(|cx| answered(dropped, Some(picture(528, 352)), cx));
    cx.update(|cx| forget(2, cx));
    let late = cx.update(|cx| asking(3, cx));
    open_card(cx);
    cx.update(|cx| answered(late, Some(picture(528, 352)), cx));
    let held = card_window(cx)
        .read_with(cx, |card, _| {
            let mut tabs: Vec<u64> = card.pictures.keys().copied().collect();
            tabs.sort();
            tabs
        })
        .unwrap();
    assert_eq!(
        held,
        [1, 3],
        "the window lost a picture taken early, or kept one forgotten"
    );
}
